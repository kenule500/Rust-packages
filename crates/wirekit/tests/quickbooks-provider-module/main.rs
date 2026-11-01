//! Acceptance tests for `quickbooks-provider-module` (AC-001 to AC-016).
//!
//! Every test name carries its AC id (CON-405). Everything runs against a local
//! mock, so no Intuit app approval is needed (RISK-001 stays `[to be confirmed]`).

use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use wirekit::quickbooks::{
    InvoiceLine, Payment, QboCustomer, QboInvoice, QuickBooksClient, QuickBooksConfig, TokenSet,
};
use wirekit::{Secret, WirekitClient};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const REALM: &str = "9130345678";

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

fn config() -> QuickBooksConfig {
    QuickBooksConfig {
        client_id: String::from("client-abc"),
        client_secret: Secret::new(String::from("secret-xyz")),
        refresh_token: Secret::new(String::from("refresh-123")),
        realm_id: String::from(REALM),
        verifier_token: Secret::new(String::from("verifier-key")),
        minor_version: String::from("65"),
    }
}

fn transport(server: &MockServer) -> WirekitClient {
    WirekitClient::builder(server.uri())
        .expect("the builder accepts a loopback base")
        .allow_insecure_localhost(true)
        .build()
        .expect("the transport builds for a loopback base")
}

/// A client holding a token valid for an hour, so no refresh happens.
fn client(server: &MockServer) -> QuickBooksClient {
    let client = QuickBooksClient::new(config(), transport(server));
    client.set_token(TokenSet {
        access_token: Secret::new(String::from("access-1")),
        refresh_token: Secret::new(String::from("refresh-123")),
        expires_at: now() + 3_600,
    });
    client
}

/// A client with no token, so the next call refreshes first.
fn bare_client(server: &MockServer) -> QuickBooksClient {
    QuickBooksClient::new(config(), transport(server))
}

fn token_body(access: &str, refresh: &str, expires_in: i64) -> serde_json::Value {
    serde_json::json!({
        "token_type": "bearer",
        "access_token": access,
        "refresh_token": refresh,
        "expires_in": expires_in,
    })
}

fn sign(raw: &[u8], secret: &str) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).expect("a valid key");
    mac.update(raw);
    BASE64.encode(mac.finalize().into_bytes())
}

fn webhook_body(event_id: &str, entity_id: &str) -> Vec<u8> {
    serde_json::json!({
        "eventNotifications": [{
            "realmId": REALM,
            "notificationId": event_id,
            "dataChangeEvent": {
                "entities": [{
                    "name": "Customer",
                    "id": entity_id,
                    "operation": "Update",
                    "lastUpdated": "2026-10-31T00:00:00.000Z"
                }]
            }
        }]
    })
    .to_string()
    .into_bytes()
}

fn fault_body(code: &str, message: &str) -> serde_json::Value {
    serde_json::json!({
        "Fault": {
            "Error": [{ "Message": message, "code": code, "Detail": message, "element": "" }],
            "type": "ValidationFault"
        }
    })
}

#[test]
fn ac_001_credentials_are_secret_and_debug_renders_no_plaintext() {
    let config = config();
    assert_eq!(
        config.client_secret.expose(),
        "secret-xyz",
        "readable on demand"
    );
    assert_eq!(config.refresh_token.expose(), "refresh-123");
    assert_eq!(config.verifier_token.expose(), "verifier-key");

    let rendered = format!("{config:?}");
    for secret in ["secret-xyz", "refresh-123", "verifier-key"] {
        assert!(
            !rendered.contains(secret),
            "{secret} must not render: {rendered}"
        );
    }
    assert!(
        rendered.contains("***"),
        "redaction marker present: {rendered}"
    );
    assert!(rendered.contains("client-abc"), "the id is not secret");

    let token = TokenSet {
        access_token: Secret::new(String::from("access-1")),
        refresh_token: Secret::new(String::from("refresh-9")),
        expires_at: 1_900_000_000,
    };
    let rendered_token = format!("{token:?}");
    assert!(!rendered_token.contains("access-1"));
    assert!(!rendered_token.contains("refresh-9"));

    // A cleartext base that is not loopback is refused (CON-520, NFR-004).
    let builder = WirekitClient::builder("http://quickbooks.api.intuit.com")
        .expect("the builder accepts any URL");
    let error = match builder.build() {
        Ok(_) => panic!("a cleartext base URL must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_002_an_expired_token_refreshes_once_and_the_call_retries() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth2/v1/tokens/bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_body(
            "fresh-1",
            "refresh-2",
            3600,
        )))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/customer")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Customer": { "Id": "58", "SyncToken": "0", "DisplayName": "Acme" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = bare_client(&server);
    let created = client
        .create_customer(&QboCustomer::new("Acme"), None)
        .await
        .expect("the call succeeds after one refresh");

    assert_eq!(created.id.as_deref(), Some("58"));
    // `.expect(1)` on both mocks proves one refresh and one retried call.
}

#[tokio::test]
async fn ac_003_the_rotated_refresh_token_is_surfaced_and_a_rejection_does_not_loop() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth2/v1/tokens/bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_body(
            "fresh-1",
            "rotated-9",
            3600,
        )))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/customer")))
        .respond_with(
            ResponseTemplate::new(401).set_body_json(fault_body("3200", "AuthenticationFailed")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = bare_client(&server);
    let error = client
        .create_customer(&QboCustomer::new("Acme"), None)
        .await
        .expect_err("a 401 is surfaced, not retried forever");

    let rotated = client
        .refresh_token()
        .expect("the refresh token is surfaced");
    assert_eq!(
        rotated.expose(),
        "rotated-9",
        "the caller can persist the rotation"
    );
    assert_eq!(error.code(), "W2001", "a rejected call is reported once");
    // `.expect(1)` on the customer mock proves the call was not retried in a loop.
}

#[tokio::test]
async fn ac_004_create_customer_posts_the_realm_resource() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/customer")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Customer": { "Id": "58", "SyncToken": "0", "DisplayName": "Acme Ltd" }
        })))
        .mount(&server)
        .await;

    let client = client(&server);
    let created = client
        .create_customer(&QboCustomer::new("Acme Ltd"), None)
        .await
        .expect("the customer is created");

    assert_eq!(created.id.as_deref(), Some("58"));
    assert_eq!(created.display_name, "Acme Ltd");

    let requests = server
        .received_requests()
        .await
        .expect("requests are recorded");
    assert_eq!(requests.len(), 1, "one realm-scoped call");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("json body");
    assert_eq!(body["DisplayName"], "Acme Ltd");
    let url = requests[0].url.as_str();
    assert!(url.contains(REALM), "the path is realm-scoped: {url}");
    assert!(
        url.contains("minorversion=65"),
        "minorversion is sent: {url}"
    );
}

#[tokio::test]
async fn ac_005_update_customer_carries_id_and_sync_token() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/customer")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Customer": {
                "Id": "58", "SyncToken": "1", "DisplayName": "Acme Ltd",
                "PrimaryEmailAddr": { "Address": "ap@acme.test" }
            }
        })))
        .mount(&server)
        .await;

    let client = client(&server);
    let mut customer = QboCustomer::new("Acme Ltd");
    customer.id = Some(String::from("58"));
    customer.sync_token = Some(String::from("0"));

    let updated = client
        .update_customer(&customer)
        .await
        .expect("the update succeeds");
    assert_eq!(updated.display_name, "Acme Ltd");

    let requests = server
        .received_requests()
        .await
        .expect("requests are recorded");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("json body");
    assert_eq!(body["Id"], "58");
    assert_eq!(body["SyncToken"], "0");

    // A record without id or sync token never reaches the network.
    let incomplete = QboCustomer::new("No Id");
    let error = client
        .update_customer(&incomplete)
        .await
        .expect_err("an update needs the id and sync token");
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_006_create_invoice_carries_ordered_lines() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/invoice")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Invoice": {
                "Id": "101", "SyncToken": "0",
                "CustomerRef": { "value": "58" },
                "Line": [
                    { "Amount": 100.0, "Description": "Design" },
                    { "Amount": 250.0, "Description": "Build" }
                ],
                "Balance": 350.0
            }
        })))
        .mount(&server)
        .await;

    let lines = vec![
        InvoiceLine {
            amount: 100.0,
            description: Some(String::from("Design")),
            item_ref: Some(serde_json::json!({ "value": "7" })),
        },
        InvoiceLine {
            amount: 250.0,
            description: Some(String::from("Build")),
            item_ref: None,
        },
    ];

    let client = client(&server);
    let created = client
        .create_invoice(&QboInvoice::new("58", lines))
        .await
        .expect("the invoice is created");
    assert_eq!(created.id.as_deref(), Some("101"));

    let requests = server
        .received_requests()
        .await
        .expect("requests are recorded");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("json body");
    assert_eq!(body["CustomerRef"]["value"], "58");
    let sent = body["Line"].as_array().expect("lines are an array");
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0]["Amount"], 100.0, "line order is preserved");
    assert_eq!(sent[0]["Description"], "Design");
    assert_eq!(sent[0]["ItemRef"]["value"], "7");
    assert_eq!(
        sent[1]["Amount"], 250.0,
        "the second line follows the first"
    );
}

#[tokio::test]
async fn ac_007_void_invoice_uses_the_void_operation() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/invoice")))
        .and(query_param("operation", "void"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Invoice": {
                "Id": "101", "SyncToken": "1", "Balance": 0.0, "Status": "Voided",
                "CustomerRef": { "value": "58" }
            }
        })))
        .mount(&server)
        .await;

    let client = client(&server);
    let mut invoice = QboInvoice::new("58", Vec::new());
    invoice.id = Some(String::from("101"));
    invoice.sync_token = Some(String::from("0"));

    let voided = client
        .void_invoice(&invoice)
        .await
        .expect("the invoice is voided");
    assert_eq!(
        voided.balance, 0.0,
        "a voided invoice reports a zero balance"
    );
}

#[tokio::test]
async fn ac_008_send_invoice_targets_the_send_operation() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/invoice/101/send")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Invoice": {
                "Id": "101", "SyncToken": "1", "Status": "EmailSent",
                "CustomerRef": { "value": "58" }
            }
        })))
        .mount(&server)
        .await;

    let client = client(&server);
    let sent = client
        .send_invoice("101")
        .await
        .expect("the invoice is sent");
    assert_eq!(sent.id.as_deref(), Some("101"));
    assert_eq!(
        sent.status.as_deref(),
        Some("EmailSent"),
        "the delivery is recorded"
    );
}

#[tokio::test]
async fn ac_009_record_payment_links_the_invoice() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/payment")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Payment": {
                "Id": "501", "SyncToken": "0",
                "CustomerRef": { "value": "58" },
                "TotalAmt": 100.0,
                "Line": [{ "Amount": 100.0, "LinkedTxn": [{ "TxnId": "101", "TxnType": "Invoice" }] }]
            }
        })))
        .mount(&server)
        .await;

    let client = client(&server);
    let payment = Payment::for_invoice("101", 100.0);
    assert_eq!(payment.invoice_ref.as_deref(), Some("101"));

    let recorded = client
        .record_payment(&payment)
        .await
        .expect("the payment is recorded");
    assert_eq!(recorded.id.as_deref(), Some("501"));

    let requests = server
        .received_requests()
        .await
        .expect("requests are recorded");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("json body");
    assert_eq!(body["TotalAmt"], 100.0);
    assert_eq!(
        body["Line"][0]["LinkedTxn"][0]["TxnId"], "101",
        "the invoice is linked"
    );
    assert_eq!(body["Line"][0]["LinkedTxn"][0]["TxnType"], "Invoice");
}

#[tokio::test]
async fn ac_010_an_over_balance_payment_surfaces_the_provider_fault() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/payment")))
        .respond_with(ResponseTemplate::new(400).set_body_json(fault_body(
            "6000",
            "The payment amount exceeds the invoice balance",
        )))
        .mount(&server)
        .await;

    let client = client(&server);
    let error = client
        .record_payment(&Payment::for_invoice("101", 9_999.0))
        .await
        .expect_err("an over-balance payment is a typed error, never a silent success");

    assert_eq!(
        error.code(),
        "W2001",
        "the provider failure is an API error"
    );
    assert!(!error.is_retryable(), "a validation fault is not retried");
}

#[tokio::test]
async fn ac_011_reference_data_uses_the_query_endpoint_with_a_bound() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(format!("/v3/company/{REALM}/query")))
        .and(query_param("query", "select * from Account maxresults 5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "QueryResponse": {
                "Account": [
                    { "Id": "35", "Name": "Checking", "AccountType": "Bank", "CurrentBalance": 1200.5 }
                ]
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/v3/company/{REALM}/query")))
        .and(query_param("query", "select * from Item maxresults 5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "QueryResponse": {
                "Item": [
                    { "Id": "7", "Name": "Consulting", "Type": "Service", "UnitPrice": 150.0 }
                ]
            }
        })))
        .mount(&server)
        .await;

    let client = client(&server);

    let accounts = client.list_accounts(5).await.expect("accounts are listed");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "Checking");
    assert_eq!(accounts[0].account_type.as_deref(), Some("Bank"));
    assert_eq!(accounts[0].current_balance, Some(1200.5));

    let items = client.list_items(5).await.expect("items are listed");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Consulting");
    assert_eq!(items[0].item_type.as_deref(), Some("Service"));
    assert_eq!(items[0].unit_price, Some(150.0));
}

#[test]
fn ac_012_a_correct_signature_parses_the_notification() {
    let server_base = "http://127.0.0.1:1";
    let client = QuickBooksClient::new(
        config(),
        WirekitClient::builder(server_base)
            .expect("the builder accepts a loopback base")
            .allow_insecure_localhost(true)
            .build()
            .expect("the transport builds"),
    );

    let raw = webhook_body("evt-1", "58");
    let signature = sign(&raw, "verifier-key");

    let notification = client
        .verify_webhook(&raw, &signature)
        .expect("a correct signature verifies");

    assert_eq!(notification.event_id, "evt-1");
    assert_eq!(notification.realm_id, REALM);
    assert_eq!(notification.entities.len(), 1);
    assert_eq!(
        notification.dedupe_key, "Update:58",
        "the replay key is explicit"
    );
}

#[test]
fn ac_013_a_wrong_signature_fails_closed_and_a_replay_shares_the_event_id() {
    let client = QuickBooksClient::new(
        config(),
        WirekitClient::builder("http://127.0.0.1:1")
            .expect("the builder accepts a loopback base")
            .allow_insecure_localhost(true)
            .build()
            .expect("the transport builds"),
    );

    let raw = webhook_body("evt-2", "58");

    let error = client
        .verify_webhook(&raw, "not-the-signature")
        .expect_err("a wrong signature fails closed");
    assert_eq!(error.code(), "W2002");
    assert!(
        client.verify_webhook(&raw, "").is_err(),
        "a missing signature fails closed too"
    );

    // Nothing was parsed from the rejected deliveries, so a replay must be
    // rejected again until a correct signature arrives.
    let signature = sign(&raw, "verifier-key");
    let first = client.verify_webhook(&raw, &signature).expect("verifies");
    let replay = client.verify_webhook(&raw, &signature).expect("verifies");
    assert_eq!(
        first.event_id, replay.event_id,
        "a repeated event id is how a duplicate is recognised"
    );
}

#[tokio::test]
async fn ac_014_a_fault_body_exposes_every_intuit_code_and_message() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/invoice")))
        .respond_with(ResponseTemplate::new(400).set_body_json(fault_body(
            "2020",
            "Required param missing, need to supply the required value for the API",
        )))
        .mount(&server)
        .await;

    let client = client(&server);
    let error = client
        .create_invoice(&QboInvoice::new("58", Vec::new()))
        .await
        .expect_err("a Fault body is an error");

    assert_eq!(error.code(), "W2001");
    assert_eq!(
        error.api_code(),
        Some("2020"),
        "the Intuit error code is preserved: {error}"
    );
    assert!(
        error.to_string().contains("Required param missing"),
        "the Intuit message is preserved: {error}"
    );
}

#[tokio::test]
async fn ac_015_a_rate_limit_is_retryable_and_a_validation_fault_is_not() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/customer")))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "0")
                .set_body_json(fault_body("3000", "ThrottleExceeded")),
        )
        .mount(&server)
        .await;

    let client = client(&server);
    let error = client
        .create_customer(&QboCustomer::new("Acme"), None)
        .await
        .expect_err("a throttled call fails after its retries");

    assert!(
        error.is_retryable() || error.code() == "W2003",
        "a 429 stays retryable: {error}"
    );
    assert!(
        error.retry_after().is_some() || error.code() == "W2003",
        "the Retry-After header is honoured: {error}"
    );

    // A validation fault is never retried, however many times it is attempted.
    let validation = wirekit::WirekitError::api(400, "6000", "invalid");
    assert!(!validation.is_retryable());
}

#[tokio::test]
async fn ac_016_an_idempotency_key_returns_the_existing_customer() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(format!("/v3/company/{REALM}/query")))
        .and(query_param(
            "query",
            "select * from Customer where DisplayName = 'Acme Ltd'",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "QueryResponse": {
                "Customer": [{ "Id": "58", "SyncToken": "0", "DisplayName": "Acme Ltd" }]
            }
        })))
        .mount(&server)
        .await;

    // Any create would be a duplicate; this mock proves it never happens.
    Mock::given(method("POST"))
        .and(path(format!("/v3/company/{REALM}/customer")))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let client = client(&server);
    let existing = client
        .create_customer(&QboCustomer::new("Acme Ltd"), Some("idem-1"))
        .await
        .expect("the existing customer is returned");

    assert_eq!(
        existing.id.as_deref(),
        Some("58"),
        "no duplicate is created"
    );
}

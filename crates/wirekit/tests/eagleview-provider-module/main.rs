//! Acceptance tests for `eagleview-provider-module` (AC-001 to AC-013).
//!
//! Every test name carries its AC id (CON-405). Everything runs against a local
//! mock, so no EagleView commercial access is needed (RISK-001 stays
//! `[to be confirmed]`).

use std::time::{SystemTime, UNIX_EPOCH};

use wirekit::eagleview::{
    AccessToken, EagleViewClient, EagleViewConfig, MeasurementRequest, MeasurementStatus,
    SegmentKind,
};
use wirekit::{Secret, WirekitClient};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

fn config() -> EagleViewConfig {
    EagleViewConfig {
        client_id: String::from("client-abc"),
        client_secret: Secret::new(String::from("secret-xyz")),
    }
}

fn transport(server: &MockServer) -> WirekitClient {
    WirekitClient::builder(server.uri())
        .expect("the builder accepts a loopback base")
        .allow_insecure_localhost(true)
        .build()
        .expect("the transport builds for a loopback base")
}

/// A client holding a token valid for an hour, so no token call happens.
fn client(server: &MockServer) -> EagleViewClient {
    let client = EagleViewClient::new(config(), transport(server));
    client.set_token(AccessToken {
        access_token: Secret::new(String::from("access-1")),
        expires_at: now() + 3_600,
    });
    client
}

/// A client with no token, so the next call authenticates first.
fn bare_client(server: &MockServer) -> EagleViewClient {
    EagleViewClient::new(config(), transport(server))
}

fn token_body(access: &str, expires_in: i64) -> serde_json::Value {
    serde_json::json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": expires_in,
    })
}

fn order_json(id: &str, status: &str) -> serde_json::Value {
    serde_json::json!({ "id": id, "status": status })
}

fn request() -> MeasurementRequest {
    MeasurementRequest::new("1 Main St", "Springfield", "IL", "62704")
}

fn report_json(order_id: &str) -> serde_json::Value {
    serde_json::json!({
        "orderId": order_id,
        "totalArea": { "squares": 28.5, "facets": 12, "wasteFactor": 0.12 },
        "pitch": { "ratio": "6/12", "riseOverRun": 0.5 },
        "segments": [
            { "kind": "RIDGE", "areaSquares": 2.5, "pitch": { "ratio": "6/12", "riseOverRun": 0.5 } },
            { "kind": "VALLEY", "areaSquares": 3.0, "pitch": { "ratio": "8/12", "riseOverRun": 0.667 } }
        ],
        "documentUrl": "https://api.eagleview.com/v1/orders/o-1/report/document"
    })
}

fn fault(status: u16, code: &str, message: &str) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(serde_json::json!({
        "code": code,
        "message": message,
    }))
}

#[test]
fn ac_001_credentials_are_secret_and_debug_renders_no_plaintext() {
    let config = config();
    assert_eq!(
        config.client_secret.expose(),
        "secret-xyz",
        "readable on demand"
    );

    let rendered = format!("{config:?}");
    assert!(
        !rendered.contains("secret-xyz"),
        "the secret must not render: {rendered}"
    );
    assert!(
        rendered.contains("***"),
        "redaction marker present: {rendered}"
    );
    assert!(rendered.contains("client-abc"), "the id is not secret");

    let token = AccessToken {
        access_token: Secret::new(String::from("access-9")),
        expires_at: 1_900_000_000,
    };
    assert!(!format!("{token:?}").contains("access-9"));

    // The client renders its config, so the secret must stay hidden there too.
    let transport = WirekitClient::builder("http://127.0.0.1:1")
        .expect("the builder accepts a loopback base")
        .allow_insecure_localhost(true)
        .build()
        .expect("the transport builds");
    let rendered_client = format!("{:?}", EagleViewClient::new(config, transport));
    assert!(
        !rendered_client.contains("secret-xyz"),
        "redacted: {rendered_client}"
    );
}

#[tokio::test]
async fn ac_002_a_cached_token_is_used_without_another_token_call() {
    let server = MockServer::start().await;

    // Never expected: the client already holds a valid token.
    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_body("fresh", 3600)))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/o-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(order_json("o-1", "PROCESSING")))
        .expect(1)
        .mount(&server)
        .await;

    let cached = client(&server);
    let order = cached.order_status("o-1").await.expect("the call succeeds");
    assert_eq!(order.status, MeasurementStatus::Processing);

    // Without a token the module authenticates once, then reuses it.
    let fresh_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_body("minted", 3600)))
        .mount(&fresh_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/orders/o-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(order_json("o-2", "ACCEPTED")))
        .mount(&fresh_server)
        .await;

    let bare = bare_client(&fresh_server);
    bare.order_status("o-2")
        .await
        .expect("authenticates then calls");
    bare.order_status("o-2")
        .await
        .expect("reuses the cached token");

    let token_calls = fresh_server
        .received_requests()
        .await
        .expect("requests are recorded")
        .into_iter()
        .filter(|request| request.url.path() == "/oauth2/token")
        .count();
    assert_eq!(token_calls, 1, "the token is fetched exactly once");
}

#[tokio::test]
async fn ac_003_an_order_posts_the_request_and_returns_its_id_and_status() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(order_json("o-77", "ACCEPTED")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let order = client
        .order_measurement(&request(), "estimate-42", true)
        .await
        .expect("the order is created");

    assert_eq!(order.id, "o-77");
    assert_eq!(order.status, MeasurementStatus::Accepted);
    assert!(order.cost_acknowledged, "the acknowledgement is recorded");

    let sent = server
        .received_requests()
        .await
        .expect("requests are recorded");
    let body: serde_json::Value = serde_json::from_slice(&sent[0].body).expect("json body");
    assert_eq!(body["addressLine"], "1 Main St");
    assert_eq!(body["city"], "Springfield");
    assert_eq!(body["state"], "IL");
    assert_eq!(body["postalCode"], "62704");
    assert_eq!(
        sent[0]
            .headers
            .get("idempotency-key")
            .map(|value| value.to_str().unwrap()),
        Some("estimate-42"),
        "the caller key travels with the order"
    );
}

#[tokio::test]
async fn ac_004_a_repeated_key_returns_the_order_without_a_second_charge() {
    let server = MockServer::start().await;

    // Exactly one order may reach the provider for this key.
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(order_json("o-88", "ACCEPTED")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let first = client
        .order_measurement(&request(), "estimate-99", true)
        .await
        .expect("the first order succeeds");
    let second = client
        .order_measurement(&request(), "estimate-99", true)
        .await
        .expect("the repeat returns the same order");

    assert_eq!(first.id, second.id, "no second billable order is created");

    // The same key against a different address is refused (EC-004).
    let other = MeasurementRequest::new("9 Other Rd", "Shelbyville", "IL", "62565");
    let error = client
        .order_measurement(&other, "estimate-99", true)
        .await
        .expect_err("a key reuse with another address is an error");
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_005_no_order_without_an_explicit_cost_acknowledgement() {
    let server = MockServer::start().await;

    // The guard is local: nothing may reach the provider.
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let client = bare_client(&server);
    let error = client
        .order_measurement(&request(), "estimate-1", false)
        .await
        .expect_err("an unacknowledged paid order is refused");

    assert_eq!(error.code(), "W2006");
    assert!(
        error.to_string().to_lowercase().contains("cost"),
        "the error names the missing acknowledgement: {error}"
    );
}

#[tokio::test]
async fn ac_006_order_status_maps_into_the_typed_state() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/o-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(order_json("o-1", "PROCESSING")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/o-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(order_json("o-2", "COMPLETE")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);

    let processing = client.order_status("o-1").await.expect("status read");
    assert_eq!(processing.status, MeasurementStatus::Processing);
    assert!(!processing.status.is_complete());
    assert!(!processing.status.is_terminal());

    let complete = client.order_status("o-2").await.expect("status read");
    assert_eq!(complete.status, MeasurementStatus::Complete);
    assert!(complete.status.is_complete());
    assert!(complete.status.is_terminal());
}

#[tokio::test]
async fn ac_007_a_failed_order_surfaces_its_reason_and_is_not_retried() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/o-bad"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "o-bad",
            "status": "FAILED",
            "failureReason": "imagery unavailable for this address"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let order = client
        .order_status("o-bad")
        .await
        .expect("the status is readable");

    assert_eq!(order.status, MeasurementStatus::Failed);
    assert_eq!(
        order.failure_reason(),
        Some("imagery unavailable for this address"),
        "the reason is surfaced"
    );
    assert!(order.status.is_terminal(), "waiting longer cannot help");
    // `.expect(1)` proves the failed order was not polled again by the module.
}

#[tokio::test]
async fn ac_008_the_report_carries_area_pitch_and_ordered_segments() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/o-1/report"))
        .respond_with(ResponseTemplate::new(200).set_body_json(report_json("o-1")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let report = client
        .fetch_report("o-1")
        .await
        .expect("the report is fetched");

    assert_eq!(report.order_id, "o-1");
    assert_eq!(report.total_area.squares, 28.5);
    assert_eq!(report.total_area.facets, 12);
    assert_eq!(report.total_area.waste_factor, 0.12);
    assert_eq!(report.pitch.ratio, "6/12");
    assert_eq!(report.pitch.rise_over_run, 0.5);

    assert_eq!(report.segments.len(), 2);
    assert_eq!(
        report.segments[0].kind,
        SegmentKind::Ridge,
        "provider order is preserved"
    );
    assert_eq!(report.segments[0].area_squares, 2.5);
    assert_eq!(report.segments[1].kind, SegmentKind::Valley);
    assert_eq!(report.segments[1].pitch.ratio, "8/12");
    assert!(report.document_url.is_some());
}

#[tokio::test]
async fn ac_009_the_pdf_download_validates_the_content_type() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/o-1/report/document"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/pdf")
                .set_body_bytes(b"%PDF-1.7 report".to_vec()),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/o-2/report/document"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string("<html>not a report</html>"),
        )
        .mount(&server)
        .await;

    let client = client(&server);

    let document = client
        .download_pdf("o-1")
        .await
        .expect("the pdf is returned");
    assert!(document.content_type.starts_with("application/pdf"));
    assert_eq!(
        document.bytes, b"%PDF-1.7 report",
        "the body is returned bounded"
    );

    let error = client
        .download_pdf("o-2")
        .await
        .expect_err("a non-pdf document is refused");
    assert_eq!(
        error.code(),
        "W2007",
        "a wrong content type is a decode error"
    );
}

#[tokio::test]
async fn ac_010_reports_are_listed_for_an_address_with_a_bound() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/reports"))
        .and(query_param("address", "1 Main St, Springfield, IL 62704"))
        .and(query_param("limit", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "reports": [report_json("o-1"), report_json("o-9")]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let reports = client
        .list_reports("1 Main St, Springfield, IL 62704", 5)
        .await
        .expect("the listing succeeds");

    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].order_id, "o-1");
    assert_eq!(reports[1].order_id, "o-9");

    let error = client
        .list_reports("1 Main St, Springfield, IL 62704", 0)
        .await
        .expect_err("an unbounded listing is refused");
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_011_rejection_unmeasurable_and_processing_failure_stay_distinct() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .respond_with(fault(400, "ORDER_REJECTED", "the order was rejected"))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/unmeasurable"))
        .respond_with(fault(
            422,
            "ADDRESS_NOT_MEASURABLE",
            "no imagery for this address",
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/broken"))
        .respond_with(fault(
            500,
            "PROCESSING_FAILED",
            "measurement processing failed",
        ))
        .mount(&server)
        .await;

    let client = client(&server);

    let rejected = client
        .order_measurement(&request(), "key-1", true)
        .await
        .expect_err("a rejected order is an error");
    let unmeasurable = client
        .order_status("unmeasurable")
        .await
        .expect_err("an unmeasurable address is an error");
    let failed = client
        .order_status("broken")
        .await
        .expect_err("a processing failure is an error");

    assert_eq!(rejected.api_code(), Some("ORDER_REJECTED"));
    assert_eq!(unmeasurable.api_code(), Some("ADDRESS_NOT_MEASURABLE"));
    assert_eq!(failed.api_code(), Some("PROCESSING_FAILED"));
    assert_ne!(
        rejected.api_code(),
        unmeasurable.api_code(),
        "a rejection is distinguishable from an unmeasurable address"
    );
    assert_ne!(
        unmeasurable.api_code(),
        failed.api_code(),
        "and both from a processing failure"
    );
}

#[tokio::test]
async fn ac_012_a_rate_limit_is_retryable_and_a_rejection_is_not() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "0")
                .set_body_json(serde_json::json!({ "code": "THROTTLED", "message": "slow down" })),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/orders/o-rejected"))
        .respond_with(fault(400, "ORDER_REJECTED", "the order was rejected"))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);

    let throttled = client
        .order_measurement(&request(), "key-2", true)
        .await
        .expect_err("a throttled order fails after its retries");
    assert!(
        throttled.is_retryable() || throttled.code() == "W2003",
        "a 429 stays retryable: {throttled}"
    );

    let rejected = client
        .order_status("o-rejected")
        .await
        .expect_err("a rejection is surfaced");
    assert!(
        !rejected.is_retryable(),
        "a rejected order is never retried"
    );
    // `.expect(1)` proves the rejection was attempted exactly once.
}

#[test]
fn ac_013_a_cleartext_base_url_is_rejected() {
    // The transport refuses cleartext for anything that is not loopback, so a
    // misconfigured client cannot send credentials in the open (CON-520).
    let builder =
        WirekitClient::builder("http://api.eagleview.com").expect("the builder accepts any URL");
    let error = match builder.build() {
        Ok(_) => panic!("a cleartext base URL must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "W2006");

    // The module keeps no environment state: credentials come from the config.
    let loopback = WirekitClient::builder("http://127.0.0.1:1")
        .expect("the builder accepts a loopback base")
        .allow_insecure_localhost(true)
        .build()
        .expect("the transport builds");
    let client = EagleViewClient::new(config(), loopback);
    assert_eq!(client.config().client_id, "client-abc");
}

//! Acceptance tests for the Klarna module (AC-001 to AC-005).

use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::json;
use wirekit::klarna::{
    CaptureRequest, CheckoutSessionRequest, KlarnaClient, KlarnaConfig, KlarnaRegion, MerchantUrls,
    OrderLine, RefundRequest,
};
use wirekit::{Secret, WirekitError};
use wiremock::MockServer;

use crate::common::{self, Fixture};

fn config(server: &MockServer) -> KlarnaConfig {
    KlarnaConfig::new(
        Secret::new(String::from("merchant-user")),
        Secret::new(String::from(common::API_KEY)),
        KlarnaRegion::NorthAmerica,
    )
    .with_base_url(server.uri())
}

fn client(server: &MockServer) -> KlarnaClient {
    KlarnaClient::new(common::transport(server), config(server)).expect("the client builds")
}

fn session_request() -> CheckoutSessionRequest {
    CheckoutSessionRequest::new("US", "USD", "en-US", 120_000)
        .with_order_tax_amount(9_600)
        .with_order_line(
            OrderLine::new("Roof replacement", 1, 120_000, 120_000).with_reference("line-1"),
        )
        .with_merchant_urls(
            MerchantUrls::new()
                .with_confirmation("https://example.com/confirmed")
                .with_notification("https://example.com/klarna/webhook"),
        )
}

fn session_json() -> serde_json::Value {
    json!({
        "session_id": "s1",
        "client_token": "client-token",
        "payment_method_categories": [{
            "identifier": "pay_later",
            "name": "Pay later",
            "asset_urls": { "descriptive": "https://example.com/pay-later.svg" }
        }]
    })
}

fn order_json() -> serde_json::Value {
    json!({
        "order_id": "o1",
        "status": "complete",
        "fraud_status": "ACCEPTED",
        "order_amount": 120_000,
        "original_order_amount": 120_000,
        "captured_amount": 0,
        "refunded_amount": 0,
        "purchase_currency": "USD",
        "order_lines": [{
            "name": "Roof replacement",
            "quantity": 1,
            "unit_price": 120_000,
            "total_amount": 120_000
        }],
        "klarna_reference": "K123",
        "created_at": "2026-11-24T09:00:00Z"
    })
}

#[tokio::test]
async fn ac_001_klarna_surface_matches_the_catalogue() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/payments/v1/sessions",
        Fixture::json(200, session_json()),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/payments/v1/sessions/s1",
        Fixture::json(200, session_json()),
    )
    .await;
    common::mount(
        &server,
        "POST",
        "/payments/v1/authorizations/auth-1/order",
        Fixture::json(
            200,
            json!({
                "order_id": "o1",
                "status": "complete",
                "fraud_status": "ACCEPTED",
                "redirect_url": "https://example.com/confirmed"
            }),
        ),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/ordermanagement/v1/orders/o1",
        Fixture::json(200, order_json()),
    )
    .await;
    common::mount(
        &server,
        "POST",
        "/ordermanagement/v1/orders/o1/captures",
        Fixture::json(201, json!({ "capture_id": "c1" })),
    )
    .await;
    common::mount(
        &server,
        "POST",
        "/ordermanagement/v1/orders/o1/refunds",
        Fixture::json(201, json!({ "refund_id": "r1" })),
    )
    .await;

    let client = client(&server);

    let session = client
        .create_session(&session_request())
        .await
        .expect("session creation succeeds");
    assert_eq!(session.session_id, "s1");
    assert_eq!(session.client_token.as_deref(), Some("client-token"));
    assert_eq!(session.payment_method_categories.len(), 1);
    assert_eq!(
        session.payment_method_categories[0].identifier.as_deref(),
        Some("pay_later")
    );

    let read = client
        .get_session("s1")
        .await
        .expect("session read succeeds");
    assert_eq!(read.session_id, "s1");

    let created = client
        .create_order("auth-1")
        .await
        .expect("order creation succeeds");
    assert_eq!(created.order_id, "o1");
    assert_eq!(created.status.as_deref(), Some("complete"));
    assert_eq!(created.fraud_status.as_deref(), Some("ACCEPTED"));
    assert_eq!(
        created.redirect_url.as_deref(),
        Some("https://example.com/confirmed")
    );

    let order = client.get_order("o1").await.expect("order read succeeds");
    assert_eq!(order.order_id, "o1");
    assert_eq!(order.captured_amount, Some(0));
    assert_eq!(order.refunded_amount, Some(0));
    assert_eq!(order.purchase_currency.as_deref(), Some("USD"));
    assert_eq!(order.order_lines[0].name, "Roof replacement");
    assert_eq!(order.klarna_reference.as_deref(), Some("K123"));

    let capture = client
        .capture_order(
            "o1",
            &CaptureRequest::new(120_000).with_description("Full capture"),
        )
        .await
        .expect("capture succeeds");
    assert_eq!(capture.capture_id, "c1");

    let refund = client
        .refund_order(
            "o1",
            &RefundRequest::new(20_000).with_description("Partial refund"),
        )
        .await
        .expect("refund succeeds");
    assert_eq!(refund.refund_id, "r1");

    let requests = common::requests(&server).await;
    assert_eq!(requests.len(), 6);

    let expected_auth = format!(
        "Basic {}",
        BASE64.encode("merchant-user:test_credential_do_not_log_1234")
    );
    for request in &requests {
        assert_eq!(
            common::header(request, "authorization"),
            Some(expected_auth.as_str()),
            "every call uses the shared Basic auth layer"
        );
    }

    let create = common::json(&requests[0]);
    assert_eq!(create["purchase_country"], "US");
    assert_eq!(create["purchase_currency"], "USD");
    assert_eq!(create["order_amount"], 120_000);
    assert_eq!(create["order_tax_amount"], 9_600);
    assert_eq!(create["order_lines"][0]["name"], "Roof replacement");
    assert_eq!(create["order_lines"][0]["reference"], "line-1");
    assert_eq!(
        create["merchant_urls"]["confirmation"],
        "https://example.com/confirmed"
    );

    assert_eq!(common::json(&requests[4])["captured_amount"], 120_000);
    assert_eq!(common::json(&requests[5])["refunded_amount"], 20_000);

    // The region constants are the documented hosts, and the override wins.
    assert_eq!(KlarnaRegion::Europe.base_url(), "https://api.klarna.com");
    assert_eq!(
        KlarnaRegion::NorthAmerica.base_url(),
        "https://api-na.klarna.com"
    );
    assert_eq!(
        KlarnaRegion::Oceania.base_url(),
        "https://api-oc.klarna.com"
    );
    assert_eq!(
        client.config().base_url.as_deref(),
        Some(server.uri().as_str())
    );
}

#[tokio::test]
async fn ac_002_klarna_failures_are_wirekit_errors() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/payments/v1/sessions",
        Fixture::json(
            401,
            json!({ "error_code": "UNAUTHORIZED", "error_messages": ["Bad credentials"] }),
        ),
    )
    .await;
    let error = client(&server)
        .create_session(&session_request())
        .await
        .expect_err("a 401 is an error");
    assert!(matches!(error, WirekitError::Auth { .. }), "got {error:?}");

    let error = KlarnaClient::new(
        common::transport(&server),
        KlarnaConfig::new(
            Secret::new(String::new()),
            Secret::new(String::from(common::API_KEY)),
            KlarnaRegion::Europe,
        ),
    )
    .err()
    .expect("an empty merchant id is invalid");
    assert!(
        matches!(error, WirekitError::InvalidConfig { .. }),
        "got {error:?}"
    );

    let error = KlarnaClient::new(
        common::transport(&server),
        config(&server).with_base_url("   "),
    )
    .err()
    .expect("a blank override is invalid");
    assert!(
        matches!(error, WirekitError::InvalidConfig { .. }),
        "got {error:?}"
    );
}

#[tokio::test]
async fn ac_003_klarna_rejects_an_error_body_at_http_200() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/payments/v1/sessions/s1",
        Fixture::json(
            200,
            json!({ "error_code": "DECLINED", "error_messages": ["Application declined"] }),
        ),
    )
    .await;
    let error = client(&server)
        .get_session("s1")
        .await
        .expect_err("a 200 with an error envelope is not a success");
    assert!(
        matches!(error, WirekitError::Api { status: 200, .. }),
        "got {error:?}"
    );
    assert_eq!(error.api_code(), Some("DECLINED"));
    assert!(format!("{error}").contains("Application declined"));
}

#[tokio::test]
async fn ac_004_klarna_honours_retry_after_and_never_retries_a_decline() {
    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "GET",
        "/payments/v1/sessions/s1",
        Fixture::json(200, session_json())
            .first_status(429)
            .header("Retry-After", "1"),
    )
    .await;
    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let client = KlarnaClient::new(transport, config(&server)).expect("the client builds");

    let started = Instant::now();
    client.get_session("s1").await.expect("the retry succeeds");
    assert_eq!(fixture.hits(), 2);
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "Retry-After: 1 must delay the retry, elapsed {:?}",
        started.elapsed()
    );

    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "POST",
        "/payments/v1/sessions",
        Fixture::json(
            400,
            json!({ "error_code": "DECLINED", "error_messages": ["Application declined"] }),
        ),
    )
    .await;
    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let client = KlarnaClient::new(transport, config(&server)).expect("the client builds");
    let _ = client.create_session(&session_request()).await;
    assert_eq!(fixture.hits(), 1, "a decline is not retried");
}

#[tokio::test]
async fn ac_005_klarna_secrets_are_redacted() {
    let server = common::server().await;
    let rendered = format!("{:?}", config(&server));
    assert!(rendered.contains("***"));
    assert!(!rendered.contains(common::API_KEY));
    assert!(!rendered.contains("merchant-user"));
}

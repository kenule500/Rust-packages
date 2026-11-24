//! Acceptance tests for the Lob module (AC-001 to AC-005).

use std::time::{Duration, Instant};

use serde_json::json;
use wirekit::lob::{
    Address, LetterRequest, LobClient, LobConfig, MailKind, PostcardRequest, PostcardSize,
};
use wirekit::{RateLimitConfig, Secret, WirekitError};
use wiremock::MockServer;

use crate::common::{self, Fixture};

fn config() -> LobConfig {
    LobConfig::new(Secret::new(String::from(common::API_KEY))).with_test_mode()
}

fn client(server: &MockServer) -> LobClient {
    LobClient::new(common::transport(server), config()).expect("the client builds")
}

fn address() -> Address {
    Address::new("Ada Lovelace", "1 Main St", "Austin", "TX", "78701")
}

fn piece_json(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "status": "processed",
        "expected_delivery_date": "2026-12-01",
        "tracking_events": [{
            "id": "evt-1",
            "name": "postcard.created",
            "date_created": "2026-11-24T09:00:00Z",
            "resource_id": id
        }]
    })
}

#[tokio::test]
async fn ac_001_lob_surface_matches_the_catalogue() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/us_verifications",
        Fixture::json(
            200,
            json!({
                "id": "adr_1",
                "deliverability": "deliverable",
                "primary_line": "1 MAIN ST",
                "city": "AUSTIN",
                "state": "TX",
                "zip_code": "78701"
            }),
        ),
    )
    .await;
    common::mount(
        &server,
        "POST",
        "/postcards",
        Fixture::json(200, piece_json("psc_1")),
    )
    .await;
    common::mount(
        &server,
        "POST",
        "/letters",
        Fixture::json(200, piece_json("ltr_1")),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/postcards/psc_1",
        Fixture::json(200, piece_json("psc_1")),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/events",
        Fixture::json(
            200,
            json!({ "data": [{
                "id": "evt-1",
                "name": "postcard.delivered",
                "date_created": "2026-11-29T12:00:00Z",
                "resource_id": "psc_1"
            }] }),
        ),
    )
    .await;
    common::mount(
        &server,
        "DELETE",
        "/postcards/psc_1",
        Fixture::json(200, json!({ "id": "psc_1", "deleted": true })),
    )
    .await;

    let client = client(&server);

    let verified = client
        .verify_address(&address())
        .await
        .expect("verification succeeds");
    assert_eq!(verified.id.as_deref(), Some("adr_1"));
    assert!(verified.is_deliverable());

    let mut postcard = PostcardRequest::new(address(), "<html>front</html>", "<html>back</html>");
    postcard.size = Some(PostcardSize::Size6x9);
    let created = client
        .create_postcard(&postcard, "idem-1")
        .await
        .expect("postcard creation succeeds");
    assert_eq!(created.id, "psc_1");
    assert_eq!(created.kind, Some(MailKind::Postcard));
    assert_eq!(created.status.as_deref(), Some("processed"));
    assert_eq!(created.events.len(), 1);
    assert_eq!(created.events[0].name, "postcard.created");

    let mut letter = LetterRequest::new(address(), "<html>letter</html>");
    letter.send_date = Some(String::from("2026-12-01"));
    let created = client
        .create_letter(&letter, "idem-2")
        .await
        .expect("letter creation succeeds");
    assert_eq!(created.id, "ltr_1");
    assert_eq!(created.kind, Some(MailKind::Letter));

    let fetched = client
        .get_mail_piece(MailKind::Postcard, "psc_1")
        .await
        .expect("lookup succeeds");
    assert_eq!(fetched.id, "psc_1");

    let events = client
        .list_events("psc_1")
        .await
        .expect("event listing succeeds");
    assert_eq!(events[0].name, "postcard.delivered");

    let cancelled = client
        .cancel(MailKind::Postcard, "psc_1")
        .await
        .expect("cancellation succeeds");
    assert!(cancelled.cancelled);

    let requests = common::requests(&server).await;
    assert_eq!(requests.len(), 6);

    let verification = &requests[0];
    let body = common::json(verification);
    assert_eq!(body["address_line1"], "1 Main St");
    assert_eq!(body["city"], "Austin");
    assert_eq!(body["postal_code"], "78701");

    let postcard_request = &requests[1];
    assert_eq!(
        common::header(postcard_request, "Idempotency-Key"),
        Some("idem-1")
    );
    let body = common::json(postcard_request);
    assert_eq!(body["front"], "<html>front</html>");
    assert_eq!(body["back"], "<html>back</html>");
    assert_eq!(body["size"], "6x9");
    assert_eq!(body["to"]["name"], "Ada Lovelace");

    let letter_request = &requests[2];
    let body = common::json(letter_request);
    assert_eq!(body["file"], "<html>letter</html>");
    assert_eq!(body["send_date"], "2026-12-01");
    assert_eq!(body["from"], serde_json::Value::Null);

    assert!(common::query(&requests[4]).contains("resource_id=psc_1"));
}

#[tokio::test]
async fn ac_002_lob_failures_are_wirekit_errors() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/us_verifications",
        Fixture::json(
            401,
            json!({ "error": { "message": "Invalid API key", "code": "unauthorized" } }),
        ),
    )
    .await;

    let error = client(&server)
        .verify_address(&address())
        .await
        .expect_err("a 401 is an error");
    assert!(matches!(error, WirekitError::Auth { .. }), "got {error:?}");

    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "POST",
        "/us_verifications",
        Fixture::json(
            422,
            json!({ "error": { "message": "Undeliverable address", "code": "invalid_address" } }),
        ),
    )
    .await;
    let error = client(&server)
        .verify_address(&address())
        .await
        .expect_err("a 422 is an error");
    assert!(
        matches!(error, WirekitError::Api { status: 422, .. }),
        "got {error:?}"
    );
    assert_eq!(error.api_code(), Some("invalid_address"));
    assert_eq!(fixture.hits(), 1, "a verification failure is not retried");
}

#[tokio::test]
async fn ac_003_lob_rejects_an_error_body_at_http_200() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/us_verifications",
        Fixture::json(
            200,
            json!({ "error": { "message": "Address rejected", "code": "invalid_address" } }),
        ),
    )
    .await;

    let error = client(&server)
        .verify_address(&address())
        .await
        .expect_err("a 200 with an error envelope is not a success");
    assert!(
        matches!(error, WirekitError::Api { status: 200, .. }),
        "got {error:?}"
    );
    assert_eq!(error.api_code(), Some("invalid_address"));
}

#[tokio::test]
async fn ac_004_lob_honours_retry_after_and_never_retries_another_4xx() {
    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "POST",
        "/us_verifications",
        Fixture::json(200, json!({ "deliverability": "deliverable" }))
            .first_status(429)
            .header("Retry-After", "1"),
    )
    .await;

    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let client = LobClient::new(transport, config()).expect("the client builds");

    let started = Instant::now();
    let verified = client
        .verify_address(&address())
        .await
        .expect("the retry succeeds");
    assert!(verified.is_deliverable());
    assert_eq!(fixture.hits(), 2, "one retry after the 429");
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "Retry-After: 1 must delay the retry, elapsed {:?}",
        started.elapsed()
    );

    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "POST",
        "/us_verifications",
        Fixture::json(
            400,
            json!({ "error": { "message": "Bad request", "code": "bad_request" } }),
        ),
    )
    .await;
    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let client = LobClient::new(transport, config()).expect("the client builds");

    let error = client
        .verify_address(&address())
        .await
        .expect_err("a 400 is an error");
    assert!(
        matches!(error, WirekitError::Api { status: 400, .. }),
        "got {error:?}"
    );
    assert_eq!(fixture.hits(), 1, "a 4xx other than 429 is not retried");
}

#[tokio::test]
async fn ac_005_lob_applies_its_rate_limit_key_and_redacts_the_key() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/us_verifications",
        Fixture::json(200, json!({ "deliverability": "deliverable" })),
    )
    .await;

    let transport = common::builder(&server)
        .rate_limit_for(
            "lob",
            RateLimitConfig::new(1, 0.001).expect("a valid bucket"),
        )
        .build()
        .expect("the transport builds");
    let client = LobClient::new(transport, config()).expect("the client builds");

    client
        .verify_address(&address())
        .await
        .expect("the first call takes the only token");
    let blocked = tokio::time::timeout(
        Duration::from_millis(500),
        client.verify_address(&address()),
    )
    .await;
    assert!(
        blocked.is_err(),
        "the second call waits on the lob bucket, so the module applied its own rate-limit key"
    );

    let rendered = format!("{:?}", config());
    assert!(rendered.contains("***"));
    assert!(!rendered.contains(common::API_KEY));
}

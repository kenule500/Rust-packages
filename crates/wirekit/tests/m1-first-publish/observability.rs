//! Acceptance tests for the tracing events (AC-028 to AC-031).

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use wirekit::pipeline::RequestSpec;
use wirekit::rate_limit::TokenBucket;
use wirekit::{ApiKeyQuery, BearerAuth, JitterBounds, RateLimitConfig, RetryPolicy};

use crate::common;

#[tokio::test]
async fn ac_028_a_started_request_is_traced_with_a_redacted_url() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/v1/things",
        common::Recorder::json(200, json!({ "ok": true })),
    )
    .await;

    let client = common::client(&server);
    let spec = RequestSpec::get(client.url("v1/things").expect("the request URL"))
        .auth(Arc::new(ApiKeyQuery::new("api_key", "query_SECRET")));
    client.send(spec).await.expect("the request is sent");

    let event = capture.find("request started").expect("the start is traced");
    assert_eq!(event.target, "wirekit::request");
    assert_eq!(event.field("method"), Some("GET"));
    let url = event.field("url").expect("the URL is recorded");
    assert!(
        url.ends_with("/v1/things?api_key=***"),
        "the query value is redacted: {url}"
    );
    assert!(!url.contains("query_SECRET"));
}

#[tokio::test]
async fn ac_029_a_finished_request_records_its_duration() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/v1/timed",
        common::Recorder::json(200, json!({ "ok": true })).delay(20),
    )
    .await;

    common::client(&server)
        .get_json::<serde_json::Value>("v1/timed")
        .await
        .expect("the request succeeds");

    let event = capture
        .find("request finished")
        .expect("the completion is traced");
    assert_eq!(event.field("status"), Some("200"));
    let duration = event
        .field("duration_ms")
        .expect("the duration is recorded")
        .parse::<u64>()
        .expect("the duration is a number");
    assert!(
        duration >= 15,
        "the recorded duration must cover the server delay: {duration}"
    );
}

#[tokio::test]
async fn ac_030_retries_rate_limit_hits_and_errors_are_all_traced() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/v1/broken",
        common::Recorder::json(500, json!({ "code": "internal", "message": "boom" })),
    )
    .await;

    let client = common::builder(&server)
        .retry(RetryPolicy {
            attempts: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(1),
            jitter: JitterBounds::new(1.0, 1.0).expect("valid jitter bounds"),
        })
        .build()
        .expect("the client builds");
    let _ = client.get_json::<serde_json::Value>("v1/broken").await;

    let bucket = TokenBucket::new(RateLimitConfig::new(1, 100.0).expect("a valid bucket"));
    assert_eq!(bucket.try_acquire(), None);
    bucket.acquire().await;

    let messages = capture.messages();
    for expected in [
        "request started",
        "request finished",
        "retrying request",
        "request failed",
        "rate limit hit; waiting for a token",
    ] {
        assert!(
            messages.iter().any(|message| message == expected),
            "{expected} was not emitted; saw {messages:?}"
        );
    }

    let failed = capture.find("request failed").expect("the error is traced");
    assert_eq!(failed.field("code"), Some("W2001"));
}

#[tokio::test]
async fn ac_031_no_credential_appears_in_the_traced_output() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/v1/bearer",
        common::Recorder::json(200, json!({ "ok": true })),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/v1/query",
        common::Recorder::json(200, json!({ "ok": true })),
    )
    .await;

    let client = common::client(&server);

    let spec = RequestSpec::get(client.url("v1/bearer").expect("the request URL"))
        .auth(Arc::new(BearerAuth::new("bearer_SECRET_VALUE")));
    client.send(spec).await.expect("the request is sent");

    let spec = RequestSpec::get(client.url("v1/query").expect("the request URL"))
        .auth(Arc::new(ApiKeyQuery::new("api_key", "query_SECRET_VALUE")));
    client.send(spec).await.expect("the request is sent");

    let rendered = capture.rendered();
    assert!(!rendered.is_empty(), "events were captured");
    assert!(rendered.contains("request started"));
    for secret in ["bearer_SECRET_VALUE", "query_SECRET_VALUE"] {
        assert!(
            !rendered.contains(secret),
            "{secret} leaked into the traced output:\n{rendered}"
        );
    }
    assert!(
        rendered.contains("api_key=***"),
        "the query parameter name survives, its value does not"
    );
}

//! Acceptance tests for the shared request pipeline (AC-032 to AC-036).

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use wirekit::pipeline::RequestSpec;
use wirekit::sendgrid::ContactList;
use wirekit::{
    BasicAuth, BearerAuth, JitterBounds, RateLimitConfig, RetryPolicy, WirekitError,
};

use crate::common;

#[tokio::test]
async fn ac_032_auth_then_rate_limit_then_retry_then_decode() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        "/v1/pipeline",
        common::Recorder::json(200, json!({ "ok": true, "id": "one" })).first_status(500),
    )
    .await;

    let client = common::builder(&server)
        .rate_limit(RateLimitConfig::new(1, 4.0).expect("a valid bucket"))
        .retry(RetryPolicy {
            attempts: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(1),
            jitter: JitterBounds::new(1.0, 1.0).expect("valid jitter bounds"),
        })
        .build()
        .expect("the client builds");

    let started = Instant::now();
    let mut last_status = 0;
    for _ in 0..2 {
        let spec = RequestSpec::post(client.url("v1/pipeline").expect("the request URL"))
            .auth(Arc::new(BearerAuth::new("pipeline_token")))
            .json_body(&json!({ "hello": "world" }))
            .expect("a serializable body");
        let response = client
            .execute::<serde_json::Value>(spec)
            .await
            .expect("the retry recovers the request");
        last_status = response.status;
        assert_eq!(response.value()["id"], json!("one"));
    }
    let elapsed = started.elapsed();

    assert_eq!(last_status, 200);
    assert_eq!(
        recorder.hits(),
        3,
        "one retried request (2 attempts) plus one fresh request"
    );
    assert!(
        elapsed >= Duration::from_millis(200),
        "the token bucket waits before sending: {elapsed:?}"
    );

    for request in recorder.calls() {
        assert_eq!(
            request.header("authorization"),
            Some("Bearer pipeline_token"),
            "auth is applied to every attempt"
        );
        assert_eq!(request.method, "POST");
        assert_eq!(request.json()["hello"], json!("world"));
    }
}

#[tokio::test]
async fn ac_033_a_successful_response_is_decoded_into_the_callers_type() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/v1/list",
        common::Recorder::json(
            200,
            json!({ "id": "list_1", "name": "Newsletter", "contact_count": 42 }),
        ),
    )
    .await;

    let response = common::client(&server)
        .get_json::<ContactList>("v1/list")
        .await
        .expect("the body decodes");

    assert_eq!(response.status, 200);
    assert_eq!(response.value().id, "list_1");
    assert_eq!(response.value().name, "Newsletter");
    assert_eq!(response.value().contact_count, Some(42));
    assert_eq!(response.into_value().id, "list_1");
}

/// A GraphQL-style body hook: a 200 that carries `errors[]` is a failure.
fn graphql_hook(_status: u16, body: &[u8]) -> Option<WirekitError> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let errors = value.get("errors")?.as_array()?;
    if errors.is_empty() {
        return None;
    }
    Some(WirekitError::api(
        200,
        "graphql_error",
        "the body reported a GraphQL error",
    ))
}

#[tokio::test]
async fn ac_034_a_body_level_error_is_not_mistaken_for_success() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/v1/graphql",
        common::Recorder::json(200, json!({ "errors": [{ "message": "field is required" }] })),
    )
    .await;
    common::mount(
        &server,
        "POST",
        "/v1/graphql-ok",
        common::Recorder::json(200, json!({ "data": { "ok": true } })),
    )
    .await;

    let client = common::client(&server);

    let spec = RequestSpec::post(client.url("v1/graphql").expect("the request URL"))
        .body_error_hook(graphql_hook)
        .json_body(&json!({ "query": "{ failing }" }))
        .expect("a serializable body");
    let error = client
        .execute::<serde_json::Value>(spec)
        .await
        .expect_err("HTTP 200 with errors[] must fail");
    match error {
        WirekitError::Api {
            status,
            ref code,
            ..
        } => {
            assert_eq!(status, 200, "the status is the HTTP one");
            assert_eq!(code, "graphql_error");
        }
        other => panic!("expected Api, got {other:?}"),
    }

    let spec = RequestSpec::post(client.url("v1/graphql-ok").expect("the request URL"))
        .body_error_hook(graphql_hook)
        .json_body(&json!({ "query": "{ ok }" }))
        .expect("a serializable body");
    let response = client
        .execute::<serde_json::Value>(spec)
        .await
        .expect("a clean body succeeds");
    assert_eq!(response.value()["data"]["ok"], json!(true));
}

#[tokio::test]
async fn ac_035_a_malformed_body_is_a_decode_error_not_a_panic() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/v1/malformed",
        common::Recorder::new(200, "{not json at all"),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/v1/huge",
        common::Recorder::new(200, "0123456789abcdef"),
    )
    .await;

    let client = common::client(&server);
    let error = client
        .get_json::<serde_json::Value>("v1/malformed")
        .await
        .expect_err("invalid JSON must not decode");
    assert_eq!(error.code(), "W2007");

    let bounded = common::builder(&server)
        .max_response_bytes(8)
        .build()
        .expect("the client builds");
    let error = bounded
        .get_json::<serde_json::Value>("v1/huge")
        .await
        .expect_err("an oversized body is refused");
    assert_eq!(error.code(), "W2007");
    assert!(
        error.to_string().contains("limit"),
        "the message explains the bound: {error}"
    );
}

#[tokio::test]
async fn ac_036_missing_credentials_fail_before_any_request_is_sent() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/unauthenticated",
        common::Recorder::json(200, json!({ "ok": true })),
    )
    .await;

    let client = common::client(&server);

    let spec = RequestSpec::get(client.url("v1/unauthenticated").expect("the request URL"))
        .auth(Arc::new(BearerAuth::new("  ")));
    let error = client
        .send(spec)
        .await
        .expect_err("an empty bearer token must fail closed");
    assert_eq!(error.code(), "W2006");
    assert!(error.to_string().contains("bearer token"));

    let spec = RequestSpec::get(client.url("v1/unauthenticated").expect("the request URL"))
        .auth(Arc::new(BasicAuth::new("", "password")));
    let error = client
        .send(spec)
        .await
        .expect_err("an empty basic username must fail closed");
    assert_eq!(error.code(), "W2006");

    assert_eq!(
        recorder.hits(),
        0,
        "no request may reach the provider without a credential"
    );
}

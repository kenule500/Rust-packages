//! Acceptance tests for `RetryPolicy` and the retry loop (AC-006 to AC-011).

use std::time::Duration;

use serde_json::json;
use wirekit::{JitterBounds, RetryPolicy, WirekitError};

use crate::common;

/// A policy that retries fast and deterministically.
fn fast_policy(attempts: u32) -> RetryPolicy {
    RetryPolicy {
        attempts,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(2),
        jitter: JitterBounds::new(1.0, 1.0).expect("valid jitter bounds"),
    }
}

#[tokio::test]
async fn ac_006_a_5xx_is_retried_up_to_three_attempts_in_total() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/jobs",
        common::Recorder::json(500, json!({ "code": "internal", "message": "boom" })),
    )
    .await;

    let client = common::builder(&server)
        .retry(fast_policy(3))
        .build()
        .expect("the client builds");

    let error = client
        .get_json::<serde_json::Value>("v1/jobs")
        .await
        .expect_err("a permanent 500 must fail");

    assert_eq!(recorder.hits(), 3, "three attempts in total");
    match error {
        WirekitError::Api { status, .. } => assert_eq!(status, 500),
        other => panic!("expected the final failure to be Api, got {other:?}"),
    }
}

#[tokio::test]
async fn ac_007_retry_after_is_honoured_and_survives_an_exhausted_policy() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/limited",
        common::Recorder::json(429, json!({ "code": "rate_limited" }))
            .header("retry-after", "1"),
    )
    .await;

    let client = common::builder(&server)
        .retry(RetryPolicy {
            attempts: 2,
            base_delay: Duration::from_millis(1),
            // The ceiling keeps the test fast; the header value is still
            // recorded on the error.
            max_delay: Duration::from_millis(10),
            jitter: JitterBounds::new(1.0, 1.0).expect("valid jitter bounds"),
        })
        .build()
        .expect("the client builds");

    let error = client
        .get_json::<serde_json::Value>("v1/limited")
        .await
        .expect_err("a 429 must not be reported as success");

    assert_eq!(recorder.hits(), 2, "the 429 is retried once");
    match error {
        WirekitError::RateLimit { retry_after } => {
            assert_eq!(retry_after, Some(Duration::from_secs(1)));
        }
        other => panic!("expected RateLimit, got {other:?}"),
    }

    let retry = capture
        .find("retrying request")
        .expect("the retry is traced");
    assert_eq!(retry.field("code"), Some("W2003"));
    assert_eq!(
        retry.field("delay_ms"),
        Some("10"),
        "the delay honours the header but stays inside max_delay"
    );
}

#[tokio::test]
async fn ac_008_client_errors_are_not_retried() {
    for status in [400_u16, 401, 403, 404] {
        let server = common::server().await;
        let path = format!("/v1/status/{status}");
        let recorder = common::mount(
            &server,
            "GET",
            &path,
            common::Recorder::json(status, json!({ "code": "client_error" })),
        )
        .await;

        let client = common::builder(&server)
            .retry(fast_policy(3))
            .build()
            .expect("the client builds");

        let error = client
            .get_json::<serde_json::Value>(&format!("v1/status/{status}"))
            .await
            .expect_err("a 4xx must fail");

        assert_eq!(recorder.hits(), 1, "status {status} must not be retried");
        assert_eq!(error.code(), "W2001");
    }
}

#[tokio::test]
async fn ac_009_timeouts_and_network_failures_are_retried_then_reported() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/v1/slow",
        common::Recorder::json(200, json!({})).delay(300),
    )
    .await;

    let client = common::builder(&server)
        .timeout(Duration::from_millis(50))
        .retry(fast_policy(2))
        .build()
        .expect("the client builds");

    let timeout = client
        .get_json::<serde_json::Value>("v1/slow")
        .await
        .expect_err("a slow response must time out");
    assert_eq!(timeout.code(), "W2004");
    assert_eq!(
        capture.count("retrying request"),
        1,
        "the timeout is retried once"
    );

    // A loopback port with no listener in front of it: the connection is
    // refused, which is a transport failure, not a timeout. Port 1 is used
    // instead of a freshly released ephemeral port, which another test in this
    // parallel suite could rebind between the release and the connection.
    let base = "http://127.0.0.1:1";
    let client = wirekit::WirekitClient::builder(base)
        .expect("a loopback base URL")
        .allow_insecure_localhost(true)
        .timeout(Duration::from_secs(5))
        .retry(fast_policy(2))
        .build()
        .expect("the client builds");

    let network = client
        .get_json::<serde_json::Value>("v1/unreachable")
        .await
        .expect_err("a refused connection must fail");
    assert_eq!(
        network.code(),
        "W2005",
        "a refused connection is a transport failure, got {network}"
    );
    assert!(
        network.to_string().contains("could not connect"),
        "the message was {network}"
    );
}

#[test]
fn ac_010_delays_grow_exponentially_with_bounded_jitter() {
    let deterministic = RetryPolicy {
        attempts: 6,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_millis(500),
        jitter: JitterBounds::new(1.0, 1.0).expect("valid jitter bounds"),
    };
    assert_eq!(deterministic.delay_for(1), Duration::from_millis(100));
    assert_eq!(deterministic.delay_for(2), Duration::from_millis(200));
    assert_eq!(deterministic.delay_for(3), Duration::from_millis(400));
    assert_eq!(
        deterministic.delay_for(4),
        Duration::from_millis(500),
        "the delay is capped at max_delay"
    );
    assert_eq!(
        deterministic.delay_for(64),
        Duration::from_millis(500),
        "a large attempt number saturates instead of overflowing"
    );

    let jittered = RetryPolicy {
        jitter: JitterBounds::new(0.5, 1.5).expect("valid jitter bounds"),
        ..deterministic
    };
    for attempt in 1..=8_u32 {
        let delay = jittered.delay_for(attempt).as_secs_f64();
        let expected = 0.1_f64 * 2_f64.powi(attempt as i32 - 1);
        // The exponential growth is capped at max_delay before the jitter is
        // applied, so the window narrows once the cap is reached.
        let capped = expected.min(0.5);
        let low = capped * 0.5;
        let high = (capped * 1.5).min(0.5);
        assert!(
            delay >= low - 1e-9,
            "attempt {attempt}: {delay} is below {low}"
        );
        assert!(
            delay <= high + 1e-9,
            "attempt {attempt}: {delay} is above {high}"
        );
        assert!(
            delay <= 0.5 + 1e-9,
            "attempt {attempt}: {delay} exceeded the ceiling"
        );
    }

    assert_eq!(
        jittered.delay_before_retry(1, Some(Duration::from_secs(2))),
        Duration::from_millis(500),
        "a longer Retry-After wins, capped at max_delay"
    );
    assert_eq!(
        deterministic.delay_before_retry(1, Some(Duration::from_millis(1))),
        Duration::from_millis(100),
        "a shorter Retry-After does not shrink the backoff"
    );

    assert!(JitterBounds::new(1.5, 0.5).is_err(), "inverted bounds fail");
    assert!(JitterBounds::new(-1.0, 1.0).is_err(), "negative bounds fail");
    assert!(
        RetryPolicy {
            attempts: 0,
            ..RetryPolicy::default()
        }
        .validate()
        .is_err(),
        "zero attempts fail"
    );
    assert!(RetryPolicy::none().validate().is_ok());
    assert_eq!(RetryPolicy::none().attempts, 1);
}

#[tokio::test]
async fn ac_011_a_retry_emits_an_event_with_the_attempt_and_delay() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/v1/failing",
        common::Recorder::json(500, json!({ "code": "internal" })),
    )
    .await;

    let client = common::builder(&server)
        .retry(RetryPolicy {
            attempts: 2,
            base_delay: Duration::from_millis(5),
            max_delay: Duration::from_millis(5),
            jitter: JitterBounds::new(1.0, 1.0).expect("valid jitter bounds"),
        })
        .build()
        .expect("the client builds");

    let _ = client.get_json::<serde_json::Value>("v1/failing").await;

    let events = capture.matching("retrying request");
    assert_eq!(events.len(), 1, "one retry, one event");
    assert_eq!(events[0].field("attempt"), Some("1"));
    assert_eq!(events[0].field("delay_ms"), Some("5"));
    assert_eq!(events[0].field("code"), Some("W2001"));
    assert_eq!(events[0].target, "wirekit::request");
}

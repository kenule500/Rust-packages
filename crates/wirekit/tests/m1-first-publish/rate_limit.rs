//! Acceptance tests for the token-bucket rate limiter (AC-012 to AC-015).

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use wirekit::rate_limit::TokenBucket;
use wirekit::RateLimitConfig;

use crate::common;

#[tokio::test]
async fn ac_012_the_limiter_throttles_requests_above_the_configured_rate() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/paced",
        common::Recorder::json(200, json!({ "ok": true })),
    )
    .await;

    let client = common::builder(&server)
        .rate_limit(RateLimitConfig::new(2, 10.0).expect("a valid bucket"))
        .build()
        .expect("the client builds");

    let started = Instant::now();
    for _ in 0..5 {
        client
            .get_json::<serde_json::Value>("v1/paced")
            .await
            .expect("the request is allowed");
    }
    let elapsed = started.elapsed();

    assert_eq!(recorder.hits(), 5, "every request is still sent");
    // Two tokens are free; the other three wait one refill each (100 ms).
    assert!(
        elapsed >= Duration::from_millis(250),
        "the limiter did not throttle: {elapsed:?}"
    );
}

#[test]
fn ac_013_each_provider_uses_its_own_bucket_and_override() {
    let bursty = TokenBucket::new(RateLimitConfig::new(5, 5.0).expect("a valid bucket"));
    let strict = TokenBucket::new(RateLimitConfig::new(1, 1.0).expect("a valid bucket"));

    assert_eq!(bursty.config(), RateLimitConfig::new(5, 5.0).expect("a valid bucket"));
    assert_eq!(strict.config(), RateLimitConfig::new(1, 1.0).expect("a valid bucket"));

    for _ in 0..5 {
        assert_eq!(bursty.try_acquire(), None, "the bursty bucket has five tokens");
    }
    assert!(
        bursty.try_acquire().is_some(),
        "the bursty bucket is empty after five"
    );

    assert_eq!(strict.try_acquire(), None, "the strict bucket starts full");
    let wait = strict
        .try_acquire()
        .expect("the strict bucket is now empty");
    assert!(
        wait >= Duration::from_millis(900) && wait <= Duration::from_millis(1_100),
        "a 1/s bucket waits about a second, not {wait:?}"
    );

    // Draining one bucket leaves the other alone.
    assert!(
        bursty.available() < 1.0,
        "the bursty bucket was drained above"
    );
    assert!(
        strict.available() < 1.0,
        "the strict bucket keeps its own state"
    );
    let fresh = TokenBucket::new(RateLimitConfig::new(3, 3.0).expect("a valid bucket"));
    assert_eq!(
        fresh.available(),
        3.0,
        "a new bucket is full, independent of the others"
    );

    assert!(RateLimitConfig::new(0, 1.0).is_err(), "zero capacity fails");
    assert!(
        RateLimitConfig::new(1, 0.0).is_err(),
        "zero refill fails"
    );
    assert_eq!(
        RateLimitConfig::per_second(4.0).expect("a valid bucket"),
        RateLimitConfig::new(4, 4.0).expect("a valid bucket")
    );
}

#[tokio::test]
async fn ac_014_a_rate_limit_hit_emits_an_event() {
    let (capture, _guard) = common::capture();

    let bucket = TokenBucket::new(RateLimitConfig::new(1, 100.0).expect("a valid bucket"));
    assert_eq!(bucket.try_acquire(), None);

    bucket.acquire().await;

    let event = capture
        .find("rate limit hit; waiting for a token")
        .expect("the wait is traced");
    assert_eq!(event.target, "wirekit::rate_limit");
    assert_eq!(event.field("capacity"), Some("1"));
    assert_eq!(event.field("refill_per_second"), Some("100.0"));
    assert!(
        event.field("wait_ms").is_some(),
        "the event records how long the wait is"
    );
}

#[tokio::test]
async fn ac_015_a_waiting_request_yields_the_runtime_thread() {
    let bucket = Arc::new(TokenBucket::new(
        RateLimitConfig::new(1, 10.0).expect("a valid bucket"),
    ));
    assert_eq!(bucket.try_acquire(), None, "the bucket starts empty");

    let started = Instant::now();
    // `#[tokio::test]` uses the current-thread runtime. A blocking sleep in
    // `acquire` would starve the second future, so the observer would only run
    // after the wait finished.
    let (acquired_at, observed_at) = tokio::join!(
        async {
            bucket.acquire().await;
            started.elapsed()
        },
        async {
            tokio::task::yield_now().await;
            started.elapsed()
        }
    );

    assert!(
        observed_at < Duration::from_millis(50),
        "the runtime was blocked for {observed_at:?} instead of yielding"
    );
    assert!(
        acquired_at >= Duration::from_millis(80),
        "the empty bucket did not delay the request: {acquired_at:?}"
    );

    for (name, source) in common::crate_sources() {
        assert!(
            !source.contains("thread::sleep"),
            "{name} blocks a runtime thread with a synchronous sleep"
        );
    }
}

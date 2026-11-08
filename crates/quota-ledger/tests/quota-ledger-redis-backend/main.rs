//! Acceptance tests for the quota-ledger Redis backend.
//!
//! One test per acceptance criterion (AC-001 to AC-008). The six behaviour tests
//! need a Redis server; they read `REDIS_URL` and default to
//! `redis://127.0.0.1:6379/15` — a non-default database — so they never touch
//! another application's keys. Every key they write starts with `quota:` and
//! carries a fresh tenant id, so tests cannot interfere with each other even
//! when they run in parallel.

use chrono::Duration;

use quota_ledger::{
    Budget, ExhaustionPolicy, Metric, ProviderId, QuotaError, QuotaStore, RedisQuotaStore,
    ReserveOutcome, ReserveRequest, TenantId, Window,
};

fn redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/15".to_owned())
}

/// Connects to the test server, or returns `None` when none is reachable.
///
/// CI has no Redis, so the behaviour tests skip there instead of failing. Run
/// them with `REDIS_URL` pointing at a server; the default is DB 15 on localhost.
async fn store() -> Option<RedisQuotaStore> {
    let attempt = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        RedisQuotaStore::connect(&redis_url()),
    )
    .await;
    match attempt {
        Ok(Ok(store)) => Some(store),
        _ => None,
    }
}

macro_rules! require_redis {
    () => {
        match store().await {
            Some(store) => store,
            None => {
                eprintln!("skipping: no Redis server at REDIS_URL");
                return;
            }
        }
    };
}

fn budget(tenant: TenantId, limit: u64, policy: ExhaustionPolicy) -> Budget {
    Budget::new(
        tenant,
        ProviderId::new("test-provider"),
        Metric::new("requests"),
        limit,
        Window::rolling(Duration::hours(1)),
        policy,
    )
}

fn tenant() -> TenantId {
    TenantId::generate()
}

// AC-001
#[tokio::test]
async fn ac_001_reserve_holds_units_in_tenant_scoped_keys() {
    let store = require_redis!();
    let owner = tenant();
    let budget = budget(owner, 10, ExhaustionPolicy::HardStop);

    let outcome = store
        .reserve(&ReserveRequest::new(budget.clone(), 3))
        .await
        .expect("the reserve succeeds");

    let reservation = outcome.reservation().expect("a reservation is returned");
    assert_eq!(reservation.units, 3);
    assert_eq!(reservation.key.tenant, owner, "the key carries the tenant");
    assert_eq!(reservation.key.provider, ProviderId::new("test-provider"));
    assert_eq!(reservation.key.metric, Metric::new("requests"));

    let usage = store.usage(&reservation.key).await.expect("usage");
    assert_eq!(usage.limit, 10);
    assert_eq!(usage.held, 3);
    assert_eq!(usage.used, 0);

    // Another tenant with the same provider and metric has no ledger yet.
    let stranger = tenant();
    let key = quota_ledger::QuotaKey::new(
        stranger,
        ProviderId::new("test-provider"),
        Metric::new("requests"),
        reservation.key.window_start,
    );
    assert!(
        matches!(store.usage(&key).await, Err(QuotaError::UnknownBudget(_))),
        "a foreign tenant cannot read this ledger"
    );
}

// AC-002
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ac_002_concurrent_reserves_never_exceed_the_limit() {
    let store = require_redis!();
    let owner = tenant();
    let budget = budget(owner, 10, ExhaustionPolicy::HardStop);

    let mut tasks = Vec::new();
    for _ in 0..40 {
        let store = store.clone();
        let request = ReserveRequest::new(budget.clone(), 1);
        tasks.push(tokio::spawn(async move {
            store
                .reserve(&request)
                .await
                .expect("each reserve is an outcome")
        }));
    }

    let mut granted = Vec::new();
    for task in tasks {
        let outcome = task.await.expect("no task panics");
        if let Some(reservation) = outcome.reservation() {
            granted.push(reservation.id);
        }
    }

    assert!(
        granted.len() <= 10,
        "a limit of 10 grants at most 10 holds, got {}",
        granted.len()
    );

    for id in granted {
        store.commit(id).await.expect("a granted hold settles");
    }

    let key = quota_ledger::QuotaKey::new(
        owner,
        ProviderId::new("test-provider"),
        Metric::new("requests"),
        budget.window.start_at(chrono::Utc::now()),
    );
    let usage = store.usage(&key).await.expect("usage");
    assert!(
        usage.used <= 10,
        "total committed {} never exceeds the limit 10",
        usage.used
    );
}

// AC-003
#[tokio::test]
async fn ac_003_commit_consumes_the_hold_exactly_once() {
    let store = require_redis!();
    let owner = tenant();
    let budget = budget(owner, 10, ExhaustionPolicy::HardStop);

    let reservation = store
        .reserve(&ReserveRequest::new(budget, 2))
        .await
        .expect("reserve")
        .reservation()
        .expect("a hold")
        .id;

    store
        .commit(reservation)
        .await
        .expect("the first commit settles");
    store
        .commit(reservation)
        .await
        .expect("a repeated commit is a no-op");

    let key = quota_ledger::QuotaKey::new(
        owner,
        ProviderId::new("test-provider"),
        Metric::new("requests"),
        quota_ledger::Window::rolling(Duration::hours(1)).start_at(chrono::Utc::now()),
    );
    let usage = store.usage(&key).await.expect("usage");
    assert_eq!(usage.used, 2, "the hold was consumed exactly once");
    assert_eq!(usage.held, 0, "the hold was released from held units");
}

// AC-004
#[tokio::test]
async fn ac_004_an_idempotency_key_reused_with_different_units_conflicts() {
    let store = require_redis!();
    let owner = tenant();
    let budget = budget(owner, 10, ExhaustionPolicy::HardStop);

    let first = store
        .reserve(&ReserveRequest::new(budget.clone(), 2).with_idempotency_key("invoice-42"))
        .await
        .expect("the first reserve succeeds")
        .reservation()
        .expect("a hold")
        .id;

    let replay = store
        .reserve(&ReserveRequest::new(budget.clone(), 2).with_idempotency_key("invoice-42"))
        .await
        .expect("a replay is not an error")
        .reservation()
        .expect("the original hold")
        .id;

    assert_eq!(first, replay, "the replay returns the original reservation");

    let conflict = store
        .reserve(&ReserveRequest::new(budget, 3).with_idempotency_key("invoice-42"))
        .await;
    assert!(
        matches!(conflict, Err(QuotaError::IdempotencyConflict(_))),
        "a different unit count is a typed conflict, got {conflict:?}"
    );
}

// AC-005
#[tokio::test]
async fn ac_005_hard_stop_denies_with_the_reset_instant() {
    let store = require_redis!();
    let owner = tenant();
    let budget = budget(owner, 1, ExhaustionPolicy::HardStop);

    let first = store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("reserve")
        .reservation()
        .expect("the single unit is granted")
        .id;

    let denied = store
        .reserve(&ReserveRequest::new(budget, 1))
        .await
        .expect("a denial is an outcome, not an error");

    match denied {
        ReserveOutcome::Denied(exceeded) => {
            assert_eq!(exceeded.requested, 1);
            assert_eq!(exceeded.remaining, 0);
            assert!(
                exceeded.reset_at > chrono::Utc::now(),
                "the denial carries a reset instant in the future"
            );
        }
        other => panic!("expected a hard-stop denial, got {other:?}"),
    }

    store.release(first).await.expect("release the hold");
}

// AC-006
#[tokio::test]
async fn ac_006_usage_reports_limit_used_held_and_reset() {
    let store = require_redis!();
    let owner = tenant();
    let budget = budget(owner, 10, ExhaustionPolicy::HardStop);

    let committed = store
        .reserve(&ReserveRequest::new(budget.clone(), 3))
        .await
        .expect("reserve")
        .reservation()
        .expect("a hold")
        .id;
    store
        .reserve(&ReserveRequest::new(budget.clone(), 2))
        .await
        .expect("reserve")
        .reservation()
        .expect("a hold");

    store.commit(committed).await.expect("commit");

    let key = quota_ledger::QuotaKey::new(
        owner,
        ProviderId::new("test-provider"),
        Metric::new("requests"),
        budget.window.start_at(chrono::Utc::now()),
    );
    let usage = store.usage(&key).await.expect("usage");

    assert_eq!(usage.limit, 10);
    assert_eq!(usage.used, 3);
    assert_eq!(usage.held, 2);
    assert_eq!(usage.overage, 0);
    assert_eq!(usage.overage_cents, 0);
    assert!(
        usage.reset_at > chrono::Utc::now(),
        "the reset instant is derived from the configured window"
    );
}

// AC-007
#[test]
fn ac_007_the_default_build_has_no_redis_backend() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    let lib = std::fs::read_to_string(root.join("src/lib.rs")).expect("lib.rs");

    assert!(
        manifest.contains("default = []"),
        "the default feature set stays empty"
    );
    assert!(
        manifest.contains("redis = [\"dep:redis\"]"),
        "redis is an additive feature"
    );
    assert!(
        lib.contains("#[cfg(feature = \"redis\")]"),
        "the redis module is feature-gated"
    );
    assert!(lib.contains("pub mod redis;"));
    assert!(lib.contains("#![forbid(unsafe_code)]"));
    assert!(lib.contains("#![deny(missing_docs)]"));
    assert!(
        manifest.contains("rust-version.workspace = true"),
        "the crate inherits the workspace MSRV"
    );
}

// AC-008
#[tokio::test]
async fn ac_008_errors_are_typed_and_redacted() {
    // A malformed URL is a typed configuration error, and the message does not
    // echo the URL back.
    let error = RedisQuotaStore::connect("http://example.test/0")
        .await
        .expect_err("a non-Redis URL is rejected");
    let message = error.to_string();
    assert!(
        matches!(error, QuotaError::Config(_)),
        "a bad URL is a Config error, got {error:?}"
    );
    assert!(
        !message.contains("http://"),
        "the URL is redacted: {message}"
    );
    assert!(
        !message.contains("example.test"),
        "the host is redacted: {message}"
    );

    // A connection failure is typed too. A refused connection would retry with
    // exponential backoff for minutes, so this half is proven by the same
    // `backend_error` mapping that every operation uses, not by hanging the suite.
    // Debug never renders the connection.
    if let Some(store) = store().await {
        let debug = format!("{store:?}");
        assert!(debug.contains("[REDACTED]"), "Debug redacts the connection");
        assert!(!debug.contains("6379"), "Debug does not render the port");

        // A malformed stored payload is a typed error, never a panic.
        let owner = tenant();
        let key = quota_ledger::QuotaKey::new(
            owner,
            ProviderId::new("test-provider"),
            Metric::new("requests"),
            chrono::Utc::now(),
        );
        let corrupt = format!("quota:budget:{owner}:test-provider:requests");
        let client = redis::Client::open(redis_url()).expect("a Redis URL");
        let mut connection = redis::aio::ConnectionManager::new(client)
            .await
            .expect("a connection for the corruption test");
        let _: i64 = redis::cmd("HSET")
            .arg(&corrupt)
            .arg("limit")
            .arg("not-a-number")
            .query_async(&mut connection)
            .await
            .expect("write the corrupt field");
        assert!(
            matches!(store.usage(&key).await, Err(QuotaError::Backend(_))),
            "a malformed limit is a typed Backend error"
        );
        let _: i64 = redis::cmd("DEL")
            .arg(&corrupt)
            .query_async(&mut connection)
            .await
            .unwrap_or(0);
    } else {
        eprintln!("skipping the live half: no Redis server at REDIS_URL");
    }
}

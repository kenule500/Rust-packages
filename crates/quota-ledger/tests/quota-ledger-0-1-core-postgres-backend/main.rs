//! Acceptance tests for `quota-ledger-0-1-core-postgres-backend`
//! (AC-001 to AC-015). Every test name carries its AC id (CON-405).
//!
//! Scope: the public `QuotaStore` contract of `MemoryQuotaStore`, the window
//! arithmetic, and the crate-level hygiene the specification asks for. The
//! Postgres backend is exercised through a compile-time `QuotaStore` bound and
//! its shipped schema, because a live database is not available to the suite.
//!
//! Non-goal: these tests do not schedule a real Postgres instance and do not
//! measure runtime performance.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};
use quota_ledger::{
    Budget, ExhaustionPolicy, MemoryQuotaStore, Metric, Period, ProviderId, QuotaError, QuotaKey,
    QuotaStore, ReserveOutcome, ReserveRequest, TenantId, Window,
};

/// A rolling window long enough that no test crosses its boundary.
fn day() -> Window {
    Window::rolling(Duration::days(1))
}

/// A fresh budget for one tenant, provider, and metric.
fn budget(tenant: TenantId, metric: &'static str, limit: u64, policy: ExhaustionPolicy) -> Budget {
    Budget::new(
        tenant,
        ProviderId::new("acme"),
        Metric::new(metric),
        limit,
        day(),
        policy,
    )
}

/// The ledger key that selects `budget`'s bucket.
fn key_for(budget: &Budget) -> QuotaKey {
    QuotaKey::new(budget.tenant, budget.provider, budget.metric, Utc::now())
}

#[tokio::test]
async fn ac_001_a_quota_key_separates_metrics_for_one_provider() {
    let store = MemoryQuotaStore::new();
    let tenant = TenantId::generate();

    // Two metrics for the same tenant and provider are two isolated lines,
    // each with its own one-unit budget.
    let metric_a = budget(tenant, "requests", 1, ExhaustionPolicy::HardStop);
    let metric_b = budget(tenant, "messages", 1, ExhaustionPolicy::HardStop);

    let first = store
        .reserve(&ReserveRequest::new(metric_a.clone(), 1))
        .await
        .expect("metric A has capacity");
    assert!(first.is_allowed(), "metric A reserves its only unit");

    let second = store
        .reserve(&ReserveRequest::new(metric_b.clone(), 1))
        .await
        .expect("metric B is a separate line");
    assert!(
        second.is_allowed(),
        "filling metric A must not consume metric B's budget"
    );

    let usage_a = store
        .usage(&key_for(&metric_a))
        .await
        .expect("metric A exists");
    let usage_b = store
        .usage(&key_for(&metric_b))
        .await
        .expect("metric B exists");

    assert_eq!(usage_a.held, 1, "metric A holds its own unit");
    assert_eq!(usage_b.held, 1, "metric B holds its own unit");
    assert_eq!(
        key_for(&metric_a).metric.as_str(),
        "requests",
        "the key names exactly one metric"
    );
    assert_ne!(
        key_for(&metric_a).metric,
        key_for(&metric_b).metric,
        "different metrics are different keys"
    );
}

#[test]
fn ac_002_the_core_surface_is_available_without_the_postgres_feature() {
    // This test is compiled in the default build as well as in
    // `--all-features`; it only touches the feature-independent core.
    let store = MemoryQuotaStore::new();
    assert!(store.is_empty(), "a fresh memory store is empty");
    assert_eq!(store.len(), 0);

    let tenant = TenantId::generate();
    let budget = budget(tenant, "requests", 5, ExhaustionPolicy::HardStop);
    assert_eq!(budget.limit, 5);

    let denied = QuotaError::Exceeded {
        requested: 1,
        remaining: 0,
    };
    assert!(
        denied.to_string().starts_with("QL-EXCEEDED"),
        "the single error type is usable without the postgres feature"
    );
}

#[tokio::test]
async fn ac_003_reserve_holds_units_without_consuming_them() {
    let store = MemoryQuotaStore::new();
    let budget = budget(
        TenantId::generate(),
        "requests",
        10,
        ExhaustionPolicy::HardStop,
    );

    let outcome = store
        .reserve(&ReserveRequest::new(budget.clone(), 4))
        .await
        .expect("capacity remains");

    let reservation = outcome.reservation().expect("the hold is allowed");
    assert_eq!(
        reservation.units, 4,
        "the reservation reports the held amount"
    );
    assert_eq!(
        reservation.key.tenant, budget.tenant,
        "the hold names its tenant"
    );
    assert_eq!(
        reservation.key.provider, budget.provider,
        "the hold names its provider"
    );
    assert_eq!(
        reservation.key.metric, budget.metric,
        "the hold names its metric"
    );
    assert_ne!(
        reservation.id.to_string(),
        "00000000-0000-0000-0000-000000000000",
        "the reservation has a real identifier"
    );

    let usage = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");
    assert_eq!(usage.used, 0, "reserving must not consume");
    assert_eq!(usage.held, 4, "reserving records a hold");
}

#[tokio::test]
async fn ac_004_commit_consumes_the_hold_exactly_once() {
    let store = MemoryQuotaStore::new();
    let budget = budget(
        TenantId::generate(),
        "requests",
        10,
        ExhaustionPolicy::HardStop,
    );

    let reservation = store
        .reserve(&ReserveRequest::new(budget.clone(), 4))
        .await
        .expect("capacity remains")
        .reservation()
        .cloned()
        .expect("the hold is allowed");

    store
        .commit(reservation.id)
        .await
        .expect("the first commit settles the hold");

    let after_first = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");
    assert_eq!(after_first.used, 4, "commit consumes the held units");
    assert_eq!(after_first.held, 0, "commit releases the hold");

    // A repeated commit is idempotent and must never consume twice.
    store
        .commit(reservation.id)
        .await
        .expect("a repeated commit is a no-op");

    let after_second = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");
    assert_eq!(
        after_second.used, 4,
        "a second commit does not consume again"
    );
    assert_eq!(after_second.held, 0);
}

#[tokio::test]
async fn ac_005_release_returns_the_held_amount_to_the_budget() {
    let store = MemoryQuotaStore::new();
    let budget = budget(
        TenantId::generate(),
        "requests",
        4,
        ExhaustionPolicy::HardStop,
    );

    let reservation = store
        .reserve(&ReserveRequest::new(budget.clone(), 4))
        .await
        .expect("capacity remains")
        .reservation()
        .cloned()
        .expect("the hold is allowed");

    store
        .release(reservation.id)
        .await
        .expect("release settles the hold");

    let usage = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");
    assert_eq!(usage.held, 0, "the hold is returned");
    assert_eq!(usage.used, 0, "a released hold is not consumed");

    // The budget is spendable again.
    let again = store
        .reserve(&ReserveRequest::new(budget.clone(), 4))
        .await
        .expect("releasing frees the capacity");
    assert!(again.is_allowed(), "the full budget is available again");
}

#[tokio::test]
async fn ac_006_hard_stop_denies_with_the_reset_instant() {
    let store = MemoryQuotaStore::new();
    let budget = budget(
        TenantId::generate(),
        "requests",
        1,
        ExhaustionPolicy::HardStop,
    );

    store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("the only unit is reservable");

    let denied = store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("a denial is an outcome, not a transport error");

    let exceeded = match denied {
        ReserveOutcome::Denied(exceeded) => exceeded,
        other => panic!("HardStop must deny, got {other:?}"),
    };

    assert_eq!(exceeded.requested, 1);
    assert_eq!(
        exceeded.remaining, 0,
        "the denial reports the remaining capacity"
    );

    let usage = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");
    assert_eq!(
        exceeded.reset_at, usage.reset_at,
        "the denial carries the reset instant"
    );
    assert_eq!(usage.held, 1, "a denial creates no reservation");
}

#[tokio::test]
async fn ac_007_an_idempotent_replay_returns_the_original_reservation() {
    let store = MemoryQuotaStore::new();
    let budget = budget(
        TenantId::generate(),
        "requests",
        10,
        ExhaustionPolicy::HardStop,
    );

    let request = ReserveRequest::new(budget.clone(), 3).with_idempotency_key("retry-1");

    let original = store
        .reserve(&request)
        .await
        .expect("capacity remains")
        .reservation()
        .cloned()
        .expect("the hold is allowed");

    let replay = store
        .reserve(&request)
        .await
        .expect("a replay returns the original reservation")
        .reservation()
        .cloned()
        .expect("the replay is allowed");

    assert_eq!(
        replay.id, original.id,
        "the same idempotency key returns the same hold"
    );

    let usage = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");
    assert_eq!(usage.held, 3, "a replay takes no second hold");
}

#[tokio::test]
async fn ac_008_an_idempotency_key_reused_with_other_units_is_a_conflict() {
    let store = MemoryQuotaStore::new();
    let budget = budget(
        TenantId::generate(),
        "requests",
        10,
        ExhaustionPolicy::HardStop,
    );

    store
        .reserve(&ReserveRequest::new(budget.clone(), 3).with_idempotency_key("retry-2"))
        .await
        .expect("capacity remains");

    let conflict = store
        .reserve(&ReserveRequest::new(budget.clone(), 5).with_idempotency_key("retry-2"))
        .await;

    assert!(
        matches!(conflict, Err(QuotaError::IdempotencyConflict(_))),
        "reusing a key with a different payload is a typed conflict, got {conflict:?}"
    );

    let usage = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");
    assert_eq!(usage.held, 3, "a conflicted request takes no hold");
}

#[tokio::test]
async fn ac_009_each_exhaustion_policy_reports_its_own_outcome() {
    let policies = [
        ExhaustionPolicy::HardStop,
        ExhaustionPolicy::Overage { unit_cost_cents: 7 },
        ExhaustionPolicy::Degrade,
        ExhaustionPolicy::Queue,
    ];

    // The four policies are distinct values.
    for (index, policy) in policies.iter().enumerate() {
        for other in &policies[index + 1..] {
            assert_ne!(policy, other, "each exhaustion policy is distinct");
        }
    }

    async fn exhaust(policy: ExhaustionPolicy) -> ReserveOutcome {
        let store = MemoryQuotaStore::new();
        let budget = Budget::new(
            TenantId::generate(),
            ProviderId::new("acme"),
            Metric::new("requests"),
            1,
            day(),
            policy,
        );

        store
            .reserve(&ReserveRequest::new(budget.clone(), 1))
            .await
            .expect("the only unit is reservable");

        store
            .reserve(&ReserveRequest::new(budget, 1))
            .await
            .expect("every policy answers as an outcome")
    }

    let hard_stop = exhaust(ExhaustionPolicy::HardStop).await;
    assert!(
        matches!(hard_stop, ReserveOutcome::Denied(_)),
        "HardStop denies, got {hard_stop:?}"
    );

    let overage = exhaust(ExhaustionPolicy::Overage { unit_cost_cents: 7 }).await;
    match overage {
        ReserveOutcome::Overage {
            reservation,
            cost_cents,
        } => {
            assert_eq!(
                reservation.units, 1,
                "overage still records the requested units"
            );
            assert_eq!(cost_cents, 7, "overage reports the caller's unit cost");
        }
        other => panic!("Overage reports its own outcome, got {other:?}"),
    }

    let degrade = exhaust(ExhaustionPolicy::Degrade).await;
    assert!(
        matches!(degrade, ReserveOutcome::Degraded { .. }),
        "Degrade reports a degraded outcome, got {degrade:?}"
    );

    let queue = exhaust(ExhaustionPolicy::Queue).await;
    assert!(
        matches!(queue, ReserveOutcome::Queued { .. }),
        "Queue reports a queued outcome, got {queue:?}"
    );
}

#[tokio::test]
async fn ac_010_usage_reports_limit_used_held_and_the_next_reset() {
    let store = MemoryQuotaStore::new();
    let budget = budget(
        TenantId::generate(),
        "requests",
        10,
        ExhaustionPolicy::HardStop,
    );

    let committed = store
        .reserve(&ReserveRequest::new(budget.clone(), 3))
        .await
        .expect("capacity remains")
        .reservation()
        .cloned()
        .expect("the hold is allowed");
    store.commit(committed.id).await.expect("the hold settles");

    let held = store
        .reserve(&ReserveRequest::new(budget.clone(), 2))
        .await
        .expect("capacity remains")
        .reservation()
        .cloned()
        .expect("the hold is allowed");
    assert_eq!(held.units, 2);

    let usage = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");

    assert_eq!(usage.limit, 10, "usage reports the configured limit");
    assert_eq!(usage.used, 3, "usage reports the committed amount");
    assert_eq!(usage.held, 2, "usage reports the held amount");
    assert_eq!(usage.overage, 0, "no overage was taken");

    // The budget was configured with a one-day rolling window, so the reset
    // instant lies between now and now + 1 day.
    let now = Utc::now();
    assert!(
        usage.reset_at > now,
        "usage reports the next reset instant, which is still ahead"
    );
    assert!(
        usage.reset_at <= now + Duration::days(1) + Duration::minutes(1),
        "the reset instant comes from the configured window"
    );
}

#[tokio::test]
async fn ac_011_windows_compute_the_reset_instant_from_their_own_configuration() {
    let now: DateTime<Utc> = Utc
        .with_ymd_and_hms(2026, 3, 10, 12, 0, 0)
        .single()
        .expect("a valid instant");

    // A daily window in UTC-05:00 must reset at the provider's local midnight,
    // 05:00Z, not at UTC midnight.
    let offset = FixedOffset::west_opt(5 * 3600).expect("a valid offset");
    let daily = Window::fixed(Period::Daily, offset);

    assert_eq!(
        daily.start_at(now),
        Utc.with_ymd_and_hms(2026, 3, 10, 5, 0, 0).single().unwrap(),
        "the daily window starts at local midnight"
    );
    assert_eq!(
        daily.reset_at(now),
        Utc.with_ymd_and_hms(2026, 3, 11, 5, 0, 0).single().unwrap(),
        "the daily window resets at local midnight, not UTC midnight"
    );

    let monthly = Window::fixed(Period::Monthly, offset);
    assert_eq!(
        monthly.start_at(now),
        Utc.with_ymd_and_hms(2026, 3, 1, 5, 0, 0).single().unwrap(),
        "the monthly window starts on the first of the month"
    );
    assert_eq!(
        monthly.reset_at(now),
        Utc.with_ymd_and_hms(2026, 4, 1, 5, 0, 0).single().unwrap(),
        "the monthly window resets on the first of the next month"
    );

    let rolling = Window::rolling(Duration::hours(6));
    assert_eq!(
        rolling.start_at(now),
        now,
        "a rolling window starts at the anchor"
    );
    assert_eq!(
        rolling.reset_at(now),
        now + Duration::hours(6),
        "a rolling window resets after its configured length"
    );
}

/// Compile-time proof that the optional backend satisfies the shared contract.
#[cfg(feature = "postgres")]
fn postgres_impl_assertions() {
    fn assert_quota_store<T: QuotaStore>() {}
    assert_quota_store::<quota_ledger::postgres::PostgresQuotaStore>();
}

/// Without the feature there is no Postgres backend to bound.
#[cfg(not(feature = "postgres"))]
fn postgres_impl_assertions() {}

#[test]
fn ac_012_the_postgres_backend_implements_the_store_and_ships_its_schema() {
    postgres_impl_assertions();

    let schema =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/postgres/schema.sql"))
            .expect("the schema is shipped with the crate");

    for table in ["quota_budget", "quota_reservation", "quota_dead_hold"] {
        assert!(
            schema.contains(table),
            "the shipped schema declares {table} for the caller to apply"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn ac_013_concurrent_reservations_never_exceed_the_limit() {
    let store = Arc::new(MemoryQuotaStore::new());
    let budget = budget(
        TenantId::generate(),
        "requests",
        10,
        ExhaustionPolicy::HardStop,
    );

    let mut tasks = Vec::new();
    for _ in 0..40 {
        let store = Arc::clone(&store);
        let budget = budget.clone();
        tasks.push(tokio::spawn(async move {
            store.reserve(&ReserveRequest::new(budget, 1)).await
        }));
    }

    let mut granted = Vec::new();
    for task in tasks {
        let outcome = task
            .await
            .expect("no task panics")
            .expect("a reserve is an outcome");
        if let Some(reservation) = outcome.reservation() {
            granted.push(reservation.id);
        }
    }

    assert!(
        granted.len() <= 10,
        "a limit of 10 grants at most 10 holds, got {}",
        granted.len()
    );

    let held = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");
    assert!(
        held.used + held.held <= held.limit,
        "used {} + held {} never exceeds the limit {}",
        held.used,
        held.held,
        held.limit
    );

    for id in granted {
        store.commit(id).await.expect("a granted hold settles");
    }

    let committed = store
        .usage(&key_for(&budget))
        .await
        .expect("usage is readable");
    assert!(
        committed.used <= committed.limit,
        "total committed {} never exceeds the limit {}",
        committed.used,
        committed.limit
    );
}

#[tokio::test]
async fn ac_014_one_tenant_cannot_observe_another_tenants_usage() {
    let store = MemoryQuotaStore::new();
    let tenant_a = TenantId::generate();
    let tenant_b = TenantId::generate();

    let budget_a = budget(tenant_a, "requests", 5, ExhaustionPolicy::HardStop);
    let budget_b = budget(tenant_b, "requests", 5, ExhaustionPolicy::HardStop);

    store
        .reserve(&ReserveRequest::new(budget_a.clone(), 5))
        .await
        .expect("tenant A has capacity");

    // Tenant B's bucket does not exist, and tenant A's usage is not visible
    // under B's key.
    let cross_tenant = store.usage(&key_for(&budget_b)).await;
    assert!(
        matches!(cross_tenant, Err(QuotaError::UnknownBudget(_))),
        "a foreign tenant's key cannot read this ledger, got {cross_tenant:?}"
    );

    // The same is true in reverse: tenant B can never see A's consumption.
    store
        .reserve(&ReserveRequest::new(budget_b.clone(), 1))
        .await
        .expect("tenant B has its own capacity");

    let usage_a = store.usage(&key_for(&budget_a)).await.expect("A's usage");
    let usage_b = store.usage(&key_for(&budget_b)).await.expect("B's usage");

    assert_eq!(usage_a.held, 5, "tenant A's hold is unchanged by tenant B");
    assert_eq!(usage_b.held, 1, "tenant B only sees its own hold");
}

#[test]
fn ac_015_crate_hygiene_and_parameterized_postgres_statements() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let lib = fs::read_to_string(root.join("src/lib.rs")).expect("lib.rs is readable");
    assert!(
        lib.contains("#![forbid(unsafe_code)]"),
        "the crate forbids unsafe code"
    );
    assert!(
        lib.contains("#![deny(missing_docs)]"),
        "the crate denies missing documentation"
    );

    for licence in ["LICENSE-MIT", "LICENSE-APACHE"] {
        assert!(
            root.join(licence).exists(),
            "{licence} ships with the crate"
        );
    }

    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("the manifest is readable");
    assert!(
        manifest.contains("rust-version.workspace = true"),
        "the crate inherits the workspace MSRV"
    );

    // Every Postgres statement must be parameterized: a caller value reaching
    // SQL text would show up as a query built with format!.
    let backend =
        fs::read_to_string(root.join("src/postgres/mod.rs")).expect("the backend is readable");

    for verb in ["SELECT", "INSERT", "UPDATE", "DELETE"] {
        assert!(
            !backend.contains(&format!("format!(\"{verb}")),
            "no {verb} statement is built by string interpolation"
        );
    }

    assert!(
        backend.contains("$1"),
        "the backend binds caller values as parameters"
    );
}

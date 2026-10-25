//! AC-018 through AC-021 and AC-049: the optional Postgres backend, feature
//! gating, reclaim, tenant scoping, and the TLS default.

use std::path::Path;

use chrono::{Duration, Utc};
use quota_ledger::{
    Budget, ExhaustionPolicy, MemoryQuotaStore, Metric, ProviderId, QuotaKey, QuotaError,
    QuotaStore, ReserveRequest, TenantId, Window,
};

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

fn budget(tenant: TenantId, limit: u64) -> Budget {
    Budget::new(
        tenant,
        ProviderId::new("acme"),
        Metric::new("requests"),
        limit,
        Window::rolling(Duration::hours(1)),
        ExhaustionPolicy::HardStop,
    )
}

fn key(budget: &Budget) -> QuotaKey {
    QuotaKey::new(budget.tenant, budget.provider, budget.metric, Utc::now())
}

/// AC-018: with the `postgres` feature off, no sqlx or Postgres code compiles.
#[test]
fn ac_018_postgres_dependency_is_optional_and_gated() {
    let manifest = read("Cargo.toml");
    let lib = read("src/lib.rs");

    assert!(
        manifest.contains("postgres = [\"dep:sqlx\"]"),
        "AC-018: the postgres feature must opt into sqlx explicitly"
    );
    let sqlx_line = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("sqlx ="))
        .expect("AC-018: the manifest must declare sqlx");
    assert!(
        sqlx_line.contains("optional = true"),
        "AC-018: sqlx must be optional so a default build compiles no Postgres dependency, got {sqlx_line:?}"
    );
    assert!(
        manifest.contains("default = []"),
        "AC-018: the default feature set must not enable Postgres"
    );
    assert!(
        lib.contains("#[cfg(feature = \"postgres\")]"),
        "AC-018: the Postgres module must be feature-gated"
    );
    assert!(
        lib.contains("pub mod postgres;"),
        "AC-018: the Postgres module must exist behind the feature gate"
    );
}

/// AC-019: two concurrent reserves of the last unit against Postgres.
#[cfg(feature = "postgres")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires TEST_DATABASE_URL or DATABASE_URL pointing at a disposable Postgres"]
async fn ac_019_concurrent_postgres_reservations_never_exceed_the_limit() {
    use quota_ledger::postgres::PostgresQuotaStore;

    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("AC-019: set TEST_DATABASE_URL or DATABASE_URL to run this test");
    let store = PostgresQuotaStore::connect_insecure_for_tests(&url)
        .await
        .expect("AC-019: the test database must be reachable");
    store
        .apply_schema()
        .await
        .expect("AC-019: the bundled schema must apply");

    let tenant = TenantId::generate();
    let budget = budget(tenant, 1);
    let first = ReserveRequest::new(budget.clone(), 1);
    let second = ReserveRequest::new(budget.clone(), 1);

    let (left, right) = tokio::join!(store.reserve(&first), store.reserve(&second));
    let outcomes = [
        left.expect("AC-019: the first reserve must return an outcome"),
        right.expect("AC-019: the second reserve must return an outcome"),
    ];
    let allowed = outcomes.iter().filter(|outcome| outcome.is_allowed()).count();

    assert_eq!(
        allowed, 1,
        "AC-019: exactly one concurrent reservation of the last unit must succeed, got {outcomes:?}"
    );

    let usage = store
        .usage(&key(&budget))
        .await
        .expect("AC-019: usage must be readable");
    assert!(
        usage.used + usage.held <= usage.limit,
        "AC-019: usage must never exceed the limit, got {usage:?}"
    );
}

/// AC-020: an abandoned reservation is reclaimed exactly once.
#[tokio::test]
async fn ac_020_expired_reservation_is_reclaimed_exactly_once() {
    let tenant = TenantId::generate();
    let budget = budget(tenant, 1);
    let store = MemoryQuotaStore::new();

    let reservation = store
        .reserve(
            &ReserveRequest::new(budget.clone(), 1).with_ttl(Duration::milliseconds(20)),
        )
        .await
        .expect("AC-020: the reservation must be created")
        .reservation()
        .expect("AC-020: the reservation must be allowed")
        .id;

    std::thread::sleep(std::time::Duration::from_millis(80));

    assert_eq!(
        store.reclaim_expired(),
        1,
        "AC-020: reclaim must return the expired reservation's units once"
    );
    let after_reclaim = store
        .usage(&key(&budget))
        .await
        .expect("AC-020: usage must be readable after reclaim");
    assert_eq!(after_reclaim.held, 0, "AC-020: reclaimed units must return to the budget");
    assert_eq!(after_reclaim.used, 0, "AC-020: reclaim must not consume units");

    assert_eq!(
        store.reclaim_expired(),
        0,
        "AC-020: a second reclaim must not return the same units again"
    );
    store
        .commit(reservation)
        .await
        .expect("AC-020: a late commit must be a typed no-op");
    let after_late_commit = store
        .usage(&key(&budget))
        .await
        .expect("AC-020: usage must be readable after the late commit");
    assert_eq!(
        after_late_commit.used, 0,
        "AC-020: a late commit after reclaim must not consume units"
    );

    let replacement = store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-020: the reclaimed unit must be reservable again");
    assert!(
        replacement.is_allowed(),
        "AC-020: reclaimed capacity must be available again, got {replacement:?}"
    );
}

/// AC-021: usage is scoped to one tenant, and the Postgres query binds it.
#[tokio::test]
async fn ac_021_usage_is_scoped_to_a_single_tenant() {
    let provider = ProviderId::new("acme");
    let metric = Metric::new("requests");
    let window = Window::rolling(Duration::hours(1));
    let budget_a = Budget::new(
        TenantId::generate(),
        provider,
        metric,
        5,
        window,
        ExhaustionPolicy::HardStop,
    );
    let budget_b = Budget::new(
        TenantId::generate(),
        provider,
        metric,
        5,
        window,
        ExhaustionPolicy::HardStop,
    );
    let store = MemoryQuotaStore::new();

    store
        .reserve(&ReserveRequest::new(budget_a.clone(), 2))
        .await
        .expect("AC-021: tenant A must be able to reserve");
    store
        .reserve(&ReserveRequest::new(budget_b.clone(), 1))
        .await
        .expect("AC-021: tenant B must be able to reserve");

    let usage_a = store
        .usage(&key(&budget_a))
        .await
        .expect("AC-021: tenant A usage must be readable");
    let usage_b = store
        .usage(&key(&budget_b))
        .await
        .expect("AC-021: tenant B usage must be readable");

    assert_eq!(usage_a.held, 2, "AC-021: tenant A usage must include only tenant A");
    assert_eq!(usage_b.held, 1, "AC-021: tenant B usage must include only tenant B");
    assert_eq!(usage_a.limit, 5, "AC-021: tenant A must keep its own limit");
    assert_eq!(usage_b.limit, 5, "AC-021: tenant B must keep its own limit");

    let unknown = store
        .usage(&key(&budget(TenantId::generate(), 5)))
        .await
        .expect_err("AC-021: an unknown tenant must not receive another tenant's usage");
    assert!(
        matches!(unknown, QuotaError::UnknownBudget(_)),
        "AC-021: an unknown tenant must be a typed error, got {unknown:?}"
    );

    let sql = read("src/postgres/mod.rs");
    assert!(
        sql.contains("WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3"),
        "AC-021: every Postgres usage query must bind the tenant as a parameter"
    );
}

/// AC-049: TLS is required by default and the insecure mode is explicit.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn ac_049_postgres_requires_tls_unless_test_mode_is_explicit() {
    use quota_ledger::postgres::PostgresQuotaStore;

    let source = read("src/postgres/mod.rs");
    assert!(
        source.contains("parse_options(database_url)?.ssl_mode(PgSslMode::Require)"),
        "AC-049: the default connect path must force TLS regardless of the connection string"
    );
    assert!(
        source.contains("parse_options(database_url)?.ssl_mode(PgSslMode::Disable)"),
        "AC-049: the insecure path must be a separate, explicit entry point"
    );
    assert_eq!(
        source.matches("PgSslMode::Require").count(),
        1,
        "AC-049: TLS must be forced in exactly the default connect path"
    );
    assert_eq!(
        source.matches("PgSslMode::Disable").count(),
        1,
        "AC-049: TLS may be disabled in exactly the test-only path"
    );
    assert!(
        source.contains("connect_insecure_for_tests"),
        "AC-049: the insecure escape hatch must be named for tests"
    );
    assert!(
        source.contains("test-only"),
        "AC-049: the insecure mode must be documented as test-only"
    );

    let error = PostgresQuotaStore::connect("this is not a postgres connection string")
        .await
        .expect_err("AC-049: a malformed connection string must be refused");
    assert!(
        matches!(error, QuotaError::Config(_)),
        "AC-049: a refused connection must be a typed configuration error, got {error:?}"
    );
}

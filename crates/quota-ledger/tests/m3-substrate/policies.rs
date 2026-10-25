//! AC-014 through AC-017: every exhaustion policy produces its declared
//! outcome.

use chrono::{Duration, Utc};
use quota_ledger::{
    Budget, ExhaustionPolicy, MemoryQuotaStore, Metric, ProviderId, QuotaKey, QuotaStore,
    ReserveOutcome, ReserveRequest, TenantId, Window,
};

fn budget(tenant: TenantId, limit: u64, policy: ExhaustionPolicy) -> Budget {
    Budget::new(
        tenant,
        ProviderId::new("acme"),
        Metric::new("requests"),
        limit,
        Window::rolling(Duration::hours(1)),
        policy,
    )
}

fn key(budget: &Budget) -> QuotaKey {
    QuotaKey::new(budget.tenant, budget.provider, budget.metric, Utc::now())
}

/// AC-014: HardStop denies with the reset time and an upgrade hint.
#[tokio::test]
async fn ac_014_hard_stop_denies_with_reset_time_and_upgrade_hint() {
    let tenant = TenantId::generate();
    let budget = budget(tenant, 1, ExhaustionPolicy::HardStop);
    let store = MemoryQuotaStore::new();

    store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-014: the last unit must be reservable");

    let outcome = store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-014: an exhausted budget must return a denial");
    let denial = match outcome {
        ReserveOutcome::Denied(denial) => denial,
        other => panic!("AC-014: HardStop must deny, got {other:?}"),
    };

    let usage = store
        .usage(&key(&budget))
        .await
        .expect("AC-014: usage must be readable");
    assert_eq!(
        denial.reset_at, usage.reset_at,
        "AC-014: the denial must carry the window reset time"
    );
    assert_eq!(denial.remaining, 0, "AC-014: the denial must report zero remaining");
    assert!(
        !denial.upgrade_hint.trim().is_empty(),
        "AC-014: the denial must carry an upgrade hint"
    );
    assert!(
        denial.upgrade_hint.contains("Upgrade"),
        "AC-014: the upgrade hint must name the upgrade path, got {:?}",
        denial.upgrade_hint
    );
}

/// AC-015: Overage allows the reservation and records units and cents.
#[tokio::test]
async fn ac_015_overage_allows_and_records_units_and_cost() {
    let tenant = TenantId::generate();
    let budget = budget(tenant, 1, ExhaustionPolicy::Overage { unit_cost_cents: 25 });
    let store = MemoryQuotaStore::new();

    let first = store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-015: the last in-limit unit must be reservable");
    assert!(
        matches!(first, ReserveOutcome::Reserved(_)),
        "AC-015: an in-limit reservation must be a normal reservation, got {first:?}"
    );

    let outcome = store
        .reserve(&ReserveRequest::new(budget.clone(), 2))
        .await
        .expect("AC-015: Overage must allow the reservation");
    let (reservation, cost_cents) = match outcome {
        ReserveOutcome::Overage {
            reservation,
            cost_cents,
        } => (reservation, cost_cents),
        other => panic!("AC-015: Overage must allow and record overage, got {other:?}"),
    };

    assert_eq!(reservation.units, 2, "AC-015: the overage reservation must hold the request");
    assert_eq!(
        cost_cents, 50,
        "AC-015: two overage units at 25 cents must cost 50 cents"
    );

    let usage = store
        .usage(&key(&budget))
        .await
        .expect("AC-015: usage must be readable");
    assert_eq!(usage.overage, 2, "AC-015: the ledger must record two overage units");
    assert_eq!(
        usage.overage_cents, 50,
        "AC-015: the ledger must report the accumulated overage cost in cents"
    );
    assert_eq!(usage.held, 3, "AC-015: both reservations must remain held");
}

/// AC-016: Degrade names the reset time and consumes no units.
#[tokio::test]
async fn ac_016_degrade_returns_reset_time_without_consuming_units() {
    let tenant = TenantId::generate();
    let budget = budget(tenant, 1, ExhaustionPolicy::Degrade);
    let store = MemoryQuotaStore::new();

    store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-016: the last unit must be reservable");
    let before = store
        .usage(&key(&budget))
        .await
        .expect("AC-016: usage must be readable");

    let outcome = store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-016: Degrade must return an outcome");
    let reset_at = match outcome {
        ReserveOutcome::Degraded { reset_at } => reset_at,
        other => panic!("AC-016: Degrade must return a degraded outcome, got {other:?}"),
    };

    let after = store
        .usage(&key(&budget))
        .await
        .expect("AC-016: usage must be readable after degradation");
    assert_eq!(
        reset_at, after.reset_at,
        "AC-016: the degraded outcome must name the reset time"
    );
    assert_eq!(
        after, before,
        "AC-016: a degraded outcome must consume no units"
    );
    assert_eq!(after.held, 1, "AC-016: only the first reservation may be held");
}

/// AC-017: Queue returns the reset time and consumes no units.
#[tokio::test]
async fn ac_017_queue_returns_reset_time_without_consuming_units() {
    let tenant = TenantId::generate();
    let budget = budget(tenant, 1, ExhaustionPolicy::Queue);
    let store = MemoryQuotaStore::new();

    store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-017: the last unit must be reservable");
    let before = store
        .usage(&key(&budget))
        .await
        .expect("AC-017: usage must be readable");

    let outcome = store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-017: Queue must return an outcome");
    let reset_at = match outcome {
        ReserveOutcome::Queued { reset_at } => reset_at,
        other => panic!("AC-017: Queue must return a queued outcome, got {other:?}"),
    };

    let after = store
        .usage(&key(&budget))
        .await
        .expect("AC-017: usage must be readable after queueing");
    assert_eq!(
        reset_at, after.reset_at,
        "AC-017: the queued outcome must name the reset time"
    );
    assert_eq!(
        after, before,
        "AC-017: a queued outcome must consume no units"
    );
    assert_eq!(after.held, 1, "AC-017: only the first reservation may be held");
}

//! AC-010 through AC-013: reservation identity, settlement, and idempotency.

use std::collections::HashSet;

use chrono::{Duration, Utc};
use quota_ledger::{
    Budget, ExhaustionPolicy, MemoryQuotaStore, Metric, ProviderId, QuotaError, QuotaKey,
    QuotaStore, ReserveOutcome, ReserveRequest, TenantId, Window,
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

/// AC-010: N reservations against a limit of N all succeed with distinct ids.
#[tokio::test]
async fn ac_010_n_reservations_yield_n_distinct_ids_and_n_usage() {
    const LIMIT: u64 = 3;

    let tenant = TenantId::generate();
    let budget = budget(tenant, LIMIT, ExhaustionPolicy::HardStop);
    let store = MemoryQuotaStore::new();
    let mut reservation_ids = Vec::new();

    for index in 0..LIMIT {
        let outcome = store
            .reserve(&ReserveRequest::new(budget.clone(), 1))
            .await
            .expect("AC-010: every reservation within the limit must succeed");
        let reservation = outcome
            .reservation()
            .unwrap_or_else(|| panic!("AC-010: reservation {index} must be allowed, got {outcome:?}"));
        reservation_ids.push(reservation.id);
    }

    let unique: HashSet<_> = reservation_ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        LIMIT as usize,
        "AC-010: every reservation must return a distinct reservation id"
    );

    let usage = store
        .usage(&key(&budget))
        .await
        .expect("AC-010: usage must be readable");
    assert_eq!(usage.limit, LIMIT, "AC-010: the limit must be reported unchanged");
    assert_eq!(
        usage.held, LIMIT,
        "AC-010: usage must equal N while all N units are held"
    );

    for id in reservation_ids {
        store
            .commit(id)
            .await
            .expect("AC-010: committing a reservation must succeed");
    }
    let settled = store
        .usage(&key(&budget))
        .await
        .expect("AC-010: usage must be readable after commit");
    assert_eq!(
        settled.used, LIMIT,
        "AC-010: usage must equal N after N commits"
    );
    assert_eq!(settled.held, 0, "AC-010: no units may remain held after commit");
}

/// AC-011: a hard-stop denial carries the reset time and changes no usage.
#[tokio::test]
async fn ac_011_hard_stop_denial_reports_reset_and_keeps_usage_unchanged() {
    let tenant = TenantId::generate();
    let budget = budget(tenant, 1, ExhaustionPolicy::HardStop);
    let store = MemoryQuotaStore::new();

    store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-011: the last unit must be reservable");
    let before = store
        .usage(&key(&budget))
        .await
        .expect("AC-011: usage must be readable");

    let denied = store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-011: a denial is a successful store call");
    let denial = match denied {
        ReserveOutcome::Denied(denial) => denial,
        other => panic!("AC-011: an exhausted hard stop must deny, got {other:?}"),
    };

    let after = store
        .usage(&key(&budget))
        .await
        .expect("AC-011: usage must be readable after the denial");

    assert_eq!(
        after, before,
        "AC-011: a denied reservation must not change usage"
    );
    assert_eq!(denial.requested, 1, "AC-011: the denial must state the request");
    assert_eq!(denial.remaining, 0, "AC-011: the denial must state zero remaining");
    assert_eq!(
        denial.reset_at, after.reset_at,
        "AC-011: the denial must report the window reset time"
    );
    assert!(
        denial.reset_at > Utc::now(),
        "AC-011: the reset time must be in the future"
    );
}

/// AC-012: commit consumes once, release consumes nothing, repeats are no-ops.
#[tokio::test]
async fn ac_012_commit_consumes_once_and_release_consumes_nothing() {
    let tenant = TenantId::generate();
    let budget = budget(tenant, 5, ExhaustionPolicy::HardStop);
    let store = MemoryQuotaStore::new();

    let committed = store
        .reserve(&ReserveRequest::new(budget.clone(), 2))
        .await
        .expect("AC-012: the first reservation must succeed")
        .reservation()
        .expect("AC-012: the first reservation must be allowed")
        .id;
    store.commit(committed).await.expect("AC-012: commit must succeed");
    store
        .commit(committed)
        .await
        .expect("AC-012: repeating a commit must be a typed no-op");

    let after_commit = store
        .usage(&key(&budget))
        .await
        .expect("AC-012: usage must be readable");
    assert_eq!(after_commit.used, 2, "AC-012: commit must consume the units exactly once");
    assert_eq!(after_commit.held, 0, "AC-012: committed units must leave the hold");

    let released = store
        .reserve(&ReserveRequest::new(budget.clone(), 1))
        .await
        .expect("AC-012: the second reservation must succeed")
        .reservation()
        .expect("AC-012: the second reservation must be allowed")
        .id;
    store.release(released).await.expect("AC-012: release must succeed");
    store
        .release(released)
        .await
        .expect("AC-012: repeating a release must be a typed no-op");
    store
        .commit(released)
        .await
        .expect("AC-012: committing a released reservation must be a typed no-op");

    let after_release = store
        .usage(&key(&budget))
        .await
        .expect("AC-012: usage must be readable after release");
    assert_eq!(
        after_release.used, 2,
        "AC-012: a released reservation must consume nothing"
    );
    assert_eq!(after_release.held, 0, "AC-012: released units must return to the budget");
}

/// AC-013: the same idempotency key returns the same reservation once.
#[tokio::test]
async fn ac_013_repeated_idempotency_key_reuses_one_reservation() {
    let tenant = TenantId::generate();
    let budget = budget(tenant, 5, ExhaustionPolicy::HardStop);
    let store = MemoryQuotaStore::new();
    let request = ReserveRequest::new(budget.clone(), 2).with_idempotency_key("caller-key-1");

    let first = store
        .reserve(&request)
        .await
        .expect("AC-013: the first reservation must succeed")
        .reservation()
        .expect("AC-013: the first reservation must be allowed")
        .id;
    let second = store
        .reserve(&request)
        .await
        .expect("AC-013: repeating the same request must succeed")
        .reservation()
        .expect("AC-013: the repeated request must be allowed")
        .id;

    assert_eq!(
        first, second,
        "AC-013: the same idempotency key and payload must return the same reservation"
    );

    let usage = store
        .usage(&key(&budget))
        .await
        .expect("AC-013: usage must be readable");
    assert_eq!(
        usage.held, 2,
        "AC-013: a repeated idempotency key must count the units once"
    );

    let conflict = store
        .reserve(
            &ReserveRequest::new(budget.clone(), 3).with_idempotency_key("caller-key-1"),
        )
        .await
        .expect_err("AC-013: the same key with a different payload must be rejected");
    assert!(
        matches!(conflict, QuotaError::IdempotencyConflict(_)),
        "AC-013: a mismatched payload must return a typed conflict, got {conflict:?}"
    );
}

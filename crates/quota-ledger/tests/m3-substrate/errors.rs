//! AC-051: every `quota-ledger` error renders a stable code and implements
//! `std::error::Error`.

use quota_ledger::QuotaError;

fn assert_stable_error(error: &QuotaError, code: &str, label: &str) {
    assert!(
        error.to_string().starts_with(code),
        "AC-051: {label} must render the stable code {code}, got {error}"
    );
    let _: &dyn std::error::Error = error;
}

#[test]
fn ac_051_quota_ledger_errors_expose_stable_codes_and_implement_error() {
    assert_stable_error(
        &QuotaError::Exceeded {
            requested: 2,
            remaining: 1,
        },
        "QL-EXCEEDED",
        "exceeded",
    );
    assert_stable_error(
        &QuotaError::UnknownBudget("tenant/provider".to_owned()),
        "QL-UNKNOWN-BUDGET",
        "unknown budget",
    );
    assert_stable_error(
        &QuotaError::ReservationNotFound("id".to_owned()),
        "QL-RESERVATION-NOT-FOUND",
        "reservation not found",
    );
    assert_stable_error(
        &QuotaError::AlreadySettled("id".to_owned()),
        "QL-ALREADY-SETTLED",
        "already settled",
    );
    assert_stable_error(
        &QuotaError::IdempotencyConflict("key".to_owned()),
        "QL-IDEMPOTENCY-CONFLICT",
        "idempotency conflict",
    );
    assert_stable_error(
        &QuotaError::Backend("database unavailable".to_owned()),
        "QL-BACKEND",
        "backend",
    );
    assert_stable_error(
        &QuotaError::Config("invalid request".to_owned()),
        "QL-CONFIG",
        "config",
    );
}

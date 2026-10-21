use chrono::{DateTime, Utc};

use crate::{QuotaExceeded, ReserveOutcome};

/// The upgrade hint attached to a hard-stop denial.
pub(crate) const UPGRADE_HINT: &str = "Upgrade the plan or wait for the quota window to reset.";

/// Builds a hard-stop denial.
pub(crate) fn denial(requested: u64, remaining: u64, reset_at: DateTime<Utc>) -> ReserveOutcome {
    ReserveOutcome::Denied(QuotaExceeded {
        requested,
        remaining,
        reset_at,
        upgrade_hint: UPGRADE_HINT,
    })
}

/// Builds a degraded outcome.
pub(crate) fn degraded(reset_at: DateTime<Utc>) -> ReserveOutcome {
    ReserveOutcome::Degraded { reset_at }
}

/// Builds a queued outcome.
pub(crate) fn queued(reset_at: DateTime<Utc>) -> ReserveOutcome {
    ReserveOutcome::Queued { reset_at }
}

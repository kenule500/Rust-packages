use thiserror::Error;

/// An error returned by a quota store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum QuotaError {
    /// The requested units exceed the remaining budget.
    #[error("QL-EXCEEDED: requested {requested} units with {remaining} remaining")]
    Exceeded {
        /// The number of units requested.
        requested: u64,
        /// The number of units remaining.
        remaining: u64,
    },
    /// No ledger exists for the supplied key.
    #[error("QL-UNKNOWN-BUDGET: no ledger for {0}")]
    UnknownBudget(String),
    /// The reservation identifier is unknown.
    #[error("QL-RESERVATION-NOT-FOUND: {0}")]
    ReservationNotFound(String),
    /// The reservation was already settled.
    #[error("QL-ALREADY-SETTLED: {0}")]
    AlreadySettled(String),
    /// The idempotency key was reused with a different payload.
    #[error("QL-IDEMPOTENCY-CONFLICT: {0}")]
    IdempotencyConflict(String),
    /// The backend failed.
    #[error("QL-BACKEND: {0}")]
    Backend(String),
    /// The store configuration or request is invalid.
    #[error("QL-CONFIG: {0}")]
    Config(String),
}

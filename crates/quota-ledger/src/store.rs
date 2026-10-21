use async_trait::async_trait;

use crate::{QuotaError, QuotaKey, ReservationId, ReserveOutcome, ReserveRequest, Usage};

/// A storage backend for quota reservations.
///
/// The contract is deliberately small: reserve, settle, release, and read. A
/// backend owns transaction boundaries and durability; callers own the
/// outbound call between reserve and commit.
#[async_trait]
pub trait QuotaStore: Send + Sync {
    /// Reserves units from a budget.
    ///
    /// Repeating a request with the same idempotency key and the same unit
    /// count returns the original reservation. Reusing the key with a
    /// different unit count returns [`QuotaError::IdempotencyConflict`].
    async fn reserve(&self, request: &ReserveRequest) -> Result<ReserveOutcome, QuotaError>;

    /// Consumes a reservation exactly once.
    ///
    /// Committing an already-settled reservation is a no-op. An unknown
    /// identifier returns [`QuotaError::ReservationNotFound`].
    async fn commit(&self, id: ReservationId) -> Result<(), QuotaError>;

    /// Returns an unsettled reservation to the budget.
    ///
    /// Releasing an already-settled reservation is a no-op. An unknown
    /// identifier returns [`QuotaError::ReservationNotFound`].
    async fn release(&self, id: ReservationId) -> Result<(), QuotaError>;

    /// Reports the current usage for a ledger bucket.
    ///
    /// The tenant, provider, and metric select the bucket. `window_start` is
    /// informational; the returned [`Usage::reset_at`] is authoritative.
    async fn usage(&self, key: &QuotaKey) -> Result<Usage, QuotaError>;
}

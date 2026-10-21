//! Per-tenant, per-provider quota reservations with commit, release,
//! exhaustion policies, and an optional Postgres ledger.
//!
//! `quota-ledger` answers one question before an outbound call: may this
//! tenant spend one more unit of this provider's quota right now, and what
//! happens if they cannot?
//!
//! # Flow
//!
//! 1. [`QuotaStore::reserve`] checks capacity and records a hold atomically.
//! 2. The caller performs the outbound call.
//! 3. [`QuotaStore::commit`] consumes the hold exactly once, or
//!    [`QuotaStore::release`] returns it.
//! 4. [`QuotaStore::usage`] reports the limit, used, held, overage, and the
//!    next reset instant.
//!
//! Reservations carry an optional caller idempotency key. Repeating the same
//! key with the same payload returns the same reservation; repeating it with a
//! different payload is an [`QuotaError::IdempotencyConflict`].
//!
//! # Feature flags
//!
//! - `postgres` - the optional Postgres backend. Off by default.
//!
//! # Non-goals
//!
//! - No process-local rate limiting; that is a client concern.
//! - No billing, invoicing, or currency conversion.
//! - No Redis backend in 0.1.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod memory;
mod policy;
mod store;
mod types;
mod window;

pub use error::QuotaError;
pub use memory::MemoryQuotaStore;
pub use store::QuotaStore;
pub use types::{
    Budget, ExhaustionPolicy, Metric, Period, ProviderId, QuotaExceeded, QuotaKey, Reservation,
    ReservationId, ReserveOutcome, ReserveRequest, TenantId, Usage, Window,
};

/// The crate version, for build info and diagnostics headers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

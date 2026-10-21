//! Per-tenant, per-provider quota reservations with commit, release,
//! exhaustion policies, and an optional Postgres ledger.
//!
//! `quota-ledger` answers one question before an outbound call: may this
//! tenant spend one more unit of this provider's quota right now, and what
//! happens if they cannot?
//!
//! # Feature flags
//!
//! - `postgres` - the [`postgres::PostgresQuotaStore`] backend. Off by default.
//!
//! # Non-goals
//!
//! - No process-local rate limiting; that is a client concern.
//! - No billing, invoicing, or currency conversion.
//! - No Redis backend in 0.1.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// The crate version, for build info and diagnostics headers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

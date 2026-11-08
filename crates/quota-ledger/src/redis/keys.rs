//! Redis key construction and window arithmetic.
//!
//! A budget key deliberately **excludes** the window start. A rolling window is
//! anchored at its first reservation, so `Window::start_at(now)` changes on
//! every call; deriving the key from it would give every reserve its own empty
//! budget and break both the limit and idempotency. The window bounds live in
//! the budget hash instead, and a reserve whose window has ended replaces the
//! hash in the same script.
//!
//! Every budget and reservation key starts with `quota:` and then the tenant, so
//! a key can never address another tenant's data. Caller-supplied segments
//! (provider, metric, idempotency key) are percent-escaped so a `:` inside a
//! provider name cannot forge an extra key segment.
//!
//! `commit` and `release` take only a reservation id (the 0.1 trait signature),
//! so an index key maps each id to its tenant-scoped reservation key. That keeps
//! the tenant in every data key without giving the settlement path a tenant
//! argument it does not have.
//!
//! Scope: key naming and the window start/end pair. Non-goals: no connection
//! handling, no script execution, no retry.

use chrono::{DateTime, Utc};

use crate::types::{Metric, ProviderId, QuotaKey, ReservationId, TenantId, Window};

/// The root prefix for every key this backend writes.
const PREFIX: &str = "quota";

/// Escapes the characters that would otherwise split or corrupt a key segment.
fn escape(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for ch in segment.chars() {
        match ch {
            '%' => out.push_str("%25"),
            ':' => out.push_str("%3A"),
            ' ' => out.push_str("%20"),
            other => out.push(other),
        }
    }
    out
}

/// The key holding one budget bucket, independent of its current window.
pub(crate) fn budget_key(tenant: TenantId, provider: &ProviderId, metric: Metric) -> String {
    format!(
        "{PREFIX}:budget:{}:{}:{}",
        tenant,
        escape(provider.as_str()),
        escape(metric.as_str())
    )
}

/// The key holding one reservation.
pub(crate) fn reservation_key(tenant: TenantId, id: ReservationId) -> String {
    format!("{PREFIX}:res:{tenant}:{id}")
}

/// The prefix the reserve script prepends to a reservation id.
pub(crate) fn reservation_prefix() -> String {
    format!("{PREFIX}:res:")
}

/// The key mapping a reservation id to its tenant-scoped reservation key.
pub(crate) fn reservation_index_key(id: ReservationId) -> String {
    format!("{PREFIX}:residx:{id}")
}

/// The key mapping one caller idempotency key to its reservation.
///
/// The key is bounded by the reservation TTL, so it does not need the window.
pub(crate) fn idempotency_key(
    tenant: TenantId,
    provider: &ProviderId,
    metric: Metric,
    caller_key: &str,
) -> String {
    format!(
        "{PREFIX}:idem:{}:{}:{}:{}",
        tenant,
        escape(provider.as_str()),
        escape(metric.as_str()),
        escape(caller_key)
    )
}

/// The set of window starts a bucket has used, for diagnostics.
pub(crate) fn index_key(tenant: TenantId, provider: &ProviderId, metric: Metric) -> String {
    format!(
        "{PREFIX}:index:{}:{}:{}",
        tenant,
        escape(provider.as_str()),
        escape(metric.as_str())
    )
}

/// Resolves the window start and end for a reserve at `now`.
pub(crate) fn window_bounds(window: Window, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    (window.start_at(now), window.reset_at(now))
}

/// The key for a caller-supplied ledger bucket.
pub(crate) fn usage_key(key: &QuotaKey) -> String {
    budget_key(key.tenant, &key.provider, key.metric)
}

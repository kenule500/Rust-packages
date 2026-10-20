# 29 — quota-ledger

**Package:** `quota-ledger` (standalone crate)
**Status:** blueprint, not published
**Crate type:** library, `no_std`-friendly core with optional Postgres/Redis backends
**Licence:** MIT OR Apache-2.0

> **Name note.** `quota` is already taken on crates.io (a lane-parallel rate
> limiter, MIT, v0.3.3). The published crate name is therefore `quota-ledger`.
> This document still calls the concept `quota`; the file name is the catalogue
> number, not the crate name.

`quota` is the per-tenant, per-provider quota ledger for multi-tenant Rust
services. It answers one question before an outbound call: **may this tenant
spend one more unit of this provider's quota right now, and what happens if they
cannot?**

It is not a rate limiter for a single process, and it is not an HTTP client.
`wirekit` already has a process-local token bucket per provider. `quota` is the
shared state that survives across API pods, media workers, retries, and
restarts.

---

## Why this is not a wirekit module

| Concern | `wirekit` | `quota` |
|---|---|---|
| Scope | One outbound HTTP client | Shared state across services |
| State | In-process token bucket | Postgres/Redis-backed ledger |
| Unit | Requests per second | Provider-specific: YouTube units/day, WhatsApp messages, X pay-per-use, Twitch subscriptions |
| Failure | Backoff and retry | Hard stop, overage, degrade, or queue |
| Lifetime | Per client instance | Per tenant, per billing period |

Putting this in `wirekit` would force every consumer to adopt a database and a
tenant model. Keeping it separate lets a single-binary tool use `wirekit`
without `quota`, and lets a SaaS use `quota` with any HTTP client.

---

## Scope

### Core types

```rust
pub struct TenantId(pub Uuid);
pub struct ProviderId(pub &'static str);
pub struct Metric(pub &'static str);       // "units", "messages", "subscriptions", "cents"
pub struct Window { pub start: DateTime<Utc>, pub end: DateTime<Utc> }

pub struct Budget {
    pub tenant: TenantId,
    pub provider: ProviderId,
    pub metric: Metric,
    pub limit: u64,
    pub window: Window,
    pub policy: ExhaustionPolicy,
}

pub enum ExhaustionPolicy {
    HardStop,
    Overage { unit_cost_cents: u32 },
    Degrade,
    Queue,
}
```

### Operations

```rust
pub trait QuotaStore {
    async fn reserve(&self, req: Reserve) -> Result<Reservation, QuotaError>;
    async fn commit(&self, id: ReservationId) -> Result<(), QuotaError>;
    async fn release(&self, id: ReservationId) -> Result<(), QuotaError>;
    async fn usage(&self, key: &QuotaKey) -> Result<Usage, QuotaError>;
}
```

- **Reserve → commit → release.** A caller reserves before the provider call,
  commits on success, releases on a definite failure. This prevents two pods
  from spending the same last unit.
- **Idempotency.** A reservation carries a caller key; replaying the same
  request returns the existing reservation.
- **Window reset.** Daily, monthly, rolling, and provider-specific windows.
  YouTube resets at midnight Pacific; do not assume UTC.
- **Cost metrics.** Some providers are unit-based (YouTube), some per-message
  (WhatsApp), some monetary (X pay-per-use). The ledger stores `u64` and a
  metric label; currency conversion is the caller's job.

### Provider units to model first

| Provider | Metric | Window | Notes |
|---|---|---|---|
| YouTube Data/Live | `units` | Daily, Pacific | 10,000/day default; `videos.insert` has a separate 100/day cap; live chat costs 5/20/200 per call |
| WhatsApp Cloud | `messages` / `cents` | Monthly + 24-hour window | Template category and per-message pricing; service-window rules are separate |
| X API | `cents` | Monthly | Pay-per-use; price per endpoint |
| Twitch EventSub | `subscriptions` | Rolling | Conduit/shard limits; not a daily quota |
| Generic HTTP | `requests` | Per-second + monthly | Token bucket + monthly cap |

The provider-specific presets are data, not code paths. A caller can define its
own metric.

---

## Non-goals

- **Not an HTTP client.** Use `wirekit` or `reqwest`.
- **Not a billing system.** It records consumption; invoices and tax belong to
  the billing service.
- **Not a distributed lock service.** The default Postgres backend uses
  `SELECT … FOR UPDATE` or an atomic `UPDATE … WHERE remaining >= n`; Redis uses
  a Lua script. Do not build a consensus protocol.
- **Not a policy engine.** It enforces the policy the caller supplies; it does
  not decide pricing.

---

## Integration

1. The API/media service constructs a `QuotaKey` from the authenticated tenant
   and the provider.
2. Before the call: `reserve`. If denied, return a typed `QuotaExceeded` with
   reset time and upgrade path.
3. After the call: `commit` on success, `release` on a definite provider
   rejection. On timeout, leave the reservation pending and reconcile from the
   provider's usage headers.
4. `wirekit` exposes the provider's usage headers; the adapter feeds them into
   `quota` as an authoritative correction.
5. Emit `quota.reserved`, `quota.denied`, `quota.committed`, `quota.reconciled`
   for dashboards.

The OmniStream `quota-broker` service is a thin wrapper over this crate; the
crate owns the state and the invariants.

---

## Test plan

- Concurrent reservations from 100 tasks never exceed the limit.
- Reserve/commit/release idempotency under retry.
- Window reset at the provider's timezone (Pacific for YouTube).
- Overage policy: hard stop, overage charge, degrade, queue.
- Postgres transaction isolation and crash-between-reserve-and-commit.
- Redis Lua atomicity and failover behavior.
- Clock skew and monotonic time.
- Reconciliation from provider usage headers.
- Tenant isolation: no key can read another tenant's usage.

---

## Open questions

1. **Storage default.** Postgres is the source of truth; is Redis a cache or a
   second implementation? Recommend Postgres only for 1.0.
2. **Reservation TTL.** How long does an uncommitted reservation hold units
   before it is reclaimed? Recommend 2× the provider timeout.
3. **Monetary units.** Store cents as `u64`; who owns currency conversion?
4. **Provider presets.** Ship YouTube/WhatsApp/X/Twitch presets, or keep the
   crate provider-agnostic and put presets in a companion crate?
5. **Multi-region.** Is the ledger region-local with async reconciliation, or
   globally consistent? This is a product decision, not a library one.

---
title: quota-ledger 0.2 - Redis backend plan
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# quota-ledger 0.2 - Redis backend plan

## Approach

Add a `redis` feature to the existing crate that implements `QuotaStore` over a
caller-supplied `redis::aio::ConnectionManager`. The trait, the value types, and
`MemoryQuotaStore` are untouched. The claim is one Lua script executed with
`EVALSHA`, so capacity check, hold write, and counter update are atomic on the
server. Settlement uses two small scripts (`commit`, `release`) that consume a
hold exactly once by deleting the reservation key inside the script and checking
the deletion's result. Keys embed tenant, provider, metric, and window start, so
no operation can address another tenant's budget.

## Components

| Component | Action | Notes |
|---|---|---|
| `crates/quota-ledger/Cargo.toml` | modify | Add optional `redis` dependency and `redis = ["dep:redis"]`; keep `default = []`. |
| `crates/quota-ledger/src/lib.rs` | modify | Gate `pub mod redis` behind the feature; re-export the store. |
| `crates/quota-ledger/src/redis/mod.rs` | create | `RedisQuotaStore`: the `QuotaStore` implementation, connection handling, error mapping, and rustdoc. |
| `crates/quota-ledger/src/redis/keys.rs` | create | Key builders and window derivation; every key starts with the tenant. |
| `crates/quota-ledger/src/redis/reserve.lua` | create | Atomic claim: capacity predicate plus hold write plus counters. |
| `crates/quota-ledger/src/redis/settle.lua` | create | Commit and release in one script parameterised by mode; consumes the hold once. |
| `crates/quota-ledger/README.md` | modify | Document the backend, TLS expectation, key schema, and the no-environment rule. |

## Data model

- `quota:budget:{tenant}:{provider}:{metric}:{window_start_ms}` — hash with limit, used, held, overage_units, overage_cents, window_start, window_end.
- `quota:res:{tenant}:{reservation_id}` — hash with units, policy, unit_cost, expires, budget (the budget key), overage_cents, settled.
- `quota:residx:{reservation_id}` — the reservation key, so `commit` and `release` can settle by id (the 0.1 trait signature) while the data key keeps the tenant.
- `quota:idem:{tenant}:{provider}:{metric}:{window_start_ms}:{idempotency_key}` — the reservation id an idempotency key already produced.
- `quota:index:{tenant}:{provider}:{metric}` — set of window_start values used to find the active window.

TTLs are set to the window end plus a retention margin; the reservation TTL is the
configured expiry.

**Flow.** `reserve` computes the key set from the key and window, then runs
`reserve.lua` with limit, units, and expiry as arguments. The script returns the
reservation id, a denial with the reset instant, or an idempotency conflict.
`commit` and `release` run `settle.lua`, which deletes the reservation key and
only mutates the budget when the delete reports one key. `usage` reads the budget
hash and returns limit, used, held, overage, and reset.

## Security considerations

- **CON-512** — every key is tenant-prefixed; the store never accepts a query that
  is not scoped by the `QuotaKey`.
- **CON-516** — all values are script arguments; no key or argument is built by
  interpolating caller text into a command string.
- **CON-519** — no environment read; the caller supplies the connection.
- **CON-520** — the documentation requires a TLS connection and the crate never
  disables TLS.

## Performance considerations

- **NFR-002** — one round trip per reserve and per settle, because the decision is
  server-side rather than read-then-write.
- **RISK-001** — no Redis in CI; the script's atomicity is verified by an opt-in
  harness and the gap is reported.

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `redis` | library | add | Async, TLS-capable; version must resolve on Rust 1.87. No skill available; docs-only. |
| `tokio` | library | no_change | Already a dev-dependency. |

## Risks

- **RISK-001** — no Redis in CI, so concurrency proof needs an opt-in harness; the review records the gap instead of implying coverage.
- **RISK-002** — Lua expiry and settlement bugs are easy; each script gets failure-path coverage for missing, expired, and already-settled reservations.
- **RISK-003** — a `redis` release needing Rust ≥ 1.88 blocks the feature; pin a compatible release or stop and amend CON-102.

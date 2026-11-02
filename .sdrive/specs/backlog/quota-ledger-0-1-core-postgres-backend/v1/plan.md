---
title: quota-ledger 0.1 - technical plan
status: backlog
version: v1
created: 2026-11-02
updated: 2026-11-02
---

# quota-ledger 0.1 - technical plan

## Approach

Port the `quota-ledger` crate from `feature/m3-substrate` as prior art and adapt it to the current workspace: one library crate at `crates/quota-ledger`, feature-gated with `default = []` and an additive `postgres` feature. The core is a set of value types (identity, metric, window, budget, policy, key, reservation, usage), a single `QuotaError` enum, and an async `QuotaStore` trait with four operations. `MemoryQuotaStore` is the reference implementation and the deterministic test seam. The `postgres` feature adds a second `QuotaStore` implementation over a caller-supplied connection, whose atomicity comes from a conditional `UPDATE` rather than a lock held in the library. The crate depends on `platform-core` for tenant and provider identity so a tenant is one tenant across the stack, and never becomes a database or an HTTP client.

## Components

| Component | Action | Notes |
|---|---|---|
| `crates/quota-ledger/src/types.rs` | create | `QuotaKey`, `Metric`, `Window`, `Period`, `Budget`, `ExhaustionPolicy`, `Reservation`, `ReservationId`, `ReserveRequest`, `ReserveOutcome`, `Usage`, `QuotaExceeded`. Ported and adapted to `platform-core` identity types. |
| `crates/quota-ledger/src/error.rs` | create | The single `QuotaError` enum with stable variants, including a denial and an idempotency conflict. No second error type. |
| `crates/quota-ledger/src/store.rs` | create | The async `QuotaStore` trait covering `reserve`, `commit`, `release`, and `usage`, scoped by the tenant in the key. |
| `crates/quota-ledger/src/memory.rs` | create | `MemoryQuotaStore`: reference implementation, non-durable, for tests and single-process callers. |
| `crates/quota-ledger/src/window.rs` | create | `Window`/`Period` arithmetic, including the reset instant computed from the configured window rather than an assumed UTC midnight. |
| `crates/quota-ledger/src/policy.rs` | create | `ExhaustionPolicy` evaluation. |
| `crates/quota-ledger/src/postgres/mod.rs` | create | The `postgres`-feature backend: parameterized statements and an atomic conditional claim. |
| `crates/quota-ledger/src/postgres/schema.sql` | create | Shipped schema: budget row per `(tenant, provider, metric, window)` plus a reservation row. |
| `crates/quota-ledger/src/lib.rs` | create | Module declarations, re-exports, crate docs with scope and non-goals, lint attributes. |
| `crates/quota-ledger/Cargo.toml` | create | Dependencies declared directly, because `main` has no `[workspace.dependencies]`. |
| `crates/quota-ledger/README.md` | create | Scope, features, the reserve/commit/release flow, non-goals. |
| `LICENSE-MIT`, `LICENSE-APACHE` | reuse | Copied from the workspace root. |
| `crates/platform-core` | reuse | Tenant and provider identity are consumed, not redefined. |
| `Cargo.toml` | modify | The `crates/*` members glob already picks up the new package; verify resolution. |

## Data model

- **`Budget`** — tenant, provider, metric, limit, window, policy.
- **`Reservation`** — id, key, amount, optional idempotency key, state, created_at.
- **`Usage`** — limit, consumed, held, optional overage, resets_at.
- **Postgres `budget`** — tenant, provider, metric, window bounds, limit, consumed, held.
- **Postgres `reservation`** — id, budget reference, amount, idempotency key, state, created_at.

**Flow.** A caller derives a `QuotaKey` from the authenticated tenant and the provider, then reserves before the outbound call. `reserve` checks capacity and records a hold atomically, returning a `Reservation` or a typed `QuotaExceeded`. On success the caller `commit`s, consuming the hold exactly once; on a definite provider rejection the caller `release`s it. `usage` reports limit, consumed, held, and the next reset. In Postgres the claim is one conditional `UPDATE` whose `WHERE` clause carries the remaining-capacity predicate, so the database serializes concurrent claims and two pods cannot both take the final unit.

## Security considerations

- **NFR-001** — every store operation is keyed by the tenant, so no read or write crosses a tenant boundary.
- **NFR-002** — every Postgres statement is parameterized; caller values never reach SQL text.
- **NFR-003** — caller-supplied windows, metrics, and keys are validated; an invalid window is a typed error, not a guessed default.
- **NFR-004** — no credentials are read from the environment or a file; the caller supplies the connection, and a Postgres connection must use TLS.

## Performance considerations

- **NFR-002** — the reserve path is a single conditional statement rather than read-then-write, removing a round trip and the lost-update race.
- **NFR-005** — the memory backend shards by key so unrelated budgets are not serialized during tests.

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `platform-core` | internal | no_change | Path dependency already on `main`; supplies `TenantId` and `ProviderId`. |
| `async-trait` | library | add | Declared directly; no skill available for Rust crate dependencies, docs-only per the `platform-core` precedent. |
| `chrono` | library | add | Windows and the reset instant. |
| `thiserror` | library | add | The single error enum. |
| `uuid` | library | add | `TenantId` payload and `ReservationId`. |
| `sqlx` | library | add | Optional, behind `postgres`; parameterized queries and TLS. |
| `tokio` | library | add | Dev-dependency only, for the test agent. |

## Risks

- **The Postgres backend is the hardest part to verify without a live database**, so a defect in the atomic claim may pass review. AC-013 asserts the concurrency invariant directly, and the prior art ships a Postgres test harness to adapt.
- **Porting stale code may drag in manifests using workspace inheritance that `main` does not define.** Adapt the manifest to declare dependencies directly, as `platform-core` did.
- **A quota ledger invites scope creep into billing and pricing.** The out-of-scope list and NFR-005 are enforced at review.

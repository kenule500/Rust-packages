# Technical Plan - m3-substrate v1

> **Feature:** `m3-substrate`
> **Version:** v1
> **Status:** backlog
> **Spec:** `.sdrive/specs/backlog/m3-substrate/v1/spec.md`
> **Branch:** `feature/m3-substrate`
> **Base:** `main`

## Summary

This plan adds four library crates to the existing Cargo workspace, each with one job, plus one integration example:

1. `platform-core` - identity, capabilities, an explicit registry, a token-store contract, and secret redaction.
2. `quota-ledger` - reserve/commit/release quota state with an in-memory store and an optional Postgres backend.
3. `hookgate` - raw-byte webhook verification, HMAC presets, replay window, durable dedupe, outbox, and dead-letter.
4. `syncforge` - a durable action queue, UUID v7 idempotency, per-action batch results, and explicit conflict strategies.
5. `hookgate/examples/substrate_integration.rs` - composes `platform-core` + `quota-ledger` + `hookgate` without a framework.

The feature stops at **release-ready 0.1.0**. Publishing to crates.io is a manual post-merge step.

## Architecture Approach

### Dependency direction

```
consuming application
        |
        +--> platform-core      (identity, traits, registry; no product nouns)
        +--> quota-ledger  ---> platform-core   (reuses TenantId and ProviderId)
        +--> hookgate      ---> platform-core   (reuses TenantId and ProviderId)
        +--> syncforge          (transport/storage traits; no identity coupling)

hookgate/examples/substrate_integration  ---> platform-core, quota-ledger, hookgate
```

`quota-ledger` and `hookgate` depend on `platform-core` **only** for `TenantId` and `ProviderId`, so the ecosystem has one identity type instead of three. This is an internal path dependency, not a copy of the types. No crate depends on a consuming application (CON-110), and no crate depends on `wirekit` in this base.

### `platform-core`

```
src/
  lib.rs        crate docs, lints, feature table, re-exports
  id.rs         TenantId, ProviderId
  capability.rs ProvisioningMode, Capability
  provider.rs   Provider trait
  registry.rs   Registry (register, get, iter, capabilities)
  token.rs      Secret, TokenStore trait, MemoryTokenStore
  error.rs      IntegrationError
```

Key decisions:

- `TenantId(Uuid)` and `ProviderId(&'static str)` are `Copy`, `Eq`, `Hash`, and `Debug`. `ProviderId` is opaque: the crate never enumerates providers.
- `ProvisioningMode` is `Api`, `Partner`, `Manual`, `Unsupported`. `Capability { key: &'static str, mode: ProvisioningMode }`; the key vocabulary belongs to the adapter.
- `Registry` owns a `Vec<Arc<dyn Provider>>` plus an index. `register` returns `Err(IntegrationError::DuplicateProvider)` on a repeated id; `iter` preserves registration order. No statics, no lazy globals.
- `Secret` wraps `secrecy::SecretString`: redacted `Debug`/`Display`, no `Serialize`, explicit `expose_secret()`.
- `TokenStore` is `#[async_trait]` with `get`, `put`, and `revoke`. `MemoryTokenStore` uses a `RwLock<HashMap<(TenantId, ProviderId), Secret>>` and is the reference implementation for the contract suite.
- `IntegrationError` has variants `Auth`, `Scope`, `RateLimit`, `Quota`, `Provider`, `Transport`, `Unsupported`, `DuplicateProvider`, `Config`. Each has a stable `code()` such as `PC-AUTH`, and `Display` prints `code: message`.

Feature flags:

| Feature | Default | Adds |
|---|---|---|
| `registry` | yes | Provider trait, Registry, and capability types |
| `memory` | yes | `MemoryTokenStore` for tests and single-binary tools |

Identity types, `Secret`, `TokenStore`, and `IntegrationError` are always available. There is no `sqlx` or `axum` feature in 0.1 (deferred by decision).

### `quota-ledger`

```
src/
  lib.rs          crate docs, lints, re-exports
  types.rs        Metric, Window, Budget, QuotaKey, Reservation, ReservationId, Usage
  window.rs       reset computation for fixed and rolling windows
  policy.rs       ExhaustionPolicy and typed outcomes
  store.rs        QuotaStore trait
  memory.rs       MemoryQuotaStore
  error.rs        QuotaError
  postgres/
    mod.rs        PostgresQuotaStore (feature = "postgres")
    schema.sql    tables, unique indexes, reclaim query
```

Key decisions:

- `QuotaKey { tenant, provider, metric, window_start }` identifies one ledger row. `Budget` carries `limit`, `window`, and `policy`.
- `reserve` returns `ReserveOutcome`: `Reserved(Reservation)`, `Denied(QuotaExceeded)`, `Degraded { reset_at }`, `Queued { reset_at }`, or `Overage { reservation, cost_cents }`. The caller never inspects a boolean.
- The in-memory store performs the capacity check, hold insertion, and idempotency-key lookup under one write lock, so the check-then-act race cannot occur. The Postgres store uses a single atomic statement:

  ```sql
  UPDATE quota_budget
     SET used = used + $units, updated_at = now()
   WHERE tenant_id = $tenant
     AND provider = $provider
     AND metric = $metric
     AND window_start = $window_start
     AND used + $units <= $limit
  RETURNING used;
  ```

  Zero rows updated means denial. A unique index on `(tenant_id, provider, metric, idempotency_key)` makes idempotency enforceable, and `reservation.expires_at` plus a reclaim query returns abandoned holds once.
- `commit` moves a reservation from held to consumed exactly once; `release` deletes the hold without consumption; both are idempotent and return a typed no-op on a second call.
- `Window` is `Fixed { period: Daily | Monthly, timezone: Tz }` or `Rolling { duration }`. Reset instants are computed from the configured timezone, never the host's local time.
- The `postgres` feature adds `sqlx` with `postgres`, `runtime-tokio-rustls`, `chrono`, and `uuid`. Default features never pull a database.

### `hookgate`

```
src/
  lib.rs        crate docs, lints, re-exports
  types.rs      DeliveryId, IdempotencyKey, InboundRequest, VerifiedEvent
  verify/
    mod.rs      Verifier trait, VerifyError, ReplayWindow
    presets.rs  Meta, Stripe, Shopify, Twilio, Slack, GitHub, Generic
  store.rs      DeliveryStore, DeliveryRecord, InsertOutcome, StoreError, Outbox
  memory.rs     MemoryDeliveryStore (dedupe + outbox + dead-letter)
  error.rs      HookgateError, DeadLetterRecord
```

Key decisions:

- `InboundRequest` owns `Bytes` so the raw body is preserved exactly. `verify` receives `&InboundRequest` and never deserializes first.
- HMAC presets share one generic implementation parameterized by header, encoding (hex/base64), prefix, signed payload shape, and digest (SHA-256 or SHA-1 for Twilio). Comparison uses `subtle::ConstantTimeEq`.
- `ReplayWindow { tolerance: Duration, max_body: usize }` rejects stale and oversized requests with distinct typed errors.
- `DeliveryStore::insert_if_new` keys on `(tenant, provider, provider_event_id)` and returns `InsertOutcome::Inserted` or `Duplicate`. The in-memory store performs dedupe, delivery insert, and outbox push under one lock; a transactional backend implements the same contract in one transaction.
- `mark_failed` beyond `max_attempts` writes a `DeadLetterRecord` with the raw body and error text. `replay` re-enqueues the same `DeliveryId`, preserving identity.
- Metrics are emitted as `tracing` events: `hookgate.received`, `verified`, `duplicate`, `rejected`, `dead_lettered`.

### `syncforge`

```
src/
  lib.rs        crate docs, lints, re-exports
  action.rs     ActionId, Action, EntityType, Operation
  batch.rs      Batch, BatchResult, ActionResult, ActionStatus
  conflict.rs   ConflictStrategy, ConflictInfo, CustomMerger
  storage.rs    Storage trait + MemoryStorage
  transport.rs  Transport trait + MockTransport
  engine.rs     SyncEngine (enqueue, replay)
  timestamp.rs  client/server audit timestamps and tolerance
  error.rs      SyncError
```

Key decisions:

- `ActionId(Uuid)` is generated with `Uuid::now_v7()`. A process-local guard keeps the timestamp component non-decreasing even when the clock stalls.
- `Action` is generic over the payload type or carries `serde_json::Value` in 0.1; the id, entity, operation, client timestamp, idempotency key, and attempts are always present.
- `Storage` is `#[async_trait]` with `append`, `pending`, `mark_sent`, `mark_failed`, and `record_attempt`. `MemoryStorage` uses an ordered map so pending order is deterministic.
- `Transport` is `#[async_trait]` with `send(Batch) -> Result<BatchResult, TransportError>`. `MockTransport` scripts whole-batch and per-action outcomes.
- `SyncEngine::replay` drains pending actions in order in configurable batches, asks the conflict strategy for each `Conflict`, marks `Applied` and `Duplicate` as sent, retries `Retry` with bounded exponential backoff, and dead-letters after `max_attempts` without blocking later actions.
- Conflict strategies: `LastWriteWins` compares timestamps, `ServerWins` and `ClientWins` select a side, `Custom(Arc<dyn CustomMerger>)` delegates. A conflict without server state is a typed error.
- Timestamps: the action keeps `client_timestamp`; the result records `server_timestamp` and a tolerance decision. Client time is never silently rewritten.
- Tracing events: `syncforge.enqueued`, `replay.started`, `replay.action`, `replay.finished`, `conflict.resolved`, with queue depth, conflict count, and duration.

### Integration example

`crates/hookgate/examples/substrate_integration.rs` uses dev-dependencies on `platform-core` and `quota-ledger`:

1. Build a `Registry` and register one provider with one capability.
2. Reserve one unit of a budget, commit it, and read usage.
3. Verify a signed webhook with the generic HMAC preset, insert it, enqueue it, and read the outbox.

Putting the example inside `crates/hookgate/examples/` keeps the workspace member glob `crates/*` unchanged (CON-101) while still compiling and testing the composition.

## Components

| Component | Action | Description |
|---|---|---|
| `Cargo.toml` | modify | Add `[workspace.dependencies]` for shared versions and keep `members = ["crates/*"]`. |
| `crates/platform-core/Cargo.toml` | create | Package metadata, inherited edition/MSRV/licence, `registry` and `std` features, `async-trait`, `secrecy`, `thiserror`, `uuid`, `tracing`. |
| `crates/platform-core/src/lib.rs` | create | Crate docs, feature table, lints, re-exports. |
| `crates/platform-core/src/id.rs` | create | `TenantId`, `ProviderId`. |
| `crates/platform-core/src/capability.rs` | create | `ProvisioningMode`, `Capability`. |
| `crates/platform-core/src/provider.rs` | create | `Provider` trait. |
| `crates/platform-core/src/registry.rs` | create | Explicit `Registry`, duplicate detection, deterministic iteration. |
| `crates/platform-core/src/token.rs` | create | `Secret`, `TokenStore`, `MemoryTokenStore`. |
| `crates/platform-core/src/error.rs` | create | `IntegrationError` with stable codes. |
| `crates/platform-core/README.md` | create | Purpose, feature table, example, non-goals. |
| `crates/quota-ledger/Cargo.toml` | create | `postgres` feature, path dependency on `platform-core`, `sqlx` optional. |
| `crates/quota-ledger/src/lib.rs` | create | Crate docs, feature table, lints, re-exports. |
| `crates/quota-ledger/src/types.rs` | create | Quota types and outcomes. |
| `crates/quota-ledger/src/window.rs` | create | Fixed and rolling reset computation. |
| `crates/quota-ledger/src/policy.rs` | create | `ExhaustionPolicy` outcomes. |
| `crates/quota-ledger/src/store.rs` | create | `QuotaStore` trait. |
| `crates/quota-ledger/src/memory.rs` | create | Atomic in-memory store and idempotency. |
| `crates/quota-ledger/src/postgres/mod.rs` | create | Postgres backend behind `postgres`. |
| `crates/quota-ledger/src/postgres/schema.sql` | create | Tables, unique indexes, reclaim query. |
| `crates/quota-ledger/src/error.rs` | create | `QuotaError` with stable codes. |
| `crates/quota-ledger/README.md` | create | Purpose, backend setup, feature table, non-goals. |
| `crates/hookgate/Cargo.toml` | create | `hmac`, `sha2`, `sha1`, `subtle`, `hex`, `base64`, `http`, `bytes`; dev-deps on `platform-core` and `quota-ledger`. |
| `crates/hookgate/src/lib.rs` | create | Crate docs, feature table, lints, re-exports. |
| `crates/hookgate/src/types.rs` | create | Delivery and request types. |
| `crates/hookgate/src/verify/mod.rs` | create | `Verifier`, `VerifyError`, `ReplayWindow`. |
| `crates/hookgate/src/verify/presets.rs` | create | Meta, Stripe, Shopify, Twilio, Slack, GitHub, generic HMAC. |
| `crates/hookgate/src/store.rs` | create | `DeliveryStore`, `Outbox`, `InsertOutcome`. |
| `crates/hookgate/src/memory.rs` | create | In-memory dedupe, outbox, and dead-letter. |
| `crates/hookgate/src/error.rs` | create | `HookgateError`, `DeadLetterRecord`. |
| `crates/hookgate/examples/substrate_integration.rs` | create | End-to-end example across three crates. |
| `crates/hookgate/README.md` | create | Purpose, presets, flow, non-goals. |
| `crates/syncforge/Cargo.toml` | create | `uuid` v7, `serde`, `serde_json`, `async-trait`, `tracing`, `thiserror`. |
| `crates/syncforge/src/lib.rs` | create | Crate docs, feature table, lints, re-exports. |
| `crates/syncforge/src/action.rs` | create | Action types and UUID v7 ids. |
| `crates/syncforge/src/batch.rs` | create | Batch and per-action result types. |
| `crates/syncforge/src/conflict.rs` | create | Conflict strategies and custom merger. |
| `crates/syncforge/src/storage.rs` | create | `Storage` trait and `MemoryStorage`. |
| `crates/syncforge/src/transport.rs` | create | `Transport` trait and `MockTransport`. |
| `crates/syncforge/src/engine.rs` | create | Enqueue and replay engine. |
| `crates/syncforge/src/timestamp.rs` | create | Client/server timestamps and tolerance. |
| `crates/syncforge/src/error.rs` | create | `SyncError` with stable codes. |
| `crates/syncforge/README.md` | create | Purpose, replay model, non-goals, at-least-once contract. |
| `README.md` | modify | Add the four crates to the package table and current status. |

## Data Model

### `platform-core`

| Entity | Action | Fields |
|---|---|---|
| `TenantId` | create | `0: Uuid` |
| `ProviderId` | create | `0: &'static str` |
| `ProvisioningMode` | create | `Api`, `Partner`, `Manual`, `Unsupported` |
| `Capability` | create | `key: &'static str`, `mode: ProvisioningMode` |
| `Registry` | create | `providers: Vec<Arc<dyn Provider>>`, `index: HashMap<ProviderId, usize>` |
| `Secret` | create | opaque `secrecy::SecretString` |
| `IntegrationError` | create | `Auth`, `Scope`, `RateLimit`, `Quota`, `Provider`, `Transport`, `Unsupported`, `DuplicateProvider`, `Config` |

### `quota-ledger`

| Entity | Action | Fields |
|---|---|---|
| `Metric` | create | `0: &'static str` |
| `Window` | create | `Fixed { period, timezone }`, `Rolling { duration }` |
| `Budget` | create | `tenant`, `provider`, `metric`, `limit: u64`, `window`, `policy` |
| `QuotaKey` | create | `tenant`, `provider`, `metric`, `window_start` |
| `Reservation` | create | `id: ReservationId`, `key`, `units: u64`, `idempotency_key: Option<String>`, `expires_at`, `policy` |
| `Usage` | create | `limit: u64`, `used: u64`, `held: u64`, `overage: u64`, `overage_cents: u64`, `reset_at` |
| `ExhaustionPolicy` | create | `HardStop`, `Overage { unit_cost_cents }`, `Degrade`, `Queue` |
| `ReserveOutcome` | create | `Reserved`, `Overage`, `Denied`, `Degraded`, `Queued` |
| `QuotaError` | create | `Exceeded`, `UnknownBudget`, `ReservationNotFound`, `AlreadySettled`, `IdempotencyConflict`, `Backend`, `Config` |

### `hookgate`

| Entity | Action | Fields |
|---|---|---|
| `DeliveryId` | create | `0: Uuid` |
| `IdempotencyKey` | create | `0: String` |
| `InboundRequest` | create | `tenant`, `provider`, `headers: HeaderMap`, `body: Bytes`, `received_at` |
| `VerifiedEvent` | create | `delivery`, `tenant`, `provider`, `event_id: Option<String>`, `event_type`, `payload: serde_json::Value`, `raw: Bytes` |
| `DeliveryRecord` | create | `delivery`, `tenant`, `provider`, `event_id`, `event_type`, `raw`, `status`, `attempts` |
| `DeadLetterRecord` | create | `delivery`, `tenant`, `provider`, `raw`, `error`, `dead_lettered_at` |
| `InsertOutcome` | create | `Inserted`, `Duplicate` |
| `HookgateError` | create | `Verify`, `Store`, `Outbox`, `Config` |

### `syncforge`

| Entity | Action | Fields |
|---|---|---|
| `ActionId` | create | `0: Uuid` (v7) |
| `Action` | create | `id`, `entity: EntityType`, `operation: Operation`, `payload: serde_json::Value`, `client_timestamp`, `idempotency_key: Uuid`, `attempts: u32` |
| `Batch` | create | `actions: Vec<Action>` |
| `BatchResult` | create | `results: Vec<ActionResult>` |
| `ActionResult` | create | `action_id`, `status: ActionStatus`, `server_timestamp: Option<DateTime<Utc>>`, `conflict: Option<ConflictInfo>` |
| `ActionStatus` | create | `Applied`, `Duplicate`, `Conflict`, `Rejected`, `Retry` |
| `ConflictInfo` | create | `client_timestamp`, `server_timestamp`, `resolution: Resolution`, `server_state: Option<serde_json::Value>` |
| `ConflictStrategy` | create | `LastWriteWins`, `ServerWins`, `ClientWins`, `Custom(Arc<dyn CustomMerger>)` |
| `SyncEngine` | create | `storage: Arc<dyn Storage>`, `transport: Arc<dyn Transport>`, `conflict`, `max_attempts`, `batch_size`, `tolerance` |
| `SyncError` | create | `Storage`, `Transport`, `ConflictWithoutServerState`, `InvalidAction`, `Config` |

### Flow

`platform-core` holds no rows. `quota-ledger` rows are `(tenant, provider, metric, window_start)` with `limit`, `used`, `held`, `overage`, `overage_cents`, and a `reservation` table with `id`, `units`, `idempotency_key`, `expires_at`, `settled_at`. `hookgate` rows are `delivery` (unique on tenant, provider, event id), `outbox`, and `dead_letter`. `syncforge` rows are actions with status and attempts. All identifiers are UUIDs; all timestamps are UTC with an explicit source.

## API Contracts

No HTTP surface. These are library APIs:

| API | Action | Linked requirements |
|---|---|---|
| `platform_core::Registry::register` | create | FR-002, FR-003 |
| `platform_core::TokenStore::{get, put, revoke}` | create | FR-004, NFR-001 |
| `quota_ledger::QuotaStore::{reserve, commit, release, usage}` | create | FR-008, FR-009, FR-010 |
| `hookgate::Verifier::verify` | create | FR-014, FR-015 |
| `hookgate::DeliveryStore::insert_if_new` | create | FR-017, FR-018 |
| `syncforge::SyncEngine::{enqueue, replay}` | create | FR-024 |

## Security Considerations

| Requirement | Control |
|---|---|
| NFR-001 | Tenant is part of every store key and every SQL predicate; memory stores key on `(TenantId, ProviderId, ...)`. |
| NFR-002 | `Secret` and connection strings redact in `Debug`/`Display`; no `Serialize` for secrets; errors never include the value. |
| NFR-003 | Raw bytes are length-checked, signatures are constant-time compared, timestamps are bounded, and every failure is typed. |
| NFR-004 | Parameterized SQL only; fixed table and column names; no caller-supplied identifiers. |
| NFR-005 | One atomic statement per reservation; idempotency enforced by a unique index. |
| NFR-007 | `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` in every crate root. |
| NFR-009 | `sslmode=require` or stricter is the default; `allow_insecure_for_tests` is an explicit, documented opt-in. |

Security baseline mapping (CON-51x) is recorded in `spec.md`; every REQUIRED control has an NFR, a dependency (where needed), a task, and at least one AC in `traceability.json`.

## Performance Considerations

| Requirement | Strategy |
|---|---|
| NFR-004 | Indexes on `(tenant, provider, metric, window_start)` and `(tenant, provider, event_id)` keep lookups O(log n). |
| NFR-005 | One conditional `UPDATE` avoids an application-level read-modify-write round trip. |
| NFR-010 | In-memory storage uses ordered structures; batches are bounded; backoff is capped. |
| FR-015 | HMAC verification is linear in body size and allocation-light (`Bytes` is shared, not copied). |
| FR-027 | Tracing spans are emitted per batch, not per byte; metrics stay cheap on the hot path. |

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `uuid` | library | add | v4, v7, serde. |
| `chrono` | library | add | UTC timestamps, timezone windows, serde. |
| `serde` | library | add | Events, actions, payloads. |
| `serde_json` | library | add | Event and action payloads. |
| `async-trait` | library | add | Object-safe async traits. |
| `thiserror` | library | add | Error enums with stable codes. |
| `tracing` | library | add | Queue, replay, conflict, webhook metrics. |
| `secrecy` | library | add | Redacted token wrapper. |
| `hmac` | library | add | Webhook HMAC construction. |
| `sha2` | library | add | SHA-256 presets. |
| `sha1` | library | add | Twilio preset. |
| `subtle` | library | add | Constant-time comparison. |
| `hex` | library | add | Hex signature decoding. |
| `base64` | library | add | Shopify signature decoding. |
| `http` | library | add | `HeaderMap` in `InboundRequest`. |
| `bytes` | library | add | Raw body preservation. |
| `sqlx` | library | add | Optional, `postgres` feature only. |
| `tokio` | library | add | Dev-dependency for async tests. |
| `platform-core` | internal | add | Path dependency from `quota-ledger`, `hookgate`, and the example. |
| `crates.io` | service | no_change | Existing publication target; name check before publish. |
| `wirekit` | internal | no_change | Deliberately not added in this version; not present on `main`. |

Every `action: "add"` dependency needs a matching skill in `.sdrive/skills/` before `/sdrive:apply` writes code that uses it. `/sdrive:apply` provisions those skills; this plan only records the requirement.

## Phases

| Phase | Name | Depends on | Tasks |
|---|---|---|---|
| P1 | Workspace scaffold | - | T1.1-T1.3 |
| P2 | `platform-core` | P1 | T2.1-T2.5 |
| P3 | `quota-ledger` core | P2 | T3.1-T3.5 |
| P4 | `quota-ledger` Postgres | P3 | T4.1-T4.5 |
| P5 | `hookgate` | P2 | T5.1-T5.6 |
| P6 | `syncforge` | P1 | T6.1-T6.8 |
| P7 | Integration example and verification | P2, P3, P5 | T7.1-T7.3 |

P5 and P6 can proceed in parallel after P2/P1 respectively; they touch disjoint crates.

## Verification

Production verification only, per the no-test-tasks rule:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-features -- -D warnings`
- `cargo build --workspace --all-features`
- `cargo build --workspace --no-default-features` for the no-database guarantee where the feature graph allows it
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`
- `cargo package --workspace --list` (metadata check)
- `node .sdrive/scripts/validate-governance.js`
- `sdrive verify m3-substrate`

Test authoring and execution are `/sdrive:test` work and are intentionally absent from `tasks.json`.

## Risks

| Risk | Mitigation |
|---|---|
| Four crates in one feature create review fatigue | One crate per phase; each phase compiles and documents its public surface independently. |
| Async trait objects break object safety | `async-trait` everywhere, object-safe traits, contract suites against memory and mock implementations. |
| No Postgres available in CI | Backend behind a feature; `DATABASE_URL` opt-in tests; `/sdrive:test` decides provisioning. |
| SQL injection through dynamic SQL | Fixed schema and parameterized values only. |
| Timing or replay weaknesses in verification | Constant-time comparison, raw bytes, unique constraints, replay-window checks. |
| Crate names taken on crates.io | Check immediately before publish; stop on a collision. |
| MSRV drift | Existing MSRV CI job; pin or downgrade with a recorded decision. |
| Trait surface absorbs product nouns | Review every public type against the non-goals list. |
| Replay duplicates actions | UUID v7 keys, per-action results, deterministic mock tests, documented at-least-once contract. |
| Scope creep from deferred features | Additions require a new version. |

---
title: syncforge 0.1 - technical plan
status: ongoing
version: v1
created: 2026-11-02
updated: 2026-11-02
---

# syncforge 0.1 - technical plan

## Approach

Port the `syncforge` crate from `feature/m3-substrate` as prior art and adapt it to the current workspace: one self-contained library crate at `crates/syncforge` with `default = []` and no dependency on `wirekit` or `platform-core`. The crate is built around two abstract seams â€” `Storage` and `Transport` â€” with `MemoryStorage` and `MockTransport` as the default implementations so replay is deterministic in tests. A `SyncEngine` owns the queue, batches pending actions, sends a batch through the transport, and maps the returned per-action results onto queue state. `Action` carries a UUID v7 idempotency key created at construction, so at-least-once delivery becomes safe replay. Conflict resolution is a strategy chosen per entity rather than logic buried in a handler, and timestamp handling returns a typed decision so a skewed device clock is recorded rather than trusted silently. Retry uses the attempt count on the action with a configurable cap, after which an action is marked failed rather than blocking the queue. Instrumentation events cover queue depth, replay, conflicts, and duration.

## Components

| Component | Action | Notes |
|---|---|---|
| `crates/syncforge/src/action.rs` | create | `Action`, `ActionId` (UUID v7), `EntityType`, `Operation`, and the idempotency key created at construction. |
| `crates/syncforge/src/batch.rs` | create | `Batch` with a configured size bound, `BatchResult`, `ActionResult`, and `ActionStatus` (`Applied`, `Duplicate`, `Conflict`, `Rejected`, `Retry`). |
| `crates/syncforge/src/conflict.rs` | create | `ConflictStrategy` (`LastWriteWins`, `ServerWins`, `ClientWins`, `Custom`), `CustomMerger`, `ConflictInfo`, and `Resolution`. |
| `crates/syncforge/src/timestamp.rs` | create | The `TimestampDecision` that accepts, clamps, or rejects the client time within a configurable tolerance, recording both times. |
| `crates/syncforge/src/storage.rs` | create | The `Storage` trait plus `MemoryStorage`, the durable-queue seam. An action is persisted before enqueue reports success. |
| `crates/syncforge/src/transport.rs` | create | The `Transport` trait plus `MockTransport` for deterministic replay tests, and response validation so a malformed response is a typed error. |
| `crates/syncforge/src/engine.rs` | create | `SyncEngine`: enqueue, batch, replay, map per-action results, apply the conflict strategy, enforce the attempt cap, and emit tracing events. |
| `crates/syncforge/src/error.rs` | create | `SyncError` with stable variants. The single error type for the crate. |
| `crates/syncforge/src/lib.rs` | create | Module declarations, re-exports, crate docs with the replay model and non-goals, and the `forbid(unsafe_code)` and `deny(missing_docs)` attributes. |
| `crates/syncforge/Cargo.toml` | create | Dependencies declared directly (`main` has no `[workspace.dependencies]`). Feature `default = []`. No `wirekit` or `platform-core` dependency. |
| `crates/syncforge/README.md` | create | Crate README describing the queue, idempotency, conflict strategies, timestamp handling, and the non-goals. |
| `LICENSE-MIT`, `LICENSE-APACHE` | reuse | Copied from the workspace root. |
| `Cargo.toml` | modify | The `crates/*` members glob already picks up the new package; verify resolution. |

## Data model

- **`Action`** â€” id, entity, operation, payload, client timestamp, idempotency key, attempt count.
- **`BatchResult`** â€” one `ActionResult` per action.
- **`ActionResult`** â€” action id, status, optional server timestamp, optional conflict.
- **`ConflictInfo`** â€” entity, server value, client value, server timestamp.

**Flow.** `enqueue` persists the action through `Storage` and only then reports success, so a crash cannot lose the write. On reconnect the engine lists pending actions, groups them into batches bounded by the configured size, and sends each batch through `Transport`. The server validates, dedupes by the idempotency key, executes each action through the caller's domain service, and returns one `ActionResult` per action. The client marks each action by its own result: `Applied` and `Duplicate` become sent (a duplicate is not an error and is not retried), `Conflict` invokes the configured strategy to produce a `Resolution`, `Rejected` is terminal and surfaced, and `Retry` advances the attempt count. An action that reaches the attempt cap is marked failed and surfaced rather than blocking the queue. A server-reported conflict carries `ConflictInfo` with both sides, and the timestamp decision records both the client and server times so a skewed clock is visible rather than silently trusted.

## Security considerations

- **NFR-001** â€” the crate reads no credentials and no environment; storage and transport are supplied by the caller, so the crate never becomes a place a secret can leak from.
- **NFR-002** â€” transport responses are untrusted input: they are validated and bounded before any queue state mutates, and a malformed response is a typed `SyncError` rather than a panic.

## Performance considerations

- **NFR-003** â€” batching bounds the request count for a long offline period, and the configured batch size caps a single request so a week offline does not replay as one unbounded call.
- **NFR-004** â€” `MemoryStorage` eliminates I/O in the fast path for tests, and the `Storage` trait lets a real backend do batched writes without changing queue or conflict logic.

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `async-trait` | library | add | Declared directly; no skill available for Rust crate dependencies, docs-only per the `platform-core` precedent. |
| `chrono` | library | add | Client and server timestamps and the tolerance decision. |
| `serde` | library | add | `Action` and batch serialization across the client/server boundary. |
| `serde_json` | library | add | `Action` payload handling. |
| `uuid` | library | add | `ActionId` and the UUID v7 idempotency key. |
| `thiserror` | library | add | `SyncError` with stable variants. |
| `tracing` | library | add | Queue depth, replay rate, conflicts, and duration. |
| `tokio` | library | add | Dev-dependency only, for the test agent's async replay tests. |

## Risks

- **Queue and conflict semantics are the highest design risk in M3**, and a rushed model is expensive to change once a client depends on it. The catalogue's failure-mode list is mirrored in the edge cases and every failure mode is pinned by an AC; the test agent covers replay determinism with `MockTransport`.
- **The pre-1.0 API will change before 1.0**, and both client and server compile against the shared types, so a change breaks both sides at once. Document 0.x instability as the workspace already does for every crate, and keep the public surface to the catalogue list.
- **Porting stale code may drag in manifests using workspace inheritance that `main` does not define.** Adapt the manifest to declare dependencies directly, as `platform-core` did, and let the MSRV and clippy jobs prove it.

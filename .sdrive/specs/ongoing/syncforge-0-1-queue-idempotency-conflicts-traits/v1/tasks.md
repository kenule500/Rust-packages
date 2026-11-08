---
title: syncforge 0.1 - execution tasks
status: ongoing
version: v1
created: 2026-11-02
updated: 2026-11-02
---

# syncforge 0.1 - execution tasks

> Production code only. Test authoring and test execution belong to the test agent (`/sdrive:test`) and are not tasks here.

## P1 - Crate scaffold and core types

- [x] **T1.1 - Scaffold the syncforge crate** (`FR-001` / `AC-013`)
  Create `crates/syncforge` with a manifest that declares dependencies directly (`main` has no `[workspace.dependencies]`), feature `default = []`, the workspace-inherited `edition`, `rust-version`, `license`, and `repository` fields, the licence files, a README, and a `lib.rs` carrying `forbid(unsafe_code)`, `deny(missing_docs)`, and crate docs stating the replay model, scope, and non-goals. Lay out the modules that expose the catalogue's public surface: `action`, `batch`, `conflict`, `timestamp`, `storage`, `transport`, `engine`, and `error`. Confirm the workspace resolves the new member. The crate must not depend on `wirekit` or `platform-core`.
- [x] **T1.2 - Implement the core types, the error type, and the storage and transport seams** (`FR-001`, `FR-009`, `NFR-001` / `AC-011`, `AC-013`)
  Implement `action.rs` with `Action`, `ActionId`, `EntityType`, and `Operation`, and `error.rs` with the single `SyncError` enum. Implement `storage.rs` with the `Storage` trait and `MemoryStorage`, and `transport.rs` with the `Transport` trait and `MockTransport`. Every public item carries rustdoc and every module states scope and non-goals. The crate reads no credentials and no environment; storage and transport are caller-supplied.

## P2 - Queue, storage, and replay

- [x] **T2.1 - Implement durable enqueue and idempotency keys** (`FR-002`, `FR-003` / `AC-001`, `AC-002`, `AC-003`)
  Implement `enqueue` so an action is persisted to `Storage` before the call reports success. Every action receives a UUID v7 `ActionId` and a distinct idempotency key at construction.
- [x] **T2.2 - Implement bounded batching and per-action results** (`FR-004`, `FR-005` / `AC-005`, `AC-006`)
  Implement `batch.rs` with `Batch`, `BatchResult`, `ActionResult`, and `ActionStatus`. A batch is bounded by a configured size so a long offline period does not replay as one unbounded request, and a batch returns exactly one `ActionResult` per action with a status of `Applied`, `Duplicate`, `Conflict`, `Rejected`, or `Retry`.
- [x] **T2.3 - Implement replay, duplicate handling, and the attempt cap** (`FR-003`, `FR-004`, `FR-008` / `AC-003`, `AC-004`, `AC-005`, `AC-010`)
  Implement `engine.rs` replay: send each pending action with its idempotency key and map the returned `BatchResult` onto queue state. An `Applied` result is marked sent, a `Duplicate` is marked sent and never retried, `Rejected` is terminal and surfaced, and `Retry` advances the attempt count. An action that reaches the configured attempt cap is marked failed and surfaced rather than retried indefinitely.
- [x] **T2.4 - Harden the seams and validate transport responses** (`FR-009`, `NFR-002`, `NFR-003` / `AC-003`, `AC-011`, `AC-012`)
  Write the engine only against the `Storage` and `Transport` traits so queue and conflict logic work unchanged for any caller implementation, and so the crate forces no database or HTTP client on a consumer; `MemoryStorage` and `MockTransport` back deterministic replay tests. Validate and bound every transport response before mutating queue state, returning a typed `SyncError` for a malformed response rather than panicking.

## P3 - Conflict and timestamp

- [x] **T3.1 - Implement conflict strategies and resolution** (`FR-006` / `AC-007`, `AC-008`)
  Implement `conflict.rs` with `ConflictStrategy` (`LastWriteWins`, `ServerWins`, `ClientWins`, and `Custom` accepting a caller merger), `ConflictInfo` carrying both sides, and `Resolution`. A server-reported conflict resolves through the configured strategy for that entity to a typed `Resolution` rather than through logic invented inside a handler.
- [x] **T3.2 - Implement the timestamp tolerance decision** (`FR-007` / `AC-009`)
  Implement `timestamp.rs` so the decision returns a typed `TimestampDecision` that accepts, clamps, or rejects the client time within a configurable tolerance and records both the client and the server time, so a skewed device clock is visible rather than silently trusted.

## P4 - Wiring, docs, verification

- [x] **T4.1 - Wire the crate, document it, and run the verification checks** (`FR-010`, `FR-011`, `NFR-004`, `NFR-005` / `AC-013`)
  Confirm the workspace resolves the new member; write the README covering the queue, idempotency, conflict strategies, timestamp handling, and non-goals; ensure every public item carries rustdoc and every module states scope and non-goals; confirm `forbid(unsafe_code)` and `deny(missing_docs)` are present and the crate depends on neither `wirekit` nor `platform-core`; and keep the crate free of product or vendor vocabulary. Emit queue depth, replay success and failure, conflict count, and sync duration through `tracing`. Then run `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `cargo doc --workspace --all-features --no-deps` with `RUSTDOCFLAGS=-D warnings`, `cargo check -p syncforge --no-default-features`, and `node .sdrive/scripts/validate-governance.js`.

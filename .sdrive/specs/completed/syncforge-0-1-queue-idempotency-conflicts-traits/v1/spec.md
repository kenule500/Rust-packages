---
title: syncforge 0.1 - queue, idempotency, conflicts, traits
status: ongoing
version: v1
created: 2026-11-02
updated: 2026-11-02
---

# syncforge 0.1 - queue, idempotency, conflicts, traits

## Overview

- **Problem Statement**: An offline-capable application must answer four questions correctly the moment connectivity returns: did the action happen, has it already been sent, what if the server saw a newer version, and what time did it actually happen. Most applications answer them with a queue table, a `synced` boolean, and hope, which works until a duplicate invoice, a lost photo, or a job marked complete twice reaches support. Every offline-first app rebuilds the same primitives â€” a durable queue, idempotency keys, retry with backoff, batch upload, conflict handling, and audit timestamps â€” and rebuilds them slightly wrong each time.
- **Proposed Solution**: Port and adapt the `syncforge` crate from `feature/m3-substrate` as prior art: `Action`, `ActionId`, `EntityType`, `Operation`, `Batch`, `ActionResult`, `ActionStatus`, `BatchResult`, `ConflictInfo`, `ConflictStrategy`, `CustomMerger`, `Resolution`, `TimestampDecision`, `SyncError`, and the `Storage` and `Transport` traits, plus a `SyncEngine` and a replay model. Enqueue persists before acknowledging, every action carries a UUID v7 idempotency key, a batch returns a per-action result so a partial failure neither retries all actions nor drops one, and the conflict policy is explicit per entity rather than accidental. Ship `MemoryStorage` and `MockTransport` for deterministic replay tests. No SQLite or HTTP implementation and no connectivity detection in 0.1.
- **Target Audience**: Rust developers building offline-capable field, mobile (Tauri), desktop, or edge applications whose client and server are both Rust, and this project's field application as the first consumer.
- **Assumptions**:
  - A working implementation exists on `feature/m3-substrate` (19 files, feature `default`) and is ported and adapted rather than redesigned.
  - `main` has no `[workspace.dependencies]`, and the branch manifests use `*.workspace = true`, so every dependency is declared directly in the crate manifest.
  - The crate is self-contained: it does not depend on `wirekit` or `platform-core`, matching the catalogue.
  - The contract is at-least-once delivery plus idempotency keys; the caller's handler must be idempotent for side effects the key alone cannot dedupe.
  - The branch tests are not ported; the test agent authors tests against this specification.
  - The unmerged `m3-substrate/v1` spec covers four crates and is not reused as governance here.

## User Stories

### US-001: Enqueue an action durably
As an app developer, I want an action persisted before the UI acknowledges it, so that a crash or force-quit does not lose the write.

**Acceptance Criteria:**
- AC-001: Given an `Action` carrying an entity, an operation, a payload, and a client timestamp, when it is enqueued, then it is persisted to `Storage` before the call reports success.
- AC-002: Given a new action, when it is created, then it receives a UUID v7 `ActionId` and a distinct idempotency key.

### US-002: Replay idempotently on reconnect
As an app developer, I want pending actions replayed without duplicating side effects, so that the same action never runs twice even when a response was lost.

**Acceptance Criteria:**
- AC-003: Given pending actions and a transport, when the engine replays, then each action is sent with its idempotency key and an action reported `Duplicate` is marked sent rather than retried.
- AC-004: Given a transport that fails an action with a retryable status, when replay completes, then that action is marked retry with its attempt count advanced and the others are marked by their own results.

### US-003: Report per-action results for a batch
As an app developer, I want one result per action in a batch, so that a partial batch failure neither retries everything nor drops the failure.

**Acceptance Criteria:**
- AC-005: Given a batch of many actions where some succeed and some fail, when the result is returned, then it carries one `ActionResult` per action with a status of `Applied`, `Duplicate`, `Conflict`, `Rejected`, or `Retry`.
- AC-006: Given a batch, when it is built, then its size is bounded by a configured limit so a week offline does not replay as one unbounded request.

### US-004: Resolve conflicts explicitly
As an app developer, I want a conflict policy chosen per entity rather than invented per handler, so that an offline edit cannot silently overwrite a newer server value by accident.

**Acceptance Criteria:**
- AC-007: Given a server-reported conflict, when the result is inspected, then `ConflictInfo` carries both sides and the configured strategy resolves it to a typed `Resolution`.
- AC-008: Given the conflict strategies, when they are inspected, then `LastWriteWins`, `ServerWins`, `ClientWins`, and `Custom` are available and `Custom` accepts a caller merger.

### US-005: Preserve audit-accurate timestamps
As an app developer, I want the client time the action happened recorded and trusted within a tolerance, so that job completion time is a payroll and billing fact rather than a network artifact.

**Acceptance Criteria:**
- AC-009: Given an action and a server time, when the timestamp decision is made, then it returns a typed `TimestampDecision` that accepts, clamps, or rejects the client time within a configurable tolerance and records both times.

### US-006: Handle poison actions
As an app developer, I want an action that always fails to stop blocking the queue, so that sync does not silently stop forever because of one bad action.

**Acceptance Criteria:**
- AC-010: Given an action that keeps failing, when its attempts reach the configured maximum, then it is marked failed and surfaced instead of being retried indefinitely.

### US-007: Stay storage- and transport-agnostic
As an app developer, I want traits for storage and transport with test implementations, so that I choose my database and protocol, and I can test replay deterministically.

**Acceptance Criteria:**
- AC-011: Given the crate, when `Storage` and `Transport` are implemented by a caller, then the queue and conflict logic work unchanged, and `MemoryStorage` and `MockTransport` are provided for deterministic tests.
- AC-012: Given the crate built with `--no-default-features`, when it compiles, then the core types, error type, and both test implementations are available with no extra feature required.

### US-008: Build on the workspace baseline
As a maintainer, I want lint and MSRV conformance, so that the crate ships like the rest of the workspace.

**Acceptance Criteria:**
- AC-013: Given the crate, when it is built with all features on Rust 1.87, then it compiles with `forbid(unsafe_code)` and `deny(missing_docs)`, ships the licence files, and emits queue depth, replay rate, conflict count, and sync duration through `tracing`.

## Requirements

### Functional Requirements
- FR-001: The crate MUST expose `Action`, `ActionId`, `EntityType`, `Operation`, `Batch`, `ActionResult`, `ActionStatus`, `BatchResult`, `ConflictInfo`, `ConflictStrategy`, `CustomMerger`, `Resolution`, `TimestampDecision`, `SyncError`, and `Storage` and `Transport` traits per `catalogue/02_syncforge.md` and the shared module list.
- FR-002: Enqueue MUST persist to `Storage` before the call reports success.
- FR-003: Every action MUST receive a UUID v7 idempotency key at creation, and a `Duplicate` result MUST mark the action sent rather than retried.
- FR-004: A batch MUST return one `ActionResult` per action with a status of `Applied`, `Duplicate`, `Conflict`, `Rejected`, or `Retry`.
- FR-005: A batch MUST be bounded by a configured size.
- FR-006: `ConflictStrategy` MUST provide `LastWriteWins`, `ServerWins`, `ClientWins`, and `Custom`, and a conflict MUST resolve to a typed `Resolution` carrying `ConflictInfo`.
- FR-007: The crate MUST return a typed `TimestampDecision` that applies a configurable tolerance to the client time and records both the client and server times.
- FR-008: An action that exhausts its maximum attempts MUST be marked failed and surfaced rather than retried indefinitely.
- FR-009: `Storage` and `Transport` MUST be traits, and `MemoryStorage` and `MockTransport` MUST be provided for deterministic tests.
- FR-010: The crate MUST emit queue depth, replay success and failure, conflict count, and sync duration through `tracing`.
- FR-011: The crate MUST declare `forbid(unsafe_code)` and `deny(missing_docs)` and MUST NOT depend on `wirekit` or `platform-core`.

### Non-Functional Requirements
- NFR-001: The crate MUST NOT read or write credentials and MUST NOT read the environment; storage and transport are caller-supplied (CON-519).
- NFR-002: Untrusted transport responses MUST be validated before mutating queue state, and a malformed response MUST produce a typed `SyncError` rather than a panic (CON-515).
- NFR-003: The crate MUST be storage- and transport-agnostic and MUST NOT force a database or an HTTP client on a consumer (CON-107 in spirit: stateful concerns stay in their own crate).
- NFR-004: The crate MUST compile on Rust 1.87 with all features and MUST contain no product or vendor workflow vocabulary (CON-102, CON-106).
- NFR-005: Every public item MUST carry rustdoc and every module MUST state its scope and non-goals (CON-202, CON-203).

## UI/UX & Data Requirements

- **UI/UX**: Not applicable. This repository ships no UI layer (CON-301); a consuming application renders queue depth and pending changes.
- **Data**: `Action` carries the id, entity, operation, payload, client timestamp, idempotency key, and attempt count. `BatchResult` carries one `ActionResult` per action with its status, server timestamp, and any conflict. Storage holds the queue and its terminal states; the trait, not the crate, decides where.

## Edge Cases & Error States

- EC-001: The process dies after enqueue and before replay â†’ the action is still in `Storage` and replays on the next launch.
- EC-002: The server applied an action but the response was lost â†’ the retry carries the same idempotency key and the server reports `Duplicate`, which is not an error and is not retried.
- EC-003: A batch of many actions where one is rejected â†’ per-action results leave the rejected action marked `Rejected` and every other action at its own status.
- EC-004: One action always fails validation â†’ after the attempt cap it is marked failed and surfaced; the queue continues instead of blocking.
- EC-005: The device clock is wrong by more than the tolerance â†’ `TimestampDecision` rejects or clamps the client time and records both times rather than silently trusting it.
- EC-006: Build with `--no-default-features` â†’ core types, `SyncError`, `MemoryStorage`, and `MockTransport` compile.

## Out of Scope

- A SQLite storage implementation in 0.1; `Storage` is a trait, and SQLite is a later adapter.
- An HTTP transport implementation in 0.1; `Transport` is a trait, and HTTP is a later adapter.
- Connectivity detection and WASM packaging.
- Automatic application of a merged value on the server; the resolution is returned to the caller.
- A CRDT and document merging.
- Authentication, authorization, and permissions; the server authorizes every action.
- A schema migration tool; action versions are the caller's responsibility.
- Exactly-once semantics.

## Dependencies & Risks

- `async-trait`, `chrono`, `serde`, `serde_json`, `uuid`, `thiserror`, and `tracing` for the core; `tokio` dev-only. No dependency on `wirekit` or `platform-core`.
- **RISK-001**: Queue and conflict semantics are the highest design risk in M3, and a rushed model is expensive to change once a client depends on it. The catalogue's failure-mode list is mirrored in the edge cases and each is pinned by an AC; the test agent covers replay determinism with `MockTransport`.
- **RISK-002**: The pre-1.0 API will change before 1.0, and both client and server compile against the shared types, so a change breaks both sides at once. Document 0.x instability as the workspace already does for every crate, and keep the public surface to the catalogue list.
- **RISK-003**: Porting stale code may drag in manifests using workspace inheritance the current workspace does not define. Adapt the manifest to declare dependencies directly, as `platform-core` did.

# Tasks - m3-substrate v1

> **Feature:** `m3-substrate`
> **Version:** v1
> **Branch:** `feature/m3-substrate`
> **Spec:** `.sdrive/specs/backlog/m3-substrate/v1/spec.md`
> **Plan:** `.sdrive/specs/backlog/m3-substrate/v1/plan.md`

## Test Boundary

This task list contains **no test-writing or test-running tasks**. Test authoring and test execution belong to `/sdrive:test`. Acceptance criteria are linked below so the Test agent can map tests to AC IDs.

Verification tasks are limited to formatter, linter, documentation build, packaging, governance validation, and `sdrive verify`.

## P1 - Workspace scaffold

Depends on: none

- [x] **T1.1 - Create the four crate packages with inherited metadata and lints**
  - Description: Add `crates/platform-core`, `crates/quota-ledger`, `crates/hookgate`, and `crates/syncforge` with `version = "0.1.0"`, `edition.workspace = true`, `rust-version.workspace = true`, `license.workspace = true`, description, readme, keywords, categories, and docs.rs all-features metadata. Each crate root declares `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`. Keep `members = ["crates/*"]` unchanged.
  - Requirements: FR-028, FR-029, NFR-006, NFR-007
  - Acceptance criteria: AC-046, AC-047, AC-048, AC-053

- [x] **T1.2 - Add the workspace dependency table and feature conventions**
  - Description: Add `[workspace.dependencies]` for the shared crates and per-crate additive features. Every crate keeps `default = []` where a database or native dependency is optional. Document the default feature set in each README.
  - Requirements: FR-028, NFR-008
  - Acceptance criteria: AC-046, AC-048, AC-050

- [x] **T1.3 - Update the root README for the new crates**
  - Description: Add the four crates to the package table with one-line responsibilities and mark their status as in development. Do not claim publication before it happens.
  - Requirements: FR-029
  - Acceptance criteria: AC-053

## P2 - platform-core

Depends on: P1

- [x] **T2.1 - Define identity and capability types**
  - Description: Add `TenantId(Uuid)`, `ProviderId(&'static str)`, `ProvisioningMode` (`Api`, `Partner`, `Manual`, `Unsupported`), `Capability { key, mode }`, and the `Provider` trait returning an id and a static capability slice. Keep provider and capability vocabularies opaque.
  - Requirements: FR-001
  - Acceptance criteria: AC-003

- [x] **T2.2 - Implement the explicit registry**
  - Description: Add `Registry` with explicit `register`, `get`, `iter`, and `capabilities`. Reject a duplicate provider id with a typed error, preserve registration order, and keep all state instance-scoped. No statics and no implicit registration.
  - Requirements: FR-002, FR-003, NFR-012
  - Acceptance criteria: AC-002, AC-004, AC-005, AC-006

- [x] **T2.3 - Implement Secret, TokenStore, and MemoryTokenStore**
  - Description: Add a `Secret` wrapper over `secrecy::SecretString` with redacted `Debug`/`Display`, no `Serialize`, and explicit exposure. Add an `#[async_trait]` `TokenStore` with `get`, `put`, and `revoke`, plus `MemoryTokenStore` keyed by tenant and provider. Map backend failures to `TokenError`.
  - Requirements: FR-004, FR-005, NFR-001, NFR-002
  - Acceptance criteria: AC-001, AC-007, AC-008, AC-009

- [x] **T2.4 - Define IntegrationError with stable codes**
  - Description: Add `Auth`, `Scope`, `RateLimit`, `Quota`, `Provider`, `Transport`, `Unsupported`, `DuplicateProvider`, and `Config` variants. Each exposes a stable code; `Display` includes the code and `std::error::Error` is implemented. Messages never echo secret values.
  - Requirements: FR-006
  - Acceptance criteria: AC-051

- [x] **T2.5 - Document platform-core**
  - Description: Write the crate-level docs, module comments with scope and non-goals, feature table, a registry usage example, and the README.
  - Requirements: FR-029
  - Acceptance criteria: AC-047, AC-053

## P3 - quota-ledger core

Depends on: P2

- [ ] **T3.1 - Define the quota type model**
  - Description: Add `Metric`, `Window`, `Budget`, `QuotaKey`, `Reservation`, `ReservationId`, `Usage`, `ExhaustionPolicy`, and `ReserveOutcome`. Reuse `TenantId` and `ProviderId` from `platform-core` through a path dependency.
  - Requirements: FR-007
  - Acceptance criteria: AC-010, AC-014

- [ ] **T3.2 - Implement the in-memory quota store**
  - Description: Add an `#[async_trait]` `QuotaStore` with `reserve`, `commit`, `release`, and `usage`. Perform capacity check, hold insertion, and idempotency lookup under one write lock. `commit` consumes exactly once, `release` consumes nothing, and repeated settlement is a typed no-op. A repeated key with a different payload is an idempotency conflict.
  - Requirements: FR-008, FR-009, FR-010, NFR-001, NFR-005, NFR-012
  - Acceptance criteria: AC-010, AC-011, AC-012, AC-013

- [ ] **T3.3 - Implement window reset computation**
  - Description: Compute fixed daily and monthly reset instants from the configured timezone and rolling windows from the current instant. The reset boundary never uses the host timezone. Expose the next reset instant in `Usage`.
  - Requirements: FR-012
  - Acceptance criteria: AC-011

- [ ] **T3.4 - Implement exhaustion policies**
  - Description: Produce the declared outcome for `HardStop` (denial with reset time), `Overage` (reservation plus overage units and cost in cents), `Degrade` (degraded outcome with reset time), and `Queue` (queued outcome with reset time). Degrade and Queue consume no units.
  - Requirements: FR-011
  - Acceptance criteria: AC-014, AC-015, AC-016, AC-017

- [ ] **T3.5 - Document quota-ledger**
  - Description: Write crate docs, module scope and non-goals, the reserve/commit/release flow, feature table, and the README.
  - Requirements: FR-029
  - Acceptance criteria: AC-053

## P4 - quota-ledger Postgres

Depends on: P3

- [ ] **T4.1 - Add the postgres feature and schema**
  - Description: Add an optional `postgres` feature with `sqlx` (`postgres`, `runtime-tokio-rustls`, `chrono`, `uuid`, `migrate`). Create `schema.sql` with budget, reservation, and dead-hold tables, a unique index on tenant/provider/metric/idempotency key, and indexes for usage and reclaim. Default features must not pull `sqlx`.
  - Requirements: FR-013, NFR-008
  - Acceptance criteria: AC-018

- [ ] **T4.2 - Implement atomic reservation in Postgres**
  - Description: Implement `reserve` as one parameterized conditional `UPDATE ... WHERE used + $units <= $limit RETURNING used`, with a unique idempotency index and a follow-up insert of the hold in the same transaction. No string-built SQL and no caller-supplied identifiers.
  - Requirements: FR-013, NFR-004, NFR-005
  - Acceptance criteria: AC-019

- [ ] **T4.3 - Implement settlement, usage, and TTL reclaim**
  - Description: Implement `commit`, `release`, and `usage` with tenant-scoped predicates. Reclaim expired reservations exactly once and return the held units. Late settlement is a typed no-op.
  - Requirements: FR-010, FR-013, NFR-001, NFR-004
  - Acceptance criteria: AC-020, AC-021

- [ ] **T4.4 - Require TLS and redact connection configuration**
  - Description: Require TLS by default for Postgres connections. Add an explicit, documented test-only insecure mode. Never print the connection string in `Debug`, `Display`, or errors.
  - Requirements: NFR-002, NFR-009
  - Acceptance criteria: AC-049

- [ ] **T4.5 - Document the Postgres backend**
  - Description: Document schema setup, feature flags, TLS expectations, reservation TTL, and the reclaim query in the crate README and rustdoc.
  - Requirements: FR-029
  - Acceptance criteria: AC-053

## P5 - hookgate

Depends on: P2

- [ ] **T5.1 - Define hookgate types, traits, and errors**
  - Description: Add `DeliveryId`, `IdempotencyKey`, `InboundRequest` (tenant, provider, headers, raw `Bytes`, received_at), `VerifiedEvent`, `Verifier`, `DeliveryStore`, `Outbox`, `InsertOutcome`, `DeliveryRecord`, and typed `VerifyError`, `StoreError`, `OutboxError`. The raw body is never re-serialized before verification.
  - Requirements: FR-014, NFR-003
  - Acceptance criteria: AC-022

- [ ] **T5.2 - Implement the HMAC verifier presets**
  - Description: Add Meta Graph, Stripe, Shopify, Twilio, Slack, GitHub, and a configurable generic HMAC-SHA256 preset. Share one parameterized implementation for header name, encoding (hex/base64), prefix, signed payload shape, and digest. Compare signatures with `subtle::ConstantTimeEq`.
  - Requirements: FR-015, FR-016
  - Acceptance criteria: AC-022, AC-023, AC-025

- [ ] **T5.3 - Enforce the replay window and body limit**
  - Description: Add `ReplayWindow` with a configurable timestamp tolerance and maximum body size. Reject stale, future-beyond-tolerance, oversized, and missing-header requests with distinct typed errors before parsing the body.
  - Requirements: FR-015, NFR-003
  - Acceptance criteria: AC-023, AC-024

- [ ] **T5.4 - Implement dedupe and the transactional outbox**
  - Description: Add `MemoryDeliveryStore` with dedupe on tenant, provider, and provider event id. Insert the delivery and enqueue the outbox row under one lock so either both become visible or neither does. Report `Duplicate` without a second enqueue. Keep tenant isolation in every key.
  - Requirements: FR-017, FR-018, NFR-001
  - Acceptance criteria: AC-026, AC-027, AC-028, AC-029

- [ ] **T5.5 - Implement dead-letter and replay**
  - Description: Track attempts and write a `DeadLetterRecord` with the raw body and redacted error text after the configured maximum. Replay re-enqueues the same delivery id and reports `Duplicate` when it had already succeeded.
  - Requirements: FR-019
  - Acceptance criteria: AC-030, AC-031

- [ ] **T5.6 - Document hookgate**
  - Description: Write crate docs, preset table, the receive/verify/dedupe/enqueue flow, the at-least-once contract, feature table, and the README.
  - Requirements: FR-029
  - Acceptance criteria: AC-053

## P6 - syncforge

Depends on: P1

- [ ] **T6.1 - Define action and batch types with UUID v7 identifiers**
  - Description: Add `ActionId`, `Action`, `EntityType`, `Operation`, `Batch`, `BatchResult`, `ActionResult`, `ActionStatus`, and `ConflictInfo`. Generate action ids with `Uuid::now_v7()` and keep the timestamp component non-decreasing within the process.
  - Requirements: FR-020, FR-021
  - Acceptance criteria: AC-032, AC-035, AC-036

- [ ] **T6.2 - Implement the Storage trait and MemoryStorage**
  - Description: Add an `#[async_trait]` `Storage` with `append`, `pending`, `mark_sent`, `mark_failed`, and `record_attempt`. `MemoryStorage` preserves enqueue order deterministically and returns typed errors on failure. Storage is injected explicitly.
  - Requirements: FR-022, NFR-010, NFR-012
  - Acceptance criteria: AC-032, AC-033, AC-034

- [ ] **T6.3 - Implement the Transport trait and MockTransport**
  - Description: Add an `#[async_trait]` `Transport` that sends a batch and returns per-action results. `MockTransport` scripts whole-batch and per-action outcomes for deterministic replay.
  - Requirements: FR-023, NFR-010
  - Acceptance criteria: AC-036, AC-040

- [ ] **T6.4 - Implement the replay engine**
  - Description: Drain pending actions in order and in bounded batches. Return one result per action, mark `Applied` and `Duplicate` as sent, retry `Retry` with capped exponential backoff, and dead-letter after `max_attempts` without blocking later actions. An empty queue returns an empty result without a transport call.
  - Requirements: FR-024, NFR-003, NFR-010
  - Acceptance criteria: AC-036, AC-037, AC-039, AC-040

- [ ] **T6.5 - Implement conflict strategies**
  - Description: Add `LastWriteWins`, `ServerWins`, `ClientWins`, and `Custom(Arc<dyn CustomMerger>)`, selectable per entity or action. Record the resolution in `ConflictInfo`. A conflict without server state is a typed error.
  - Requirements: FR-025
  - Acceptance criteria: AC-038, AC-041, AC-042, AC-043, AC-044

- [ ] **T6.6 - Implement audit timestamps and tolerance**
  - Description: Preserve `client_timestamp`, record `server_timestamp`, and apply a configurable tolerance without rewriting client time. Expose the tolerance decision in the result.
  - Requirements: FR-026
  - Acceptance criteria: AC-020, AC-041

- [ ] **T6.7 - Emit tracing metrics**
  - Description: Emit events for enqueue, replay start/finish, per-action outcome, conflict resolution, queue depth, and duration.
  - Requirements: FR-027
  - Acceptance criteria: AC-052

- [ ] **T6.8 - Document syncforge**
  - Description: Write crate docs, the replay model, the at-least-once contract, the conflict-strategy table, feature table, and the README.
  - Requirements: FR-029
  - Acceptance criteria: AC-053

## P7 - Integration example and verification

Depends on: P2, P3, P5

- [ ] **T7.1 - Add the substrate integration example**
  - Description: Add `crates/hookgate/examples/substrate_integration.rs` with dev-dependencies on `platform-core` and `quota-ledger`. Register one provider, reserve and commit quota, verify a generic-HMAC webhook, insert and enqueue it, and print the outbox result.
  - Requirements: FR-028
  - Acceptance criteria: AC-045

- [ ] **T7.2 - Finalize the root and crate documentation**
  - Description: Update the root README package table and status, cross-link the crate READMEs, and confirm every crate documents its default feature set and non-goals.
  - Requirements: FR-029
  - Acceptance criteria: AC-053

- [ ] **T7.3 - Run production verification**
  - Description: Run `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `cargo build --workspace --all-features`, `cargo build --workspace --no-default-features`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`, `cargo package --workspace --list`, `node .sdrive/scripts/validate-governance.js`, and `sdrive verify m3-substrate`. This task does not write or run tests.
  - Requirements: NFR-006, NFR-007
  - Acceptance criteria: AC-046, AC-047, AC-048, AC-050

## Traceability Summary

| Phase | Requirements | Tasks |
|---|---|---|
| P1 | FR-028, FR-029, NFR-006, NFR-007, NFR-008 | T1.1-T1.3 |
| P2 | FR-001 to FR-006, NFR-001, NFR-002, NFR-012 | T2.1-T2.5 |
| P3 | FR-007 to FR-012, NFR-001, NFR-005, NFR-012 | T3.1-T3.5 |
| P4 | FR-010, FR-013, NFR-001, NFR-002, NFR-004, NFR-005, NFR-008, NFR-009 | T4.1-T4.5 |
| P5 | FR-014 to FR-019, NFR-001, NFR-003 | T5.1-T5.6 |
| P6 | FR-020 to FR-027, NFR-003, NFR-010, NFR-012 | T6.1-T6.8 |
| P7 | FR-028, FR-029, NFR-006, NFR-007 | T7.1-T7.3 |

NFR-011 (every AC covered by a test) has no implementation task by design; it is fulfilled by `/sdrive:test`.

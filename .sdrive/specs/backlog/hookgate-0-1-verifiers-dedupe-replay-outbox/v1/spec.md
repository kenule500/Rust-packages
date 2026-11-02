---
title: hookgate 0.1 - verifiers, dedupe, replay, outbox
status: backlog
version: v1
created: 2026-11-02
updated: 2026-11-02
---

# hookgate 0.1 - verifiers, dedupe, replay, outbox

## Overview

- **Problem Statement**: Inbound provider webhooks arrive at an HTTP edge that must survive a restart, and the four guarantees that matter — the signature is genuine, the event is not a replay, the same delivery is not processed twice, and a verified event is not lost between verification and handling — all require durable state that a per-provider client cannot own. `wirekit` verifies signatures for the providers it wraps, but it holds no delivery record, no replay window, and no outbox, so every edge service re-implements dedupe and retry slightly differently and loses an event the first time it crashes mid-delivery.
- **Proposed Solution**: Port and adapt the `hookgate` crate from `feature/m3-substrate` as prior art: `DeliveryId`, `IdempotencyKey`, `InboundRequest`, `VerifiedEvent`, `DeliveryRecord`, `InsertOutcome`, `DeadLetterRecord`, and the `Verifier`, `DeliveryStore`, and `Outbox` traits, plus a redacting error type and provider verifier presets. Establish the load-bearing rule that verification runs over the exact received bytes and never over a re-serialization. Ship `MemoryDeliveryStore` for tests and single-process callers, and verifier presets for Meta, Stripe, Shopify, Twilio, Slack, GitHub, and a configurable generic HMAC. No axum integration, no SendGrid ECDSA verifier, and no standalone binary in 0.1.
- **Target Audience**: Rust teams running a webhook ingress edge, and this project's service layer that now receives every provider's events.
- **Assumptions**:
  - A working implementation exists on `feature/m3-substrate` (20 files, features `default` and `presets`) and is ported and adapted rather than redesigned.
  - `main` has no `[workspace.dependencies]`, and the branch manifests use `*.workspace = true`, so every dependency is declared directly in the crate manifest.
  - The contract is at-least-once delivery with idempotent handlers, not exactly-once; this is documented rather than implied.
  - The branch tests are not ported; the test agent authors tests against this specification.
  - The unmerged `m3-substrate/v1` spec covers four crates and is not reused as governance here.

## User Stories

### US-001: Verify a signature over the raw bytes
As an edge service author, I want to verify a provider signature against the exact received body, so that a tampered or re-serialized payload is rejected rather than trusted.

**Acceptance Criteria:**
- AC-001: Given an `InboundRequest` carrying raw body bytes, when a `Verifier::verify` runs, then it computes the signature over those exact bytes and never over a re-serialization of parsed parameters.
- AC-002: Given a valid signature, when `verify` runs, then it returns a `VerifiedEvent` carrying the provider, delivery id, event id if present, event type, parsed payload, and the raw bytes.

### US-002: Reject tampered and stale requests
As a security reviewer, I want tampered or replayed requests refused, so that an attacker cannot forge or replay a webhook.

**Acceptance Criteria:**
- AC-003: Given a body whose bytes were changed after signing, when `verify` runs, then it fails with a typed verification error and no event is produced.
- AC-004: Given a valid signature whose timestamp is outside the allowed skew, when `verify` runs, then it is rejected as stale rather than accepted.

### US-003: Deduplicate deliveries
As an edge service author, I want the same event processed once, so that a provider retry does not double-apply an event.

**Acceptance Criteria:**
- AC-005: Given a delivery already stored for a provider and event id, when it is inserted again, then the outcome is `Duplicate` rather than a second insert.
- AC-006: Given the same event id arriving for two different tenants, when both are stored, then they are not duplicates of each other because the uniqueness key includes the tenant.

### US-004: Hand off through an outbox
As an edge service author, I want a verified event persisted and enqueued before acknowledging, so that a crash between verification and handling does not lose the event.

**Acceptance Criteria:**
- AC-007: Given a new verified event, when it is accepted, then the delivery record and the outbox entry are written before success is reported, so a crash afterwards leaves the event recoverable.
- AC-008: Given a stored delivery, when it is marked processed or failed, then the terminal state is recorded and a failure can be dead-lettered with its error and raw body.

### US-005: Replay a dead-lettered delivery
As an operator, I want to inspect and replay a failed delivery, so that a poison message is recoverable instead of lost.

**Acceptance Criteria:**
- AC-009: Given a dead-lettered record, when it is read, then the raw body and the failure reason are available, and replaying it is idempotent against the delivery store.

### US-006: Ship provider verifier presets
As an edge service author, I want correct verifiers for the common providers, so that I do not re-implement each provider's signature quirk.

**Acceptance Criteria:**
- AC-010: Given the `presets` feature, when it is enabled, then verifiers are provided for Meta, Stripe, Shopify, Twilio, Slack, and GitHub, and a configurable generic HMAC verifier covers an unlisted provider.
- AC-011: Given the crate built without the `presets` feature, when it compiles, then the core types, traits, and memory store are present and the preset verifiers are absent.

### US-007: Isolate tenants and never leak secrets
As a security reviewer, I want tenant scoping and redaction enforced, so that one tenant's delivery cannot be read by another and no secret reaches a log.

**Acceptance Criteria:**
- AC-012: Given a stored delivery, when it is read, then the tenant scoping prevents another tenant from reading it.
- AC-013: Given a verification or store failure, when it is formatted and logged, then the error is redacted and carries no secret material such as a signature or a shared secret.
- AC-014: Given the crate, when it is built with all features on Rust 1.87, then it compiles with `forbid(unsafe_code)` and `deny(missing_docs)` and ships the licence files.

## Requirements

### Functional Requirements
- FR-001: The crate MUST expose `DeliveryId`, `IdempotencyKey`, `InboundRequest`, `VerifiedEvent`, `DeliveryRecord`, `InsertOutcome`, `DeadLetterRecord`, and the `Verifier`, `DeliveryStore`, and `Outbox` traits per `catalogue/30_webhook_gateway.md`.
- FR-002: Verification MUST operate on the raw received bytes and MUST NOT re-serialize the body before verifying (CON-215).
- FR-003: Verification MUST be constant-time and MUST fail closed on a mismatch.
- FR-004: A timestamp tolerance MUST be enforced and a request outside it MUST be rejected as stale.
- FR-005: `DeliveryStore` MUST provide `insert_if_new` returning an outcome, `mark_processed`, and `mark_failed`, and `insert_if_new` MUST be idempotent per tenant and event id.
- FR-006: The delivery and outbox writes for a new event MUST complete before success is reported.
- FR-007: A failed delivery MUST be recordable as dead-lettered with its error and raw body, and MUST be replayable idempotently.
- FR-008: A `presets` feature MUST ship verifiers for Meta, Stripe, Shopify, Twilio, Slack, and GitHub plus a configurable generic HMAC verifier, and MUST be absent when the feature is off.
- FR-009: The crate MUST NOT contain a provider client, an outbound HTTP call, an HTTP server, or an axum integration.
- FR-010: Errors MUST be redacted and MUST NOT expose a signature, a secret, or raw credential material.
- FR-011: The crate MUST declare `forbid(unsafe_code)` and `deny(missing_docs)`.

### Non-Functional Requirements
- NFR-001: Every delivery read and write MUST be scoped by tenant, so no tenant can observe another's delivery (CON-512).
- NFR-002: Stored secrets and signature material MUST be redacted in `Debug`, `Display`, logs, and error payloads (CON-214).
- NFR-003: Untrusted input — headers, raw body, and event ids — MUST be validated and bounded before use, and an oversized or malformed request MUST produce a typed error rather than a panic (CON-515).
- NFR-004: The crate MUST NOT read secrets from the environment; the caller supplies the verifier's secret (CON-519).
- NFR-005: The crate MUST compile on Rust 1.87 with all features and MUST contain no product vocabulary (CON-102, CON-106).
- NFR-006: Every public item MUST carry rustdoc and every module MUST state its scope and non-goals (CON-202, CON-203).

## UI/UX & Data Requirements

- **UI/UX**: Not applicable. This repository ships no UI layer (CON-301); the consuming application renders delivery and dead-letter state.
- **Data**: `DeliveryRecord` carries the delivery id, tenant, provider, event id, event type, raw body, received-at, and terminal state. `DeadLetterRecord` carries the same raw body plus the failure reason. The memory store is the reference implementation; a durable store is the caller's.

## Edge Cases & Error States

- EC-001: A JSON body is parsed and re-serialized before verification → verification fails, because the canonical string no longer matches the received bytes.
- EC-002: The same event id arrives from two tenants → both are stored; they are not duplicates because the uniqueness key includes the tenant.
- EC-003: Crash after insert but before outbox enqueue → the contract requires both to commit before success; a partial write must be detectable and replayable.
- EC-004: An unlisted provider with a custom HMAC header and prefix → the generic verifier accepts the caller's header name, encoding, and prefix.
- EC-005: A poison delivery that always fails handling → it is dead-lettered with its raw body rather than blocking the outbox forever.
- EC-006: Build with `--no-default-features` → core compiles; no preset verifier is present.

## Out of Scope

- A provider client and any outbound HTTP.
- An HTTP server or axum integration in 0.1.
- A SendGrid ECDSA verifier in 0.1.
- A standalone ingress binary in 0.1.
- A queue implementation; the `Outbox` trait writes to the caller's choice of queue.
- Domain handling of any event type; `hookgate` does not know what an event means.
- Tenant resolution and authentication; the caller supplies the tenant.
- Outbound webhooks (signing, retries, customer endpoints).

## Dependencies & Risks

- `platform-core` for tenant identity; `bytes`, `http`, `hmac`, `sha1`, `sha2`, `subtle`, `base64`, `hex`, `chrono`, `serde_json`, `uuid`, `thiserror`, and `tracing` for the core and presets; `tokio` dev-only.
- **RISK-001**: The raw-body rule is easy to violate accidentally by a well-meaning refactor that parses first, which silently breaks every signature. AC-001 and AC-003 assert it directly, and EC-001 names the failure so a test can pin it.
- **RISK-002**: The delivery-and-outbox-before-acknowledge guarantee is weak when the two writes are not transactional in the memory store. State the guarantee explicitly for the store contract and pin it with AC-007 and EC-003.
- **RISK-003**: Porting stale code may drag in manifests using workspace inheritance the current workspace does not define, and the branch dev-depends on `quota-ledger`. Adapt the manifest to declare dependencies directly and drop the cross-crate dev-dependency unless the test needs it.

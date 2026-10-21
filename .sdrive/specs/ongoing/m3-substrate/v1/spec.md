---
title: M3 Integration Substrate - platform-core, quota-ledger, hookgate, and syncforge 0.1
status: backlog
version: v1
created: 2026-10-20
updated: 2026-10-20
---

# M3 Integration Substrate - platform-core, quota-ledger, hookgate, and syncforge 0.1

## Overview

- **Problem Statement**: The workspace ships only `dado`. A product that integrates many providers must still invent tenant and provider identity, an explicit provider registry, cross-process quota state, webhook ingress state, and an offline action queue. Each consumer rebuilds those primitives slightly differently, and the failure modes are expensive: a token read across tenants, two pods spending the same last quota unit, a replayed webhook processed twice, or an offline job marked complete twice. The catalogue specifies four crates for this work, but none exists in code.
- **Proposed Solution**: Add four library crates to the existing Cargo workspace, each with one job: `platform-core` (traits, registry, token-store contract), `quota-ledger` (reserve/commit/release ledger with an optional Postgres backend), `hookgate` (raw-byte webhook verification, dedupe, replay window, outbox and dead-letter), and `syncforge` (durable action queue, UUID v7 idempotency, conflict strategies, storage and transport traits). Ship each at 0.1.0 with feature-gated dependencies, rustdoc, and a README, plus one integration example that composes `platform-core` + `quota-ledger` + `hookgate`.
- **Target Audience**: External Rust developers building multi-tenant products that integrate third-party providers, including server-side services, desktop tools, and Tauri or edge clients that need offline replay. The first internal consumer is a field-service application, but no product nouns enter these crates.
- **Assumptions**:
  - The scope was confirmed by the user on 2026-10-20: BP-017, BP-018, BP-020, and BP-021 in one feature; BP-019 (Redis) deferred.
  - `platform-core`, `quota-ledger`, `hookgate`, and `syncforge` are not yet implemented; only `crates/dado` exists. The specification is based on the catalogue and user decisions, not on existing crate code.
  - The feature is based on `main` (which does not yet contain `wirekit`), so no crate in this feature may depend on `wirekit`. The `IntegrationError` mapping from `wirekit` variants is deferred until `wirekit` is on the base branch.
  - Rust 1.85 and edition 2021 remain the MSRV and edition, inherited from `[workspace.package]`.
  - Async traits use `async-trait` so that `TokenStore`, `QuotaStore`, `DeliveryStore`, `Outbox`, `Storage`, and `Transport` remain usable as trait objects.
  - Crate names must be re-checked on crates.io immediately before publication (CON-111). Publication itself is a manual post-merge step.
  - `no_std` for the `quota-ledger` core is deferred; 0.1 targets `std`.
  - Test authoring and test execution belong to `/sdrive:test`, not to this feature's implementation tasks.
- **Deferred from this version**: Redis quota backend (BP-019), `platform-core` `sqlx` and `axum` features, `syncforge` SQLite and HTTP implementations, `hookgate` SendGrid ECDSA preset and `axum` integration, `hookgate` standalone binary, `wirekit` error mapping, `no_std` core, dashboards and provider-specific quota presets.

## User Stories

### US-001: Model tenant and provider identity

As an application developer, I want typed tenant and provider identifiers with a declared provisioning mode, so that identity is explicit and no crate infers support from a dependency.

**Acceptance Criteria:**
- AC-001: Given tokens stored for two tenants, when one tenant reads a provider token, then it never receives the other tenant's token.
- AC-002: Given a provider that is not registered, when the registry is queried by id, then it returns `None` and does not panic.
- AC-003: Given a provider registered with a capability slice, when capabilities are queried, then the registry returns exactly the declared keys and provisioning modes.

### US-002: Build an explicit provider registry

As an application developer, I want to register provider families explicitly at startup, so that there is no global mutable singleton and no accidental duplication.

**Acceptance Criteria:**
- AC-004: Given a registry that already contains a provider, when the same provider id is registered again, then the second registration returns a typed duplicate error and the original entry is unchanged.
- AC-005: Given providers registered in a known order, when the registry is iterated, then providers are yielded in registration order deterministically.
- AC-006: Given two registries built in the same process, when one registers a provider, then the other registry is unaffected and no global mutable state is shared.

### US-003: Store and revoke credentials behind a contract

As an application developer, I want a storage-agnostic token-store trait plus a memory implementation, so that the adapter can supply KMS-backed storage without the crate owning encryption.

**Acceptance Criteria:**
- AC-007: Given the memory token store, when a token is put, read, revoked, and read again, then the first read returns the token and the second returns `None`.
- AC-008: Given a token stored in the memory backend, when `Debug` or `Display` is printed for the store or the token, then the token value is redacted.
- AC-009: Given a backend that returns an error, when `get`, `put`, or `revoke` runs, then the error is surfaced as a typed `TokenError` and the call does not panic.

### US-004: Reserve and settle shared provider quota

As a service developer, I want to reserve quota before an outbound call and commit or release afterwards, so that two processes cannot spend the same last unit.

**Acceptance Criteria:**
- AC-010: Given a budget with limit N, when N reservations are requested, then each request returns a distinct reservation id and usage equals N.
- AC-011: Given a budget at its limit under `HardStop`, when another reservation is requested, then it is denied with the reset time and usage is unchanged.
- AC-012: Given a reservation, when it is committed then usage is consumed exactly once; when it is released then usage is not consumed.
- AC-013: Given a repeated caller idempotency key, when reserve is called twice with the same key and payload, then the same reservation is returned and usage is counted once.

### US-005: Enforce an explicit exhaustion policy

As a service developer, I want each exhausted budget to produce the policy's declared outcome, so that hard stops, overage, degradation, and queueing are never accidental.

**Acceptance Criteria:**
- AC-014: Given `HardStop`, when the budget is exhausted, then reserve returns `QuotaExceeded` with the reset time and an upgrade hint.
- AC-015: Given `Overage { unit_cost_cents }`, when the budget is exhausted, then reserve succeeds, records overage units, and reports the accumulated cost in cents.
- AC-016: Given `Degrade`, when the budget is exhausted, then reserve returns a `Degraded` outcome naming the reset time and consumes no units.
- AC-017: Given `Queue`, when the budget is exhausted, then reserve returns a `Queued` outcome with the reset time and consumes no units.

### US-006: Persist quota in Postgres

As a service developer, I want an optional Postgres backend with atomic reservations, so that quota survives restarts and is shared across pods.

**Acceptance Criteria:**
- AC-018: Given the `postgres` feature disabled, when the crate builds, then no `sqlx` or Postgres dependency is compiled.
- AC-019: Given two concurrent tasks reserving the last unit against Postgres, when both run, then exactly one succeeds and usage never exceeds the limit.
- AC-020: Given a reservation that was never committed or released, when its TTL expires and reclaim runs, then the held units return to the budget exactly once.
- AC-021: Given rows belonging to two tenants, when usage is queried for one tenant, then only that tenant's usage is returned.

### US-007: Verify inbound webhooks on raw bytes

As a webhook-edge developer, I want signature verification over the raw request bytes with a replay window, so that tampered or stale deliveries are rejected before parsing.

**Acceptance Criteria:**
- AC-022: Given a request whose signature is valid over its raw bytes, when the provider verifier runs, then the event is accepted.
- AC-023: Given a body that is JSON-parsed and re-serialized after signing, when verification runs, then it fails.
- AC-024: Given a timestamp outside the configured tolerance, when verification runs, then the request is rejected as stale.
- AC-025: Given each shipped HMAC preset, when the provider's documented signature is computed for a fixture, then the valid signature passes and a one-byte tampered body fails.

### US-008: Deduplicate deliveries per tenant

As a webhook-edge developer, I want durable deduplication, so that a retried delivery is reported as a duplicate instead of being processed twice.

**Acceptance Criteria:**
- AC-026: Given the same provider event id delivered twice for one tenant, when the second insert runs, then it returns `Duplicate` and no second outbox row is written.
- AC-027: Given the same provider event id from two different tenants, when both arrive, then both are accepted.
- AC-028: Given a delivery already marked processed, when the same event arrives again, then it is reported as `Duplicate` and is not re-enqueued.

### US-009: Hand off to an outbox and dead-letter failures

As a webhook-edge developer, I want the delivery record and outbox row committed together, so that an accepted webhook is never lost and a poison event is isolated.

**Acceptance Criteria:**
- AC-029: Given a verified event and a backend with transactions, when the delivery insert and outbox enqueue succeed, then both rows are visible after commit; when either fails, then neither row is visible.
- AC-030: Given a handler that fails beyond the configured maximum attempts, when the delivery is marked failed, then a dead-letter record contains the raw body and the error text.
- AC-031: Given at-least-once processing, when a delivery is replayed from the dead-letter record, then the handler observes the same delivery id and the store reports `Duplicate` if it had already succeeded.

### US-010: Queue actions durably on the client

As an offline-first application developer, I want actions persisted before acknowledgement, so that a crash or lost connection does not lose a completed field action.

**Acceptance Criteria:**
- AC-032: Given an action, when `enqueue` returns `Ok`, then the action is readable from the configured storage before the call returns.
- AC-033: Given N queued actions, when pending actions are listed, then they are returned in enqueue order.
- AC-034: Given a storage write failure, when `enqueue` runs, then it returns a typed error and no success is reported.
- AC-035: Given an action created without an explicit id, when its id is generated, then it is a UUID v7 whose timestamp component is monotonic within the process.

### US-011: Replay a batch with per-action results

As an offline-first application developer, I want partial failure handled per action, so that one poison or retrying action does not duplicate or block the rest.

**Acceptance Criteria:**
- AC-036: Given a batch in which every action is applied, when replay completes, then each action is marked sent and `BatchResult` contains one `Applied` result per action.
- AC-037: Given a batch where one action returns `Retry`, when replay completes, then the other actions are marked sent and only the retrying action stays pending with attempts incremented.
- AC-038: Given a batch where the server reports `Conflict`, when replay completes, then the configured conflict strategy is applied and the result records `ConflictInfo`.
- AC-039: Given an action that always fails validation, when it reaches the maximum attempt count, then it is marked failed and later pending actions still replay.
- AC-040: Given the mock transport and in-memory storage, when the same batch is replayed twice, then the second run applies no action twice and pending duplicates are reported.

### US-012: Choose a conflict strategy per entity

As an offline-first application developer, I want explicit conflict strategies, so that resolution is a declared policy rather than an accidental merge.

**Acceptance Criteria:**
- AC-041: Given `LastWriteWins`, when client and server timestamps differ, then the later timestamp wins and the decision is recorded.
- AC-042: Given `ServerWins`, when a conflict is reported, then the server value is selected unchanged.
- AC-043: Given `ClientWins`, when a conflict is reported, then the client payload is selected.
- AC-044: Given `Custom`, when a conflict is reported, then the caller's merger returns the resolution and it is recorded in the result.

### US-013: Compose the substrate without a framework

As an external Rust developer, I want one example that composes the crates, so that I can see the intended integration without adopting a framework or a database for every consumer.

**Acceptance Criteria:**
- AC-045: Given the integration example, when it runs, then it registers a tenant-scoped provider, reserves and commits quota, and verifies and enqueues a signed webhook.
- AC-046: Given the workspace, when `cargo build --workspace --all-features` runs on Rust 1.85, then all four crates and the example compile.
- AC-047: Given each crate root, when it is inspected, then `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` are present and rustdoc builds with warnings denied.
- AC-048: Given each crate, when built with `--no-default-features`, then it compiles without a database, HTTP framework, or native dependency.
- AC-049: Given a Postgres connection string that disables TLS, when the backend connects in the default mode, then it refuses unless the caller explicitly enables an insecure test mode.
- AC-050: Given any crate with its features enabled in any combination required by the feature set, when it builds, then no feature disables another.
- AC-051: Given any crate's error type, when `Display` is formatted, then it includes a stable code and the type implements `std::error::Error`.
- AC-052: Given a replay and a webhook flow with tracing enabled, when they run, then queue depth, replay outcome, conflict count, duration, and webhook outcome events are emitted.
- AC-053: Given each crate, when `cargo package --list` runs, then the README, license files, and 0.1.0 metadata are included.

## Requirements

### Functional Requirements

- FR-001: `platform-core` MUST define `TenantId`, `ProviderId`, `ProvisioningMode`, `Capability`, `Provider`, `TokenStore`, `Secret`, and one `IntegrationError` type with stable error codes.
- FR-002: `platform-core` MUST provide a `Registry` with explicit `register`, `get`, `iter`, and capability lookup, and MUST reject duplicate provider registration.
- FR-003: The registry MUST be instance-scoped; the crate MUST NOT expose a global mutable singleton or implicit registration.
- FR-004: `TokenStore` MUST be storage-agnostic with async `get`, `put`, and `revoke`, and MUST ship an in-memory implementation for tests and single-binary tools.
- FR-005: Secret material MUST be redacted in `Debug` and `Display`, MUST NOT implement `Serialize`, and MUST NOT be logged.
- FR-006: `IntegrationError` MUST distinguish `Auth`, `Scope`, `RateLimit`, `Quota`, `Provider`, `Transport`, and `Unsupported`, and MUST implement `Display` and `std::error::Error`.
- FR-007: `quota-ledger` MUST define `TenantId`, `ProviderId`, `Metric`, `Window`, `Budget`, `QuotaKey`, `Reservation`, `ReservationId`, `Usage`, `ExhaustionPolicy`, and `QuotaError`.
- FR-008: `QuotaStore` MUST implement async `reserve`, `commit`, `release`, and `usage`, with a caller idempotency key on `reserve`.
- FR-009: Reservation MUST be atomic: the store MUST check remaining capacity and record the hold in one operation, and MUST NOT allow concurrent reserves to exceed the limit.
- FR-010: `commit` MUST consume a reservation exactly once and `release` MUST consume nothing; repeated or late calls MUST be typed no-ops rather than double counts.
- FR-011: Each `ExhaustionPolicy` MUST produce its declared outcome: `HardStop` denies, `Overage` records overage and cost, `Degrade` returns a degraded outcome, and `Queue` returns a queued outcome.
- FR-012: `Window` MUST support fixed daily and monthly windows with an explicit timezone plus a rolling window, and reset boundaries MUST use the configured timezone rather than the host clock's.
- FR-013: `quota-ledger` MUST provide an optional `postgres` backend that uses parameterized SQL only, an atomic conditional update or row lock for reservation, and TTL-based reclaim of abandoned reservations.
- FR-014: `hookgate` MUST define `InboundRequest`, `VerifiedEvent`, `DeliveryId`, `IdempotencyKey`, `Verifier`, `DeliveryStore`, `Outbox`, and typed verify/store/outbox errors.
- FR-015: Verification MUST operate on the raw request bytes, use constant-time comparison, and enforce a configurable timestamp tolerance or replay window.
- FR-016: `hookgate` MUST ship HMAC verifier presets for Meta Graph, Stripe, Shopify, Twilio, Slack, GitHub, and a configurable generic HMAC-SHA256 verifier.
- FR-017: `DeliveryStore` MUST deduplicate on the tuple of tenant, provider, and provider event id, and MUST report `Duplicate` without enqueueing again.
- FR-018: Delivery insert and outbox enqueue MUST commit as one transaction when the backend supports transactions.
- FR-019: A delivery that exceeds the maximum attempts MUST be recorded as dead-lettered with its raw body and error text, and MUST be replayable with the same delivery identity.
- FR-020: `syncforge` MUST define `Action`, `ActionId`, `EntityType`, `Operation`, `Batch`, `BatchResult`, `ActionResult`, `ActionStatus`, and `ConflictInfo`.
- FR-021: `ActionId` MUST be a UUID v7 generated at action creation, with a monotonic timestamp component within the process.
- FR-022: `Storage` MUST persist append, list pending, mark sent, mark failed, and attempt counts, and MUST ship an in-memory implementation with deterministic ordering.
- FR-023: `Transport` MUST send a batch and return per-action results, and MUST ship a mock transport for deterministic tests.
- FR-024: The replay engine MUST process actions in order, return one result per action, retry retryable failures with bounded backoff, and dead-letter an action after the configured maximum attempts without blocking later actions.
- FR-025: `syncforge` MUST implement `LastWriteWins`, `ServerWins`, `ClientWins`, and `Custom` conflict strategies selectable per entity or action.
- FR-026: Actions MUST carry the client timestamp, the server MUST record its own timestamp, and the server MUST apply a configurable tolerance rather than silently rewriting the client time.
- FR-027: The feature MUST emit tracing events for queue depth, replay outcome, conflict count, and duration.
- FR-028: The workspace MUST add `crates/platform-core`, `crates/quota-ledger`, `crates/hookgate`, and `crates/syncforge` as 0.1.0 members, and MUST include one integration example that composes `platform-core`, `quota-ledger`, and `hookgate`.
- FR-029: Each crate MUST ship a README with a documented default feature set, rustdoc module comments that state scope and non-goals, and publishable metadata inherited from the workspace.

### Non-Functional Requirements

- NFR-001: Tenant isolation MUST hold in every store: no key, query, or delivery lookup may return another tenant's token, usage, or event.
- NFR-002: Secrets, signatures, and connection strings MUST be redacted in `Debug`, `Display`, logs, traces, and error payloads.
- NFR-003: All external input MUST be treated as untrusted; malformed bytes, missing headers, oversized bodies, bad timestamps, and malformed actions MUST return typed errors, never panic or hang.
- NFR-004: All SQL MUST be parameterized. Dynamic identifiers or string-built queries MUST NOT be used with caller input.
- NFR-005: Quota enforcement MUST remain correct under concurrency: concurrent reservations MUST NOT exceed the limit, and settlement MUST be idempotent.
- NFR-006: Every crate MUST compile on Rust 1.85 with edition 2021.
- NFR-007: Every crate MUST keep `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`, and rustdoc MUST build with warnings denied.
- NFR-008: Features MUST be additive and MUST NOT force a database or HTTP framework on a consumer; `--no-default-features` MUST build every crate.
- NFR-009: Postgres connections MUST require TLS by default; a non-TLS mode MUST be explicit, documented as test-only, and never the default.
- NFR-010: Replay MUST be deterministic with the in-memory storage and mock transport so that duplicate and partial-failure behavior is reproducible.
- NFR-011: Every acceptance criterion MUST be covered by at least one test supplied by `/sdrive:test`.
- NFR-012: Registration and configuration MUST be explicit; no crate may rely on ambient global state to discover providers, storage, or transports.

## UI/UX & Data Requirements

- **UI/UX**: This repository ships no UI (CON-301). The crates expose typed data and outcomes; the consuming application renders denial messages, upgrade paths, queue status, and dead-letter dashboards. Nothing in this feature defines presentation, design tokens, or copy.
- **Data**: `platform-core` stores no data; it defines identity and a `TokenStore` contract. `quota-ledger` stores budgets, reservations, and usage rows keyed by tenant, provider, and metric, with a unique index that makes reservation idempotency enforceable. `hookgate` stores delivery records, outbox rows, and dead-letter records keyed by tenant, provider, and event id. `syncforge` persists actions with id, entity, operation, payload, client timestamp, idempotency key, attempts, and status. All identifiers are UUIDs except opaque metric and capability keys; all timestamps are UTC with an explicit source (client or server).

## Security Baseline

The constitution's Security Baseline (CON-51x) applies because this feature handles credentials, tenant-scoped state, untrusted webhook input, and a database surface.

| Control | Status | Decision and expansion |
|---|---|---|
| CON-511 Authentication | REQUIRED | `platform-core` owns the token-store contract and secret redaction; `hookgate` owns signature verification. Expanded to NFR-001, NFR-002, FR-004, FR-005, FR-014, FR-015, FR-016. |
| CON-512 Authorization | REQUIRED | Tenant isolation is enforced in `TokenStore`, `QuotaStore`, and `DeliveryStore`. Expanded to NFR-001, FR-017, FR-019, AC-001, AC-021, AC-027. |
| CON-513 Security Headers | NOT APPLICABLE | No crate owns an HTTP server or response pipeline. `hookgate` is library-first; the consuming edge documents HSTS and header policy. |
| CON-514 Rate Limiting | REQUIRED | `quota-ledger` is the shared quota enforcement point for this feature (`wirekit` owns per-process rate limiting and is not in this base). Expanded to FR-009, FR-011, NFR-005. |
| CON-515 Input Validation | REQUIRED | Raw webhook bytes, headers, timestamps, and action payloads are untrusted. Expanded to FR-015, FR-024, NFR-003. |
| CON-516 NoSQL / Injection Protection | REQUIRED | The Postgres backend is the only query surface. Expanded to FR-013, NFR-004. |
| CON-517 XSS Protection | NOT APPLICABLE | No crate renders HTML or markup; `hookgate` returns parsed events, not pages. |
| CON-518 CSRF Protection | NOT APPLICABLE | No crate owns a browser session or cookie state; signatures and tenant-scoped tokens are the boundary. |
| CON-519 Environment Security | REQUIRED | Connection strings and signing keys are caller-supplied configuration, never committed. Expanded to NFR-002, NFR-009. |
| CON-520 HTTPS / HSTS | REQUIRED (database portion) | This feature adds no HTTP client and no HTTP server, so the HTTPS-client portion is not applicable here; the Postgres connection MUST require TLS by default. Expanded to NFR-009 and AC-049. HSTS remains the deployment edge's responsibility. |

## Anti-Redundancy Findings

- The workspace contains exactly one crate, `crates/dado` (`Cargo.toml`, `crates/`). No existing module implements tenant identity, quota, webhooks, or offline replay.
- All four crates are therefore `create` decisions. There is no existing registry, token store, quota store, verifier, delivery store, action queue, storage, transport, or conflict strategy to `reuse` or `modify`.
- Existing assets to `reuse`: the root workspace and `[workspace.package]` inheritance, the crate-level lint and feature conventions established by `dado`, the `crates/<crate>/tests/<feature>/` test layout (CON-406), and the CI workflow created by `m0-foundation`.
- `wirekit` is not present in this base and MUST NOT be added as a dependency; `dado` is unrelated and MUST NOT be modified.

## Edge Cases & Error States

- EC-001: Empty registry → `get` returns `None`, `iter` yields nothing, and no panic or global fallback registration occurs.
- EC-002: Duplicate provider registration → typed duplicate error; the existing entry is preserved.
- EC-003: Unknown provider id → `None`; capability lookup never infers support from crate presence.
- EC-004: Token backend failure → typed `TokenError`; no partial write is reported as success.
- EC-005: Budget limit of zero → every reserve follows the configured policy (`HardStop` denies immediately, `Overage` needs an explicit non-zero allowance).
- EC-006: Requested units greater than the remaining budget → typed denial that states requested and remaining.
- EC-007: Reservation TTL expiry → held units are reclaimed exactly once; a late commit is a typed no-op.
- EC-008: Same idempotency key with a different payload → typed conflict error, not a silent second reservation.
- EC-009: Window boundary or provider timezone (for example Pacific midnight) → reset uses the configured timezone; the usage query reports the next reset instant.
- EC-010: Missing or empty signature header → typed verification error; the body is not parsed.
- EC-011: Body above the configured maximum → typed size error before verification.
- EC-012: Signature in uppercase or lowercase hex, or with the documented prefix → normalized per preset; an unknown format fails.
- EC-013: Timestamp in the future beyond tolerance → rejected as stale or invalid, with the same typed error family.
- EC-014: Two deliveries with the same event id racing → exactly one insert wins; the other reports `Duplicate`.
- EC-015: Outbox enqueue fails after delivery insert → the transaction rolls back and the request is not acknowledged.
- EC-016: Empty queue → replay returns an empty `BatchResult` and emits no transport call.
- EC-017: Transport returns a whole-batch failure → no action is marked sent; retryable actions stay pending with bounded backoff.
- EC-018: Conflict reported without server state → typed error rather than an invented resolution.
- EC-019: Poison action reaches maximum attempts → dead-lettered; later actions still replay.
- EC-020: Client clock far in the past or future → client time is preserved, server time is recorded, and the tolerance decision is visible in the result.
- EC-021: `--no-default-features` build → core types and traits compile with no database or HTTP dependency.
- EC-022: Insecure Postgres connection string in default mode → refused with a typed configuration error unless the explicit test-only mode is enabled.

## Out of Scope

- The Redis quota backend (BP-019) and any Redis dependency.
- `platform-core` `sqlx` token storage and the `axum` tenant extractor.
- `syncforge` SQLite storage, HTTP transport, connectivity detection adapters, and WASM packaging.
- `hookgate` SendGrid ECDSA verification, the `axum` integration, and a standalone binary.
- Any dependency on `wirekit`, including `IntegrationError` mapping from `wirekit` variants. That mapping is a later version once `wirekit` is on the base branch.
- Provider-specific quota presets (YouTube, WhatsApp, X, Twitch) as code; they are data supplied by callers.
- Billing, invoicing, currency conversion, dunning, or tax logic.
- A dashboard, UI, design tokens, or operator console for quota, webhooks, or sync.
- `no_std` support for `quota-ledger`.
- Publishing any crate to crates.io from the feature branch; publication is manual and post-merge.
- Changes to `dado` or its public API.
- A second error type per crate; each crate exposes one error enum with stable codes.

## Dependencies & Risks

- **Dependencies**: `uuid` (v4/v7, serde), `chrono` (serde), `serde`, `serde_json`, `async-trait`, `thiserror`, `tracing`, `secrecy`, `hmac`, `sha2`, `sha1`, `subtle`, `hex`, `base64`, `http`, `bytes`, and optional `sqlx` (Postgres, runtime-tokio-rustls, chrono, uuid) for `quota-ledger`. Dev-dependencies: `tokio` with macros and rt-multi-thread. All are recorded in `plan.json` with an action once the plan is written.
- **RISK-001**: Four crates in one feature create review fatigue and a long-lived branch. Mitigation: one crate per plan phase, each phase compiles and documents its public surface on its own, and the integration example comes last.
- **RISK-002**: Async traits used as trait objects can break object safety or add hidden allocation costs. Mitigation: standardize on `async-trait`, keep traits object-safe, and prove the contract suites against the memory and mock implementations.
- **RISK-003**: Postgres integration cannot run everywhere (no local server or CI service). Mitigation: keep the backend behind `postgres`, provide a SQL schema, require a contract test suite runnable with `DATABASE_URL`, and let `/sdrive:test` decide CI provisioning.
- **RISK-004**: Database identifiers or caller data could be interpolated into SQL. Mitigation: fixed schema, parameterized values only, and a review check for string-built queries (NFR-004).
- **RISK-005**: Signature comparison or deduplication could leak timing or allow replay. Mitigation: constant-time comparison, raw-byte verification, unique constraints, and replay-window tests.
- **RISK-006**: One or more crate names are taken on crates.io. Mitigation: check each name immediately before publication (CON-111); if taken, stop and choose a fallback before coding the publish step.
- **RISK-007**: New dependencies raise the effective MSRV above 1.85. Mitigation: the existing MSRV CI job covers the workspace; pin or downgrade with a recorded decision.
- **RISK-008**: The trait surface becomes too abstract or absorbs product nouns. Mitigation: enforce the catalogue non-goals, keep capability and metric keys opaque, and review every public type against the out-of-scope list.
- **RISK-009**: `syncforge` replay and conflict logic silently duplicates actions. Mitigation: per-action results, UUID v7 idempotency keys, deterministic mock transport tests, and an explicit at-least-once contract in the docs.
- **RISK-010**: Scope creep from the deferred list (Redis, SQLite, HTTP, axum, SendGrid). Mitigation: deferred items are explicit in this specification; adding one requires a new version, not a silent expansion.

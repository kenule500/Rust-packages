---
title: quota-ledger 0.1 - core reservations and Postgres backend
status: backlog
version: v1
created: 2026-11-02
updated: 2026-11-02
---

# quota-ledger 0.1 - core reservations and Postgres backend

## Overview

- **Problem Statement**: A process-local token bucket in `wirekit` cannot answer the question a multi-tenant service actually asks before an outbound provider call: may this tenant spend one more unit of this provider's quota right now, and what happens if they cannot? Without shared state, two API pods each believe they hold the last unit, a retry spends quota twice, and a restart forgets the month's consumption. Every service then invents its own ledger, and its own answer to reserve-then-fail.
- **Proposed Solution**: Port and adapt the `quota-ledger` crate from `feature/m3-substrate` as prior art: `TenantId`, `ProviderId`, `Metric`, `Window`, `Period`, `Budget`, `ExhaustionPolicy`, `QuotaKey`, `Reservation`, `ReserveRequest`, `ReserveOutcome`, `Usage`, and `QuotaExceeded`, behind an async `QuotaStore` trait with `reserve`, `commit`, `release`, and `usage`. Ship `MemoryQuotaStore` for tests and single-process callers, and an optional `postgres` feature adding a transactional backend that uses an atomic conditional UPDATE so concurrent reservations cannot exceed a limit. No Redis backend in 0.1 (BP-019).
- **Target Audience**: Rust teams running more than one instance of a service that calls rate-limited or metered providers, and this project's service layer that brokers provider quota across pods.
- **Assumptions**:
  - A working implementation exists on `feature/m3-substrate` (18 files, features `default` and `postgres`) and is ported and adapted rather than redesigned.
  - `main` has no `[workspace.dependencies]`, and the branch manifests use `dep.workspace = true`, so `async-trait`, `chrono`, `thiserror`, `uuid`, `sqlx`, and `platform-core` are declared directly in the crate manifest.
  - `main` already depends on `platform-core` for `TenantId` and `ProviderId` identity; `quota-ledger` reuses that identity so a tenant is one tenant across the stack.
  - The branch tests are not ported; the test agent authors tests against this specification.
  - The unmerged `m3-substrate/v1` spec covers four crates and is not reused as governance here.

## User Stories

### US-001: Key quota by tenant, provider, and metric
As a service author, I want a typed key that names whose quota, which provider, and which unit, so that one ledger can hold every provider's meter without collisions.

**Acceptance Criteria:**
- AC-001: Given a tenant, provider, and metric, when a `QuotaKey` is constructed, then it identifies exactly one budget line and two different metrics for one provider are separate lines.
- AC-002: Given the crate built with `--no-default-features`, when it compiles, then the core types, error type, and memory store are available without the `postgres` feature.

### US-002: Reserve, then commit or release
As a service author, I want to hold quota before a call and settle it after, so that two pods cannot spend the same last unit and a failed call does not burn quota.

**Acceptance Criteria:**
- AC-003: Given capacity remaining, when `reserve` is called, then it returns a reservation with an identifier and the held amount without consuming it.
- AC-004: Given an open reservation, when `commit` is called, then it consumes the hold exactly once, and a second `commit` for the same reservation is rejected or idempotent and never consumes twice.
- AC-005: Given an open reservation, when `release` is called, then the held amount returns to the available budget.
- AC-006: Given no capacity remaining and a policy of `HardStop`, when `reserve` is called, then it is denied with a typed `QuotaExceeded` carrying the reset instant and does not create a reservation.

### US-003: Idempotent reservations under retry
As a service author, I want a retried reserve to return the original reservation, so that a lost response does not hold quota twice.

**Acceptance Criteria:**
- AC-007: Given a caller idempotency key already used with the same payload, when `reserve` is called again, then the existing reservation is returned and no second hold is taken.
- AC-008: Given a caller idempotency key previously used with a different payload, when `reserve` is called, then it fails with a typed idempotency-conflict error rather than returning the wrong reservation.

### US-004: Apply an exhaustion policy
As a service author, I want to choose what happens when a budget runs out, so that the ledger enforces the decision instead of each caller inventing one.

**Acceptance Criteria:**
- AC-009: Given a budget, when `ExhaustionPolicy` is inspected, then it distinguishes `HardStop`, `Overage`, `Degrade`, and `Queue`, and `HardStop` denies while the others report the outcome the caller asked for.

### US-005: Report usage and the next reset
As a service author, I want to read limit, used, held, and the next reset, so that a dashboard and a caller both see the same truth.

**Acceptance Criteria:**
- AC-010: Given a key with reservations, when `usage` is read, then it reports the limit, the consumed amount, the held amount, and the next reset instant.
- AC-011: Given a window period, when it is configured, then `Daily`, `Monthly`, and `Rolling` windows are supported and the reset instant is computed from the window rather than assumed to be UTC midnight.

### US-006: Share the ledger across processes
As a service author, I want the `postgres` backend behind the same trait, so that quota survives restarts and is consistent across pods.

**Acceptance Criteria:**
- AC-012: Given the `postgres` feature, when the backend is built, then it implements `QuotaStore` over a connection and its schema is shipped as SQL the caller applies.
- AC-013: Given concurrent reservations from many tasks against one limited budget, when all complete, then the total committed never exceeds the limit.

### US-007: Isolate tenants and build on 1.87
As a security reviewer, I want tenant scoping and the workspace baseline enforced, so that one tenant can never read or spend another's quota.

**Acceptance Criteria:**
- AC-014: Given a tenant, when usage or a reservation is read, then no key can observe another tenant's usage, and every store operation is scoped by the tenant in the key.
- AC-015: Given the crate, when it is built with all features on Rust 1.87, then it compiles with `forbid(unsafe_code)` and `deny(missing_docs)`, ships the licence files, and every Postgres statement is parameterized rather than string-interpolated.

## Requirements

### Functional Requirements
- FR-001: The crate MUST expose `TenantId`, `ProviderId`, `Metric`, `Window`, `Period`, `Budget`, `ExhaustionPolicy`, `QuotaKey`, `Reservation`, `ReservationId`, `ReserveRequest`, `ReserveOutcome`, `Usage`, and `QuotaExceeded` per `catalogue/29_quota.md`.
- FR-002: Features MUST be additive and default to none; a `--no-default-features` build MUST compile the core without Postgres.
- FR-003: `QuotaStore` MUST be async and cover `reserve`, `commit`, `release`, and `usage`.
- FR-004: `commit` MUST consume a hold exactly once; a repeated commit MUST NOT spend quota twice.
- FR-005: A reservation MUST carry an optional caller idempotency key; a repeated key with the same payload MUST return the existing reservation and a repeated key with a different payload MUST fail as a conflict.
- FR-006: A denial MUST be a typed `QuotaExceeded` carrying the reset instant, never a generic error.
- FR-007: `ExhaustionPolicy` MUST distinguish `HardStop`, `Overage`, `Degrade`, and `Queue`.
- FR-008: `Window` and `Period` MUST support daily, monthly, and rolling windows and compute the reset instant from the configured window.
- FR-009: A `postgres` feature MUST add a `QuotaStore` implementation backed by a shipped schema and must be absent when the feature is off.
- FR-010: The postgres backend MUST use an atomic conditional statement so concurrent reservations cannot exceed the limit.
- FR-011: Every store operation MUST be scoped by the tenant carried in the key.
- FR-012: The crate MUST declare `forbid(unsafe_code)` and `deny(missing_docs)`, and MUST NOT introduce a second error type.

### Non-Functional Requirements
- NFR-001: No API may return or spend another tenant's quota; every operation is scoped by the tenant and provider key (CON-512).
- NFR-002: Every Postgres statement MUST be parameterized; no query may interpolate caller values into SQL text (CON-516).
- NFR-003: All caller-supplied keys, metrics, and windows MUST be validated before use, and an invalid window MUST produce a positioned error rather than a guessed default (CON-515).
- NFR-004: The crate MUST NOT read credentials from the environment or a file; the caller supplies the connection (CON-519), and a Postgres connection MUST require TLS (CON-520).
- NFR-005: The crate MUST compile on Rust 1.87 with all features and MUST contain no product or vendor workflow vocabulary (CON-102, CON-106).
- NFR-006: Every public item MUST carry rustdoc and every module MUST state its scope and non-goals (CON-202, CON-203).

## UI/UX & Data Requirements

- **UI/UX**: Not applicable. This repository ships no UI layer (CON-301); a consuming application renders `Usage` and `QuotaExceeded`.
- **Data**: The Postgres backend stores a budget row per `(tenant, provider, metric, window)` with the limit, consumed count, held count, and window bounds, plus a reservation row keyed by `ReservationId` carrying the idempotency key, amount, and state. The schema is shipped as SQL; the caller applies it.

## Edge Cases & Error States

- EC-001: Concurrent reserve calls race for the final unit → exactly one succeeds; the others are denied; the total committed never exceeds the limit.
- EC-002: Process crashes between reserve and commit → the hold remains and is reclaimable after the reservation TTL; the ledger never silently consumes it.
- EC-003: Idempotency key replayed with a different metric or amount → typed idempotency-conflict error; no second reservation is created.
- EC-004: Window boundary crossed mid-reservation → the reservation settles against the window it was created in; the reset instant is derived from the window, not the host clock's midnight.
- EC-005: Build with `--no-default-features` → core compiles with no `sqlx` in the dependency graph.

## Out of Scope

- A Redis backend; the catalogue leaves it as BP-019, a separate later item.
- Process-local rate limiting; that remains the `wirekit` token bucket.
- Billing, invoicing, and currency conversion; the ledger records consumption only.
- A distributed lock service or consensus protocol.
- A policy engine that decides pricing; the caller supplies the policy.
- Any product or vendor workflow vocabulary (CON-106).

## Dependencies & Risks

- `platform-core` for tenant and provider identity; `async-trait`, `chrono`, `thiserror`, `uuid` for the core; optional `sqlx` behind `postgres`; `tokio` dev-only.
- **RISK-001**: The Postgres backend is the hardest part to verify without a live database, so a defect in the atomic claim may pass review. AC-013 asserts the concurrency invariant directly, and the branch ships a Postgres test harness to adapt.
- **RISK-002**: Porting stale code may drag in manifests using workspace inheritance the current workspace does not define. Adapt the manifest to declare dependencies directly, as `platform-core` did.
- **RISK-003**: A quota ledger invites scope creep into billing and pricing. The out-of-scope list and NFR-005 are enforced at review.

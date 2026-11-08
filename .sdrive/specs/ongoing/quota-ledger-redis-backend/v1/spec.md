---
title: quota-ledger 0.2 - Redis backend
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# quota-ledger 0.2 - Redis backend

## Overview

- **Problem Statement**: `quota-ledger` 0.1 ships a Postgres backend. Services that already run Redis but not Postgres — edge workers, single-region APIs, and apps whose only durable store is Redis — cannot share quota state without adding a database, so they fall back to a per-process counter that resets on deploy and lets two pods spend the same last unit.
- **Proposed Solution**: Add an additive `redis` feature to `quota-ledger` that implements the existing `QuotaStore` trait over a caller-supplied Redis connection. The atomic claim becomes a server-side Lua script so concurrent reservations cannot exceed a budget, and every operation keeps the 0.1 contract: tenant-scoped keys, commit-once, release, idempotency conflict, and a typed `QuotaExceeded` carrying the reset instant. Postgres stays the default backend; enabling `redis` must not compile `sqlx`.
- **Target Audience**: Rust services that already operate Redis and need shared provider quota without a relational database.
- **Assumptions**:
  - The 0.1 `QuotaStore` contract is frozen enough to implement against; no trait change is expected.
  - The caller supplies a connected `redis::aio::ConnectionManager`; the crate reads no environment variable and no connection string.
  - The `redis` crate version must resolve on the workspace MSRV (1.87). If no version does, this feature stops and reports rather than raising the MSRV (CON-102 amendment required).
  - Redis 7 or later, so `EVALSHA` scripting and `TIME` are available.
  - A live Redis is not available in this repository's CI; concurrency proof therefore needs an opt-in harness, recorded as a risk.

## User Stories

### US-001: Share quota state through Redis
As a service author whose only durable store is Redis, I want the ledger to live there, so that quota survives restarts and is consistent across pods without adding Postgres.

**Acceptance Criteria:**
- AC-001: Given a `redis` feature build and a caller-supplied connection, when `reserve` is called with remaining capacity, then it records a hold and returns a reservation with an identifier, using only Redis keys scoped by tenant, provider, metric, and window.
- AC-002: Given concurrent reserve calls against one limited budget, when all complete, then the total committed never exceeds the limit because the claim is one server-side script.
- AC-003: Given an open reservation, when `commit` is called twice, then the hold is consumed exactly once and a repeated commit is rejected or idempotent.
- AC-004: Given a caller idempotency key already used with a different payload, when `reserve` is called again, then it fails with the typed idempotency conflict and takes no second hold.
- AC-005: Given no capacity and `HardStop`, when `reserve` is called, then it returns a typed `QuotaExceeded` carrying the reset instant and creates no reservation.
- AC-006: Given a key with reservations, when `usage` is read, then it reports limit, consumed, held, and the next reset instant computed from the configured window.
- AC-007: Given the crate built with default features, when it compiles, then no Redis type is present and Postgres remains the only optional backend.
- AC-008: Given a Redis failure or a malformed payload, when an operation runs, then it returns a typed `QuotaError` and the error never renders the connection string or a credential.

## Requirements

### Functional Requirements
- FR-001: A `redis` feature MUST add a `QuotaStore` implementation backed by a caller-supplied connection and MUST be absent when the feature is off.
- FR-002: The reserve claim MUST be a single server-side script so concurrent claims cannot exceed the limit.
- FR-003: `commit` MUST consume a hold exactly once; `release` MUST return held units.
- FR-004: Idempotency keys MUST be honored per tenant, provider, metric, and window, and a conflicting payload MUST return a typed conflict.
- FR-005: Budget and reservation keys MUST embed the tenant, so no operation can read or spend another tenant's quota (CON-512).
- FR-006: Windows MUST support daily, monthly, and rolling resets computed from the window, not from host midnight.
- FR-007: The crate MUST NOT read environment variables, connection strings, or credential files (CON-519).
- FR-008: Errors MUST NOT render a connection string, a password, or a Redis URI (CON-214).

### Non-Functional Requirements
- NFR-001: Every Redis key MUST be namespaced by tenant and MUST NOT collide across providers or metrics (CON-512).
- NFR-002: The script and every command MUST pass caller values as arguments, never as interpolated command text (CON-516).
- NFR-003: Caller-supplied windows, metrics, and unit counts MUST be validated and bounded before reaching Redis (CON-515).
- NFR-004: The crate MUST compile on Rust 1.87 with all features (CON-102).
- NFR-005: Every public item MUST carry rustdoc and every module MUST state its scope and non-goals (CON-202, CON-203).

## UI/UX & Data Requirements

- **UI/UX**: Not applicable; the crate ships no UI (CON-301).
- **Data**: `quota_budget:{tenant}:{provider}:{metric}:{window_start}` holds limit, used, held, overage fields; `quota_reservation:{reservation_id}` holds units, idempotency key, expiry, policy; `quota_idem:{tenant}:{provider}:{metric}:{key}` maps an idempotency key to its reservation. TTLs follow the window end plus a retention margin.

## Edge Cases & Error States

- EC-001: Two pods reserve the final unit simultaneously → exactly one script call succeeds; the other returns `QuotaExceeded`.
- EC-002: Redis restarts between reserve and commit → a missing reservation returns a typed not-found error, never a synthetic success.
- EC-003: A reservation outlives its window → it settles against the window it was created in; the reset instant is derived from the key, not the clock.
- EC-004: Feature off → the crate compiles and links no Redis client, and `sqlx` stays the only optional backend.
- EC-005: Connection failure mid-call → typed `QuotaError::Backend`, redacted, with no partial hold.

## Out of Scope

- Replacing Postgres as the default backend.
- A Redis cluster or sentinel configuration helper; the caller configures the client.
- Migration of existing Postgres ledger rows into Redis.
- Changes to the `QuotaStore` trait or the 0.1 types.

## Dependencies & Risks

- `redis` (async, TLS-capable) behind the `redis` feature; `tokio` dev-only. Version must resolve on 1.87.
- **RISK-001**: No Redis in CI, so AC-002's concurrency proof needs an opt-in harness; the script's atomicity is asserted by a Redis-backed test when a server is available and documented otherwise.
- **RISK-002**: Lua scripts are easy to get subtly wrong around expiry; the script is the review's focus and needs its own failure-path tests.
- **RISK-003**: A `redis` version that needs Rust ≥ 1.88 blocks the feature; the mitigation is to pin a compatible release or stop and amend the constitution, never to raise the MSRV silently.

## Security Baseline

- CON-511 Authentication: NOT APPLICABLE — the caller supplies an authenticated connection.
- CON-512 Authorization: REQUIRED — every key is tenant-scoped (NFR-001, AC-005).
- CON-513 Security Headers: NOT APPLICABLE — no HTTP surface.
- CON-514 Rate Limiting: REQUIRED — this crate is the quota control.
- CON-515 Input Validation: REQUIRED — NFR-003.
- CON-516 Injection: REQUIRED — NFR-002, arguments only.
- CON-517 XSS: NOT APPLICABLE — no markup.
- CON-518 CSRF: NOT APPLICABLE — no browser session.
- CON-519 Environment Security: REQUIRED — FR-007.
- CON-520 HTTPS: REQUIRED — the caller's Redis client MUST use TLS in production; this crate documents it and never disables it.

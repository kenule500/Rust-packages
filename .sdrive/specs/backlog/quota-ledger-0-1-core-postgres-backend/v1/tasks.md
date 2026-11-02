---
title: quota-ledger 0.1 - execution tasks
status: backlog
version: v1
created: 2026-11-02
updated: 2026-11-02
---

# quota-ledger 0.1 - execution tasks

> Production code only. Test authoring and test execution belong to the test agent (`/sdrive:test`) and are not tasks here.

## P1 - Crate scaffold and core types

- [ ] **T1.1 - Scaffold the quota-ledger crate** (`FR-001`, `FR-002`, `FR-012` / `AC-002`, `AC-015`)
  Create `crates/quota-ledger` with a manifest that declares dependencies directly (`main` has no `[workspace.dependencies]`), features `default = []` and `postgres = [dep:sqlx]`, the workspace-inherited `edition`, `rust-version`, `license`, and `repository` fields, the licence files, a README, and a `lib.rs` carrying `forbid(unsafe_code)`, `deny(missing_docs)`, and crate docs stating scope and non-goals. Confirm the workspace resolves the new member.
- [ ] **T1.2 - Implement core value types and the single error enum** (`FR-001`, `FR-007`, `NFR-003` / `AC-001`, `AC-002`, `AC-009`)
  Implement `types.rs` with `QuotaKey`, `Metric`, `Window`, `Period`, `Budget`, `ExhaustionPolicy`, `Reservation`, `ReservationId`, `ReserveRequest`, `ReserveOutcome`, `Usage`, and `QuotaExceeded`, reusing `platform-core` `TenantId` and `ProviderId`. Implement `error.rs` as the single `QuotaError` enum with stable variants including denial and idempotency conflict. Implement `policy.rs`. Validate caller-supplied windows and metrics and return a typed error rather than a guessed default.

## P2 - Memory store and reservation lifecycle

- [ ] **T2.1 - Define QuotaStore and window arithmetic** (`FR-003`, `FR-008`, `FR-011`, `NFR-001` / `AC-003`, `AC-010`, `AC-011`, `AC-014`)
  Define the async `QuotaStore` trait covering `reserve`, `commit`, `release`, and `usage`, with every operation scoped by the tenant in the `QuotaKey`. Implement `window.rs` so the reset instant is computed from the configured window (daily, monthly, rolling) rather than an assumed UTC midnight.
- [ ] **T2.2 - Implement MemoryQuotaStore reserve, commit, and release** (`FR-003`, `FR-004` / `AC-003`, `AC-004`, `AC-005`)
  `reserve` records a hold without consuming it and returns a `Reservation` with an identifier; `commit` consumes the hold exactly once and a repeated commit never consumes twice; `release` returns the held amount to the available budget. Shard by key so unrelated budgets are not serialized.
- [ ] **T2.3 - Add idempotent reservation handling** (`FR-003`, `FR-005` / `AC-007`, `AC-008`)
  Carry an optional caller idempotency key. A repeated key with the same payload returns the existing reservation and takes no second hold; a repeated key with a different payload fails with a typed idempotency conflict rather than returning the wrong reservation.
- [ ] **T2.4 - Enforce exhaustion policy and report usage** (`FR-003`, `FR-006` / `AC-006`, `AC-010`)
  Denial is a typed `QuotaExceeded` carrying the reset instant; `HardStop` denies while `Overage`, `Degrade`, and `Queue` report the caller's chosen outcome. `usage` reports limit, consumed, held, optional overage, and the next reset instant.

## P3 - Postgres backend

- [ ] **T3.1 - Ship the Postgres schema and connection setup** (`FR-009`, `NFR-004` / `AC-012`)
  Add `src/postgres/schema.sql` with a budget row per `(tenant, provider, metric, window)` and a reservation row keyed by `ReservationId`. Gate the backend behind the `postgres` feature so no `sqlx` type enters the default build. Require TLS; read no credentials from the environment.
- [ ] **T3.2 - Implement the Postgres QuotaStore with an atomic claim** (`FR-009`, `FR-010`, `NFR-002` / `AC-012`, `AC-013`, `AC-015`)
  Use only parameterized statements. Make the reserve claim a single conditional `UPDATE` whose `WHERE` clause carries the remaining-capacity predicate, so concurrent reservations cannot exceed the limit. Reject a repeated idempotency key with a differing payload as a typed conflict.

## P4 - Wiring, docs, verification

- [ ] **T4.1 - Wire the crate, document it, and run the verification checks** (`FR-012`, `NFR-005`, `NFR-006` / `AC-015`)
  Confirm the workspace resolves the new member; write the README; ensure every public item carries rustdoc and every module states scope and non-goals; keep the crate free of product or vendor vocabulary. Then run `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `cargo doc --workspace --all-features --no-deps` with `RUSTDOCFLAGS=-D warnings`, `cargo check -p quota-ledger --no-default-features`, and `node .sdrive/scripts/validate-governance.js`.

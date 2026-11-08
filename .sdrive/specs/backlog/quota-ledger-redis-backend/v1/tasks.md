---
title: quota-ledger 0.2 - Redis backend tasks
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# quota-ledger 0.2 - Redis backend tasks

> Production code only. Test authoring and execution belong to `/sdrive:test`.

## P1 - Feature wiring and key schema

- [ ] **T1.1 - Add the `redis` feature and gate the module** (`FR-001`, `AC-007`, `NFR-004` / `AC-007`)
  Add an optional `redis` dependency and `redis = ["dep:redis"]` to the crate manifest, keeping `default = []`. Gate `pub mod redis` behind the feature in `lib.rs` and re-export `RedisQuotaStore`. Confirm a default build links no Redis type and the workspace still resolves.
- [ ] **T1.2 - Implement the key schema and window derivation** (`FR-005`, `FR-006`, `NFR-001` / `AC-001`, `AC-006`)
  Add `redis/keys.rs` with builders for the budget hash, reservation hash, idempotency string, and the window index. Every key starts with the tenant and includes provider, metric, and window start. Derive window start and reset from the configured `Window`, never from host midnight.

## P2 - Store implementation

- [ ] **T2.1 - Implement the atomic reserve script** (`FR-002`, `NFR-002`, `NFR-003` / `AC-002`, `AC-005`)
  Add `redis/reserve.lua`: validate the remaining-capacity predicate, write the reservation hash, increment held units, register the idempotency key, and set TTLs, all in one script. Return a tagged result the Rust side maps to a reservation, `QuotaExceeded` with the reset instant, or an idempotency conflict. Pass every caller value as an argument.
- [ ] **T2.2 - Implement commit and release settlement** (`FR-003` / `AC-003`)
  Add `redis/settle.lua` with a mode argument. Delete the reservation key and mutate the budget only when the delete reports one key, so a repeated commit or release is a no-op rather than a second settlement.
- [ ] **T2.3 - Implement usage and idempotent reserve replay** (`FR-003`, `FR-004` / `AC-004`, `AC-006`)
  Read the budget hash for `usage` (limit, used, held, overage, reset). On reserve, look up the idempotency key first: the same payload returns the existing reservation, a different payload returns the typed conflict.

## P3 - Wiring, docs, verification

- [ ] **T3.1 - Map errors with redaction and document the backend** (`FR-007`, `FR-008`, `NFR-005` / `AC-008`)
  Map Redis failures into `QuotaError::Backend` without rendering the connection string, and document the key schema, TLS expectation, TTLs, and the no-environment rule in the crate README and module docs. Every public item carries rustdoc.
- [ ] **T3.2 - Run the verification checks** (`NFR-004`, `NFR-005` / `AC-007`)
  Run `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `cargo doc --workspace --all-features --no-deps` under `RUSTDOCFLAGS=-D warnings`, `cargo check -p quota-ledger` and `-p quota-ledger --no-default-features`, and `node .sdrive/scripts/validate-governance.js`.

# Test Report: syncforge 0.1 - queue, idempotency, conflicts, traits (v1)

## 1. Summary
- **Date:** 2026-11-08
- **Feature:** syncforge-0-1-queue-idempotency-conflicts-traits
- **Version:** v1
- **Total ACs:** 13
- **Tested ACs:** 13
- **Coverage:** 100%
- **Tests Passed:** 13/13

## 2. AC → Test Mapping
| AC ID | Test ID | File | Status |
|-------|---------|------|--------|
| AC-001 | TEST-001 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-002 | TEST-002 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-003 | TEST-003 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-004 | TEST-004 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-005 | TEST-005 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-006 | TEST-006 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-007 | TEST-007 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-008 | TEST-008 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-009 | TEST-009 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-010 | TEST-010 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-011 | TEST-011 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-012 | TEST-012 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |
| AC-013 | TEST-013 | crates/syncforge/tests/syncforge-0-1-queue-idempotency-conflicts-traits/main.rs | ✅ Pass |

## 3. Failures
| Test | Reason | Blocking |
|------|--------|----------|
| — | None | No |

## 4. Notes

- **Method.** Queue behaviour is driven through the public `Storage` and `Transport` traits. The suite defines its own `ScriptedTransport` (records every batch, answers with per-action statuses) and `RejectingStorage` (a failing append), so the trait seams are exercised the way a caller would use them rather than only via the shipped mocks.
- **Durability (AC-001).** Enqueue is asserted to leave the action readable in storage before returning, and a storage that rejects `append` must make `enqueue` return a typed `SyncError::Storage` rather than success.
- **Idempotency (AC-002, AC-003).** Two actions get distinct UUID v7 ids (timestamp component non-zero and non-decreasing) and distinct idempotency keys. The replay test asserts the wire batch carries those keys, and that a `Duplicate` settles the action so a second replay makes no transport call.
- **Per-action outcomes (AC-004, AC-005, AC-006).** A `Retry` advances only that action's attempt count while the others settle; a mixed batch returns one result per action in enqueue order; five actions with `with_batch_size(2)` produce batches of 2, 2, and 1.
- **Conflicts (AC-007, AC-008).** `ConflictInfo` is asserted to carry the entity, both values, and both timestamps. All four strategies are exercised, including `Custom` with a caller merger that returns `Resolution::Merged`.
- **Timestamps (AC-009).** `decide` returns `Accepted` within tolerance, `Clamped` when the client clock ran ahead, and `Rejected` when it ran behind; both original timestamps are preserved and `effective_client_timestamp` follows the verdict.
- **Poison actions (AC-010).** With `with_max_attempts(2)` and a transport that always retries, the second replay reports `Rejected` and leaves the pending queue empty.
- **Traits and core surface (AC-011, AC-012).** The engine runs against `Arc<dyn Storage>` / `Arc<dyn Transport>` supplied by the test. AC-012 asserts the manifest declares exactly `default = []` and that the core types, error, and both reference implementations exist; the `--no-default-features` build itself is proven by `cargo check -p syncforge --no-default-features`.
- **Tracing (AC-013).** A minimal `tracing::Subscriber` captures formatted events during a real enqueue plus a conflict replay and asserts `syncforge.enqueue`, `syncforge.replay.start`, `syncforge.replay.finish`, and `syncforge.conflict` are emitted with `queue_depth`, `conflicts`, and `duration_ms` in the payload.
- **Test quality.** 13 tests named `ac_0NN_*`, 76 assertions, no `.only`/`xit`/`#[ignore]`, no empty assertions. Red-before-green is not enabled in `config.json`.
- **Dev-dependencies added for the suite:** `async-trait`, `chrono`, `serde_json`, and `tracing` — all already crate dependencies, so no new library enters the graph. `tokio` was already a dev-dependency.

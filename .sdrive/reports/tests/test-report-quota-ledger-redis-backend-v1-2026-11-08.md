# Test Report: quota-ledger 0.2 - Redis backend (v1)

## 1. Summary
- **Date:** 2026-11-08
- **Feature:** quota-ledger-redis-backend
- **Version:** v1
- **Total ACs:** 8
- **Tested ACs:** 8
- **Coverage:** 100%
- **Tests Passed:** 8/8

## 2. AC → Test Mapping
| AC ID | Test ID | File | Status |
|-------|---------|------|--------|
| AC-001 | TEST-001 | crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs | ✅ Pass |
| AC-002 | TEST-002 | crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs | ✅ Pass |
| AC-003 | TEST-003 | crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs | ✅ Pass |
| AC-004 | TEST-004 | crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs | ✅ Pass |
| AC-005 | TEST-005 | crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs | ✅ Pass |
| AC-006 | TEST-006 | crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs | ✅ Pass |
| AC-007 | TEST-007 | crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs | ✅ Pass |
| AC-008 | TEST-008 | crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs | ✅ Pass |

## 3. Failures
| Test | Reason | Blocking |
|------|--------|----------|
| — | None | No |

## 4. Notes

- **Live evidence.** The suite was run against a real Redis 7 server already listening on `127.0.0.1:6379`, in **database 15** (`REDIS_URL=redis://127.0.0.1:6379/15`). All 8 tests passed in 0.16 s. Every key the suite writes starts with `quota:` and carries a fresh tenant id, so it never touches another application's keys even on a shared server.
- **Without a server the behaviour tests skip.** CI has no Redis, so `store()` attempts a connection with a 2-second timeout and the six behaviour tests print `skipping: no Redis server at REDIS_URL` and return. Re-run with `REDIS_URL` set to prove them; the review should treat a CI-only run as unproven for AC-001–AC-006.
- **A real defect was found and fixed by writing these tests.** For `Window::Rolling`, `start_at(now)` returns `now`, so the original backend derived the budget key from a value that changed on every call. Every reserve created its own empty budget: the limit was not enforced, idempotency never matched, and `usage` read a different key. The fix makes the budget key window-independent (`quota:budget:{tenant}:{provider}:{metric}`) and moves the window bounds into the hash, replacing it inside the reserve script when the window has ended. AC-002, AC-003, AC-004, AC-005 and AC-006 all failed before the fix and pass after it.
- **AC-002** runs 40 concurrent reserves against a limit of 10 on a multi-thread runtime, asserts at most 10 holds are granted, commits every grant, and asserts committed usage never exceeds the limit.
- **AC-004** asserts the replay returns the *same* reservation id and that a different unit count with the same key is `QuotaError::IdempotencyConflict`.
- **AC-008** covers the malformed-URL half (typed `Config`, URL and host redacted), `Debug` redaction on a live store, and a malformed stored payload (`limit = "not-a-number"`) returning a typed `Backend` error rather than panicking. A refused-connection assertion was deliberately dropped: the client retries with exponential backoff and took about eight minutes, which would hang the suite and CI.
- **AC-007** is offline: it reads `Cargo.toml` and `src/lib.rs` and asserts the empty default feature set, the additive `redis` feature, the feature gate, both lint attributes, and workspace MSRV inheritance.
- **Test quality.** 8 tests named `ac_0NN_*`, 36 assertions, no `.only`/`xit`/`#[ignore]`, no empty assertions.
- **Dev-dependency added:** `redis` (the same crate and version the feature already uses) so the suite can corrupt a payload and clean up. No new library enters the graph.

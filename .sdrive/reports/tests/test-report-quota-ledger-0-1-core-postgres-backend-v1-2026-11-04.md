# Test Report: quota-ledger 0.1 - core reservations and Postgres backend (v1)

## 1. Summary
- **Date:** 2026-11-04
- **Feature:** quota-ledger-0-1-core-postgres-backend
- **Version:** v1
- **Total ACs:** 15
- **Tested ACs:** 15
- **Coverage:** 100%
- **Tests Passed:** 15/15

## 2. AC → Test Mapping

| AC ID | Test ID | File | Status |
|-------|---------|------|--------|
| AC-001 | TEST-001 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-002 | TEST-002 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-003 | TEST-003 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-004 | TEST-004 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-005 | TEST-005 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-006 | TEST-006 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-007 | TEST-007 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-008 | TEST-008 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-009 | TEST-009 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-010 | TEST-010 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-011 | TEST-011 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-012 | TEST-012 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-013 | TEST-013 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-014 | TEST-014 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |
| AC-015 | TEST-015 | crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs | ✅ Pass |

## 3. Failures

| Test | Reason | Blocking |
|------|--------|----------|
| — | None | No |

## 4. Notes

- Framework: Rust's built-in harness (CON-401), run with `node scripts/rust-tests.js`, which executes `cargo test --workspace --all-features`. Workspace run: 234 passed, 0 failed, 2 skipped (236 total), exit code 0. The `quota-ledger-0-1-core-postgres-backend` target contributed 15 of those and finished in 0.24s.
- AC-012 is proven in two halves. The shipped schema is read from disk and asserted to declare `quota_budget`, `quota_reservation`, and `quota_dead_hold`, which holds in every feature configuration. The `QuotaStore` implementation bound on `PostgresQuotaStore` is a compile-time assertion compiled only under `--features postgres`; the workspace run uses `--all-features`, so it is exercised there.
- AC-013 exercises the invariant on `MemoryQuotaStore` (40 tasks racing for a limit of 10, then committing every grant). It proves the ledger never exceeds its limit under concurrent reserve, but it does **not** prove the Postgres conditional `UPDATE` serializes two pods — that needs a live database and is left to a database-backed harness.
- AC-002 is asserted through the feature-independent core surface. The `--no-default-features` build itself is verified by `cargo check -p quota-ledger --no-default-features`, not by this test.
- AC-015 checks the lint attributes, both licence files, the workspace MSRV inheritance, and that no `SELECT`/`INSERT`/`UPDATE`/`DELETE` statement is built with `format!`. The MSRV 1.87 build itself is a CI job, not a unit assertion.
- Machine note: building the `postgres` feature on this host needs `CRATE_CC_NO_DEFAULTS=1` and `CFLAGS_x86_64_pc_windows_msvc=/MD /O2 /DNDEBUG`, because MSVC 14.50.35717 `cl.exe` fails with `D8050` under cc-rs's default `/Z7` when compiling `ring`. That is a local toolchain defect, not a crate defect.
- `chrono` was added to `[dev-dependencies]`: `Window::fixed` and `Window::rolling` take `chrono` types, and integration tests only see their own dependency edge.
- Raw run log: `.sdrive/reports/tests/raw/quota-ledger-0-1-core-postgres-backend-v1-2026-11-04.log`.

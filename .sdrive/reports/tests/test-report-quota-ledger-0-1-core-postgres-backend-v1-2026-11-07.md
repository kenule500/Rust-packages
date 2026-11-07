# Test Report: quota-ledger 0.1 - core reservations and Postgres backend (v1)

## 1. Summary
- **Date:** 2026-11-07
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
- **Why this report exists.** Review round 2 failed because the previous report (2026-11-04) predated the T5.2 plan correction. This run re-executes the suite at the approved plan and re-stamps the report; no test file changed, because every AC already had a passing `ac_0NN_*` test.
- **Plan gate.** The planning gate was re-approved by the human on 2026-11-07 before this run (`specdrive(approve)` `a2033b3`), so this report is written against an approved plan. The reviewed evidence commit is `a2033b3f30a887fadb370062750e4a7eacb1331e`, equal to HEAD.
- **Framework.** Rust's built-in harness (CON-401), run with `node scripts/rust-tests.js`, which executes `cargo test --workspace --all-features` (CON-402). Integration tests live under `crates/quota-ledger/tests/<feature>/main.rs` (CON-403, CON-406). Workspace run: **234 passed, 0 failed, 2 skipped (236 total), exit code 0** — the `quota-ledger-0-1-core-postgres-backend` target contributed all 15 of its tests.
- **Withdrawn criteria.** None. All 15 ACs are in scope and every one is tested.
- **AC-012** is proven in two halves: the shipped `schema.sql` is read from disk and asserted to declare `quota_budget`, `quota_reservation`, and `quota_dead_hold`; the `QuotaStore` implementation bound on `PostgresQuotaStore` is a compile-time assertion compiled under `--features postgres` (the `--all-features` run exercises it).
- **AC-013** exercises the limit invariant on `MemoryQuotaStore` (40 tasks racing for a limit of 10, then settling every grant). It proves the ledger never exceeds its limit under concurrent reserve, but it does **not** prove the Postgres conditional `UPDATE` serializes two pods — that needs a live database, which this environment does not provide. This remains the acknowledged RISK-001 gap.
- **AC-002** is asserted through the feature-independent core surface; the `--no-default-features` build itself is verified by `cargo check -p quota-ledger --no-default-features`, not by a test assertion.
- **AC-015** checks the lint attributes, both licence files, the workspace MSRV inheritance, and that no `SELECT`/`INSERT`/`UPDATE`/`DELETE` statement is built with `format!`. The 1.87 build itself is a CI job; it was also reproduced locally this round with `cargo +1.87.0 build --workspace --all-features` (exit 0).
- **Test quality.** 15 tests named `ac_0NN_*`, 64 assertions, no `.only`/`xit`/`#[ignore]`, no empty assertions, no `todo!`/`unimplemented!`. Red-before-green is not enabled in `config.json`, so `--prove` evidence is not required.
- **Machine note.** Building the `postgres` feature on this host needs `CRATE_CC_NO_DEFAULTS=1`, because MSVC 14.50.35717 `cl.exe` fails with `D8050` under cc-rs's default `/Z7` when compiling `ring`. That is a local toolchain defect, not a crate defect.
- **Raw run log:** `.sdrive/reports/tests/raw/quota-ledger-0-1-core-postgres-backend-v1-2026-11-07.log`.

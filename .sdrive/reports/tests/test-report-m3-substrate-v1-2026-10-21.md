# Test Report: M3 Integration Substrate - platform-core, quota-ledger, hookgate, and syncforge 0.1 (v1)

## 1. Summary
- **Date:** 2026-10-21
- **Feature:** m3-substrate
- **Version:** v1
- **Total ACs:** 53
- **Tested ACs:** 53
- **Coverage:** 100%
- **Tests Passed:** 55/57

## 2. AC → Test Mapping

| AC ID | Test ID | File | Status |
|-------|---------|------|--------|
| AC-001 | TEST-001 | crates/platform-core/tests/m3-substrate/registry.rs | ✅ Pass |
| AC-002 | TEST-002 | crates/platform-core/tests/m3-substrate/registry.rs | ✅ Pass |
| AC-003 | TEST-003 | crates/platform-core/tests/m3-substrate/registry.rs | ✅ Pass |
| AC-004 | TEST-004 | crates/platform-core/tests/m3-substrate/registry.rs | ✅ Pass |
| AC-005 | TEST-005 | crates/platform-core/tests/m3-substrate/registry.rs | ✅ Pass |
| AC-006 | TEST-006 | crates/platform-core/tests/m3-substrate/registry.rs | ✅ Pass |
| AC-007 | TEST-007 | crates/platform-core/tests/m3-substrate/tokens.rs | ✅ Pass |
| AC-008 | TEST-008 | crates/platform-core/tests/m3-substrate/tokens.rs | ✅ Pass |
| AC-009 | TEST-009 | crates/platform-core/tests/m3-substrate/tokens.rs | ✅ Pass |
| AC-010 | TEST-010 | crates/quota-ledger/tests/m3-substrate/ledger.rs | ✅ Pass |
| AC-011 | TEST-011 | crates/quota-ledger/tests/m3-substrate/ledger.rs | ✅ Pass |
| AC-012 | TEST-012 | crates/quota-ledger/tests/m3-substrate/ledger.rs | ✅ Pass |
| AC-013 | TEST-013 | crates/quota-ledger/tests/m3-substrate/ledger.rs | ✅ Pass |
| AC-014 | TEST-014 | crates/quota-ledger/tests/m3-substrate/policies.rs | ✅ Pass |
| AC-015 | TEST-015 | crates/quota-ledger/tests/m3-substrate/policies.rs | ✅ Pass |
| AC-016 | TEST-016 | crates/quota-ledger/tests/m3-substrate/policies.rs | ✅ Pass |
| AC-017 | TEST-017 | crates/quota-ledger/tests/m3-substrate/policies.rs | ✅ Pass |
| AC-018 | TEST-018 | crates/quota-ledger/tests/m3-substrate/postgres.rs | ✅ Pass |
| AC-019 | TEST-019 | crates/quota-ledger/tests/m3-substrate/postgres.rs | ⚠️ Skipped |
| AC-020 | TEST-020 | crates/quota-ledger/tests/m3-substrate/postgres.rs | ✅ Pass |
| AC-021 | TEST-021 | crates/quota-ledger/tests/m3-substrate/postgres.rs | ✅ Pass |
| AC-022 | TEST-022 | crates/hookgate/tests/m3-substrate/verify.rs | ✅ Pass |
| AC-023 | TEST-023 | crates/hookgate/tests/m3-substrate/verify.rs | ✅ Pass |
| AC-024 | TEST-024 | crates/hookgate/tests/m3-substrate/verify.rs | ✅ Pass |
| AC-025 | TEST-025 | crates/hookgate/tests/m3-substrate/verify.rs | ✅ Pass |
| AC-026 | TEST-026 | crates/hookgate/tests/m3-substrate/delivery.rs | ✅ Pass |
| AC-027 | TEST-027 | crates/hookgate/tests/m3-substrate/delivery.rs | ✅ Pass |
| AC-028 | TEST-028 | crates/hookgate/tests/m3-substrate/delivery.rs | ✅ Pass |
| AC-029 | TEST-029 | crates/hookgate/tests/m3-substrate/delivery.rs | ✅ Pass |
| AC-030 | TEST-030 | crates/hookgate/tests/m3-substrate/dead_letter.rs | ✅ Pass |
| AC-031 | TEST-031 | crates/hookgate/tests/m3-substrate/dead_letter.rs | ✅ Pass |
| AC-032 | TEST-032 | crates/syncforge/tests/m3-substrate/storage.rs | ✅ Pass |
| AC-033 | TEST-033 | crates/syncforge/tests/m3-substrate/storage.rs | ✅ Pass |
| AC-034 | TEST-034 | crates/syncforge/tests/m3-substrate/storage.rs | ✅ Pass |
| AC-035 | TEST-035 | crates/syncforge/tests/m3-substrate/storage.rs | ✅ Pass |
| AC-036 | TEST-036 | crates/syncforge/tests/m3-substrate/replay.rs | ✅ Pass |
| AC-037 | TEST-037 | crates/syncforge/tests/m3-substrate/replay.rs | ✅ Pass |
| AC-038 | TEST-038 | crates/syncforge/tests/m3-substrate/replay.rs | ✅ Pass |
| AC-039 | TEST-039 | crates/syncforge/tests/m3-substrate/replay.rs | ✅ Pass |
| AC-040 | TEST-040 | crates/syncforge/tests/m3-substrate/replay.rs | ✅ Pass |
| AC-041 | TEST-041 | crates/syncforge/tests/m3-substrate/conflict.rs | ✅ Pass |
| AC-042 | TEST-042 | crates/syncforge/tests/m3-substrate/conflict.rs | ✅ Pass |
| AC-043 | TEST-043 | crates/syncforge/tests/m3-substrate/conflict.rs | ✅ Pass |
| AC-044 | TEST-044 | crates/syncforge/tests/m3-substrate/conflict.rs | ✅ Pass |
| AC-045 | TEST-045 | crates/hookgate/tests/m3-substrate/compose.rs | ✅ Pass |
| AC-046 | TEST-046 | crates/hookgate/tests/m3-substrate/workspace.rs | ✅ Pass |
| AC-047 | TEST-047 | crates/hookgate/tests/m3-substrate/workspace.rs | ✅ Pass |
| AC-048 | TEST-048 | crates/hookgate/tests/m3-substrate/workspace.rs | ✅ Pass |
| AC-049 | TEST-049 | crates/quota-ledger/tests/m3-substrate/postgres.rs | ✅ Pass |
| AC-050 | TEST-050 | crates/hookgate/tests/m3-substrate/workspace.rs | ✅ Pass |
| AC-051 | TEST-051 | crates/platform-core/tests/m3-substrate/errors.rs<br>crates/quota-ledger/tests/m3-substrate/errors.rs<br>crates/hookgate/tests/m3-substrate/errors.rs<br>crates/syncforge/tests/m3-substrate/errors.rs | ✅ Pass |
| AC-052 | TEST-052 | crates/hookgate/tests/m3-substrate/tracing.rs<br>crates/syncforge/tests/m3-substrate/tracing.rs | ❌ Fail |
| AC-053 | TEST-053 | crates/hookgate/tests/m3-substrate/workspace.rs | ✅ Pass |

## 3. Failures

| Test | Reason | Blocking |
|------|--------|----------|
| TEST-052 (`hookgate::tracing::ac_052_a_webhook_flow_emits_outcome_events`) | `crates/hookgate/src` contains no `tracing` calls, so the webhook half of AC-052 emits no events. The capturing subscriber recorded `[]`. The syncforge replay half of AC-052 passes. | Yes |
| TEST-019 (`quota_ledger::postgres::ac_019_concurrent_postgres_reservations_never_exceed_the_limit`) | Ignored: the test needs a live PostgreSQL instance. Neither `DATABASE_URL` nor `TEST_DATABASE_URL` is set and the Docker daemon is not running. | No (environmental) |

## 4. Notes

- Framework: the Rust built-in test harness, run with `cargo test --workspace --all-features --no-fail-fast` per CON-401 and CON-402. Raw log: `.sdrive/reports/tests/raw/m3-substrate-v1-2026-10-21.log`.
- Feature suite totals: platform-core 10/10 passed; quota-ledger 13/14 (1 ignored); hookgate 17/18 (1 failed); syncforge 15/15. Across the feature that is 55 passed, 1 failed, and 1 ignored out of 57 tests. The pre-existing dado `m0-foundation` suite in the same workspace run reports 36 passed and 2 ignored and is not part of this feature's totals.
- AC-051 has one test per crate (4 tests total). AC-052 has two tests: the syncforge replay test passes, the hookgate webhook test fails.
- AC-052 is a production gap, not a test defect. The test phase forbids production changes, so this run stops and reports it. Recommended follow-up: `/sdrive:apply` to add hookgate tracing for verification and delivery outcomes, then rerun the suite. `/sdrive:review` follows after that.
- AC-019 is written and compiled but marked `#[ignore]` because it needs a live PostgreSQL instance. It remains in the suite as the regression test for the Postgres concurrency guarantee.
- Build-matrix evidence, all exit code 0:
  - `cargo build --workspace --all-features`
  - `cargo build -p <crate> --no-default-features` for platform-core, quota-ledger, hookgate, and syncforge.
  - `RUSTDOCFLAGS=-D warnings cargo doc --workspace --all-features --no-deps`
  - `cargo package --list --allow-dirty -p <crate>` for all four crates. Each package lists `README.md`, `LICENSE-MIT`, `LICENSE-APACHE`, `Cargo.toml`, and `src/**`; hookgate also lists `examples/substrate_integration.rs`.
  - `cargo run -p hookgate --example substrate_integration` registers one provider, reserves and commits one quota unit (1 used, 0 held, 2 remaining), verifies `evt_example_1`, inserts the delivery, and enqueues the outbox message.
- AC-046, AC-047, AC-048, AC-050, and AC-053 assert manifest and source invariants inside the suite; the cargo commands above are recorded as external evidence for them.
- AC-053 used `cargo package --list --allow-dirty` because the test files were uncommitted at run time.
- No production code was modified. The only manifest change is the `tokio` dev-dependency in `crates/platform-core/Cargo.toml`, needed to drive the async token-store trait in tests.

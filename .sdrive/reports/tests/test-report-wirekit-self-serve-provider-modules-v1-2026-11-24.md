# Test Report: wirekit - self-serve provider modules (v1)

## 1. Summary
- **Date:** 2026-11-24
- **Feature:** wirekit-self-serve-provider-modules
- **Version:** v1
- **Total ACs:** 7
- **Tested ACs:** 7
- **Coverage:** 100%
- **Tests Passed:** 37/37

## 2. AC → Test Mapping
| AC ID | Test ID | File | Status |
|-------|---------|------|--------|
| AC-001 | TEST-001 | crates/wirekit/tests/wirekit-self-serve-provider-modules/accuweather.rs | ✅ Pass |
| AC-002 | TEST-002 | crates/wirekit/tests/wirekit-self-serve-provider-modules/accuweather.rs | ✅ Pass |
| AC-003 | TEST-003 | crates/wirekit/tests/wirekit-self-serve-provider-modules/accuweather.rs | ✅ Pass |
| AC-004 | TEST-004 | crates/wirekit/tests/wirekit-self-serve-provider-modules/accuweather.rs | ✅ Pass |
| AC-005 | TEST-005 | crates/wirekit/tests/wirekit-self-serve-provider-modules/accuweather.rs | ✅ Pass |
| AC-001 | TEST-006 | crates/wirekit/tests/wirekit-self-serve-provider-modules/contract.rs | ✅ Pass |
| AC-002 | TEST-007 | crates/wirekit/tests/wirekit-self-serve-provider-modules/contract.rs | ✅ Pass |
| AC-004 | TEST-008 | crates/wirekit/tests/wirekit-self-serve-provider-modules/contract.rs | ✅ Pass |
| AC-005 | TEST-009 | crates/wirekit/tests/wirekit-self-serve-provider-modules/contract.rs | ✅ Pass |
| AC-006 | TEST-010 | crates/wirekit/tests/wirekit-self-serve-provider-modules/contract.rs | ✅ Pass |
| AC-007 | TEST-011 | crates/wirekit/tests/wirekit-self-serve-provider-modules/contract.rs | ✅ Pass |
| AC-001 | TEST-012 | crates/wirekit/tests/wirekit-self-serve-provider-modules/deepgram.rs | ✅ Pass |
| AC-002 | TEST-013 | crates/wirekit/tests/wirekit-self-serve-provider-modules/deepgram.rs | ✅ Pass |
| AC-003 | TEST-014 | crates/wirekit/tests/wirekit-self-serve-provider-modules/deepgram.rs | ✅ Pass |
| AC-004 | TEST-015 | crates/wirekit/tests/wirekit-self-serve-provider-modules/deepgram.rs | ✅ Pass |
| AC-004 | TEST-016 | crates/wirekit/tests/wirekit-self-serve-provider-modules/deepgram.rs | ✅ Pass |
| AC-005 | TEST-017 | crates/wirekit/tests/wirekit-self-serve-provider-modules/deepgram.rs | ✅ Pass |
| AC-001 | TEST-018 | crates/wirekit/tests/wirekit-self-serve-provider-modules/klarna.rs | ✅ Pass |
| AC-002 | TEST-019 | crates/wirekit/tests/wirekit-self-serve-provider-modules/klarna.rs | ✅ Pass |
| AC-003 | TEST-020 | crates/wirekit/tests/wirekit-self-serve-provider-modules/klarna.rs | ✅ Pass |
| AC-004 | TEST-021 | crates/wirekit/tests/wirekit-self-serve-provider-modules/klarna.rs | ✅ Pass |
| AC-005 | TEST-022 | crates/wirekit/tests/wirekit-self-serve-provider-modules/klarna.rs | ✅ Pass |
| AC-001 | TEST-023 | crates/wirekit/tests/wirekit-self-serve-provider-modules/lob.rs | ✅ Pass |
| AC-002 | TEST-024 | crates/wirekit/tests/wirekit-self-serve-provider-modules/lob.rs | ✅ Pass |
| AC-003 | TEST-025 | crates/wirekit/tests/wirekit-self-serve-provider-modules/lob.rs | ✅ Pass |
| AC-004 | TEST-026 | crates/wirekit/tests/wirekit-self-serve-provider-modules/lob.rs | ✅ Pass |
| AC-005 | TEST-027 | crates/wirekit/tests/wirekit-self-serve-provider-modules/lob.rs | ✅ Pass |
| AC-001 | TEST-028 | crates/wirekit/tests/wirekit-self-serve-provider-modules/pagerduty.rs | ✅ Pass |
| AC-002 | TEST-029 | crates/wirekit/tests/wirekit-self-serve-provider-modules/pagerduty.rs | ✅ Pass |
| AC-003 | TEST-030 | crates/wirekit/tests/wirekit-self-serve-provider-modules/pagerduty.rs | ✅ Pass |
| AC-004 | TEST-031 | crates/wirekit/tests/wirekit-self-serve-provider-modules/pagerduty.rs | ✅ Pass |
| AC-005 | TEST-032 | crates/wirekit/tests/wirekit-self-serve-provider-modules/pagerduty.rs | ✅ Pass |
| AC-001 | TEST-033 | crates/wirekit/tests/wirekit-self-serve-provider-modules/yelp.rs | ✅ Pass |
| AC-002 | TEST-034 | crates/wirekit/tests/wirekit-self-serve-provider-modules/yelp.rs | ✅ Pass |
| AC-003 | TEST-035 | crates/wirekit/tests/wirekit-self-serve-provider-modules/yelp.rs | ✅ Pass |
| AC-004 | TEST-036 | crates/wirekit/tests/wirekit-self-serve-provider-modules/yelp.rs | ✅ Pass |
| AC-005 | TEST-037 | crates/wirekit/tests/wirekit-self-serve-provider-modules/yelp.rs | ✅ Pass |

## 3. Failures
| Test | Reason | Blocking |
|------|--------|----------|
| — | No failures in this run | No |

## 4. Notes
- Harness: Rust built-in harness (`cargo test`, CON-401). The acceptance target is `wirekit-self-serve-provider-modules`, declared in `crates/wirekit/Cargo.toml` with `required-features = ["accuweather", "lob", "pagerduty", "deepgram", "yelp", "klarna"]`.
- Command: `cargo test -p wirekit --test wirekit-self-serve-provider-modules --features 'accuweather,lob,pagerduty,deepgram,yelp,klarna'` — exit code 0, 37 passed, 0 failed.
- Every module is exercised against a loopback `wiremock` server. The earlier PagerDuty/Klarna coverage gap is closed: commit `18dea52` added `PagerDutyConfig::with_events_base_url` / `with_rest_base_url` and `KlarnaConfig::with_base_url` (defaults unchanged), so all six modules — including their HTTP request and response paths — are mock-tested with no sandbox credential.
- AC-006 and AC-007 are crate-level: the tests read `Cargo.toml`, `src/lib.rs`, and the six module sources. Build-level evidence was captured alongside the suite and is green: `cargo build -p wirekit --all-features` (exit 0) and `cargo test -p wirekit --all-features --doc` (exit 0).
- PagerDuty AC-003 pins the Events API `status: "invalid event"` envelope mapping to `invalid_event`; that mapping lives in `crates/wirekit/src/pagerduty.rs` and is an implementation change, not a test change.
- Line-ending portability (review finding F-001): the file readers in `contract.rs` now normalise CRLF to LF before the assertions, because AC-006 and AC-007 compare against `\n`-joined literals. Verified both ways: the AC-006 test passes with LF sources and again with `src/lib.rs` and `Cargo.toml` temporarily converted to CRLF (conversion reverted before this commit), and the full suite is 37/37 either way. Before the fix, Windows CI failed at `contract.rs:228` in run 45415969525.

# Test Report: hookgate 0.1 - verifiers, dedupe, replay, outbox (v1)

## 1. Summary
- **Date:** 2026-11-08
- **Feature:** hookgate-0-1-verifiers-dedupe-replay-outbox
- **Version:** v1
- **Total ACs:** 14
- **Tested ACs:** 14
- **Coverage:** 100%
- **Tests Passed:** 14/14

## 2. AC → Test Mapping
| AC ID | Test ID | File | Status |
|-------|---------|------|--------|
| AC-001 | TEST-001 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-002 | TEST-002 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-003 | TEST-003 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-004 | TEST-004 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-005 | TEST-005 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-006 | TEST-006 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-007 | TEST-007 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-008 | TEST-008 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-009 | TEST-009 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-010 | TEST-010 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-011 | TEST-011 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-012 | TEST-012 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-013 | TEST-013 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |
| AC-014 | TEST-014 | crates/hookgate/tests/hookgate-0-1-verifiers-dedupe-replay-outbox/main.rs | ✅ Pass |

## 3. Failures
| Test | Reason | Blocking |
|------|--------|----------|
| — | None | No |

## 4. Notes

- **Method.** Every signature is computed inside the test over the exact bytes the request carries, using the same `hmac`/`sha1`/`sha2`/`base64`/`hex` primitives the crate uses. A verifier that parsed and re-serialized the body first would fail AC-001 and AC-003 rather than pass them.
- **Raw-body rule (AC-001, EC-001).** The test signs the received bytes and asserts acceptance, then signs the compact re-serialization of the same document and asserts the raw request is rejected. That is the behaviour EC-001 names.
- **Presets (AC-010).** All six providers plus the generic verifier are exercised end to end: Meta (`sha256=` hex), Stripe (`t`/`v1` with a fresh timestamp), Shopify (base64 plus event headers), Twilio (HMAC-SHA1 over the URL and sorted form fields), Slack (`v0=` over `v0:{timestamp}:{body}`), GitHub (hex with delivery headers), and a custom-header generic HMAC.
- **Feature gating (AC-011).** A runtime test cannot assert that a module is absent, so the test proves the seven verifiers exist and that `lib.rs` gates the `presets` re-export behind `#[cfg(feature = "presets")]`. Absence in the core-only build is proven by `cargo check -p hookgate --no-default-features`, which is also a task check in T4.1.
- **Store tests (AC-005 to AC-009, AC-012)** drive `MemoryDeliveryStore` through the `DeliveryStore` and `Outbox` traits: duplicate insert leaves the outbox at one row, tenant isolation covers both `get` and `dead_letters`, and dead-letter capture keeps the raw body and the redacted reason.
- **Redaction (AC-013).** `redact_error` masks `secret=`, `token=`, and `signature=` values, and `Debug` for a preset renders `[REDACTED]` instead of the shared secret.
- **Hygiene (AC-014).** The test reads `src/lib.rs` and `Cargo.toml` from `CARGO_MANIFEST_DIR` and asserts the lint attributes, both licence files, and the workspace-inherited edition and MSRV.
- **Test quality.** 14 tests named `ac_0NN_*`, 78 assertions, no `.only`/`xit`/`#[ignore]`, no empty assertions. Red-before-green is not enabled in `config.json`.
- **Dev-dependencies added for the suite:** `hmac`, `sha1`, `sha2`, `base64`, `hex`, `http` — all already crate dependencies, so no new library enters the graph. `tokio` was already a dev-dependency.
- **Machine note.** The `postgres` feature of the sibling `quota-ledger` crate needs `CRATE_CC_NO_DEFAULTS=1` on this host (MSVC `cl.exe` D8050); hookgate itself has no C-compiled dependency.

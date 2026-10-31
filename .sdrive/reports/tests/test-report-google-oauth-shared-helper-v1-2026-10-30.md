# Test Report: Google OAuth shared helper (v1)

## 1. Summary
- **Date:** 2026-10-30
- **Feature:** google-oauth-shared-helper
- **Version:** v1
- **Total ACs:** 9
- **Tested ACs:** 9
- **Coverage:** 100%
- **Tests Passed:** 10/10

## 2. AC → Test Mapping
| AC ID | Test ID | File | Status |
|-------|---------|------|--------|
| AC-001 | TEST-001 | crates/wirekit/tests/google-oauth-shared-helper/main.rs | ✅ Pass |
| AC-002 | TEST-002 | crates/wirekit/tests/google-oauth-shared-helper/main.rs | ✅ Pass |
| AC-003 | TEST-003 | crates/wirekit/tests/google-oauth-shared-helper/main.rs | ✅ Pass |
| AC-004 | TEST-004 | crates/wirekit/tests/google-oauth-shared-helper/main.rs | ✅ Pass |
| AC-005 | TEST-005 | crates/wirekit/tests/google-oauth-shared-helper/main.rs | ✅ Pass |
| AC-006 | TEST-006 | crates/wirekit/tests/google-oauth-shared-helper/main.rs | ✅ Pass |
| AC-007 | TEST-007 | crates/wirekit/tests/google-oauth-shared-helper/main.rs | ✅ Pass |
| AC-008 | TEST-008 | crates/wirekit/tests/google-oauth-shared-helper/main.rs | ✅ Pass |
| AC-009 | TEST-009 | crates/wirekit/tests/google-oauth-shared-helper/main.rs | ✅ Pass |

TEST-001 `ac_001_the_authorization_url_carries_state_and_an_s256_challenge`,
TEST-002 `ac_002_each_request_uses_a_fresh_state_and_verifier`,
TEST-003 `ac_003_no_request_can_omit_the_pkce_challenge`,
TEST-004 `ac_004_exchange_code_returns_a_typed_token_with_absolute_expiry`,
TEST-005 `ac_005_a_provider_error_becomes_an_auth_error_with_its_code`,
TEST-006 `ac_006_an_expired_token_is_refreshed_once_through_the_store` and
`ac_006_a_token_without_a_refresh_token_fails_rather_than_looping`,
TEST-007 `ac_007_revoke_calls_the_endpoint_once_and_clears_the_store`,
TEST-008 `ac_008_no_secret_renders_in_debug_or_display`,
TEST-009 `ac_009_the_module_performs_no_io_of_its_own`.

## 3. Failures
| Test | Reason | Blocking |
|------|--------|----------|
| — | None in the recorded run. | — |

One failure was found and resolved before the recorded run: `TEST-008` failed on
`!rendered.contains(request.state().expose())` because `AuthorizationRequest`'s
`Debug` printed the authorization URL verbatim, and that URL carries the `state`
query parameter. Production fixed it in `specdrive(apply): mask the OAuth state
in AuthorizationRequest Debug`; no assertion was weakened.

## 4. Notes
- Framework: Rust built-in harness (CON-401). Command: `cargo test --workspace
  --all-features` from the repository root (CON-402). Test layout follows
  CON-406: `crates/wirekit/tests/google-oauth-shared-helper/`.
- Full workspace result: 156 passed, 0 failed, 2 ignored (158 total); exit code 0.
  The 10 feature tests all pass; the two ignored tests belong to other modules.
- Every test is local: pure computation or a `wiremock` server on loopback. No
  live Google account and no credentials are used.
- Harness repairs made by this command, both test-side only:
  1. Removed an unbalanced `}` that stopped the test target from compiling.
  2. Added `.allow_insecure_localhost(true)` to the shared client the tests
     build, because `WirekitClient` rejects plain-HTTP loopback URLs that the
     mock server needs.
- `.sdrive/config.json` sets `test.command` to `node .sdrive/scripts/rust-tests.js`,
  which does not exist in this repository (`run-tests.js` is the Node runner).
  CON-402 was used instead; the stale config entry should be corrected.
- The recorded run used the working tree that contains both the production fix
  (`1f80712`, `AuthorizationRequest` `Debug` masking `state`) and the two
  test-harness repairs above. `test-evidence.json` names `b57ed32`, the commit
  that holds that production code and this test target; the run also included
  rustfmt-only reformatting, which changes no behaviour.
- `cargo fmt --all --check` passes. The only blocker was the test target's
  unbalanced brace; it was removed, `cargo fmt --all` applied, and the ten tests
  re-ran green afterwards. `rustfmt --check` also passed on `src/google_oauth.rs`
  and `src/google_lsa.rs` during apply.
- `sdrive verify` returns **WARN**: 38 matched, 0 missing, 8 extra. The earlier
  `missing` was a plan naming defect — `plan.json` listed both crates in one
  entry, `serde and serde_json` — and has been split into two entries. The 8
  extras are other features' modules that the crate-wide scan sees and this plan
  does not own.

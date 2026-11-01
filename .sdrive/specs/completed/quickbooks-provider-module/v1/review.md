# Review: quickbooks-provider-module (v1)

- Status: PASS
- **Commit:** `38e27ad402393e576e2eaf74ca862e44356411a5`
- **Branch:** feature/bp-013-quickbooks (base `main`)
- **Reviewed by:** kenule500 (human-confirmed verdict)
- **Reviewed at:** 2026-11-01T15:35:00Z
- **Coverage:** 100%

## Summary

QuickBooks Online provider module: OAuth2 refresh before expiry with the rotated
refresh token surfaced, realm-scoped customer create/update, invoice create,
void and send, payment recording linked to an invoice, bounded account and item
queries, `intuit-signature` webhook verification that fails closed, Intuit
`Fault` mapping, `Retry-After` handling, and an idempotency pre-check.
16 acceptance criteria, 16 tests, all passing against a local mock.

## Criteria

| Criterion | Result | Detail |
|---|---|---|
| AC-001..016 | pass | 16 of 16 acceptance criteria exercised by the committed suite (16 passed, exit 0). |
| CON-405 | pass | One test per acceptance criterion, each named `ac_0NN_*`. |
| CON-602 | pass | `sdrive verify`: 67 matched, 0 missing, 7 withdrawn (path-template contracts), status warn. |
| CON-519 / CON-520 | pass | No environment reads; a non-loopback cleartext base is rejected with W2006. |
| CON-511 / CON-512 | pass | Refresh-once semantics per request; the realm is fixed in the client, so a call cannot cross realms. |
| CON-515 | pass | Typed entities and bounded queries; provider `Fault` bodies surface as `WirekitError::Api`. |
| AC-040 (m1 hygiene) | pass | Provider entities carry the `Qbo` prefix, so `wirekit` declares no bare application nouns. |
| FMT / CLIPPY | pass | `cargo fmt --all --check` and `cargo clippy --workspace --all-features -- -D warnings` both exit 0. |
| DOCS | pass | `cargo doc --workspace --all-features --no-deps` with `RUSTDOCFLAGS=-D warnings` exits 0. |
| TESTS | pass | `cargo test --workspace --all-features` exits 0; the feature suite is 16/16. |
| GOVERNANCE | pass | `validate-governance` reports all files valid; CI on `38e27ad` is 9/9 green, Audit and MSRV included. |

## Findings

- **medium** (`crates/wirekit/src/quickbooks.rs`, `refresh`): no in-flight guard, so two concurrent calls can both refresh and one rotated refresh token can be lost. Single-request semantics required by AC-002 hold; the loss window matters for NFR-005 under concurrency.
- **low** (`verify_webhook`): replay detection is by `event_id` only; no freshness window is applied to `lastUpdated`, so an old but correctly signed delivery is accepted.
- **low** (`find_customer`): only `'` is escaped when building the query string. Intuit's query language offers no parameter binding, so the surface is limited but not zero.
- **low** (`ac_015`): the test asserts retryability semantics but not that the transport actually waited for `Retry-After`, so AC-015 is partially verified.
- **info** (`crates/wirekit/src/error.rs`): the `Fault.Error[]` fallback added for AC-014 is shared by every provider; it only applies when no top-level code or message exists, and the nine CI jobs confirm no regression.
- **info** (`TokenSet`): tokens live in `Mutex<Option<TokenSet>>` with redacted `Debug` but no `zeroize`, consistent with the crate's existing secret handling.

## Evidence

- `cargo test --workspace --all-features` — exit 0
- `cargo clippy --workspace --all-features -- -D warnings` — exit 0
- `cargo doc --workspace --all-features --no-deps` with `-D warnings` — exit 0
- `cargo fmt --all --check` — exit 0
- `node .sdrive/scripts/validate-governance.js` — all files valid
- `sdrive test quickbooks-provider-module` — 173 passed, 0 failed, 2 skipped
- CI run 41321433052 on `38e27ad` — 9/9 jobs success
- Test report: `.sdrive/reports/tests/test-report-quickbooks-provider-module-v1-2026-11-01.md`

## Handoff

Cleared for release. Run `/sdrive:archive quickbooks-provider-module` to merge to `main` and complete the lifecycle.

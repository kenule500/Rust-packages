# Review: eagleview-provider-module (v1)

- Status: PASS
- **Commit:** `3deba127ef7a74d6cc6bea0de46e8c4247f41a71`
- **Branch:** feature/bp-012-eagleview (base `main`)
- **Reviewed by:** kenule500 (human-confirmed verdict)
- **Reviewed at:** 2026-11-01T16:40:00Z
- **Coverage:** 100%

## Summary

EagleView satellite roof measurement: client-credentials tokens with a cached
access token, an idempotent **paid** order that refuses to call the provider
without an explicit cost acknowledgement, a typed asynchronous order lifecycle,
report fetch, a content-type-validated and size-bounded PDF download, and
address-scoped report listing. 13 acceptance criteria, 13 tests, all passing
against a local mock.

## Criteria

| Criterion | Result | Detail |
|---|---|---|
| AC-001..013 | pass | 13 of 13 acceptance criteria exercised by the committed suite (13 passed, exit 0). |
| CON-405 | pass | One test per acceptance criterion, each named `ac_0NN_*`. |
| CON-602 | pass | `sdrive verify`: 50 matched, 0 missing, 6 withdrawn (path-template contracts), status warn. |
| CON-511 / CON-519 | pass | Client secret and access token live in `Secret`, render `***`, and no environment is read. |
| CON-515 / NFR-002 | pass | Responses are typed before use and the PDF body is bounded before it is returned. |
| CON-520 | pass | A non-loopback cleartext base URL is refused at construction (`W2006`). |
| AC-040 (m1 hygiene) | pass | Provider schema names only; no application-domain noun is declared in `wirekit`. |
| AC-012 | warn | The suite asserts retryability semantics, not that the transport waited for `Retry-After`. |
| FMT / CLIPPY / DOCS | pass | fmt, clippy `--workspace --all-features -D warnings`, and rustdoc `-D warnings` all exit 0. |
| TESTS | pass | `cargo test --workspace --all-features` exits 0; the feature suite is 13/13. |
| GOVERNANCE | pass | All governance files valid; CI on `3deba12` is 9/9 green. |

## Findings

- **medium** (`crates/wirekit/src/eagleview.rs`, order ledger): idempotency is process-local. The replay map lives in the client instance, so a restart or a second client loses it and a cross-process retry depends on the provider honouring the `idempotency-key` header — which RISK-001 leaves `[to be confirmed]`.
- **low** (`MeasurementOrder.cost_acknowledged`): the flag is set locally when the order is created; `order_status` on a fresh client reports `false` for the same paid order, which reads as if the cost were unacknowledged.
- **low** (`MeasurementRequest::address_fingerprint`): trimming and lowercasing only, so "1 Main St" and "1 Main Street" fingerprint differently and would place a second order under the same key.
- **low** (`download_pdf`): `MAX_PDF_BYTES` is checked after buffering, so the effective ceiling is the transport's `max_response_bytes`; the module's check is a second line rather than the primary bound.
- **info** (polling): RISK-002's mitigation says "poll with bounded backoff", but v1 ships `order_status` and leaves the loop to the caller, as the plan records.
- **info** (idempotency header): the caller key travels as `idempotency-key`; whether the provider honours it is unconfirmed, so the ledger is what the module actually guarantees today.

## Evidence

- `cargo test --workspace --all-features` — exit 0
- `cargo clippy --workspace --all-features -- -D warnings` — exit 0
- `cargo doc --workspace --all-features --no-deps` with `-D warnings` — exit 0
- `cargo fmt --all --check` — exit 0
- `node .sdrive/scripts/validate-governance.js` — all files valid
- `sdrive test eagleview-provider-module` — evidence recorded, feature suite 13/13
- CI runs 41352136095 (`860995a`) and 41352877218 (`3deba12`) — success
- Test report: `.sdrive/reports/tests/test-report-eagleview-provider-module-v1-2026-11-01.md`

## Handoff

Cleared for release. Run `/sdrive:archive eagleview-provider-module` to merge to `main` and complete the lifecycle.

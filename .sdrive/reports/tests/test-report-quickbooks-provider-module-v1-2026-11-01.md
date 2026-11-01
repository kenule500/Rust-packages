# Test Report: quickbooks-provider-module (v1)

- **Feature:** quickbooks-provider-module
- **Version:** v1
- **Date:** 2026-11-01
- **Branch:** feature/bp-013-quickbooks
- **Suite:** `cargo test -p wirekit --features quickbooks --test quickbooks-provider-module`

## Summary

- **Total ACs:** 16
- **Tested ACs:** 16
- **Coverage:** 100%
- **Tests Passed:** 16/16

## Results

| AC | Test | Result |
|---|---|---|
| AC-001 | `ac_001_credentials_are_secret_and_debug_renders_no_plaintext` | pass |
| AC-002 | `ac_002_an_expired_token_refreshes_once_and_the_call_retries` | pass |
| AC-003 | `ac_003_the_rotated_refresh_token_is_surfaced_and_a_rejection_does_not_loop` | pass |
| AC-004 | `ac_004_create_customer_posts_the_realm_resource` | pass |
| AC-005 | `ac_005_update_customer_carries_id_and_sync_token` | pass |
| AC-006 | `ac_006_create_invoice_carries_ordered_lines` | pass |
| AC-007 | `ac_007_void_invoice_uses_the_void_operation` | pass |
| AC-008 | `ac_008_send_invoice_targets_the_send_operation` | pass |
| AC-009 | `ac_009_record_payment_links_the_invoice` | pass |
| AC-010 | `ac_010_an_over_balance_payment_surfaces_the_provider_fault` | pass |
| AC-011 | `ac_011_reference_data_uses_the_query_endpoint_with_a_bound` | pass |
| AC-012 | `ac_012_a_correct_signature_parses_the_notification` | pass |
| AC-013 | `ac_013_a_wrong_signature_fails_closed_and_a_replay_shares_the_event_id` | pass |
| AC-014 | `ac_014_a_fault_body_exposes_every_intuit_code_and_message` | pass |
| AC-015 | `ac_015_a_rate_limit_is_retryable_and_a_validation_fault_is_not` | pass |
| AC-016 | `ac_016_an_idempotency_key_returns_the_existing_customer` | pass |

Run: `16 passed; 0 failed; 0 ignored`, exit code 0. Workspace suite at the same
commit: `cargo test --workspace --all-features` exit code 0; clippy
`--workspace --all-features -- -D warnings` exit code 0.

All tests run against a local mock, so no Intuit app approval was required and
RISK-001 stays `[to be confirmed]`.

## Defects found

- **Fixed (AC-014):** `WirekitError::api_from_body` did not unwrap Intuit's
  `Fault.Error[]`, so a provider rejection surfaced as `http_400` /
  "provider reported a failure". The mapper now falls back to
  `Fault.Error[0].code` and `Fault.Error[0].Message`, preserving the Intuit code
  and message as the criterion requires. Approved by kenule500 before the change.
- **Fixed (AC-040 regression):** the module originally declared `struct Customer`,
  `struct Invoice`, and `struct Account` inside `wirekit`, which the archived
  `m1-first-publish` hygiene criterion forbids. The provider entities are now
  `QboCustomer`, `QboInvoice`, and `QboAccount`; the Intuit wire shape is
  unchanged because serde keeps the `PascalCase` names.
- **Test defects:** the void/send fixtures omitted `CustomerRef` (required by
  `QboInvoice`), and the send fixture used `EmailStatus` where the approved data
  model maps `Status`. Both were fixture corrections, not production changes.

## Withdrawn criteria

None.

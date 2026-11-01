# Test Report: eagleview-provider-module (v1)

- **Feature:** eagleview-provider-module
- **Version:** v1
- **Date:** 2026-11-01
- **Branch:** feature/bp-012-eagleview
- **Suite:** `cargo test -p wirekit --features eagleview --test eagleview-provider-module`

## Summary

- **Total ACs:** 13
- **Tested ACs:** 13
- **Coverage:** 100%
- **Tests Passed:** 13/13

## Results

| AC | Test | Result |
|---|---|---|
| AC-001 | `ac_001_credentials_are_secret_and_debug_renders_no_plaintext` | pass |
| AC-002 | `ac_002_a_cached_token_is_used_without_another_token_call` | pass |
| AC-003 | `ac_003_an_order_posts_the_request_and_returns_its_id_and_status` | pass |
| AC-004 | `ac_004_a_repeated_key_returns_the_order_without_a_second_charge` | pass |
| AC-005 | `ac_005_no_order_without_an_explicit_cost_acknowledgement` | pass |
| AC-006 | `ac_006_order_status_maps_into_the_typed_state` | pass |
| AC-007 | `ac_007_a_failed_order_surfaces_its_reason_and_is_not_retried` | pass |
| AC-008 | `ac_008_the_report_carries_area_pitch_and_ordered_segments` | pass |
| AC-009 | `ac_009_the_pdf_download_validates_the_content_type` | pass |
| AC-010 | `ac_010_reports_are_listed_for_an_address_with_a_bound` | pass |
| AC-011 | `ac_011_rejection_unmeasurable_and_processing_failure_stay_distinct` | pass |
| AC-012 | `ac_012_a_rate_limit_is_retryable_and_a_rejection_is_not` | pass |
| AC-013 | `ac_013_a_cleartext_base_url_is_rejected` | pass |

Run: `13 passed; 0 failed; 0 ignored`, exit code 0.

All tests run against a local mock, so no EagleView commercial contract was
needed and RISK-001 stays `[to be confirmed]`.

## Defects found

- **Method defect (not production):** the module needs no production change. The
  suite was written with `wiremock` response mocks only; the two guard rules
  (cost acknowledgement, idempotency replay) are asserted with `.expect(0)` and
  `.expect(1)` counts, which is what makes "no second charge" verifiable rather
  than assumed.
- **Fixture notes:** the order and report fixtures use the provider's camelCase
  field names, and `SegmentKind` is asserted through its `SCREAMING_SNAKE_CASE`
  wire values (`RIDGE`, `VALLEY`), so a silent serde rename would fail the suite.

## Notable coverage

- AC-004 asserts **exactly one** `POST /v1/orders` across two calls with the same
  key, and that reusing the key for a different address is refused (`EC-004`).
- AC-005 asserts **zero** requests of any kind (orders and token) when the cost is
  not acknowledged — the guard is local, not a provider round trip.
- AC-011 asserts three distinct `api_code()` values, so the caller can tell a
  rejection from an unmeasurable address from a processing failure without
  parsing messages.

## Withdrawn criteria

None.

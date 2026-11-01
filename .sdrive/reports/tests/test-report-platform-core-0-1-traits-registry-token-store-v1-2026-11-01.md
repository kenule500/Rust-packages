# Test Report: platform-core-0-1-traits-registry-token-store (v1)

- **Feature:** platform-core-0-1-traits-registry-token-store
- **Version:** v1
- **Date:** 2026-11-01
- **Branch:** feature/meta-stripe-platform-core
- **Suite:** `cargo test -p platform-core --all-features --test platform-core-0-1-traits-registry-token-store`

## Summary

- **Total ACs:** 13
- **Tested ACs:** 13
- **Coverage:** 100%
- **Tests Passed:** 13/13

## Results

| AC | Test | Result |
|---|---|---|
| AC-001 | `ac_001_the_identity_and_token_surface_exists_without_the_registry` | pass |
| AC-002 | `ac_002_tenant_and_provider_ids_are_cheap_value_types` | pass |
| AC-003 | `ac_003_provisioning_modes_are_distinct` | pass |
| AC-004 | `ac_004_a_capability_pairs_an_opaque_key_with_its_mode` | pass |
| AC-005 | `ac_005_capability_keys_are_opaque` | pass |
| AC-006 | `ac_006_a_registered_provider_is_visible_through_the_registry` | pass |
| AC-007 | `ac_007_a_duplicate_provider_id_is_rejected` | pass |
| AC-008 | `ac_008_registration_is_explicit_and_there_is_no_global_registry` | pass |
| AC-009 | `ac_009_the_token_store_is_keyed_by_tenant_and_provider` | pass |
| AC-010 | `ac_010_the_memory_store_is_gated_and_tracks_its_size` | pass |
| AC-011 | `ac_011_a_secret_never_renders_its_value` | pass |
| AC-012 | `ac_012_an_absent_token_is_none_rather_than_an_error` | pass |
| AC-013 | `ac_013_the_crate_declares_its_lints_and_ships_its_licences` | pass |

Run: `13 passed; 0 failed; 0 ignored`, exit code 0.

All tests run against the in-process registry and `MemoryTokenStore`, so no
external provider account was needed and RISK-001 stays `[to be confirmed]`.

## Defects found

- **Test defect (AC-011):** the suite asserted the `***` redaction marker, but
  `Secret` renders `Secret([REDACTED])` in both `Debug` and `Display`. The test
  now asserts the marker the crate actually emits, in both traits. No production
  change was required.

## Withdrawn criteria

None.

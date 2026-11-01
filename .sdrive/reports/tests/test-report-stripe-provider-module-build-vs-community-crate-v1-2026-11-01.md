# Test Report: stripe-provider-module-build-vs-community-crate (v1)

- **Feature:** stripe-provider-module-build-vs-community-crate
- **Version:** v1
- **Date:** 2026-11-01
- **Branch:** feature/meta-stripe-platform-core
- **Suite:** `cargo test -p wirekit --test stripe-provider-module-build-vs-community-crate`

## Summary

- **Total ACs:** 5
- **Tested ACs:** 5
- **Coverage:** 100%
- **Tests Passed:** 5/5

## Results

| AC | Test | Result |
|---|---|---|
| AC-001 | `ac_001_both_options_are_scored_against_the_criteria` | pass |
| AC-002 | `ac_002_the_chosen_option_and_approver_are_named` | pass |
| AC-003 | `ac_003_the_rejected_build_option_records_the_design_file_rule` | pass |
| AC-004 | `ac_004_the_community_crate_is_pinned_with_licence_and_msrv` | pass |
| AC-005 | `ac_005_the_revisit_triggers_are_explicit` | pass |

Run: `5 passed; 0 failed; 0 ignored`, exit code 0. These are repository-level
assertions because the feature is a decision record: no runtime behaviour exists
to exercise, and the tests prove the decision is recorded, evidenced, published
where a consumer will look, and that no `wirekit` Stripe module was added.

## Defects found

- Test-side only (no production change): the spec path was resolved against the
  wrong root, two tests read the root README instead of
  `crates/wirekit/README.md`, and AC-005 grepped for the heading word "Revisit",
  which the spec states as the triggers themselves in AC-005 while the README
  carries the "Revisit when..." sentence.

## Withdrawn criteria

None. AC-003 is retained even though the build option was not chosen: it records
that a build would have required a `catalogue/38_stripe.md` design file under
CON-105.

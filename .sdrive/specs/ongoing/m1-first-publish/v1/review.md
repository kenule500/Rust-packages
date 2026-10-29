---
title: M1 First publish - review
status: PASS
version: v1
created: 2026-10-29
updated: 2026-10-29
---

# M1 First publish - Review

**Status: PASS**
**Commit:** `924ce720a5b4fa187b1999757cf60cdd169760bd`
**Branch:** `feature/m1-first-publish`
**Base:** `main`
**Reviewed by:** kenule500 (admin)
**Reviewed at:** 2026-10-29T07:00:00Z

## Verdict

Cleared for release. Every audit criterion passes at this commit, including the two
that failed the first audit: the formatter now exits 0 and CI is green.

The first audit failed on one item - `cargo fmt --all --check` - and fixing it
exposed a second, latent defect: `ac_017` and `ac_018` were racing a 10 ms / 50 ms
client timeout against a 120 ms mock responder, so under load the client gave up
before the request reached the recorder and `recorder.last()` panicked. The suite
was therefore intermittent: 127 passed at `3ed27d0`, 117 passed / 1 failed at
`97b01be`, and 2 fails then 1 pass across three isolated repeats of the same tree.
Both tests now use a 300 ms timeout against a 3000 ms responder; the assertions
(`W2004` plus the `x-wirekit-test` marker reaching the provider) are unchanged and
the AC intent is preserved. The suite is green at this commit: 127 passed, 0
failed, 2 ignored, 129 total.

## Criteria

| # | Criterion | Status | Evidence |
|---|---|---|---|
| 1 | `sdrive verify` returns pass or warn | **pass** | `PASS` - 180 matched, 0 missing, 0 renamed, 0 extra |
| 2 | Every AC has a linked test, status complete | **pass** | 82 ACs, 82 tested, coverage 100.0% |
| 3 | Test evidence exists and binds to HEAD | **pass** | commit `924ce72…` = HEAD; `exitCode 0`; 127 passed / 0 failed / 2 ignored / 129 total |
| 4 | Test report exists and is not stale | **pass** | `test-report-m1-first-publish-v1-2026-10-29.md` (2026-10-29) is newer than `spec.md` and `plan.md` (2026-10-21) |
| 5 | Test quality (AC IDs, no `.only`/`xit`, skips justified) | **pass** | 85 AC-named test fns; the only `.only`-class match was the substring inside `fn exit(`; both ignored tests carry recorded reasons |
| 6 | Governance artifacts validate | **pass** | `spec.json`, `plan.json`, `tasks.json`, `traceability.json`, `test-evidence.json` all valid |
| 7 | Diff scope matches the plan | **pass** | 72 files, +16659/-262; all commits belong to this feature, none to another in-flight feature (CON-611) |
| 8 | Branch is based on its recorded base (CON-612) | **pass** | `feature.base = main`; `aa7c3b9` records main as the merge target; no foreign commits in `main..HEAD` |
| 9 | Crate-level lints (CON-201, CON-202) | **pass** | `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` in `crates/wirekit/src/lib.rs:59-60` |
| 10 | Linter | **pass** | CI `Clippy` job green at HEAD |
| 11 | Documentation present | **pass** | `crates/wirekit/README.md` and `CHANGELOG.md` exist; CI `Docs` job green |
| 12 | MSRV 1.85 respected | **pass** | CI `MSRV (1.85)` job green at HEAD |
| 13 | Packaging | **pass** | CI `Package` job green at HEAD |
| 14 | Secret scan | **pass (manual)** | No `gitleaks`/`trufflehog` installed; pattern scan clean; 5 `sk_live_` matches inspected and confirmed as redaction-test placeholders |
| 15 | Formatter | **pass** | `cargo fmt --all --check` exits 0 at HEAD `924ce72` |
| 16 | CI green for HEAD | **pass** | CI run `40831862065` (test-fix commit) = success, all jobs green including Format; the run for the evidence commit was in progress at audit time |

## Findings

| # | Severity | Issue | Location | Fix required |
|---|---|---|---|---|
| F1 | high | **Resolved.** `cargo fmt --all --check` failed at `3ed27d0` with 45 diff locations in the feature's test files, turning the CI `Format` job red. Fixed in `97b01be`; the formatter now exits 0 and CI is green. | `crates/wirekit/tests/m1-first-publish/*.rs` | no - closed |
| F2 | high | **Resolved.** `ac_017` and `ac_018` raced a 10 ms / 50 ms client timeout against a 120 ms mock responder and then asserted the recorder had seen the request, so the suite was intermittent under load. Fixed in `2f2d5e5` by widening the margins to 300 ms / 3000 ms; assertions unchanged. | `crates/wirekit/tests/m1-first-publish/client.rs` | no - closed |
| F3 | info | Two tests are `ignored` with recorded reasons ("requires `dado` 0.1.0 to be published on crates.io", "requires the `dado-v0.1.0` release tag to exist"). They belong to M0, not M1. | `crates/dado/tests/m0-foundation/release_readiness.rs` | no |
| F4 | low | No automated secret scanner is installed, so criterion 14 rests on a pattern scan plus manual inspection rather than a deterministic tool. | repository tooling | no |
| F5 | low | `.sdrive/config.json` still names `.sdrive/scripts/rust-tests.js`, which framework 0.1.46 deletes on `main`. `sdrive test` will fail on any main-derived branch until the reporter is restored or the command repointed. | `.sdrive/config.json` | no - tracked for the merge |

## Fix list

1. **F1 - closed.** `97b01be` applied the formatter to the 12 test files; `cargo fmt --all --check` exits 0 and CI `Format` is green.
2. **F2 - closed.** `2f2d5e5` removed the timeout race in `ac_017` and `ac_018` without changing either assertion.
3. **F3 - recorded.** `924ce72` recorded green evidence and the 2026-10-29 test report at HEAD.

Nothing outstanding blocks release. Follow-ups, not blocking: appoint an automated
secret scanner (F4), and restore the `rust-tests.js` reporter on `main` before
testing any branch derived from it (F5).

## Evidence

| Item | Path |
|---|---|
| Verification report | `.sdrive/governance/m1-first-publish/v1/verification.json` - `status: pass`, 180 matched |
| Test evidence | `.sdrive/governance/m1-first-publish/v1/test-evidence.json` - commit `924ce72…`, `exitCode 0` |
| Test report | `.sdrive/reports/tests/test-report-m1-first-publish-v1-2026-10-29.md` |
| Raw test log | `.sdrive/reports/tests/raw/m1-first-publish-v1-2026-10-29.log` |
| CI run (green) | `40831862065` on the test-fix commit - all jobs passed, Format included |
| Structured record | `.sdrive/governance/m1-first-publish/v1/review.json` |

## Notes

- The review ran on `feature/m1-first-publish` at `924ce72`, which matches the
  commit recorded in `test-evidence.json`. The working tree was clean before and
  after the audit.
- No source file was modified by this review; the two code fixes it called for were
  committed by the implementer in `97b01be` and `2f2d5e5` and re-audited here.
- `sdrive verify` rewrote the local, git-ignored
  `.sdrive/governance/m1-first-publish/v1/verification.json` during the audit; its
  result is still `pass` with the same 180/0/0/0 summary.

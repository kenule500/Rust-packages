---
status: FAIL
feature: public-docs-site-and-provider-matrix
version: v1
commit: be5ffb5dca4d8334eb084e8644131c4942021a03
branch: feature/public-docs-site-and-provider-matrix
base: main
reviewedBy: kenule500
reviewedAt: 2026-11-23T16:30:00.000Z
coverage: 100
evidence: .sdrive/governance/public-docs-site-and-provider-matrix/v1/test-evidence.json
---

# Review Report: Public docs site and provider matrix (v1)

## 1. Audit Summary
- **Status:** FAIL
- **Date:** 2026-11-23
- **Reviewer:** kenule500 (independent: no — no assignee is recorded and the reviewer is the implementing account; flagged, human confirmation required)
- **Version:** v1
- **Commit:** be5ffb5dca4d8334eb084e8644131c4942021a03
- **Branch / base:** feature/public-docs-site-and-provider-matrix / main

## 2. Evidence
- **Test evidence:** `.sdrive/governance/public-docs-site-and-provider-matrix/v1/test-evidence.json` — exitCode 0, 281 passed, 0 failed, 2 skipped, total 283, `commit` be5ffb5 = HEAD
- **Test report:** `.sdrive/reports/tests/test-report-public-docs-site-and-provider-matrix-v1-2026-11-23.md`
- **Verification:** `.sdrive/governance/public-docs-site-and-provider-matrix/v1/verification.json` — **warn** (14 matched, 0 missing, 0 renamed, 2 extra)
- **Audit chain:** `.sdrive/audit-log.json` — `sdrive audit:verify` ok, 153 hashed events verified
- **CI:** **red** — run [45206300962](https://github.com/kenule500/Rust-packages/actions/runs/45206300962) (Test (windows-latest) failed)

## 3. Criteria Check
- [x] Spec Convergence: all six ACs implemented — five-page book, generated crate index and provider matrix, generated error reference, drift/link/reachability check, CI docs job, README link and upgrade policy.
- [x] Task Completion: T1.1–T3.2 checked in `tasks.md` and `completed: true` in `tasks.json`. Review fix task T4.1 added by this review and outstanding.
- [x] Traceability: 12/12 rows carry TEST-001…TEST-006, status `complete`.
- [x] Coverage: 100% (6/6 in-scope ACs, 0 withdrawn; 281/283 tests passing, 0 failed).
- [x] Test Report: exists, and its timestamp (16:24) is newer than `spec.md` and `plan.md` (15:34).
- [x] Test Evidence: `commit` equals HEAD, exit code 0, zero failures, non-zero total.
- [~] Change Scope: 32 files; all 12 planned components present. Four unplanned paths, each disclosed and committed separately (see §4).
- [x] Test Quality: six tests named `ac_001_…` … `ac_006_…`; no `.only`, no `xit`, no `#[ignore]`, no empty assertions (52 assertions). AC-002 and AC-003 assert the *failure* path, so they cannot pass vacuously.
- [x] Code Quality: `cargo fmt --all --check`, `cargo clippy --all-targets -D warnings`, `RUSTDOCFLAGS=-D warnings cargo doc`, and `node scripts/docs-check.js` all clean.
- [x] Security: no new dependency, no secret-shaped string in the diff, CON-517/519/520 wired and tested. Secret scanner absent — recorded `not_verified`, not claimed clean.
- [ ] Testing: **FAIL** — the suite is green locally (Windows) and on ubuntu and macos in CI, but `Test (windows-latest)` fails on HEAD.
- [x] Documentation: README docs-site section added, five pages exist, catalogue status paragraph corrected. No schema change, so no migration note needed.
- [x] NFR Evidence: measured — NFR-001 (0 external asset refs), NFR-002 (byte-identical across two generations), NFR-003 (median 962 ms vs 120 s), NFR-004 (flat SUMMARY, one click).
- [ ] Definition of Done: implementation and test DoD met; release blocked by red CI.
- [x] Spec-to-Code Verification: `warn` (pass/warn required).
- [ ] CI: **FAIL** — no green run for HEAD.
- [x] Permission: `check-permission.js slash /sdrive:review` allowed (kenule500, admin).
- [~] Reviewer Independence: no assignee recorded; reviewer is the implementer. Recorded, not silently self-approved.

## 4. Change Scope
- **Files changed:** 32 (3057 insertions, 5 deletions)
- **Planned files changed:** 12/12 plan components
- **Unplanned files:** `.sdrive/scripts/verify.js` (adds `.github` to the verifier's scan roots so the planned CI workflow can be verified at all), `catalogue/00_provider_index.md` (its status paragraph claimed nine provider modules and 175 tests; the source has eleven and 275), `.gitignore` (`docs/book/`), `crates/public-docs-site-and-provider-matrix/` (test-only package added by `/sdrive:test`), plus the expected `.sdrive/` spec, governance, workflow-state and report artifacts.
- **Other-feature commits:** none (CON-611 satisfied)
- **Working tree:** clean
- **Base:** `git log main..HEAD` contains only this feature's 13 commits; CON-612 satisfied

## 5. Issues Found
| Severity | Issue | Location | AC | Fix Required | Fix with |
|----------|-------|----------|----|--------------|----------|
| High | The drift check is line-ending sensitive. Generated pages are written and compared as LF-only text, but the `windows-latest` runner checks out with `core.autocrlf=true` and the repository has no `.gitattributes`, so `docs/src/errors.md` and `docs/src/crates.md` arrive as CRLF and `--check` reports them stale. Confirmed locally: converting `docs/src/errors.md` to CRLF makes `--check` exit 1; converting back restores exit 0. Fails `ac_005` on Windows, and `ac_002` next for the same reason. | `scripts/docs-errors.js`, `scripts/docs-matrix.js` | AC-002, AC-005 | Yes | `/sdrive:apply` |
| Medium | `sdrive check` exits 4: *doctor/permissions: No rule for mutating command(s): review:cold*. `.sdrive/commands/permissions.json` holds rules only for `/sdrive:*`, so mutating CLI commands are deny-by-default. **Pre-existing and repo-wide** — the file is unchanged on this branch. | `.sdrive/commands/permissions.json` | — | No (not this feature) | `sdrive update` |
| Low | Four paths outside the plan components, each disclosed in its own commit and described in §4. | see §4 | — | No | — |

## 6. Recommendations
- Fix the line-ending defect at the source rather than in the test: make the generators compare normalised content (or always emit `\n`) **and** add a repository `.gitattributes` (`* text=auto eol=lf`), so a Windows checkout and a Linux checkout produce the same working tree. Either change alone fixes CI; both together remove the class of bug for every generated file the repo will add later.
- Consider making the drift check cover its own precondition: the `Docs site` job passing on ubuntu while `Test (windows-latest)` failed shows that one platform is not enough to prove a byte-comparison check is portable.
- `sdrive check`'s permissions blocker deserves its own pass — it is not caused by this feature but it will block every future gate in this repository.
- Non-blocking: `scripts/docs-partners.js` and `scripts/rust-tests.js` remain the two "extra" components in `sdrive verify`. They are legitimate pre-existing scripts; no action.

## 7. Security & Performance
| Area | Status | Reason / measurement |
|------|--------|----------------------|
| Security — secrets, dependencies, CON-51x baseline | pass | No new dependency (empty `[dependencies]` in the new package); 0 secret-shaped strings in the diff; CON-517 (escaping, no `<script>`, no `additional-js`), CON-519 (placeholders, no credential) and CON-520 (link check rejects `http://`) verified in the shipped path; remaining controls `not_applicable` with reasons. `secrets: not_verified` — `gitleaks`/`trufflehog` are not installed, so the scan was manual pattern matching, not a deterministic tool. |
| Performance — NFRs, SEO | pass | NFR-001: 0 external asset references in rendered HTML. NFR-002: identical SHA-1 across two generations. NFR-003: median 962 ms (mdbook 296 ms + docs-check 665 ms), 5 samples, high and low discarded, against a 120 s target. NFR-004: flat SUMMARY, one click to every page. SEO `not_applicable`. |

## 8. Handoff

### Findings to fix
1. **R1-1 (high, T4.1)** — line-ending sensitivity in the generated-page drift check. Owner: `/sdrive:apply public-docs-site-and-provider-matrix`.
2. **R1-2 (medium, no task)** — `sdrive check` permissions blocker for `review:cold`. Pre-existing, repo-wide; owner: `sdrive update`.

### Why the recommendation differs
The feature is functionally complete and every acceptance criterion is covered by a passing test on this machine. The verdict is still FAIL, for two reasons that a PASS would misrepresent:

- **CI is red for HEAD.** The `ci` and `testing` criteria are not negotiable, and a green local run is not evidence about a Windows checkout. The failure is real, reproducible, and traced to a specific mechanism — not a flake.
- **The defect is in the shipped check, not in the test.** The acceptance test correctly demands that `--check` pass on a clean tree. Relaxing the test to tolerate a stale page would delete the AC-002/AC-005 protection; the portability bug belongs in the generators.

### Next
- Run `/sdrive:apply public-docs-site-and-provider-matrix` to fix T4.1, then `sdrive test public-docs-site-and-provider-matrix`, then `/sdrive:review public-docs-site-and-provider-matrix` again (round 1 of 3).
- The review did not modify code, did not merge, and did not move the feature folder. A fix moves `HEAD` and invalidates this verdict.
- **Round cap:** if round 3 fails on the same finding, stop and hand it to a human.

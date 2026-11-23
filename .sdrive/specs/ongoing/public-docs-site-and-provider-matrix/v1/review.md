---
status: PASS
feature: public-docs-site-and-provider-matrix
version: v1
commit: 046ac8b65d06aa642930c7f82274872b9b313532
branch: feature/public-docs-site-and-provider-matrix
base: main
reviewedBy: kenule500
reviewedAt: 2026-11-23T17:20:00.000Z
coverage: 100
evidence: .sdrive/governance/public-docs-site-and-provider-matrix/v1/test-evidence.json
---

# Review Report: Public docs site and provider matrix (v1)

## 1. Audit Summary
- **Status:** PASS
- **Date:** 2026-11-23
- **Reviewer:** kenule500 (independent: no — no assignee is recorded and the reviewer is the implementing account; flagged, and the verdict was confirmed explicitly rather than self-approved)
- **Version:** v1
- **Commit:** 046ac8b65d06aa642930c7f82274872b9b313532
- **Branch / base:** feature/public-docs-site-and-provider-matrix / main
- **Review round:** 2 of 3

## 2. Evidence
- **Test evidence:** `.sdrive/governance/public-docs-site-and-provider-matrix/v1/test-evidence.json` — exitCode 0, 281 passed, 0 failed, 2 skipped, total 283, `commit` 046ac8b = HEAD
- **Test report:** `.sdrive/reports/tests/test-report-public-docs-site-and-provider-matrix-v1-2026-11-23.md`
- **Verification:** `.sdrive/governance/public-docs-site-and-provider-matrix/v1/verification.json` — warn (14 matched, 0 missing, 0 renamed, 2 extra)
- **Audit chain:** `.sdrive/audit-log.json` — `sdrive audit:verify` ok, 155 hashed events verified
- **CI:** **green** — run [45210391595](https://github.com/kenule500/Rust-packages/actions/runs/45210391595) at `046ac8b`: Format, Clippy, Docs, Docs site, Package, MSRV (1.87), Audit, and Test on ubuntu, windows and macos

## 3. Criteria Check
- [x] Spec Convergence: all six ACs implemented — five-page book, generated crate index and provider matrix, generated error reference, drift/link/reachability check, CI docs job, README link and upgrade policy.
- [x] Task Completion: T1.1–T3.2 and the review fix task T4.1 checked in `tasks.md` and `completed: true` in `tasks.json`.
- [x] Traceability: 12/12 rows carry TEST-001…TEST-006, status `complete`.
- [x] Coverage: 100% (6/6 in-scope ACs, 0 withdrawn; 281/283 tests passing, 0 failed).
- [x] Test Report: exists and is newer than `spec.md` and `plan.md`.
- [x] Test Evidence: `commit` equals HEAD, exit code 0, zero failures, non-zero total.
- [x] Change Scope: 34 files, 15 commits, all this feature's own — 12 planned components, the T4.1 fix, the test-only package, and disclosed supporting changes. No other feature's commits (CON-611); branch based on `main` (CON-612).
- [x] Test Quality: six tests named `ac_001_…` … `ac_006_…`; no `.only`, no `xit`, no `#[ignore]`, 52 assertions. AC-002 and AC-003 assert the *failure* path.
- [x] Code Quality: `cargo fmt --all --check`, `cargo clippy --all-targets -D warnings`, `RUSTDOCFLAGS=-D warnings cargo doc`, `node scripts/docs-check.js` all clean. Round 2 changed no Rust.
- [x] Security: no new dependency, no secret-shaped string in the round-2 diff, CON-517/519/520 wired and tested. Secret scanner absent — `not_verified`, not claimed clean.
- [x] Testing: 6/6 locally; 281/283 recorded; CI Test jobs green on all three platforms.
- [x] Documentation: README docs-site section, five pages, corrected catalogue status paragraph. No schema change.
- [x] NFR Evidence: measured — NFR-001 (0 external asset refs), NFR-002 (identical hashes across two generations), NFR-003 (median 1164 ms vs 120 s), NFR-004 (flat SUMMARY, one click).
- [x] Definition of Done: implementation, test and release DoD met.
- [x] Spec-to-Code Verification: `warn` (pass/warn required).
- [x] CI: green for HEAD.
- [x] Permission: `check-permission.js slash /sdrive:review` allowed (kenule500, admin).
- [~] Reviewer Independence: no assignee recorded; reviewer is the implementer. Recorded, and the PASS was confirmed by the human.

## 4. Change Scope
- **Files changed:** 34 (3213 insertions, 5 deletions)
- **Planned files changed:** 12/12 plan components
- **Round-2 changes only:** `scripts/docs-matrix.js`, `scripts/docs-errors.js`, `.gitattributes`, `tasks.md`, `tasks.json` — the T4.1 fix and its task record
- **Unplanned files:** `.sdrive/scripts/verify.js` (adds `.github` to the verifier's scan roots), `catalogue/00_provider_index.md` (stale status paragraph corrected), `.gitignore` (`docs/book/`), `crates/public-docs-site-and-provider-matrix/` (test-only package), plus expected `.sdrive/` artifacts
- **Other-feature commits:** none
- **Working tree:** clean

## 5. Issues Found
| Severity | Issue | Location | AC | Fix Required | Fix with |
|----------|-------|----------|----|--------------|----------|
| Info | **R1-1 resolved.** Drift check is now line-ending agnostic: both generators normalise CRLF before comparing, and `.gitattributes` declares `text eol=lf`. Re-proved: all three generated pages converted to CRLF pass `--check`; a bogus appended row on a CRLF page still fails it; `windows-latest` is green. | `scripts/docs-matrix.js`, `scripts/docs-errors.js`, `.gitattributes` | AC-002, AC-005 | No | — |
| Medium | `sdrive check` exits 4: *doctor/permissions: No rule for mutating command(s): review:cold*. **Pre-existing and repo-wide** — `.sdrive/commands/permissions.json` has the identical SHA-1 `928f350…` on `main` and at HEAD, the branch never touches `.sdrive/commands/`, and the rule set comes from the installed CLI. Not repairable by any `/sdrive:*` command. | `.sdrive/commands/permissions.json` | — | No (not this feature) | `sdrive update` |
| Low | Supporting paths outside the plan components, each disclosed in its own commit (see §4). | see §4 | — | No | — |

## 6. Recommendations
- Run `sdrive update` at some point to regenerate `.sdrive/commands/permissions.json`, then re-run `sdrive verify`: that update may also revert the `.github` scan-root patch in `.sdrive/scripts/verify.js`, which this feature's component matching relies on. Treat the two together.
- The round-1 failure is worth keeping as a habit: one platform is not enough to prove a byte-comparison check is portable. The `Docs site` job was green on ubuntu the whole time the Windows test job was red.
- Non-blocking: `scripts/docs-partners.js` and `scripts/rust-tests.js` remain the two "extra" components in `sdrive verify`. They are legitimate pre-existing scripts.

## 7. Security & Performance
| Area | Status | Reason / measurement |
|------|--------|----------------------|
| Security — secrets, dependencies, CON-51x baseline | pass | No new dependency (empty `[dependencies]` in the new package); 0 secret-shaped strings in the diff; CON-517 (escaping, no `<script>`, no `additional-js`), CON-519 (placeholders, no credential) and CON-520 (link check rejects `http://`) verified in the shipped path; remaining controls `not_applicable` with reasons. `secrets: not_verified` — no `gitleaks`/`trufflehog`, so the scan was manual pattern matching. |
| Performance — NFRs, SEO | pass | NFR-001: 0 external asset references. NFR-002: identical SHA-1 across two generations. NFR-003: median 1164 ms (5 samples, high and low discarded) against a 120 s target. NFR-004: flat SUMMARY, one click. SEO `not_applicable`. |

## 8. Handoff
- **PASS.** Run `/sdrive:archive public-docs-site-and-provider-matrix` to merge into `main` and archive the version. Archiving is a sign-off, so it stays with you.
- The review modified no code and moved no feature folder. Only v1 was reviewed; no other version exists.
- Round 2 of 3: the single round-1 blocker is cleared and the round cap was never reached.

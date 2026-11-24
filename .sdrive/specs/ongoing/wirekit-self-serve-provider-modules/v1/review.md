---
status: FAIL
feature: wirekit-self-serve-provider-modules
version: v1
commit: 27ffefeeba9e478e616881d3017db2843a148144
branch: feature/wirekit-self-serve-provider-modules
base: main
reviewedBy: kenule500
reviewedAt: 2026-11-24T14:57:03Z
coverage: 100
evidence: .sdrive/governance/wirekit-self-serve-provider-modules/v1/test-evidence.json
---

# Review Report: wirekit - self-serve provider modules (v1)

## 1. Audit Summary
- **Status:** FAIL
- **Date:** 2026-11-24
- **Reviewer:** kenule500 (independent: no assignee recorded on the feature, so independence could not be confirmed)
- **Version:** v1
- **Commit:** 27ffefeeba9e478e616881d3017db2843a148144
- **Branch / base:** feature/wirekit-self-serve-provider-modules / main

The implementation is sound: all seven acceptance criteria are tested, the local
gates are green, and the security baseline is wired into every module. The release
is blocked by one defect in the acceptance suite, not in the provider code. The
AC-006 test asserts a `\n`-joined string against a raw file read, so it fails on
any checkout whose source files carry CRLF line endings. Windows CI reproduces it
on every run; a Linux checkout does not, which is why the local suite stays green.

## 2. Evidence
- Test evidence: `.sdrive/governance/wirekit-self-serve-provider-modules/v1/test-evidence.json` - exit 0, 318 passed / 0 failed / 2 skipped of 320, commit 27ffefe (matches HEAD).
- Test report: `.sdrive/reports/tests/test-report-wirekit-self-serve-provider-modules-v1-2026-11-24.md` - 7/7 ACs, 37 acceptance tests.
- Verification: `.sdrive/governance/wirekit-self-serve-provider-modules/v1/verification.json` - status `warn`, 12 matched / 0 missing / 19 extra (pre-existing crate files outside the plan).
- Audit chain: `sdrive audit:verify` - ok, 163 hashed events verified.
- CI: run [45415969525](https://github.com/kenule500/Rust-packages/actions/runs/45415969525) - **failure**, job `Test (windows-latest)` (138522121145). Format, Docs, MSRV (1.87), Package, Clippy, Audit, and both other test platforms passed in the same run.

## 3. Criteria Check
- [x] Spec Convergence: six modules, each behind its own additive feature, each implementing its catalogue surface; Yelp leads are withdrawn with a citation (T2.5, RISK-001) rather than faked.
- [x] Task Completion: T1.1, T2.1-T2.6, T3.1, T3.2 all checked in `tasks.md` and `complete` in `tasks.json`.
- [x] Traceability: 12/12 FR/NFR rows carry test IDs (TEST-001...TEST-037) with status `complete`.
- [x] Coverage: 100% (7/7 ACs tested, 37/37 acceptance tests passing locally, 318/320 workspace-wide with 2 pre-existing skips).
- [x] Test Report: present, dated 2026-11-24, newer than `spec.md` and `plan.md`.
- [x] Test Evidence: exit 0, zero failures, commit equals HEAD.
- [x] Change Scope: 27 files, all planned or framework artifacts; no other-feature commits (CON-611).
- [ ] Test Quality: one portability defect - `contract.rs:227-231` builds the module gate with `\n` while `lib_source()` returns the file verbatim, so the assertion is line-ending sensitive.
- [x] Code Quality: `cargo fmt --all --check`, `cargo clippy -p wirekit --all-features -- -D warnings`, and `RUSTDOCFLAGS=-D warnings cargo doc -p wirekit --all-features --no-deps` all exit 0.
- [x] Security: CON-511/514/515/519/520 implemented and wired; manual secret scan of the diff clean; no dependency changes.
- [ ] Testing: local suite green, but the suite is not green on Windows CI, so the feature cannot be called release-ready.
- [x] Documentation: crate docs, README provider sections, and the feature table describe all six modules and the partner-gated exclusion.
- [x] NFR Evidence: `cargo +1.87.0 check -p wirekit --all-features` exit 0 (NFR-001); rustdoc and module scope/non-goals assertions cover NFR-002; typed decoding and panic-free assertions cover NFR-003; contract tests assert the tracing events for NFR-004.
- [x] Definition of Done: production work is complete and green locally; the failing criterion is test portability, tracked as finding F-001.
- [x] Spec-to-Code Verification: every endpoint and type in the six modules traces to its catalogue design file; no endpoint or vendor workflow vocabulary added.
- [ ] CI: red on Windows at HEAD (see finding F-001).
- [x] Permission: `check-permission.js slash /sdrive:review` allowed user `kenule500` (admin).
- [~] Reviewer Independence: the feature has no recorded assignee, so the reviewer-to-assignee comparison could not be made. Record an assignee before archive.

## 4. Change Scope
- Files changed: 27
- Planned files changed: 12 components from `plan.json` (manifest, crate root, six modules, README, crate docs) plus the governance, spec, report, and test artifacts the workflow produces.
- Unplanned files: none
- Other-feature commits: none
- Working tree: clean

## 5. Issues Found

| Severity | Issue | Location | AC | Fix Required | Fix with |
|----------|-------|----------|----|--------------|----------|
| High | The module-gate assertion compares a `\n`-joined string against a raw file read, so it fails on CRLF checkouts and turns Windows CI red deterministically. Reproduced twice at HEAD and at the previous commit. | `crates/wirekit/tests/wirekit-self-serve-provider-modules/contract.rs:227-231` | AC-006 | Yes | `/sdrive:test` |
| Medium | `sdrive check` exits 4 on `doctor/permissions: No rule for mutating command(s): review:cold`. Pre-existing tooling drift, untouched by this feature, but it keeps the one-pass governance check red. | `.sdrive/permissions.json` | - | No | `sdrive update` |

### Evidence for F-001
- HEAD run 45415969525, job 138522121145: `thread 'contract::ac_006_features_are_independent_and_default_is_empty' panicked at crates\wirekit\tests\wirekit-self-serve-provider-modules\contract.rs:228:9`.
- Previous commit 401744e, run 45412755649, job 138510885336: identical failure.
- Local reproduction of the mechanism: the same assertion holds with LF (`true`) and fails once the file text is CRLF (`false`); the runner checks out with CRLF while this clone uses `core.autocrlf=input`.

## 6. Recommendations
- Normalise line endings at the boundary, not at each assertion: make `lib_source()`, `manifest()`, and `module_source()` return `read_to_string(...).replace("\r\n", "\n")`, so every string assertion becomes platform-independent.
- Add a `.gitattributes` with `*.rs text eol=lf` so a Windows checkout and CI agree on line endings by construction.
- Add the missing `review:cold` permission rule and drop the stale `amend`/`reopen` entries (`sdrive update` refreshes them).
- Record an assignee on the feature so the next review can state reviewer independence rather than `not_verified`.

## 7. Findings to fix
1. **F-001 (High)** - Normalise line endings in the acceptance-suite file readers, then re-run `/sdrive:test` and `/sdrive:review`. The provider code needs no change.
2. **F-002 (Medium)** - Refresh `.sdrive/permissions.json` so `sdrive check` exits 0.

## 8. Why the recommendation differs
The local gates are green and every AC is tested, which would normally clear the
feature. The suite itself is the defect: it encodes the reviewer's line endings, so
it passes on the machine that wrote it and fails on Windows CI. A suite that is only
green on one platform cannot certify six provider modules, and the test phase cannot
be trusted to catch regressions until the readers are normalised. The fix is two
lines in a test helper, so the feature is close to release rather than in doubt.

## 9. Handoff
No merge was performed and the feature stayed in `.sdrive/specs/ongoing/`.

**Routing correction (recorded after `/sdrive:apply` ran).** The fix for F-001 lives
in `crates/wirekit/tests/wirekit-self-serve-provider-modules/contract.rs`, and the
implementation agent is forbidden from creating, editing, or deleting anything under
`tests/` (07-implementation.md, rules 40 and 43). `/sdrive:apply` therefore has no
legal way to land it: all nine production tasks are already complete, and the crate
needs no code change. F-001 belongs to `/sdrive:test`, which owns the suite:

1. Run `/sdrive:test wirekit-self-serve-provider-modules`.
2. Make `lib_source()`, `manifest()`, and `module_source()` return
   `read_to_string(...).replace("\r\n", "\n")`, so every string assertion is
   platform-independent (optionally add `.gitattributes` with `*.rs text eol=lf`).
3. Re-run `/sdrive:test` so the evidence is stamped at the new commit, then
   `/sdrive:review wirekit-self-serve-provider-modules` again.

F-002 stays non-blocking: `.sdrive/commands/permissions.json` is a framework file and
`sdrive update` does not rewrite it, so adding the `review:cold` rule is a separate
governance decision, not a feature fix.

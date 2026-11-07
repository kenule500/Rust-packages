---
status: FAIL
feature: quota-ledger-0-1-core-postgres-backend
version: v1
commit: 09e2298dbd65160b22b12a84ea0ae1398504d5d0
branch: feature/w3-p1
base: main
reviewedBy: kenule500
reviewedAt: 2026-11-07T14:51:48Z
coverage: 100
evidence: .sdrive/governance/quota-ledger-0-1-core-postgres-backend/v1/test-evidence.json
---

# Review Report: quota-ledger 0.1 - core reservations and Postgres backend (v1)

## 1. Audit Summary
- **Status:** FAIL
- **Date:** 2026-11-07
- **Reviewer:** kenule500 (independent: no — this agent line wrote the code under review; human sign-off is required before any PASS)
- **Version:** v1
- **Commit:** 09e2298dbd65160b22b12a84ea0ae1398504d5d0
- **Branch / base:** feature/w3-p1 / main
- **Round:** 2 of 3

Round 1 failed on two High findings: RUSTSEC-2023-0071 in `Cargo.lock` (F1) and an
unresolvable 1.87 MSRV floor (F2). Both are now cleared and independently
re-verified: `cargo audit` exits 0 over 292 resolved crates, and
`cargo +1.87.0 build --workspace --all-features` exits 0. The crate itself is in
the best shape it has been in: fmt, clippy, rustdoc, the `--no-default-features`
check, all 15 acceptance tests, the 234-test workspace suite and CI at HEAD are
green.

This round fails on **artifact consistency, not code**. The test report is older
than the plan it is supposed to certify, and the planning gate is still pending
because the plan was corrected after Gate 2 approval. Clearing the feature now
would archive it against an unre-approved plan and a stale test report.

## 2. Evidence
- Test evidence: `.sdrive/governance/quota-ledger-0-1-core-postgres-backend/v1/test-evidence.json` — exit 0, 236 total, 234 passed, 0 failed, 2 skipped, commit `09e2298` (matches HEAD), recorded 2026-11-05T01:11:17Z
- Test report: `.sdrive/reports/tests/test-report-quota-ledger-0-1-core-postgres-backend-v1-2026-11-04.md` — **stale** (see `test-report` criterion)
- Verification: `.sdrive/governance/quota-ledger-0-1-core-postgres-backend/v1/verification.json` — status `pass`, 39 matched, 0 missing, 0 renamed, 0 extra, 33 withdrawn
- Audit chain: `sdrive audit:verify --json` — ok, 89 hashed events verified
- CI: run [41946604778](https://github.com/kenule500/Rust-packages/actions/runs/41946604778) — success, `headSha` `09e2298` (the reviewed commit)

Commands run in this audit (repository root):

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all --check` | exit 0 |
| Lint | `cargo clippy --workspace --all-features -- -D warnings` | exit 0 |
| Docs | `cargo doc --workspace --all-features --no-deps` (`RUSTDOCFLAGS=-D warnings`) | exit 0 |
| Feature isolation | `cargo check -p quota-ledger --no-default-features` | exit 0 |
| MSRV | `cargo +1.87.0 build --workspace --all-features` | exit 0 |
| Dependencies | `cargo audit` | exit 0, 1422 advisories loaded, 292 crates |
| Suite | `node scripts/rust-tests.js` | exit 0, 234 passed / 0 failed / 2 skipped |

Environment notes: this host needs `CRATE_CC_NO_DEFAULTS=1` for the `postgres`
feature (MSVC 14.50.35717 `cl.exe` fails with D8050 under cc-rs's default `/Z7`),
as the earlier test report already records. The sandbox also blocked cargo's TLS
download of `mime 0.3.18` until network access was granted; that is an
environment artifact, not a repository defect.

## 3. Criteria Check
- [x] **Spec Convergence**: all 15 acceptance criteria are implemented in `crates/quota-ledger` and each has a matching `ac_0NN_*` test.
- [x] **Task Completion**: 11/11 tasks are `completed: true` (T1.1-T4.1 plus the review-round-1 fixes T5.1 and T5.2).
- [x] **Traceability**: 18/18 rows are `complete`; all 15 ACs appear in at least one row and every row carries its `TEST-0NN` ids.
- [x] **Coverage**: 100% (15/15 in-scope ACs tested, 15/15 tests passing). No criterion is withdrawn.
- [ ] **Test Report**: **FAIL** — the only report is dated 2026-11-04 (14:37 local). `plan.md` was last modified 2026-11-05 01:49 local during the T5.2 realignment in `e978410`, and `plan.json` again in `09e2298`. The report predates both corrections, so it cannot certify the current plan.
- [x] **Test Evidence**: exists, `exitCode: 0`, `summary.failed: 0`, `summary.total: 236`, `commit` equals HEAD, branch matches.
- [x] **Change Scope**: 40 files, +5590 lines. Planned crate files plus governance, spec, review, test and security artifacts; `.gitignore` is unplanned but justified. `4f24afb` also carries the hookgate and syncforge *proposal* artifacts by design (shared `feature/w3-p1` branch); no other feature's implementation is on the branch, and the branch is based on the recorded `base`.
- [x] **Test Quality**: 15 tests named `ac_0NN_*`, 64 assertions, no `.only`/`xit`, no `#[ignore]`, no empty assertions, no `todo!`/`unimplemented!`. Red-before-green is not enabled in `config.json`, so `--prove` evidence is not required.
- [x] **Code Quality**: fmt, clippy `-D warnings`, and rustdoc `-D warnings` are clean; `--no-default-features` compiles.
- [x] **Security**: `cargo audit` clean (292 crates, no advisory — the F1 regression is proven gone by a lockfile with no `rsa`); TLS required on `connect`; every statement binds `$n`; no secret in the diff. Secret scanning is a manual pattern scan because `gitleaks`/`trufflehog` are not installed — recorded as a limitation, not as a tool pass.
- [x] **Testing**: workspace suite 234 passed, 0 failed, 2 skipped, exit 0 — reproduced locally this round.
- [x] **Documentation**: crate README covers scope, features, reserve/commit/release, non-goals, the three tables, TTL reclaim and TLS policy; the root README indexes `quota-ledger`. There is no CHANGELOG convention in this repository. Schema rollback is still undocumented (low).
- [x] **NFR Evidence**: no performance, accessibility, or SEO NFR is declared, so the performance audit is `not_applicable` with a reason. NFR-001 to NFR-004 and NFR-006 are covered; NFR-005 (build on 1.87) **passes** this round.
- [ ] **Definition of Done**: **WARN** — CI is green at HEAD, but the planning gate is pending (see below), so the feature is not release-ready.
- [x] **Spec-to-Code Verification**: `sdrive verify` status `pass`, 0 missing, 0 extra.
- [x] **CI**: green run exists for HEAD (`41946604778`, success, `headSha` `09e2298`).
- [x] **Permission**: `kenule500` (admin) is allowed to run `/sdrive:review`. Note: `check-permission.js` cannot spawn `git` under the DSH sandbox (EPERM on the pipe), so it fell back to the OS account name and denied; re-running with `SDRIVE_ACTOR=kenule500`, the repository's actual `git config user.name`, returns allowed. Recorded rather than hidden.
- [x] **Reviewer Independence**: **WARN** — the same agent line wrote the code and this report. A human must confirm any PASS; this report is a FAIL, so nothing is being self-approved.

## 4. Change Scope
- Files changed: 40
- Planned files changed: 23 (11 crate files listed in `plan.json`, the two licence files, the schema, the test suite, and the governance/spec/report artifacts)
- Unplanned files: `.gitignore` (adds `.sdrive/handoff.json` and `.commandcode/` as local tool state — justified, unplanned)
- Missing planned files: none (`sdrive verify` reports 0 missing; the 33 withdrawn items are recorded with reasons in `verification.json`)
- Other-feature commits: `4f24afb` carries the hookgate and syncforge proposal artifacts, as the shared-branch proposal intended. No implementation of another feature is present.
- Working tree: clean at review start
- Base check: `feature/w3-p1` is based on `main` at `0154648`, matching `workflow-state.json`

## 5. Issues Found
| Severity | Issue | Location | AC | Fix Required | Fix with |
|----------|-------|----------|----|--------------|----------|
| High | Planning gate is pending: `plan.json`/`plan.md` were edited after Gate 2 approval (T5.2 realignment in `e978410`, then the sqlx-facade supersession in `09e2298`), so the approved planning artifact no longer matches the implementation. | `.sdrive/workflow-state.json:676-773`; `.sdrive/governance/quota-ledger-0-1-core-postgres-backend/v1/plan.json` | — | Yes | `sdrive approve quota-ledger-0-1-core-postgres-backend planning` (human sign-off) |
| High | The test report is stale: dated 2026-11-04 14:37, while `plan.md` changed 2026-11-05 01:49 and `plan.json` changed again in `09e2298`. It cannot certify the current plan. | `.sdrive/reports/tests/test-report-quota-ledger-0-1-core-postgres-backend-v1-2026-11-04.md:4` | — | Yes | `/sdrive:test quota-ledger-0-1-core-postgres-backend` |
| Medium | `connect_insecure_for_tests` is public API in the default set of the `postgres` feature, so a production caller can disable TLS with no compile-time guard. Documented as test-only, but documentation is not enforcement. Carried from round 1, still open. | `crates/quota-ledger/src/postgres/mod.rs:81` | AC-012 | No | `/sdrive:apply` (gate behind a `test-utils` feature) |
| Low | The commit adds `.sdrive/handoff.json` and `.commandcode/` to `.gitignore`; neither appears in `plan.json`. Justified (local tool state must not dirty the tree) but unplanned scope. | `.gitignore:10` | — | No | — |
| Low | The shipped schema has no rollback or drop script, and the README documents only how to apply it. An adopter has no documented reverse. Carried from round 1. | `crates/quota-ledger/src/postgres/schema.sql:1` | AC-012 | No | — |
| Low | `spec.md`, `plan.md` and `tasks.md` still carry `status: backlog` in their front matter while the feature lives in `specs/ongoing/`. Cosmetic drift that misleads a reader. | `.sdrive/specs/ongoing/quota-ledger-0-1-core-postgres-backend/v1/spec.md:3` (also `plan.md:3`, `tasks.md:3`) | — | No | — |
| Info | AC-013 proves the limit invariant on `MemoryQuotaStore` (40 tasks racing for a limit of 10). The Postgres conditional `UPDATE` remains unproven without a database-backed harness, as RISK-001 records. | `crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs:557` | AC-013 | No | `/sdrive:test` (when a DB harness exists) |

## 6. Recommendations
- Re-approve the planning gate first, then re-run `/sdrive:test` so the report is written against the approved plan, then re-review. That order matters: regenerating the report before the plan is approved would produce a report that is stale again the moment the gate is signed off.
- Gate `connect_insecure_for_tests` behind a non-default `test-utils` feature so disabling TLS cannot be reached from a normal build.
- Add a `schema_rollback.sql` (or a documented `DROP TABLE` sequence) next to `schema.sql`, and link it from the README.
- Start the Postgres harness for AC-013: it is the one acceptance criterion whose mechanism is asserted indirectly.
- Sync the front-matter `status` fields in the spec folder to `ongoing` when the plan is re-approved.

## 7. Security & Performance
| Area | Status | Reason / measurement |
|------|--------|----------------------|
| Security — secrets, dependencies, CON-51x baseline | pass | `cargo audit` exit 0 (292 crates, 1422 advisories; RUSTSEC-2023-0071 gone because the `sqlx` facade was replaced by `sqlx-core` + `sqlx-postgres`); `connect` forces `PgSslMode::Require`; every statement binds parameters; no credential read from environment or file; `Debug` redacts the pool. Secret scan manual (no `gitleaks`/`trufflehog`) — clean. CON-511/513/517/518 not applicable; CON-512/514/515/516/519/520 pass. |
| Performance — NFRs, SEO | not_applicable | The specification declares no performance, accessibility, or SEO NFR, and the crate exposes no measurable service surface. The reserve path is a single conditional statement plus an advisory lock; no declared target to measure against. |

## 8. Handoff

**Findings to fix**
1. **Planning gate pending** — run `sdrive approve quota-ledger-0-1-core-postgres-backend planning`. The plan correction is real: T5.1 replaced the `sqlx` facade with the two driver crates and T5.2 lowered the `uuid` floor, and both are already reflected in `plan.json`/`plan.md`.
2. **Stale test report** — run `/sdrive:test quota-ledger-0-1-core-postgres-backend` after the planning gate is approved, so the report timestamp lands after the final `plan.md`.
3. Optional, non-blocking: gate `connect_insecure_for_tests`, add the schema rollback, and sync the front-matter status.

**Why the recommendation differs from round 1**

Round 1 recommended fixes for two High code findings; those fixes landed and verified clean. The feature is not being held for code this round: it is held because the evidence trail is one step behind the plan. A PASS here would let `/sdrive:archive` merge a feature whose plan gate is `pending` and whose only test report describes an earlier dependency set — exactly the quiet mismatch this review exists to catch.

**If PASS later:** run `/sdrive:archive quota-ledger-0-1-core-postgres-backend` to merge and archive.
**Round cap:** if round 3 fails on the same finding, stop and hand it to a human. This review did not modify code, merge anything, or move the feature folder.

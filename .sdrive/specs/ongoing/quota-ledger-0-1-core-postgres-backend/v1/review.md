---
status: FAIL
feature: quota-ledger-0-1-core-postgres-backend
version: v1
commit: 2035b9aafa955381b787a76a5af25c049c0e9573
branch: feature/w3-p1
base: main
reviewedBy: CommandCode (self-review, no independent reviewer)
reviewedAt: 2026-11-04T14:50:00Z
coverage: 100
evidence: .sdrive/governance/quota-ledger-0-1-core-postgres-backend/v1/test-evidence.json
---

# Review Report: quota-ledger 0.1 - core reservations and Postgres backend (v1)

## 1. Audit Summary
- **Status:** FAIL
- **Date:** 2026-11-04
- **Reviewer:** CommandCode agent (independent: no — the reviewer wrote this code in the same session; human sign-off still required)
- **Version:** v1
- **Commit:** 2035b9aafa955381b787a76a5af25c049c0e9573
- **Branch / base:** feature/w3-p1 / main

## 2. Evidence
- Test evidence: `.sdrive/governance/quota-ledger-0-1-core-postgres-backend/v1/test-evidence.json` — exit 0, 236 total, 234 passed, 0 failed, 2 skipped, commit `2035b9a` (matches HEAD)
- Test report: `.sdrive/reports/tests/test-report-quota-ledger-0-1-core-postgres-backend-v1-2026-11-04.md`
- Verification: `.sdrive/governance/quota-ledger-0-1-core-postgres-backend/v1/verification.json` — pass (38 matched, 0 missing, 32 withdrawn, 0 renamed, 0 extra)
- Audit chain: `sdrive audit:verify --json` — ok, 86 hashed events verified
- CI: not verified for `2035b9a` (not pushed). The only run on this branch is `41371059017` for the proposal commit `4f24afb`.

## 3. Criteria Check
- [x] Spec Convergence: all 15 acceptance criteria are implemented in `crates/quota-ledger` and each has a matching test.
- [x] Task Completion: T1.1-T4.1 were complete before review (9/9). This review adds T5.1, which is open by design.
- [x] Traceability: 18/18 rows complete, every row carries its `TEST-0NN` id.
- [x] Coverage: 100% (15/15 in-scope ACs tested, 15/15 tests passing; no criteria withdrawn).
- [x] Test Report: present, dated 2026-11-04 14:37Z, newer than `spec.md` (2026-11-02) and `plan.md` (2026-11-04 14:22Z).
- [x] Test Evidence: exit 0, zero failures, 236 results, `commit === HEAD`.
- [x] Change Scope: 23 files — planned crate files plus governance, test, and report artifacts. `.gitignore` is unplanned (finding F3). No other feature's implementation commits on the branch; the base commit carries the sibling proposals by design.
- [x] Test Quality: 15 tests named `ac_0NN_*`, no `.only`/`xit`/`#[ignore]`, no empty assertions, no `todo!`/`unimplemented!` in the crate. Red-before-green is not enabled in `config.json`, so `--prove` evidence is not required.
- [x] Code Quality: `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, and `cargo doc --workspace --all-features --no-deps` under `RUSTDOCFLAGS=-D warnings` are all clean.
- [ ] **Security: FAIL** — `cargo audit` exits 1 on RUSTSEC-2023-0071, introduced by this commit (F1). Secrets: clean (manual scan; `gitleaks`/`trufflehog` not installed). Baseline CON-512/514/515/516/519/520 pass, CON-511/513/517/518 not applicable.
- [x] Testing: workspace suite 234 passed, 0 failed, 2 skipped, exit 0.
- [ ] **MSRV: FAIL** — `cargo +1.87.0 build -p quota-ledger --all-features` fails on `uuid@1.29.0 requires rustc 1.89.0`, and the crate's `uuid = "1.28.0"` floor makes 1.87 unresolvable (1.28.0 also requires 1.89). CI's `MSRV (1.87)` job runs exactly that build with no committed lockfile, so it fails (F2).
- [x] Documentation: crate README covers scope, features, flow, non-goals and the schema; root README already indexes `quota-ledger`. No CHANGELOG exists in this repository. Schema rollback is undocumented (F4).
- [x] NFR Evidence: no performance, accessibility, or SEO NFR is declared — `not_applicable` with reason in `review.json.performance`. NFR-001 to NFR-004 and NFR-006 are covered above; NFR-005 (build on 1.87) fails (F2).
- [x] Definition of Done: satisfied except a green CI run for HEAD, which does not exist.
- [x] Spec-to-Code Verification: `sdrive verify` exit 0, status pass.
- [ ] **CI: FAIL** — no run exists for `2035b9a`. Reproducing the two deterministic jobs locally shows both would fail: `cargo generate-lockfile && cargo audit` exits 1 (F1) and `cargo +1.87.0 build -p quota-ledger --all-features` exits 101 (F2).
- [x] Permission: `kenule500` (admin) is allowed to run `/sdrive:review`.
- [ ] Reviewer Independence: WARN — same agent implemented and reviewed. Human confirmation is required before any PASS.

## 4. Change Scope
- Files changed: 23
- Planned files changed: 17 (11 crate files listed in `plan.json`, the schema, governance files, the spec move)
- Unplanned files: `.gitignore` (adds `.commandcode/` and `.sdrive/handoff.json` as local tool state)
- Other-feature commits: none. Commit `4f24afb` on this branch carries the hookgate and syncforge *proposal* artifacts; those are sibling specs on the shared branch, not implementations.
- Working tree: clean

## 5. Issues Found (If any)
| Severity | Issue | Location | AC | Fix Required | Fix with |
|----------|-------|----------|----|--------------|----------|
| High | `cargo generate-lockfile && cargo audit` exits 1 on RUSTSEC-2023-0071 (`rsa 0.9.10`, medium 5.9, no fixed upgrade). The advisory is unreachable from `quota-ledger` at build time, but it enters `Cargo.lock` because the `sqlx` facade resolves `sqlx-mysql` even when only `postgres` is enabled. `.cargo/audit.toml` has an empty ignore list, so CI's Audit job goes red at this commit. | `crates/quota-ledger/Cargo.toml:40` | — | Yes | `/sdrive:secure` |
| High | `cargo +1.87.0 build -p quota-ledger --all-features` fails: `error: rustc 1.87.0 is not supported by the following package: uuid@1.29.0 requires rustc 1.89.0`. Forcing an older resolve fails too, because `uuid 1.28.0` also requires 1.89, so the `uuid = "1.28.0"` floor leaves no version that builds on 1.87. CI's `MSRV (1.87)` job runs that build against a fresh resolve, so it fails. | `crates/quota-ledger/Cargo.toml:35` | AC-015 | Yes | `/sdrive:apply` |
| Medium | `connect_insecure_for_tests` is public API in the default feature set, so a production caller can disable TLS with no compile-time guard. It is documented as test-only, but documentation is not enforcement. | `crates/quota-ledger/src/postgres/mod.rs:65` | AC-012 | No | `/sdrive:apply` |
| Low | The commit adds `.commandcode/` and `.sdrive/handoff.json` to `.gitignore`; neither appears in `plan.json`. The change is justified (local tool and session state must not make the tree dirty) but it is unplanned scope. | `.gitignore:10` | — | No | — |
| Low | The shipped schema has no rollback or drop script, and the README documents only how to apply it. A caller who adopts `schema.sql` has no documented reverse. | `crates/quota-ledger/src/postgres/schema.sql:1` | AC-012 | No | — |
| Info | AC-013 proves the limit invariant on `MemoryQuotaStore` only. The Postgres conditional `UPDATE` remains unproven without a database-backed harness, as RISK-001 records. | `crates/quota-ledger/tests/quota-ledger-0-1-core-postgres-backend/main.rs` | AC-013 | No | `/sdrive:test` |

## 6. Recommendations
- Prefer `sqlx-postgres` + `sqlx-core` over the `sqlx` facade. That removes `sqlx-mysql` and `sqlx-sqlite` from the lockfile permanently, which is a real fix rather than an audit ignore.
- If an ignore is chosen instead, `.cargo/audit.toml` already states the rule: record the RUSTSEC id, the reason, and the condition that removes the entry.
- Relax `uuid` to `"1"` as `platform-core` already does. The crate only needs the `v4`, `v7`, and `serde` features, all of which predate the 1.89 floor, and a caret floor of 1.28 is what makes a 1.87 resolve impossible.
- `TEST-015` checks the manifest, the lint attributes, the licence files, and the SQL parameterisation, but it cannot prove the 1.87 build that AC-015 also requires. After T5.2, the MSRV job is the real evidence for that half of the criterion.
- Consider gating `connect_insecure_for_tests` behind a `test-utils` feature so disabling TLS cannot be reached from a default build.
- Add a database-backed concurrency test for the atomic claim when a harness is available; it is the one acceptance criterion whose mechanism is asserted indirectly.

## 7. Security & Performance
| Area | Status | Reason / measurement |
|------|--------|----------------------|
| Security — secrets, dependencies, CON-51x baseline | fail | Secrets clean; dependencies vulnerable (RUSTSEC-2023-0071 via `sqlx-mysql` in the lockfile); CON-512/514/515/516/519/520 pass, CON-511/513/517/518 not applicable. |
| Performance — NFRs, SEO | not_applicable | The specification declares no performance, accessibility, or SEO NFR and the crate exposes no measurable service surface. |

## 8. Handoff
- **If FAIL:** run the command named in the `Fix with` column for each finding, then `sdrive test quota-ledger-0-1-core-postgres-backend`, then `/sdrive:review quota-ledger-0-1-core-postgres-backend` again (round 1 of 3). The review does not modify code — a fix moves `HEAD` and invalidates this verdict.
- **Round cap:** if round 3 fails on the same finding, stop and hand it to a human.
- This review did not merge anything and did not move the feature folder.

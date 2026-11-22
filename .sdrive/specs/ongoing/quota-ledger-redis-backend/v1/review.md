---
status: PASS
feature: quota-ledger-redis-backend
version: v1
commit: 00007430ddea44d0e2a4ee2573e4fb258c03c24c
branch: feature/w4
base: main
reviewedBy: kenule500
reviewedAt: 2026-11-08T12:33:10Z
coverage: 100
evidence: .sdrive/governance/quota-ledger-redis-backend/v1/test-evidence.json
---

# Review Report: quota-ledger 0.2 - Redis backend (v1)

## 1. Audit Summary
- **Status:** PASS (confirmed by the human reviewer)
- **Date:** 2026-11-08
- **Reviewer:** kenule500 (independent: no — this agent line wrote the code; the human reviewer confirmed the verdict explicitly)
- **Version:** v1
- **Commit:** 00007430ddea44d0e2a4ee2573e4fb258c03c24c
- **Branch / base:** feature/w4 / main
- **Round:** 1 of 3

The test phase found a real defect before this review: the budget key was
derived from `Window::start_at(now)`, which for a rolling window is `now`, so
every reserve created its own empty budget. The fix made the key
window-independent and moved the bounds into the hash, replaced inside the
reserve script when a window ends. The five affected criteria now pass.

## 2. Evidence
- Test evidence: `.sdrive/governance/quota-ledger-redis-backend/v1/test-evidence.json` — exit 0, 271 total, 269 passed, 0 failed, 2 skipped, commit `0000743` (matches HEAD)
- Test report: `.sdrive/reports/tests/test-report-quota-ledger-redis-backend-v1-2026-11-08.md` — newer (13:09) than `plan.md` (12:54) and `spec.md` (11:39)
- Verification: `.sdrive/governance/quota-ledger-redis-backend/v1/verification.json` — status `pass`, 7 matched, 0 missing, 0 extra, 2 withdrawn (the Lua scripts, which the scanner cannot read)
- Audit chain: `sdrive audit:verify --json` — 126 hashed events verified
- CI: run [42479566800](https://github.com/kenule500/Rust-packages/actions/runs/42479566800) — success at `0000743`

Commands run in this audit:

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all --check` | exit 0 |
| Lint | `cargo clippy --workspace --all-features -- -D warnings` | exit 0 |
| Lint (targets) | `cargo clippy -p quota-ledger --all-features --all-targets -- -D warnings` | exit 0 |
| Docs | `cargo doc --workspace --all-features --no-deps` (`RUSTDOCFLAGS=-D warnings`) | exit 0 |
| Feature isolation | `cargo check -p quota-ledger --no-default-features` | exit 0 |
| MSRV | `cargo +1.87.0 build --workspace --all-features` | exit 0 |
| Dependencies | `cargo audit` | exit 0, 302 crates |
| Suite | `node scripts/rust-tests.js` (with `REDIS_URL` at `127.0.0.1:6379/15`) | exit 0, 269 passed / 0 failed / 2 skipped |

## 3. Criteria Check
- [x] **Spec Convergence**: all 8 acceptance criteria are implemented and each has a matching `ac_0NN_*` test.
- [x] **Task Completion**: 7/7 tasks are `completed: true`.
- [x] **Traceability**: 13/13 rows `complete`; all 8 ACs mapped; every row carries its test ids.
- [x] **Coverage**: 100% (8/8 in-scope ACs tested, 8/8 passing). No criterion withdrawn.
- [x] **Test Report**: present, dated 2026-11-08, newer than the spec and plan.
- [x] **Test Evidence**: exit 0, 0 failed, 271 total, commit equals HEAD.
- [x] **Change Scope**: 38 files. The crate change (manifest, `lib.rs`, four `src/redis/*` files, README), the acceptance suite, the spec move, and governance. `README.md` and `plan.md` edits document the final key schema and are task-driven.
- [x] **Test Quality**: 8 tests named `ac_0NN_*`, 36 assertions, no `.only`/`xit`/`#[ignore]`, no empty assertions. The six Redis-backed tests return early when no server is reachable — recorded as a finding, not hidden.
- [x] **Code Quality**: fmt, clippy in CI scope, clippy on the crate's targets, and rustdoc are clean; both the default and `--no-default-features` builds compile.
- [x] **Security**: no environment or credential read; the caller supplies the connection; keys are tenant-scoped and escaped; every value reaches Redis as a script argument; errors and `Debug` redact URLs and secrets. Secret scan is a manual pattern scan because `gitleaks`/`trufflehog` are not installed.
- [x] **Testing**: 269 passed, 0 failed, 2 skipped at HEAD with a live Redis.
- [x] **Documentation**: crate README documents the feature, key schema, TTLs, TLS expectation, and the reservation-id index; the module docs state scope and non-goals.
- [x] **NFR Evidence**: no performance, accessibility, or SEO NFR is declared, so performance is `not_applicable` with a reason. NFR-001 to NFR-005 hold, including the 1.87 build.
- [x] **Definition of Done**: every DoD item is met — CI green at HEAD, prerequisite gates approved.
- [x] **Spec-to-Code Verification**: `sdrive verify` status `pass`, 0 missing, 0 extra.
- [x] **CI**: green run for HEAD (`42479566800`, success, `headSha` `0000743`).
- [x] **Permission**: `kenule500` (admin) allowed; run with `SDRIVE_ACTOR` because the sandbox cannot spawn `git` from the checker.
- [x] **Reviewer Independence**: **WARN, escalated and resolved** — the human reviewer confirmed the PASS explicitly.

## 4. Change Scope
- Files changed on the branch: 38 (this feature only; the other wave-4 features have not started)
- Missing planned files: none (2 withdrawn items carry reasons)
- Working tree: clean at review start

## 5. Issues Found
| Severity | Issue | Location | AC | Fix Required | Fix with |
|----------|-------|----------|----|--------------|----------|
| Medium | CI has no Redis, so the six behaviour tests return early and pass by skipping. A regression in the reserve or settlement path would not be caught in CI; only a run with `REDIS_URL` proves them. | `.github/workflows/ci.yml`; `crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs:21` | AC-001–AC-006 | No | `/sdrive:apply` (add a Redis service to the CI job) |
| Low | `usage` reads the budget hash even after the window has ended, so between `window_end` and the key's TTL it reports the previous window's used/held with a past reset instant. The next reserve resets the hash; a read-only caller sees stale numbers until then. | `crates/quota-ledger/src/redis/mod.rs` (`usage`) | AC-006 | No | `/sdrive:apply` |
| Low | The crate version is still `0.1.0` while the feature and spec are titled 0.2. Not one of T1.1–T3.2; a release action should bump it. | `crates/quota-ledger/Cargo.toml` | — | No | release step |
| Info | AC-008's live connection-failure half is not asserted: the client retries with exponential backoff for minutes, so the test covers the malformed-URL path, `Debug` redaction, and a malformed stored payload instead. | `crates/quota-ledger/tests/quota-ledger-redis-backend/main.rs` (AC-008) | AC-008 | No | — |
| Info | The rolling-window key defect (budget key derived from a per-call window start) was found by the acceptance tests and fixed before this review; AC-002/003/004/005/006 were red before the fix. | `crates/quota-ledger/src/redis/keys.rs` | multiple | No | — |

No High finding, and no open finding is `fixRequired`.

## 6. Recommendations
- Add a Redis service container to the CI job so the six behaviour tests actually run there; the suite already supports `REDIS_URL` and skips cleanly without it.
- Have `usage` treat an expired window as a fresh, empty window (or document that the next reserve is what rolls it) so a monitoring read never shows a stale reset.
- Bump the crate to 0.2.0 at release time.

## 7. Security & Performance
| Area | Status | Reason / measurement |
|------|--------|----------------------|
| Security — secrets, dependencies, CON-51x baseline | pass | `cargo audit` clean over 302 crates; no environment or credential read; caller-supplied connection; tenant-scoped escaped keys; every value is a script argument; URL/host redacted in `Config` errors and `Debug`. CON-511/513/516/517/518 not applicable; CON-512/514/515/519/520 pass. |
| Performance — NFRs, SEO | not_applicable | No performance, accessibility, or SEO NFR declared. Each reserve is one `EVALSHA`, each settlement one `EVALSHA`; no target was declared to measure against. |

## 8. Handoff
- **If PASS:** run `sdrive approve quota-ledger-redis-backend review`, then `/sdrive:archive` once the rest of wave 4 is ready.
- **Shared-branch gate:** `feature/w4` also carries `wirekit-self-serve-provider-modules`, `docs-site-and-provider-matrix`, and `design-partner-program-docs`, all still in `backlog`; CON-611 keeps the branch from merging until every member is review-ready.
- **Round cap:** round 1 passed. Any source change moves HEAD and requires a fresh `/sdrive:test` and review.
- This review did not modify code, merge anything, or move the feature folder.

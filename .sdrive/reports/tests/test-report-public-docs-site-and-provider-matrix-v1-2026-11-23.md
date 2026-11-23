# Test Report: Public docs site and provider matrix (v1)

## 1. Summary
- **Date:** 2026-11-23
- **Feature:** public-docs-site-and-provider-matrix
- **Version:** v1
- **Total ACs:** 6
- **Tested ACs:** 6
- **Coverage:** 100%
- **Tests Passed:** 6/6

## 2. AC → Test Mapping
| AC ID | Test ID | File | Status |
|-------|---------|------|--------|
| AC-001 | TEST-001 | crates/public-docs-site-and-provider-matrix/tests/public-docs-site-and-provider-matrix/book.rs | ✅ Pass |
| AC-002 | TEST-002 | crates/public-docs-site-and-provider-matrix/tests/public-docs-site-and-provider-matrix/matrix.rs | ✅ Pass |
| AC-003 | TEST-003 | crates/public-docs-site-and-provider-matrix/tests/public-docs-site-and-provider-matrix/links.rs | ✅ Pass |
| AC-004 | TEST-004 | crates/public-docs-site-and-provider-matrix/tests/public-docs-site-and-provider-matrix/ci.rs | ✅ Pass |
| AC-005 | TEST-005 | crates/public-docs-site-and-provider-matrix/tests/public-docs-site-and-provider-matrix/errors.rs | ✅ Pass |
| AC-006 | TEST-006 | crates/public-docs-site-and-provider-matrix/tests/public-docs-site-and-provider-matrix/book.rs | ✅ Pass |

Test names carry the AC ID:

| Test ID | Test function | Asserts |
|---------|---------------|---------|
| TEST-001 | `ac_001_book_builds_and_exposes_the_five_pages` | `create-missing = false`; all five chapters listed in SUMMARY.md and present on disk; `mdbook build docs` exits 0 and emits the five `.html` pages when mdBook is on `PATH` |
| TEST-002 | `ac_002_matrix_is_generated_from_source_and_drift_fails` | `docs-matrix.js --check` exits 0; twilio reports *implemented* and `src/twilio.rs` exists; adp reports *feature declared, no module* and `src/adp.rs` does not exist; accuweather reports *documented only*; a hand edit to `crates.md` makes `--check` exit 1 naming the stale page; reverting restores a clean check |
| TEST-003 | `ac_003_broken_internal_links_fail_the_check` | `docs-check.js` exits 0 on the clean tree; a missing target, an `http://` link, and a `../` link escaping `docs/src` each make it exit 1 naming the target and the page |
| TEST-004 | `ac_004_ci_builds_the_book_and_runs_checks_on_pull_requests` | workflow triggers on `pull_request`; the `book` job runs both `node scripts/docs-check.js` and `mdbook build docs`, with no `continue-on-error` and no disabling condition; `MDBOOK_VERSION` is pinned and `cargo install mdbook` is absent |
| TEST-005 | `ac_005_error_reference_matches_the_crates_source` | `docs-errors.js --check` exits 0; all of `W2001`–`W2007` appear on the page and in `crates/wirekit/src`; one code from each of the six crates appears on the page and in that crate's own source; the page reports its totals |
| TEST-006 | `ac_006_site_needs_no_js_bundle_or_external_service` | `docs/book.toml` declares no `additional-js`, `additional-css`, custom `theme`, or MathJax, and no `http://` reference; `docs/src` holds no `.js`/`.css` and no page embeds `<script>`; when the book is built, no rendered page loads a remote asset or links over plain HTTP |

## 3. Failures
| Test | Reason | Blocking |
|------|--------|----------|
| — | No failing tests in this run. | No |

## 4. Notes
- **Framework:** Rust's built-in harness (CON-401/402). The suite ran as `cargo test --workspace --all-features` through `sdrive test`: **281 passed, 0 failed, 2 skipped**, which is the 275 pre-existing tests plus these 6.
- **Placement:** a new `publish = false` package, `crates/public-docs-site-and-provider-matrix`, with targets declared by path so sources live at `tests/public-docs-site-and-provider-matrix/<subfeature>.rs` (CON-406). It mirrors `design-partner-program-docs`, the package already in this repo for acceptance-testing Markdown docs, which also drives the shipped checker rather than re-implementing it.
- **The tests drive the shipped scripts.** AC-002, AC-003 and AC-005 shell out to `scripts/docs-matrix.js`, `scripts/docs-errors.js` and `scripts/docs-check.js`. A second implementation of the drift or link rules would agree only until one of the two was edited.
- **Negative cases are real, not assumed.** AC-002 and AC-003 assert that the check *fails*, so a checker that silently passed everything would fail these tests. The mutations are reverted through a `Drop` guard, so a panicking assertion cannot leave the working tree dirty; each mutator lives in a different test binary and cargo runs test binaries sequentially, so they cannot race.
- **Vacuity check (run by hand, not committed):** removing `docs/src/upgrading.md` made TEST-001 fail, and appending `<script src="http://cdn.example/x.js">` to a page made TEST-006 fail. Both were restored and both passed again.
- **AC-001 fallback:** the CI `test` job (three OSes) does not install mdBook, so a hard dependency would break it. TEST-001 runs `mdbook build docs` when mdBook is on `PATH` and otherwise asserts the structural guarantee — `create-missing = false` plus every SUMMARY chapter present, which together make a successful build unable to omit a page. The built-site path was verified locally with mdBook 0.5.4; the CI `book` job runs the same command on every pull request, which TEST-004 checks.
- **Generated pages were regenerated** when this package was added, because the crate index and the error reference read `crates/*/Cargo.toml` and `crates/*/src`. That is the drift mechanism working as specified, not a test edit.
- **Nothing was descoped:** all six criteria are in scope and tested; there are no withdrawn criteria.

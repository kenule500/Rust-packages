---
title: Docs site and provider matrix tasks
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# Docs site and provider matrix tasks

> Production code and content only. Test authoring and execution belong to `/sdrive:test`.

## P1 - Book scaffold and generators

- [ ] **T1.1 - Scaffold the mdBook and its pages** (`FR-001`, `NFR-004` / `AC-001`)
  Add `docs/book.toml`, `docs/src/SUMMARY.md`, `docs/src/introduction.md`, and `docs/src/upgrading.md`. No remote theme or asset; the summary exposes every page from the landing page.
- [ ] **T1.2 - Implement the matrix and crate-index generator** (`FR-002`, `FR-007`, `NFR-002` / `AC-002`, `AC-006`)
  Add `scripts/docs-matrix.js`: read `catalogue/00_provider_index.md` and each crate manifest, write `docs/src/crates.md` and `docs/src/provider-matrix.md` deterministically (stable order, no timestamps), and support `--check` to fail when the generated text differs from disk.
- [ ] **T1.3 - Implement the error-reference generator** (`FR-003` / `AC-005`)
  Add `scripts/docs-errors.js`: scan the crates' error enums for documented stable codes and generate `docs/src/errors.md`, supporting `--check`.

## P2 - Checks and CI

- [ ] **T2.1 - Implement the drift and link checker** (`FR-004`, `FR-007` / `AC-003`, `AC-002`)
  Add `scripts/docs-check.js`: run both generators in check mode and validate every relative link in `docs/src`, failing on a broken link or an `http://` external link.
- [ ] **T2.2 - Wire the docs job into CI** (`FR-005`, `NFR-001`, `NFR-003` / `AC-004`, `AC-006`)
  Add a docs job to `.github/workflows/ci.yml`: install a pinned mdBook (prebuilt, cached), run `mdbook build docs`, then `node scripts/docs-check.js`. The job fails if either step fails.

## P3 - Wiring and verification

- [ ] **T3.1 - Link the site from the README and document the upgrade policy** (`FR-008` / `AC-001`)
  Add the docs-site link to the README and finish `docs/src/upgrading.md` with the 0.x-to-1.0 policy and where breaking changes will be recorded.
- [ ] **T3.2 - Run the verification checks** (`NFR-002`, `NFR-003` / `AC-001`, `AC-004`)
  Run `mdbook build docs`, `node scripts/docs-check.js`, `node .sdrive/scripts/validate-governance.js`, and the existing `cargo fmt --all --check` to confirm the repository baseline is untouched.

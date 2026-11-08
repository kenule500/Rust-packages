---
title: Docs site and provider matrix plan
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# Docs site and provider matrix plan

## Approach

Add an mdBook under `docs/` that is built by CI and publishable as a static site.
Three committed Node scripts already fit the repository's existing `scripts/`
convention: one generates the provider matrix and crate index from the catalogue
and each crate's manifest, one generates the error-code reference by scanning the
crates' source, and one checks for drift and broken internal links. CI installs a
pinned mdBook binary, builds the book, and runs both checks, so the site cannot
claim a status the source contradicts.

## Components

| Component | Action | Notes |
|---|---|---|
| `docs/book.toml` | create | mdBook configuration; no remote theme or assets. |
| `docs/src/SUMMARY.md` | create | Landing, crate index, provider matrix, error reference, upgrade guide. |
| `docs/src/introduction.md` | create | What the ecosystem is and how to read the site. |
| `docs/src/crates.md` | generate | Written by `docs-matrix.js`. |
| `docs/src/provider-matrix.md` | generate | Written by `docs-matrix.js`. |
| `docs/src/errors.md` | generate | Written by `docs-errors.js`. |
| `docs/src/upgrading.md` | create | 0.x-to-1.0 policy and where breaking changes are recorded. |
| `scripts/docs-matrix.js` | create | Generates the crate index and provider matrix; `--check` fails on drift. |
| `scripts/docs-errors.js` | create | Generates the stable error-code reference from source. |
| `scripts/docs-check.js` | create | Drift check plus internal link check over `docs/src`. |
| `.github/workflows/ci.yml` | modify | A docs job: install pinned mdBook, build, run both checks. |
| `README.md` | modify | Link to the docs site. |

## Data model

No runtime data. Inputs: `catalogue/*.md` (provider status), each crate's
`Cargo.toml` (name, description, feature list), and the error enums in
`crates/*/src/error*.rs`. Outputs are three generated Markdown files whose content
is deterministic: no timestamps, stable ordering.

**Flow.** `docs-matrix.js` reads the catalogue index and every crate manifest,
writes `crates.md` and `provider-matrix.md`, and with `--check` compares the
regenerated text to disk. `docs-errors.js` scans the error enums for documented
`XX-*` codes and writes `errors.md`. `docs-check.js` runs both generators in check
mode and verifies every relative link in `docs/src` resolves inside it.

## Security considerations

- **CON-517** — the site is generated from static Markdown; no user input is rendered.
- **CON-519** — examples use placeholders such as `<your-api-key>`; no secret is committed.
- **CON-520** — every external link uses HTTPS; the link check rejects `http://`.
- **NFR-001** — the build performs no network fetch for themes or assets.

## Performance considerations

- **NFR-003** — the mdBook binary is cached; the docs job targets under two minutes.
- **NFR-002** — deterministic generation keeps the drift check stable across machines.

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `mdbook` | external | add | CI tool, pinned; not a crate dependency. |
| `node` | external | no_change | Already used by `scripts/`. |

## Risks

- **RISK-001** — `cargo install mdbook` is slow; use a pinned prebuilt release and cache it.
- **RISK-002** — generated docs drift from source; the check runs in CI, not only locally.
- **RISK-003** — a docs site can over-scope into marketing; the out-of-scope list and AC-006 keep it a static book.

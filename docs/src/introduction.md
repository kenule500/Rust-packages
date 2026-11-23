# Introduction

This is the documentation site for the public Rust packages that GIA and other
projects build and consume. It exists to answer three questions in one place:

- What packages exist, and what state is each one in?
- Which providers are supported, and behind which `wirekit` feature?
- What changes when a crate leaves `0.x`?

The repository is the source of truth. Every status on this site is read from it,
so a page cannot claim a crate or a provider module the code does not contain.

## How to read this site

| Page | What it answers | Where the content comes from |
|---|---|---|
| [Crates](crates.md) | Which packages exist, at what version, and with which features | `crates/*/Cargo.toml` and the package entries in `catalogue/` |
| [Provider matrix](provider-matrix.md) | Which of the 26 providers is implemented, and behind which feature | `catalogue/00_provider_index.md`, `crates/wirekit/Cargo.toml`, and `crates/wirekit/src/` |
| [Error codes](errors.md) | What a stable error code means | The error types in each crate's `src/` |
| [Upgrading](upgrading.md) | What the `0.x` to `1.0` policy is and where breaking changes are recorded | `RELEASING.md` and each crate's `CHANGELOG.md` |

Crates, Provider matrix, and Error codes are generated. Editing them by hand has
no effect: `node scripts/docs-check.js` fails when a generated page no longer
matches the source, and the CI docs job runs that check on every pull request.
Introduction and Upgrading are written by hand.

## What this site is not

- **Not the API reference.** rustdoc on docs.rs is the API reference for each
  crate once it is published. This site links to the crates and explains status,
  not signatures.
- **Not application UI.** The crates expose typed data; the consuming
  application renders it (CON-301). Nothing here is a library feature merely
  because it appears in a provider's documentation (CON-302).
- **Not a product site.** There is no analytics, no custom design system, and no
  JavaScript build step for these pages (AC-006).

## Working on the site

```sh
mdbook serve docs     # live preview at http://localhost:3000
mdbook build docs     # static site in docs/book
node scripts/docs-check.js
```

`node scripts/docs-check.js` runs both generators in check mode and verifies that
every relative link resolves inside `docs/src`. Run it before opening a pull
request; the CI docs job runs the same command.

## Status vocabulary

The generated pages use three provider states and two crate states. They are
computed, never asserted by hand:

- **implemented** — the `wirekit` feature is declared, the module source file
  exists, and the module is declared in `crates/wirekit/src/lib.rs`.
- **feature declared, no module** — the Cargo feature exists but no module source
  does. The feature enables nothing.
- **documented only** — the catalogue has a design file, but there is no feature
  and no module.

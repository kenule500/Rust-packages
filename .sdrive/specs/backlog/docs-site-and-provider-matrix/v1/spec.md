---
title: Docs site and provider matrix
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# Docs site and provider matrix

## Overview

- **Problem Statement**: The repository has no single place a developer can browse. Discovery is the README table plus twelve catalogue files, the provider matrix does not exist, and the README's status paragraph has already gone stale once (it claimed three crates when six were implemented). docs.rs will host API docs at publish time but cannot answer "what exists, which providers are supported, and how do I upgrade".
- **Proposed Solution**: Add an in-repo mdBook under `docs/`, built by CI and publishable as a static site. It carries a landing page, a crate index, a provider matrix generated from the catalogue and the crate manifests, an error-code reference, and an upgrade guide. `mdbook build` plus a matrix-drift check run in CI, so the site cannot claim a status the source disagrees with.
- **Target Audience**: Developers evaluating or adopting the packages, and maintainers who need one canonical status page.
- **Assumptions**:
  - mdBook is the toolchain: a Rust binary, installed in CI, no Node or bundler for the site itself.
  - Node is already used by the repository's test wrapper, so a Node matrix script is consistent with existing tooling and adds no new runtime for consumers.
  - The site is static; no search service beyond mdBook's built-in search.
  - Publishing (GitHub Pages or similar) is a deployment decision, recorded but not required for the feature to pass.

## User Stories

### US-001: Find what exists and how to use it
As a developer evaluating the packages, I want one browsable site, so that I can see the crates, providers, and status without reading the repository tree.

**Acceptance Criteria:**
- AC-001: Given the repository, when I run `mdbook build docs`, then it produces a static site with a landing page, a crate index, a provider matrix, an error-code reference, and an upgrade guide.
- AC-002: Given a crate whose status changes in `Cargo.toml` or its catalogue file, when the matrix is regenerated, then the provider matrix and crate index reflect that status and a stale hand-written status fails the drift check.
- AC-003: Given the built site, when internal links are checked, then every link resolves and the check fails CI on a broken one.
- AC-004: Given the CI workflow, when it runs on a pull request, then it builds the book and runs the drift and link checks, and the job fails if either fails.
- AC-005: Given a published API, when a developer reads the error-reference page, then every documented `WirekitError` code and every crate's stable error codes are listed from source rather than typed by hand.
- AC-006: Given the site, when it is built, then it requires no JavaScript bundle, no Node build step for the pages themselves, and no external service.

## Requirements

### Functional Requirements
- FR-001: A `docs/` mdBook MUST exist with `book.toml` and the five required pages.
- FR-002: The provider matrix MUST be generated from the catalogue and crate manifests by a committed script, and a check MUST fail when the generated file is stale.
- FR-003: The error-code reference MUST be generated from the crates' source, not hand-maintained.
- FR-004: A link check MUST run over the built site and fail on a broken internal link.
- FR-005: CI MUST build the book and run both checks on every pull request.
- FR-006: The site MUST NOT introduce a JavaScript build step or a Node runtime dependency for consumers (CON-301 applies to crates only; this is repo documentation).
- FR-007: The site MUST NOT claim a crate or provider status that the source contradicts (the 2026-11-08 stale README is the failure mode).
- FR-008: The upgrade guide MUST document the 0.x-to-1.0 policy and where breaking changes will be recorded.

### Non-Functional Requirements
- NFR-001: `mdbook build` MUST run without a network connection once mdBook is installed (no remote theme or asset fetch).
- NFR-002: The generated pages MUST be reproducible: same input, same bytes, no timestamps in content.
- NFR-003: The docs MUST build on the repository's CI runner within the existing job budget (target: under two minutes for the docs job).
- NFR-004: Every page MUST be reachable from the landing page within two clicks (SUMMARY structure).

## UI/UX & Data Requirements

- **UI/UX**: mdBook's default theme; no custom design system (CON-303 records that none exists).
- **Data**: Inputs are `catalogue/*.md`, each crate's `Cargo.toml`, and the crates' error enums. The generator writes `docs/src/provider-matrix.md`, `docs/src/errors.md`, and `docs/src/crates.md`.

## Edge Cases & Error States

- EC-001: A crate is added to the workspace but not to the matrix → the drift check names the missing crate and fails.
- EC-002: A provider is withdrawn from a crate but its catalogue file remains → the matrix shows the catalogue status and the crate status separately rather than guessing.
- EC-003: mdBook is missing in CI → the job installs it explicitly and fails loudly rather than silently skipping the build.
- EC-004: A relative link points outside `docs/src` → the link check reports it as broken instead of resolving silently.

## Out of Scope

- A hosted domain, custom design, analytics, or a CMS.
- A JavaScript documentation framework (Docusaurus, Nextra, VitePress).
- API reference generation: rustdoc on docs.rs remains the API reference; this site links to it.
- Translating the docs.

## Dependencies & Risks

- `mdbook` (CI tool, not a crate dependency); Node is already present for `scripts/`.
- **RISK-001**: `cargo install mdbook` is slow in CI; use a pinned prebuilt binary action or a cached install, and record the pin.
- **RISK-002**: Generated documentation can drift from source; the drift check is the mitigation and must run in CI, not just locally.
- **RISK-003**: A docs site is easy to over-scope into a marketing site; the out-of-scope list and AC-006 keep it a static book.

## Security Baseline

- CON-511 Authentication: NOT APPLICABLE — static documentation.
- CON-512 Authorization: NOT APPLICABLE — no tenant data.
- CON-513 Security Headers: NOT APPLICABLE at build time; the deployment edge documents headers.
- CON-514 Rate Limiting: NOT APPLICABLE — no service.
- CON-515 Input Validation: NOT APPLICABLE — no untrusted input at runtime.
- CON-516 Injection: NOT APPLICABLE — no database.
- CON-517 XSS: REQUIRED — generated HTML MUST escape content and the site MUST NOT embed raw user input.
- CON-518 CSRF: NOT APPLICABLE — no forms.
- CON-519 Environment Security: REQUIRED — no secrets in docs; examples use placeholders.
- CON-520 HTTPS: REQUIRED — every external link uses HTTPS.

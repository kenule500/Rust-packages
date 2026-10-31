---
title: Stripe provider — decision record — execution tasks
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Execution Tasks: Stripe provider decision record (Option B)

> Documentation only. No task writes code, adds a dependency, or runs tests.

## P1 — Publish the decision

- [ ] **T1.1 — Add the Stripe guidance to the wirekit README**
  - Description: a "Payments (Stripe)" section stating there is no in-crate module, pinning `async-stripe 0.41.0` (MIT OR Apache-2.0, rust-version 1.82.0), recommending restricted keys, and pointing at its `webhook-events` feature for signature verification.
  - Requirements: FR-002, FR-004, NFR-002 · AC: AC-002, AC-004

- [ ] **T1.2 — Record the decision in the provider index**
  - Description: an entry in `catalogue/00_provider_index.md` naming Option B, recording that the build option was rejected (so CON-105's design file is not required), and listing the revisit triggers.
  - Requirements: FR-001, FR-002, FR-003, FR-005, NFR-001 · AC: AC-001, AC-002, AC-003, AC-005

- [ ] **T1.3 — Cross-reference from the root README**
  - Description: a pointer from the ecosystem README so it does not imply Stripe support, linking to the `wirekit` section and the provider index entry.
  - Requirements: FR-004 · AC: AC-004

- [ ] **T1.4 — Verify documentation and governance**
  - Description: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`, `node .sdrive/scripts/validate-governance.js`, and a link check of the three edited Markdown files.
  - Requirements: NFR-003 · AC: AC-002

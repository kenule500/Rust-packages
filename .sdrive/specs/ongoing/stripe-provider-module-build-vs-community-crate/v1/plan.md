---
title: Stripe provider — decision record — technical plan
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Technical Plan: Stripe provider decision record (Option B)

## 1. Approach

Documentation only. The decision is Option B: `wirekit` does not build a Stripe module and does not depend on a Stripe crate. The work is to publish the decision where a developer will look — the `wirekit` README, the provider index, and the root README — pinned to `async-stripe 0.41.0` (`MIT OR Apache-2.0`, `rust-version = "1.82.0"`, published 2025-04-08).

No source file, Cargo manifest, or catalogue design file changes. That is deliberate: `CON-103` (one job per package) and `NFR-003` forbid scope creep from a decision into an implementation.

## 2. Components

| Component | Action | Notes |
|---|---|---|
| `crates/wirekit/README.md` | modify | Add a "Payments (Stripe)" section: no in-crate module, use `async-stripe 0.41`, restricted keys, webhook verification via its `webhook-events` feature, MSRV note |
| `catalogue/00_provider_index.md` | modify | Record the Stripe decision: not a `wirekit` module, documented community crate, revisit triggers |
| `README.md` | modify | Cross-reference the decision from the provider table area so the ecosystem README does not imply Stripe support |
| `crates/wirekit/src/*` | no_change | Explicitly untouched (NFR-003) |
| `crates/wirekit/Cargo.toml` | no_change | No dependency added |

Referenced requirements: FR-001 … FR-005, NFR-001 … NFR-003.

## 3. Data model

- **Entities**: none. This record ships no code and models no data.
- **Flow**: a developer reads the README section or the provider index, sees Option B with the pinned version, licence, and MSRV, and adds `async-stripe` in their own application.

## 4. API contracts

None. No endpoint is implemented by this feature.

## 5. Security considerations

- **NFR-001:** the published guidance cites crates.io with dates for the version, licence, and MSRV; nothing is asserted without a source (CON-704).
- **NFR-002:** the guidance recommends restricted keys and states the licence and MSRV so a consumer can check compatibility before adopting.
- **NFR-003:** no code and no dependency enter `wirekit`; the security surface of Stripe stays in the consuming application.

## 6. Performance considerations

None. Documentation only; no runtime path changes.

## 7. Dependencies

| Name | Type | Action | Notes |
|---|---|---|---|
| `async-stripe` | library | no_change | Documented for consumers, never added to `wirekit`. Pinned at 0.41.0, MSRV 1.82.0. |

## 8. Risks

- **RISK-001** (closed): `async-stripe` 0.41.0 declares `rust-version = "1.82.0"`, inside the workspace MSRV.
- **RISK-002**: the documentation could be read as endorsement. Mitigation: state the scope limits and the revisit triggers in both places.
- **RISK-003**: `async-stripe` 1.0 ships with a 1.88 MSRV and becomes the obvious recommendation. Mitigation: revisit trigger recorded; the record then changes, not the code.

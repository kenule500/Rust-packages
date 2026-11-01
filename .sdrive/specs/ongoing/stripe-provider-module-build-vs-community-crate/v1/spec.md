---
title: Stripe provider — build vs community crate (decision record)
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Stripe provider — build vs community crate (decision record)

## Overview

- **Problem Statement**: `BP-014` is a decision before it is code. The catalogue lists 26 providers but Stripe is absent (`catalogue/00_provider_index.md` records the gap) even though the product material references payments. Developers must either build their own Stripe client or pick a community crate with unknown maintenance, licence, and MSRV fit. Neither path is written down, so `BP-016` cannot say whether Stripe is in `wirekit` 0.2.x.
- **Proposed Solution**: This is a **decision record**, not an implementation. It scores two options against fixed criteria, records a recommendation, and — once the human picks one at Gate 1 — records the chosen option and its consequences. Implementation, if chosen, is a later version.
- **Target Audience**: The maintainer deciding `wirekit` 0.2.x scope, and Rust developers who need payments.
- **Assumptions**:
  - `wirekit` has no Stripe code today (no `stripe` module in `crates/wirekit/src`).
  - `CON-104` requires providers to ship as `wirekit` modules; `CON-105` requires a catalogue design file first.
  - Evidence below comes from the crates.io API on 2026-10-31; no further external review was performed (CON-704).

## Decision criteria

| # | Criterion | Why it matters |
|---|---|---|
| C1 | Maintenance and release recency | A payments client that stops tracking Stripe's API is a liability. |
| C2 | MSRV fit (workspace is 1.85) | `CON-102` pins `rust-version = "1.85"`; a dependency above it breaks the MSRV job. |
| C3 | Licence compatibility | The workspace is `MIT OR Apache-2.0`; a copyleft dependency is excluded. |
| C4 | Surface we control (errors, retry, redaction) | `CON-107` and the one-model promise. |
| C5 | Security surface (webhook signature, secrets) | Payments input is the highest-risk input in the ecosystem. |
| C6 | Cost to build and cost to exit | Time to first release, and how hard a reversal is. |

## Options evaluated

### Option A — build a `stripe` module in `wirekit`

- **Evidence**: no Stripe code exists; the crate already carries `reqwest`, `serde_json`, and `hmac`, so a narrow surface needs no new dependency.
- **Surface that would be built**: `PaymentIntent` create/confirm/retrieve, `Customer` create/retrieve, and webhook verification (`Stripe-Signature`, HMAC-SHA256, constant time) plus typed error mapping. Everything else is out of scope.
- **C1 pass** (we own it) · **C2 pass** (1.85) · **C3 pass** · **C4 pass** (reuses `WirekitError`, retry, rate limit, `Secret`) · **C5 pass with work** (signature verification and redaction are ours) · **C6 cost high** — needs a design file (CON-105) plus the surface above.
- **Risk**: Stripe's breadth tempts scope creep; the module must stay narrow or it becomes a framework (CON-103).

### Option B — document a community crate

- **`async-stripe`**: active; `1.0.0-rc.10` published 2026-10-26, stable line `0.41.0`, ~5.8M downloads, `MIT OR Apache-2.0`, repository `github.com/arlyon/async-stripe`. Recent 1.0 alphas declare `rust-version = 1.88.0`.
- **`stripe-rust`**: last release `0.12.3` on **2020-05-17**; no release in six years, so it is treated as unmaintained.
- **C1 mixed** (async-stripe yes; stripe-rust no) · **C2 at risk** (1.0 declares 1.88; `0.41.0` needs verification) · **C3 pass** · **C4 no** (errors, retry, and webhook handling live outside `wirekit`) · **C5 depends on the crate** · **C6 cost low** — documentation only.
- **Risk**: a pre-1.0 crate couples `wirekit`'s MSRV and release cadence to it; documenting `stripe-rust` would point users at dead code.

## Recommendation (confirmed at Gate 1)

1. **Preferred: Option B with `async-stripe` 0.x**, conditional on a verified MSRV of 1.85 or lower for the chosen 0.41.x release. Documentation only; no new dependency in `wirekit`.
2. **Fallback: Option A, narrow surface** if no `async-stripe` release fits 1.85 and raising the MSRV is not approved. Then the module covers only the surface listed above, after a `catalogue/38_stripe.md` design file.
3. **Rejected: `stripe-rust`** — no release since 2020.

## User Stories

### US-001: Decide with evidence
As a maintainer, I want both options scored against fixed criteria, so that `0.2.x` scope is a recorded decision rather than an assumption.

**Acceptance Criteria:**
- AC-001: Given the decision record, when it is read, then both options are scored against C1..C6 with crates.io evidence and dates.
- AC-002: Given Gate 1 approval, when the record is finalised, then it names the chosen option and the approver, with no option left implied.
- AC-003: Given the build option is chosen, when the record is finalised, then it states that CON-105 requires a catalogue design file and enumerates the exact surface to build.
- AC-004: Given the community-crate option is chosen, when the record is finalised, then it pins crate name, version, licence, MSRV, and the documentation pointer `wirekit` will publish.
- AC-005: Given the record, when the decision later needs revisiting, then the triggers are explicit (async-stripe 1.0 stable, MSRV change, a Stripe API break, or the crate becoming unmaintained).

## Requirements

### Functional Requirements
- FR-001: The record MUST evaluate both options against the six criteria with cited evidence (AC-001).
- FR-002: The record MUST name the chosen option and its rationale after Gate 1 approval (AC-002).
- FR-003: If the build option is chosen, the record MUST require a catalogue design file under CON-105 and enumerate the exact surface (AC-003).
- FR-004: If a community crate is chosen, the record MUST pin name, version, licence, MSRV, and the documentation pointer (AC-004).
- FR-005: The record MUST define revisit triggers (AC-005).

### Non-Functional Requirements
- NFR-001: Evidence MUST carry source and date; unknown facts MUST stay marked `[to be confirmed]` (CON-704).
- NFR-002: A chosen community crate MUST be licence-compatible (MIT/Apache-2.0) and MUST NOT raise the workspace MSRV without a constitution amendment (CON-102, CON-111).
- NFR-003: This record MUST NOT add code, dependencies, or a catalogue file; it is a decision only (CON-103).

### Security Baseline
| Control | Status | Decision for this feature |
|---|---|---|
| CON-511 Authentication | REQUIRED | The chosen option must state how API keys are supplied and redacted. |
| CON-512 Authorization | REQUIRED | Restricted versus secret keys must be part of the chosen option's guidance. |
| CON-513 Security Headers | NOT APPLICABLE | No HTTP server is added. |
| CON-514 Rate Limiting | REQUIRED | The chosen option keeps `wirekit`'s token bucket or documents the alternative. |
| CON-515 Input Validation | REQUIRED | Webhook payload validation is part of the chosen option's surface. |
| CON-516/517/518 | NOT APPLICABLE | No database, no markup, no browser session. |
| CON-519 Environment Security | REQUIRED | Keys come from the caller or a secret store; no `.env` reads. |
| CON-520 HTTPS / HSTS | REQUIRED | Any client used must be TLS-only. |

## Edge Cases & Error States
- EC-001: `async-stripe` publishes a stable release whose MSRV exceeds 1.85 -> the fallback (Option A) becomes the recommendation.
- EC-002: No community crate supports 1.85 -> Option A, narrow surface, is the only viable path.
- EC-003: Stripe ships a breaking API change that neither option covers -> the revisit trigger fires.

## Out of Scope
- Any Stripe implementation, dependency, or catalogue design file.
- Billing domain logic (invoices, subscription lifecycles, tax) — that belongs to the consuming application (CON-106).

## Dependencies & Risks
- **Dependencies**: none added by this record. Option A would add none; Option B adds a dependency only in the consuming application.
- **RISK-001**: The recommendation may rest on an unverified MSRV for `async-stripe` 0.41.x. Mitigation: verify the exact version before Gate 1 finalisation.
- **RISK-002**: A decision record can decay into a stale opinion. Mitigation: explicit revisit triggers (AC-005).

## Decision (Gate 1, 2026-10-31)

- **Chosen option:** **B — document the community crate.** No Stripe code is added to `wirekit` by this record.
- **Pinned crate:** `async-stripe` **`0.41.0`** (stable line), `MIT OR Apache-2.0`, repository `github.com/arlyon/async-stripe`, published 2025-04-08, ~2.48M downloads on that version.
- **Verified MSRV:** `rust-version = "1.82.0"`, edition 2021 — compatible with the workspace's 1.85, so no constitution amendment is required (CON-102). RISK-001 is closed by this verification.
- **1.0 line:** `1.0.0-rc.10` (2026-10-26) declares `rust-version = "1.88.0"`; it stays out of scope until it ships stable and the MSRV question is revisited (EC-001).
- **Rejected:** `stripe-rust` (no release since 2020-05-17).
- **Approver:** kenule500, Gate 1, 2026-10-31T12:32:41Z.
- **Consequences:** `wirekit` gains documentation and a provider-index entry only. The dependency belongs to the consuming application, and `wirekit`'s MSRV, licence, and release cadence are unchanged.




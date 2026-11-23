---
title: wirekit - self-serve provider modules
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# wirekit - self-serve provider modules

## Overview

- **Problem Statement**: `wirekit` ships eleven provider modules, but six providers with self-serve access — AccuWeather, Lob, PagerDuty, Deepgram, Yelp, and Klarna — have catalogue designs and no client. Developers using them still hand-roll an HTTP client, a second error type, and a retry policy that disagrees with the rest of the workspace. The partner-gated providers (ATTOM, CoreLogic, BuildZoom, ADP, Gusto, Google Business, Jobber, Wisetack, LeadPerfection) cannot be verified without contracts, so they are deliberately excluded.
- **Proposed Solution**: Add six provider modules to `wirekit`, each behind its own additive feature, each mapping into the shared `WirekitError`, retry policy, rate limiter, and auth layer. Every module implements the surface in its catalogue design file and nothing beyond it. The default build stays empty, so a consumer pays only for the providers it enables.
- **Target Audience**: Rust developers who use these providers and want one client, one error model, and one retry policy across them.
- **Assumptions**:
  - The catalogue design files `08_accuweather.md`, `18_lob.md`, `22_pagerduty.md`, `24_deepgram.md`, `19_yelp.md`, and `25_klarna.md` are the contract; no endpoint is invented.
  - Sandbox credentials exist for at least the providers with free tiers (PagerDuty, Deepgram, Lob test mode, AccuWeather trial); others are verified against recorded fixtures.
  - Partner-gated providers stay `needs discovery` and are NOT part of this feature.
  - Each provider's feature is additive and independent (CON-204); enabling one never enables another.

## User Stories

### US-001: Use a self-serve provider through the shared client
As a Rust developer, I want a typed client for my provider, so that I get the workspace's error, retry, rate-limit, and auth behaviour without writing my own.

**Acceptance Criteria:**
- AC-001: Given the `accuweather`, `lob`, `pagerduty`, `deepgram`, `yelp`, or `klarna` feature, when the module is enabled, then it exposes the request and response types listed in its catalogue design file and no endpoint that the file does not list.
- AC-002: Given any of the six modules, when a provider call fails, then the failure is a `WirekitError` variant and no module introduces a second error type (CON-207).
- AC-003: Given a provider that returns a body-level error with HTTP 200, when the call is parsed, then it is reported as an error, not a success (CON-213).
- AC-004: Given a 429 with `Retry-After`, when the call is retried, then the delay honours the header; a 4xx other than 429 is never retried (CON-208).
- AC-005: Given each module, when the client is built, then the provider's rate limiter is applied and its secret is redacted in `Debug`, logs, and errors (CON-214).
- AC-006: Given the default feature set, when the crate builds, then none of the six modules is compiled, and each feature enables exactly its own module.
- AC-007: Given the crate with all features on Rust 1.87, when it builds, then it compiles with `forbid(unsafe_code)` and `deny(missing_docs)` and every module states its scope and non-goals.

## Requirements

### Functional Requirements
- FR-001: `wirekit` MUST gain `accuweather`, `lob`, `pagerduty`, `deepgram`, `yelp`, and `klarna` modules, each behind its own additive feature.
- FR-002: Every module MUST map failures into `WirekitError` (CON-207).
- FR-003: Auth MUST use the shared auth layer (API key header or query, bearer, basic) and MUST NOT hand-roll a header.
- FR-004: Retry and rate limiting MUST use the shared policy; `Retry-After` MUST be honored (CON-208, CON-209).
- FR-005: A provider body-level error with HTTP 200 MUST be an error (CON-213).
- FR-006: Secrets MUST be redacted everywhere they could render (CON-214).
- FR-007: The default feature set MUST remain empty and features MUST be independent (CON-204).
- FR-008: The modules MUST NOT add application or vendor workflow vocabulary (CON-106).

### Non-Functional Requirements
- NFR-001: The crate MUST compile on Rust 1.87 with all features (CON-102).
- NFR-002: Every public item MUST carry rustdoc and every module MUST state scope and non-goals (CON-202, CON-203).
- NFR-003: Untrusted provider JSON MUST be validated into typed values; a malformed body MUST be a typed error, not a panic (CON-515).
- NFR-004: Every outbound call MUST emit request start, finish, retry, rate-limit, and error tracing events (CON-211).

## UI/UX & Data Requirements

- **UI/UX**: Not applicable (CON-301).
- **Data**: Each module carries the request/response structs from its catalogue file; no persistence, no database.

## Edge Cases & Error States

- EC-001: A provider returns 200 with an error envelope → typed error (e.g. Yelp `error`, PagerDuty `error.message`).
- EC-002: A paginated endpoint runs past the last page → an empty typed page, never an infinite loop.
- EC-003: A provider rotates its API key → the client is rebuilt; no credential is cached in the crate.
- EC-004: A feature is enabled alone → only that module compiles; no other module's dependency appears.
- EC-005: An upstream 5xx survives the retry budget → `WirekitError` with the final status and the request id, no secret.

## Out of Scope

- ATTOM, CoreLogic, BuildZoom, ADP, Gusto, Google Business, Jobber, Wisetack, and LeadPerfection: partner-gated, recorded as `needs discovery`, not part of this feature.
- Stripe: decided as a community crate (BP-014).
- Any new dependency unless a provider's auth or payload shape requires it; the shared stack covers HMAC, SHA-1/256, base64, and JSON.

## Dependencies & Risks

- No new library is expected: the six modules reuse `reqwest`, `serde`, `serde_json`, `hmac`, `sha2`, `sha1`, `base64`, and `hex` already in the crate.
- **RISK-001**: Yelp's leads endpoint may be partner-only; if the catalogue marks it restricted, the leads surface is withdrawn with a citation rather than faked.
- **RISK-002**: Six modules in one feature is a large review unit; each module is reviewed against its catalogue file and can be withdrawn independently without touching the others.
- **RISK-003**: Sandbox access may be missing for some providers, so acceptance tests use recorded fixtures and the report says which providers were not exercised live.

## Security Baseline

- CON-511 Authentication: REQUIRED — the shared auth layer per provider.
- CON-512 Authorization: NOT APPLICABLE — the crate holds no tenant state.
- CON-513 Security Headers: NOT APPLICABLE — no server surface.
- CON-514 Rate Limiting: REQUIRED — the per-provider token bucket.
- CON-515 Input Validation: REQUIRED — NFR-003.
- CON-516 Injection: NOT APPLICABLE — no database or query construction.
- CON-517 XSS: NOT APPLICABLE — no markup.
- CON-518 CSRF: NOT APPLICABLE — no browser session.
- CON-519 Environment Security: REQUIRED — credentials come from the caller, never the environment.
- CON-520 HTTPS: REQUIRED — every base URL is HTTPS and non-HTTPS URLs are rejected.

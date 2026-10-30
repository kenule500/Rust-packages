---
title: Google OAuth shared helper - wirekit::google_oauth 0.2.0
status: backlog
version: v1
created: 2026-10-30
updated: 2026-10-30
---

# Google OAuth shared helper

## Overview

### Problem Statement

`wirekit` has no OAuth2 helper. `wirekit::google_lsa` exists and works, but it takes
a bare access token the caller obtained elsewhere; every future Google module
(Business Profile, Gmail, LSA notifications) would have to re-implement
authorization URL construction, PKCE, code exchange, refresh, and revocation. That
duplication is where the security errors live: an implicit flow slipping in, a
missing PKCE verifier, a state value reused, a refresh loop on 401, a token cached
past expiry.

`catalogue/28_google_oauth.md` specifies this module, and `CON-212` already requires
Authorization Code + PKCE and forbids the implicit flow. Nothing implements it yet.

### Proposed Solution

Add `wirekit::google_oauth` behind an additive `google-oauth` feature:
`GoogleOAuthClient` (client id/secret, redirect URI, scopes), `AuthorizationRequest`
(authorization URL, state, PKCE challenge), `TokenResponse` (access, refresh, expiry,
scope), a `TokenStore` trait, and `RevokeRequest`. The module builds authorization
URLs, exchanges codes, refreshes access tokens, revokes them, and validates state and
PKCE.

**Token storage decision.** `wirekit` performs no I/O and stores nothing (CON-108), so
the crate ships the `TokenStore` trait and an in-memory implementation suitable for
tests and single-process callers. Durable, encrypted persistence is the implementor's
contract, documented in the module. The catalogue's open question 4 is answered by
making the boundary explicit rather than by reading or writing a file.

### Target Audience

- **Primary:** external Rust developers connecting a Google account from a server
  process who need PKCE, refresh, and revocation correct without re-deriving OAuth.
- **Secondary:** maintainers of this workspace, who need one auth path shared by every
  Google module instead of one per provider.

### Assumptions

- `wirekit` core (`BP-005`) is on `main`; it is, after `m1-first-publish` archived.
  `[to be confirmed]` only for the exact `WirekitError::Auth` variants available.
- One Google Cloud project serves all callers; per-tenant apps are out of scope
  (catalogue open question 1).
- Google's `oauth2.googleapis.com` contract is as documented in `catalogue/28`; no
  live Google account is used by any task here.
- No external research was available for this spec. Every claim comes from the
  catalogue, the constitution, and the existing `wirekit` source.

### Out of Scope

- Reading, writing, encrypting, or rotating tokens on disk: `TokenStore` is a trait.
- Google Business Profile, Gmail, or LSA notification modules themselves.
- Per-tenant Google Cloud projects, app verification, and branding review.
- Incremental-consent UX beyond accepting the scope set the caller supplies.
- Any implicit-flow support (`CON-212` forbids it).
- Application UI, product nouns, and billing (CON-106).
- Test authoring and execution (`/sdrive:test` owns them).

### Constitution Alignment

| Rule | How this feature complies |
|---|---|
| CON-103 | Auth helper only; no framework, no HTTP client of its own beyond core |
| CON-104 | Ships as a `wirekit` feature module, not a crate |
| CON-108 | No I/O: tokens are handed to a `TokenStore` the caller supplies |
| CON-109 | No global token cache; every client is constructed and injected |
| CON-201 | `wirekit` keeps `#![forbid(unsafe_code)]` |
| CON-202 | Every public item documented; `deny(missing_docs)` stays |
| CON-207 | Failures map into `WirekitError`; no second error type |
| CON-208 | One refresh on 401, then fail; no retry loop |
| CON-209 | Token-endpoint rate limiting uses the shared bucket |
| CON-211 | Authorization, exchange, refresh, and revoke emit tracing events |
| CON-212 | Authorization Code + PKCE only; implicit flow impossible by construction |
| CON-214 | Client secret and refresh token are redacted and never `Serialize` |
| CON-520 | The token endpoint must be HTTPS; loopback allowed only when explicit |

### Anti-Redundancy Check

| Checked | Result | Decision |
|---|---|---|
| `crates/**` for `google_oauth` | **None** | `create` — genuinely unbuilt |
| `wirekit::google_lsa` | Exists; accepts a caller-supplied token | `modify` — accepts the helper as its token source |
| `wirekit::auth` | OAuth2 client-credentials only (`CON-212` is authorization code) | `reuse` — PKCE path is new |
| `wirekit` secret type | Exists | `reuse` |
| Any existing PKCE implementation | None | `create` |

## User Stories

### US-001: Start a Google authorization safely

As a developer connecting a Google account, I want an authorization URL with state
and PKCE, so that a stolen callback cannot be replayed or forged.

**Acceptance Criteria:**

- AC-001: Given a client id, redirect URI, and scope set, when `authorization_url` is called, then the URL targets `https://oauth2.googleapis.com/o/oauth2/v2/auth` with `response_type=code`, the scopes, the redirect URI, a random `state`, and `code_challenge` with `code_challenge_method=S256`.
- AC-002: Given the returned request, when the same client builds another, then `state` and `code_verifier` differ, and neither is derived from the client secret.
- AC-003: Given a caller asks for the implicit flow, when the request is built, then no such option exists in the API — there is no parameter that omits PKCE.

### US-002: Exchange, refresh, and revoke

As a developer holding a callback, I want to exchange the code, refresh on expiry,
and revoke on offboarding, so that I never write OAuth myself.

**Acceptance Criteria:**

- AC-004: Given a valid code and matching verifier, when `exchange_code` runs, then it POSTs the code and verifier to the token endpoint and returns a typed `TokenResponse` with access token, optional refresh token, absolute expiry, and scopes.
- AC-005: Given the endpoint answers `400` with an `error`/`error_description` body, when the exchange fails, then the error is `WirekitError::Auth` carrying the provider code, and no token is returned.
- AC-006: Given a stored token that is past expiry minus the safety margin, when a token is requested, then exactly one refresh occurs, the store is updated, and a second 401 fails without a second attempt.
- AC-007: Given a refresh token, when `revoke` runs, then the token endpoint is called once and the store record is removed.

### US-003: Keep credentials out of logs and state out of the library

As a security reviewer, I want the secret and tokens never rendered and the library
to own no persistence, so that the blast radius of a log line or a crash is bounded.

**Acceptance Criteria:**

- AC-008: Given a client, `TokenResponse`, and every error path, when `Debug`, `Display`, tracing, and error payloads are inspected, then the client secret, access token, and refresh token render `***`, and their types do not implement `Serialize`.
- AC-009: Given the crate is compiled, when its dependencies are inspected, then it performs no file, database, environment, or network read or write beyond the OAuth endpoints, and a `TokenStore` implementation is supplied by the caller.

## Requirements

### Functional Requirements

- FR-001: `wirekit` MUST provide `google_oauth::{GoogleOAuthClient, AuthorizationRequest, TokenResponse, TokenStore, RevokeRequest}` behind an additive `google-oauth` feature that is default-off and enables no other feature.
- FR-002: `authorization_url` MUST build a URL against `https://oauth2.googleapis.com/o/oauth2/v2/auth` carrying `response_type=code`, the caller's scopes, the redirect URI, a CSPRNG `state`, and a `code_challenge` derived from a freshly generated verifier using `S256`.
- FR-003: The API MUST NOT offer a way to omit PKCE or to use the implicit flow; `CON-212` is enforced by the type, not by documentation.
- FR-004: `exchange_code` MUST POST the code and verifier to `https://oauth2.googleapis.com/token` and MUST return a typed `TokenResponse` whose expiry is absolute, computed from the provider's `expires_in` and the caller's clock source.
- FR-005: The module MUST map Google's `error` and `error_description` into `WirekitError::Auth` for credential and request failures and `WirekitError::Api` for provider-side failures, preserving the provider code (CON-207).
- FR-006: Token acquisition MUST refresh at most once on `401`, MUST persist the refreshed token through the `TokenStore` trait, and MUST fail rather than loop (CON-208).
- FR-007: `revoke` MUST call the revocation endpoint once and MUST remove the record through the `TokenStore`.
- FR-008: `TokenStore` MUST be a trait with object-safe load, save, and delete operations, and the crate MUST ship an in-memory implementation for tests and single-process callers.
- FR-009: The module MUST accept a configurable token endpoint base URL for sandbox and test use, MUST default to the documented Google endpoint, and MUST reject a non-HTTPS override unless it is loopback and explicitly allowed (CON-520).
- FR-010: The client secret, access token, and refresh token MUST be held in the `wirekit` secret type, MUST render `***` in `Debug`, `Display`, tracing, and errors, and their types MUST NOT implement `Serialize` (CON-214).
- FR-011: The module MUST emit tracing events for authorization, exchange, refresh, revoke, and every error, and MUST NOT record a token, verifier, or secret (CON-211).
- FR-012: `wirekit::google_lsa` MUST accept a `GoogleOAuthClient` token source instead of a bare token string, and MUST keep working for callers that supply a static token.

### Non-Functional Requirements

- NFR-001: Credentials MUST come from the caller at construction; none may be embedded, defaulted, or read from the environment (CON-511, CON-519).
- NFR-002: No global or static token cache; every client owns its own state (CON-109, CON-512).
- NFR-003: The token endpoint MUST be rate-limited through the shared bucket with a caller override (CON-209, CON-514).
- NFR-004: Every untrusted input — provider error bodies, redirect parameters, scope strings — MUST be validated and MUST produce a typed error rather than a panic (CON-515).
- NFR-005: The crate MUST keep `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`, MSRV 1.85, edition 2021 (CON-201, CON-202, CON-102).
- NFR-006: Security Baseline `CON-516` is **NOT APPLICABLE** — no database, cache, or query surface. `CON-513`, `CON-517`, and `CON-518` are also not applicable: no server responses, markup, or browser sessions.

## Edge Cases & Error States

- EC-001: Callback `state` does not match the stored value → `WirekitError::Auth`; no exchange attempted.
- EC-002: Authorization code replayed → provider error surfaced as `Auth`; the store is not written.
- EC-003: Redirect URI mismatch → provider error surfaced verbatim, never silently rewritten.
- EC-004: Refresh token absent because the provider omitted it → typed error telling the caller to re-authorize; no panic.
- EC-005: Two callers refresh the same record concurrently → at most one network refresh per client; last write wins and is documented.
- EC-006: Clock returns a time before the recorded issue time → expiry treated as elapsed, one refresh occurs.
- EC-007: Non-HTTPS token endpoint override that is not loopback → rejected at construction.
- EC-008: Provider returns an unrecognised error body → `WirekitError::Api` with the status and a redacted message; the body is never echoed.
- EC-009: Empty scope set → the authorization URL is still built and the provider's response surfaces as `Auth`, never a silent retry.

## Dependencies & Risks

| Dependency | Type | Notes |
|---|---|---|
| `wirekit` core (`BP-005`) | internal | On `main` after `m1-first-publish`; retry, bucket, tracing, secret, error |
| `wirekit` secret type and `WirekitError` | internal | Reused; no second error type |
| `rand`, `sha2`, `base64` | library | PKCE `S256` challenge; `rand` is already a dependency |
| `url` | library | URL construction and validation |
| `serde` | library | `TokenResponse` decoding; already a dependency |
| Google OAuth app registration | external | Client id from a Google Cloud project; sensitive-scope verification `[to be confirmed]` |

- RISK-001: Google requires app verification for sensitive scopes, delaying M2 (`catalogue/28` open question 3). Mitigation: scopes are caller-supplied; each module requests only its own.
- RISK-002: Refresh-token storage is security-critical and lives outside the crate. Mitigation: the trait boundary is documented and the in-memory store is marked non-durable.
- RISK-003: Changing `google_lsa` to accept a token source could break callers. Mitigation: FR-012 retains the static-token path.
- RISK-004: Concurrent refresh storms. Mitigation: EC-005 documents single-flight per client; a shared store needs an implementor-level lock.

## Sources

Repository only; no external research was available.

- `catalogue/28_google_oauth.md` — surface, endpoints, PKCE requirement, refresh policy, open questions.
- `catalogue/21_google_lsa.md` — the consumer that needs a token source.
- `.sdrive/constitution.md` — CON-102, 103, 104, 108, 109, 201, 202, 207–209, 211–214, 511–520.
- `crates/wirekit/src/auth.rs`, `secret.rs`, `error.rs` — the contract this module plugs into.



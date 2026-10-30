---
title: Google OAuth shared helper - execution tasks
status: backlog
version: v1
created: 2026-10-30
updated: 2026-10-30
---

# Google OAuth shared helper - Execution Tasks

Production code only. No task writes or runs tests; `/sdrive:test` owns them.

## P1 - Module skeleton

Depends on: none.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T1.1 | Add the `google-oauth` feature and register the module | FR-001 | `google-oauth = []` exists, default-off; `#[cfg(feature = "google-oauth")] pub mod google_oauth;` in `lib.rs` |
| T1.2 | Define the five public types | FR-001, FR-008 | `GoogleOAuthClient`, `AuthorizationRequest`, `TokenResponse`, `TokenStore`, `RevokeRequest` exist with rustdoc and the `TokenStore` trait is object-safe |

## P2 - Authorization, exchange, refresh, revoke

Depends on: P1.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T2.1 | Build the authorization URL with state and PKCE | FR-002, FR-003 | The URL carries `response_type=code`, scopes, redirect URI, a CSPRNG `state`, and `code_challenge` with `code_challenge_method=S256`; no code path omits PKCE |
| T2.2 | Exchange the code for tokens | FR-004 | `exchange_code` posts code and verifier to the token endpoint and returns a typed response with absolute expiry from the injected clock |
| T2.3 | Refresh once on 401 and persist | FR-006 | At most one refresh per request; the store is updated through the trait; a second 401 fails |
| T2.4 | Revoke a token | FR-007 | The revocation endpoint is called once and the store record is deleted |

## P3 - Hardening and integration

Depends on: P2.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T3.1 | Map Google errors | FR-005 | `error`/`error_description` map to `WirekitError::Auth`, provider-side failures to `Api`, and the provider code survives |
| T3.2 | Validate untrusted input | NFR-004 | Error bodies, redirect parameters, and scope strings produce typed errors; no panic path |
| T3.3 | Redact secrets and scope client state | FR-010, NFR-001, NFR-002 | Secret, access token, and refresh token render `***`; no global cache; credentials only from construction |
| T3.4 | Enforce HTTPS and endpoint override | FR-009 | Non-HTTPS overrides are rejected unless loopback and explicitly allowed |
| T3.5 | Rate-limit the token endpoint | NFR-003 | The shared bucket is used with a caller override and one timestamp update per request |
| T3.6 | Emit tracing without values | FR-011 | Authorization, exchange, refresh, revoke, and errors are traced; no token, verifier, or secret is recorded |
| T3.7 | Accept the helper as a `google_lsa` token source | FR-012 | `google_lsa` accepts the helper and still accepts a static token |

## P4 - Verification

Depends on: P3.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T4.1 | Run the formatter check | NFR-005 | `cargo fmt --all --check` passes |
| T4.2 | Run the linter | NFR-005 | `cargo clippy -p wirekit --all-features -- -D warnings` passes |
| T4.3 | Build the documentation | NFR-005 | `cargo doc` passes with `RUSTDOCFLAGS="-D warnings"` |
| T4.4 | Compile the feature combinations | NFR-005 | Baseline, `google-oauth`, and `google-lsa` each compile on Rust 1.85 |
| T4.5 | Run the governance validator | NFR-005 | `node .sdrive/scripts/validate-governance.js` reports every artifact valid |

## Implementation checklist

- [x] **T1.1** Add the `google-oauth` feature and register the module.
- [x] **T1.2** Define the five public types and the non-durable store.
- [x] **T2.1** Build the authorization URL with `state` and an `S256` PKCE challenge.
- [x] **T2.2** Exchange the code for tokens with an absolute expiry from the injected clock.
- [x] **T2.3** Refresh at most once and persist through the `TokenStore`.
- [x] **T2.4** Revoke a token and delete the record.
- [x] **T3.1** Map Google errors into `WirekitError::Auth` / `WirekitError::Api`.
- [x] **T3.2** Validate untrusted input and bound the response body.
- [x] **T3.3** Redact secrets and keep client state per instance.
- [x] **T3.4** Enforce HTTPS with an explicit loopback exception.
- [x] **T3.5** Charge the token endpoint to the `google-oauth` bucket.
- [x] **T3.6** Emit tracing without values.
- [x] **T3.7** Accept the helper as a `google_lsa` token source.
- [ ] **T4.1** `cargo fmt --all --check` - blocked: the check stops at `tests/google-oauth-shared-helper/main.rs:341`, an unbalanced brace in a test file `/sdrive:apply` must not edit. `rustfmt --check` passes on `src/google_oauth.rs` and `src/google_lsa.rs`.
- [x] **T4.2** `cargo clippy -p wirekit --all-features -- -D warnings` - clean.
- [x] **T4.3** `cargo doc` with `RUSTDOCFLAGS="-D warnings"` - clean.
- [x] **T4.4** Feature combinations - baseline, `google-lsa`, and `google-lsa google-oauth` all compile.
- [x] **T4.5** `node .sdrive/scripts/validate-governance.js` - every artifact valid.

## Notes

- No task requires a live Google account; the endpoint is injectable (FR-009).
- Test authoring and execution stay with `/sdrive:test`.

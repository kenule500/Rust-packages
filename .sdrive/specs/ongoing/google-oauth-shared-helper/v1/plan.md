---
title: Google OAuth shared helper - technical plan
status: backlog
version: v1
created: 2026-10-30
updated: 2026-10-30
---

# Google OAuth shared helper - Technical Plan

## Approach

One feature-flagged module inside `wirekit`, `src/google_oauth.rs`, behind an additive
`google-oauth` feature. It reuses the crate's client, retry policy, token bucket,
tracing, secret type, and `WirekitError`; it adds no crate, no runtime, and no I/O
(CON-103, CON-104, CON-108).

Two flows only. **Authorization Code + PKCE** is the only flow the types can express:
`AuthorizationRequest` is produced by a constructor that always generates a verifier,
so there is no code path that omits PKCE (CON-212). Client credentials already exist in
`auth.rs` and are not duplicated.

**Storage is a trait.** `TokenStore` is object-safe with load, save, and delete. The
crate ships `MemoryTokenStore` for tests and single-process callers, marked
non-durable in its rustdoc. Durable encryption is the implementor's contract; the
library never touches a file, a database, or an environment variable (CON-108,
CON-519).

## Module Map

| Item | Kind | Notes |
|---|---|---|
| `google_oauth::GoogleOAuthClient` | create | Client id, secret, redirect URI, scopes, token endpoint, clock source |
| `google_oauth::AuthorizationRequest` | create | URL, `state`, `code_verifier`, `code_challenge` (`S256`) |
| `google_oauth::TokenResponse` | create | Access token, optional refresh token, absolute expiry, scopes |
| `google_oauth::TokenStore` | create | Object-safe trait: load, save, delete |
| `google_oauth::MemoryTokenStore` | create | Non-durable implementation for tests and single-process callers |
| `google_oauth::RevokeRequest` | create | Token, optional hint |
| `wirekit::client`, `retry`, `rate_limit`, `trace`, `secret`, `error` | reuse | Consumed as-is from `BP-005` |
| `wirekit::google_lsa` | modify | Accept a token source; keep the static-token path (FR-012) |

## Data Model

| Entity | Fields |
|---|---|
| `GoogleOAuthClient` | `client_id`, `client_secret` (`Secret<String>`), `redirect_uri`, `scopes`, `token_endpoint`, `clock` |
| `AuthorizationRequest` | `url`, `state` (`Secret<String>`), `code_verifier` (`Secret<String>`) |
| `TokenResponse` | `access_token` (`Secret<String>`), `refresh_token` (`Option<Secret<String>>`), `expires_at`, `scopes` |
| `RevokeRequest` | `token` (`Secret<String>`), `hint` (`Option<String>`) |

No persistence anywhere. Every secret is held in `Secret<String>`, renders `***`, and
does not implement `Serialize` (CON-214).

## Security Baseline Expansion

| Control | Status | Requirement | Task |
|---|---|---|---|
| CON-511 Credentials | REQUIRED | NFR-001 | T3.3 |
| CON-512 Authorization / isolation | REQUIRED | NFR-002 | T3.3 |
| CON-513 Security headers | NOT APPLICABLE - no server responses | - | - |
| CON-514 Rate limiting | REQUIRED | NFR-003 | T3.5 |
| CON-515 Input validation | REQUIRED | NFR-004 | T3.2 |
| CON-516 Injection | NOT APPLICABLE - no database, cache, or query surface | NFR-006 | - |
| CON-517 XSS | NOT APPLICABLE - no markup | - | - |
| CON-518 CSRF | NOT APPLICABLE - no browser session; `state` covers callback replay | - | - |
| CON-519 Environment security | REQUIRED | NFR-001 | T3.3 |
| CON-520 HTTPS | REQUIRED | FR-009 | T3.4 |

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `wirekit` core (`BP-005`) | internal | no_change | On `main`; retry, bucket, tracing, secret, error |
| `rand` | library | no_change | Already a dependency; CSPRNG for `state` and verifier |
| `sha2`, `base64` | library | no_change | `S256` challenge encoding |
| `url` | library | no_change | URL construction; already a dependency |
| `serde` | library | no_change | `TokenResponse` decoding; already a dependency |
| `serde_json` | library | no_change | Token and error body parsing; already a dependency |
| Google OAuth app registration | external | no_change | Live-use gate only; no task needs it |

No new dependency is introduced.

## Risks

| Risk | Mitigation |
|---|---|
| Sensitive scopes need app verification | Scopes are caller-supplied; each module requests only its own |
| Refresh-token storage is outside the crate | Trait boundary documented; memory store marked non-durable |
| `google_lsa` change breaks callers | FR-012 keeps a static-token path |
| Concurrent refresh | Single-flight per client; shared stores need an implementor lock |
| Clock skew | Clock is injected; a time before issue is treated as elapsed (EC-006) |

## Open Questions

1. `[to be confirmed]` Exact `WirekitError::Auth` variants available from `BP-005`, so
   the mapping in FR-005 uses the closest existing variant.
2. `[to be confirmed]` Whether Google requires verification for the first consumer's
   scope set (`adwords`); does not block any task.

## Verification

```sh
cargo fmt --all --check
cargo clippy -p wirekit --all-features -- -D warnings
cargo doc -p wirekit --all-features --no-deps          # RUSTDOCFLAGS="-D warnings"
cargo check -p wirekit --no-default-features
cargo check -p wirekit --no-default-features --features google-oauth
cargo check -p wirekit --no-default-features --features google-lsa
node .sdrive/scripts/validate-governance.js
```

Test authoring and execution belong to `/sdrive:test`; no task here writes or runs tests.

---
title: wirekit core - error, retry, rate limit, pooled client, tracing, auth
status: backlog
version: v1
created: 2026-10-20
updated: 2026-10-20
---

# wirekit core - technical plan

## Approach

Add `crates/wirekit` as a new workspace member (`members = ["crates/*"]` already covers it, so the root `Cargo.toml` needs no change). The crate is the provider-independent core only: a single error type, a single retry policy, a single per-provider rate limiter, a single shared HTTP client, one auth layer, one tracing story, and one request pipeline that providers plug into.

The core is always compiled. `default = []` means *no provider module enabled*, not *no core*; every provider module added later (BP-006 to BP-009) is an additive feature that depends on this core and adds no second error type, retry loop, or client (CON-207).

No application domain types, no database, no tenant model, and no global mutable singleton (CON-106, CON-107, CON-109). Configuration is passed in explicitly by the caller.

## Architecture

```
crates/wirekit/
  Cargo.toml         # 0.1.0, default = [], provider features additive, docs.rs all-features
  README.md          # crate overview and feature table
  src/
    lib.rs           # crate lints, module docs, feature documentation
    error.rs         # WirekitError (non_exhaustive, W#### codes), mapping helpers
    client.rs        # WirekitClient builder, shared reqwest::Client, HTTPS validation
    retry.rs         # RetryPolicy, backoff with jitter, Retry-After handling
    rate_limit.rs    # per-provider token bucket and overrides
    auth.rs          # AuthScheme + Bearer, Basic, ApiKey*, Hmac, OAuth2 client credentials
    secret.rs        # Secret<T> redaction wrapper
    pipeline.rs      # RequestSpec -> auth -> rate limit -> retry -> send -> decode
    trace.rs         # tracing events: start, finish, retry, rate-limit hit, error
```

Request flow through the pipeline:

```
provider declares URL + method + body + response type
        |
        v
  [auth] apply scheme      -> fail closed with InvalidConfig if credentials missing
        |
        v
  [rate limit] acquire token (per provider) -> async wait, emits metric
        |
        v
  [retry loop] send -> classify -> backoff (jitter / Retry-After) -> resend
        |
        v
  [decode] serde -> body-level error hook -> Ok(T) or WirekitError
```

## Components

| Component | Action | Responsibility |
|---|---|---|
| `crates/wirekit/Cargo.toml` | create | Package metadata, `default = []`, additive provider features, docs.rs all-features |
| `crates/wirekit/src/lib.rs` | create | `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, module/feature docs (CON-201, CON-202) |
| `crates/wirekit/src/error.rs` | create | `#[non_exhaustive] WirekitError`, `code()`, `Display`, `Error`, status/body mapping |
| `crates/wirekit/src/client.rs` | create | `WirekitClient` builder, one shared `reqwest::Client`, injection path, HTTPS-only (CON-210, CON-520) |
| `crates/wirekit/src/retry.rs` | create | `RetryPolicy` (3 attempts default), exponential backoff with jitter, `Retry-After` (CON-208) |
| `crates/wirekit/src/rate_limit.rs` | create | Hand-rolled per-provider token bucket, overrides, async acquire (CON-209) |
| `crates/wirekit/src/auth.rs` | create | `AuthScheme` + Bearer, Basic, API key header/query, HMAC, OAuth2 client credentials |
| `crates/wirekit/src/secret.rs` | create | `Secret<T>`: redacted `Debug`/`Display`, no `Serialize` (CON-214) |
| `crates/wirekit/src/pipeline.rs` | create | Ordered pipeline, serde decode, body-level error hook (CON-213), response size bound |
| `crates/wirekit/src/trace.rs` | create | tracing events for start, finish (duration), retry, rate-limit hit, error (CON-211) |
| `crates/wirekit/README.md` | create | Crate purpose, feature table, one minimal usage sketch |
| `.github/workflows/ci.yml` | modify | Add a wirekit feature-matrix build (default, each provider feature) and the MSRV build |

## Data Model

Types introduced by this feature (all `create`; no persisted entities):

| Type | Fields | Notes |
|---|---|---|
| `WirekitError` | `Api`, `Auth`, `RateLimit`, `Timeout`, `Network`, `InvalidConfig`, `Decode` | `#[non_exhaustive]`; each variant has a stable `W####` code |
| `RetryPolicy` | `attempts`, `base_delay`, `max_delay`, `jitter` | Defaults: 3 attempts, exponential, bounded jitter |
| `RateLimitConfig` | `capacity`, `refill_per_second` | Per provider; overrides the default |
| `TokenBucket` | `capacity`, `available`, `refill_rate`, `last_refill` | In-memory, per provider |
| `Secret<T>` | `inner` | Redacted in `Debug`/`Display`; no `Serialize` |
| `AuthScheme` | `Bearer`, `Basic`, `ApiKeyHeader`, `ApiKeyQuery`, `Hmac`, `OAuth2ClientCredentials` | Applied by the pipeline before every request |
| `OAuth2Token` | `access_token`, `expires_at` | Cached per client; refreshed before expiry with a safety margin |
| `WirekitClient` | `http`, `retry`, `rate_limit`, `auth`, `base_url` | Owns the shared `reqwest::Client`; explicit construction only |
| `RequestSpec` | `method`, `url`, `headers`, `body`, `response_type`, `body_error_hook` | What a provider supplies |

No data is stored or transmitted by the crate itself; all state is in-process and dropped with the caller's client.

## Feature Flags

- `default = []` - no provider module enabled; the core is always present.
- Each provider module added in BP-006 to BP-009 becomes an additive feature (`noaa`, `twilio`, `sendgrid`, `slack`).
- A TLS feature selects the backend: `rustls-tls` (default) or `native-tls`.
- Enabling any feature MUST NOT enable or disable another (CON-204).

## Security Considerations

The CON-51x baseline is expanded into requirements, dependencies, and tasks, not silently skipped.

| Control | Status | Requirement | Handling |
|---|---|---|---|
| CON-511 Authentication | REQUIRED | NFR-011 | Pipeline applies the configured scheme and fails closed with `InvalidConfig` |
| CON-512 Authorization | NOT APPLICABLE | - | No tenant isolation; that is `platform-core`/`quota-ledger` (CON-107) |
| CON-513 Security Headers | NOT APPLICABLE | - | No HTTP server or response pipeline |
| CON-514 Rate Limiting | REQUIRED | NFR-012 | Per-provider token bucket enforced before every request |
| CON-515 Input Validation | REQUIRED | NFR-013 | URLs, headers, and bodies validated; typed errors, never panic |
| CON-516 Injection Protection | NOT APPLICABLE | - | No database or NoSQL backend |
| CON-517 XSS Protection | NOT APPLICABLE | - | No HTML rendering |
| CON-518 CSRF Protection | NOT APPLICABLE | - | No browser session or cookie state |
| CON-519 Environment Security | REQUIRED | NFR-014 | Caller-supplied credentials only; no `.env` reading |
| CON-520 HTTPS / HSTS | REQUIRED | NFR-015 | Non-HTTPS base URLs rejected before any socket (documented at the deployment edge) |

## Performance Considerations

| Requirement | Strategy |
|---|---|
| NFR-005 | Token acquisition and backoff `await` a timer; no `std::thread::sleep` and no busy-wait |
| NFR-006 | Default request timeout and connection-pool limits on the shared client; one pool per process |
| NFR-009 | The pipeline is allocation-light: one shared client, one bucket lookup per request, one decode |

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `reqwest` | library | add | `rustls-tls` default; the single shared client (CON-210) |
| `tokio` | library | add | `rt`, `time`, `macros` for async and timers |
| `tracing` | library | add | Request/retry/rate-limit/error events (CON-211) |
| `thiserror` | library | add | `WirekitError` derive |
| `serde` | library | add | Typed bodies |
| `serde_json` | library | add | JSON encode/decode |
| `http` | library | add | `HeaderMap`, `StatusCode`, `Method` |
| `rand` | library | add | Backoff jitter |
| `async-trait` | library | add | Object-safe async auth trait |
| `url` | library | add | URL parsing and HTTPS validation |
| `wiremock` | library | add | Dev-dependency only: local HTTP mock for provider tests |
| `crates.io` | service | no_change | Publication target (BP-010); name checked before publish (CON-111) |
| `GitHub Actions` | service | no_change | CI from BP-002; add a feature-matrix and MSRV build |

## Risks

- A single crate with 26+ provider features becomes a build bottleneck -> core only here; measure and propose a companion crate if needed.
- 0.x churn in the shared contract breaks providers -> `#[non_exhaustive]`, stable codes, additive-only features until the 1.0 freeze.
- A dependency raises its MSRV above 1.85 -> the MSRV CI job fails fast; pin or downgrade with a recorded decision.
- Secret leakage into logs, traces, or errors -> `Secret<T>`, no `Serialize` for secrets, redacted tracing fields, property tests from the Test agent.
- A provider needs behavior the pipeline cannot express -> body-level error hook, per-provider overrides, injectable client.
- Scope creep into provider modules or stateful crates -> the explicit out-of-scope list.

## Verification

Verification tasks are limited to formatter, linter, build/typecheck, the MSRV build, the feature-matrix build, `sdrive verify`, `node .sdrive/scripts/validate-governance.js`, and manual checks. Writing and running tests belongs to `/sdrive:test` (CON-404, No Test Tasks Rule).

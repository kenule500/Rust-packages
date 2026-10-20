---
title: wirekit core - error, retry, rate limit, pooled client, tracing, auth
status: backlog
version: v1
created: 2026-10-20
updated: 2026-10-20
---

# wirekit core - error, retry, rate limit, pooled client, tracing, auth

## Overview

### Problem Statement

The workspace holds one crate, `dado`. Every provider integration named in `catalogue/` will be a `wirekit` module, but `wirekit` does not exist: there is no shared error type, retry policy, rate limiter, HTTP client pool, tracing story, or auth layer. If the four first-publish providers (NOAA, Twilio, SendGrid, Slack) are built without that shared core, each will grow its own error enum, its own retry loop, and its own auth handling - the exact fragmentation `wirekit` exists to remove, and a direct violation of CON-207.

The backlog records this as BP-005 and marks it the shared model for all of M1, M2 (`wirekit` providers, `platform-core`), and M3. The M1 note is explicit: *"Do not start provider modules until the error, retry, rate-limit, and auth contracts are stable enough to test."*

### Proposed Solution

Add a new workspace crate, `crates/wirekit` at version 0.1.0, that ships the provider-independent core and nothing else:

1. **One error type.** A `#[non_exhaustive]` `WirekitError` enum with stable `W####` codes, `Display`, `std::error::Error`, and secret-safe output (CON-205, CON-207, CON-214).
2. **One retry policy.** Exponential backoff with jitter, 3 attempts by default, retry on 5xx/429/network errors, honor `Retry-After`, never retry other 4xx (CON-208).
3. **One rate limiter.** A hand-rolled per-provider token bucket with per-provider overrides and tracing metrics (CON-209).
4. **One pooled client.** A shared `reqwest::Client` per process with an explicit injection path, HTTPS-only base URLs, and a configurable builder (CON-210, CON-520).
5. **One auth layer.** Bearer, Basic, API key (header and query), HMAC signature, and OAuth2 client credentials, over a `Secret<T>` wrapper that redacts in `Debug`/`Display` (CON-214).
6. **One observability story.** Tracing events for request start, request finish (with duration), retry, rate-limit hit, and error (CON-211).
7. **One request pipeline.** A provider implements a URL, method, body, and response type; the core orders auth → rate limit → retry → send → decode, including the body-level error hook required by CON-213.

Provider modules, provider feature flags, examples, docs, and publishing belong to later backlog items (BP-006 to BP-010) and are out of scope here.

### Target Audience

- **Primary:** external Rust developers who will install `wirekit` 0.1.0 and expect one error type, one retry policy, and one auth layer across providers.
- **Secondary:** the providers in BP-006 to BP-009 and the M2/M3 modules (`platform-core`, `quota-ledger`, `hookgate`) that consume this contract.

### Assumptions

- The crate name `wirekit` is available on crates.io; it MUST be re-checked immediately before publication (CON-111). Availability is not verified by this feature.
- The chosen dependency versions are MSRV 1.85 compatible and stay so; an MSRV CI job guards this (CON-102).
- `rustls` is the preferred TLS backend; `native-tls` is a documented alternative, not the default.
- Provider integrations remain feature-gated modules of `wirekit`, never separate crates (CON-104).
- The core is always compiled; `default = []` means "no provider module enabled", not "no core".
- The `dado` dependency in the backlog is a sequencing dependency, not a code dependency; `wirekit` does not call `dado`.
- Test authoring belongs to `/sdrive:test`, not this feature's implementation tasks.
- Actual publication happens in BP-010 from `main` after review and archive.

### Confirmed Decisions

| Decision | Choice |
|---|---|
| Dependency posture | `reqwest` (rustls-tls), `tokio` (rt, time, macros), `tracing`, `thiserror`, `serde`, `serde_json`, `http`, `rand`, `async-trait` |
| `WirekitError` shape | `#[non_exhaustive]` from 0.1 |
| Auth schemes in core | Bearer, Basic, API key (header + query), HMAC, OAuth2 client credentials (5 server-side schemes) |
| Auth-code + PKCE | Out of scope; owned by BP-015 (Google OAuth helper) per CON-212 |
| Rate limiter | Hand-rolled per-provider token bucket (no `governor`) |
| Error codes | Stable `W####` codes, mirroring `dado`'s `E1001`/`D1001` convention |
| Provider test doubles | `wiremock` as a dev-dependency only |
| Crate baseline | `wirekit` 0.1.0, `default = []`, core always compiled, no code dependency on `dado` |

### Sources

- `.sdrive/constitution.md` (CON-101 to CON-111, CON-201 to CON-216, CON-501 to CON-520, CON-601 to CON-614)
- `.sdrive/context/code-standards.md` §Error, retry, and rate-limit contract; §Secret handling; §Protocol and payload handling
- `.sdrive/context/architecture-context.md` §Provider clients are modules, not crates; §Why stateful packages are separate
- `.sdrive/product/backlog.md` (BP-005, M1 notes, open question BP-005)
- `.sdrive/product/overview.md` (first publish scope, candidate dependencies, JTBD-2/JTBD-3/JTBD-5)
- `catalogue/01_wirekit.md` (error variants, retry rules, rate limits, auth schemes, tracing, shared pool)
- `catalogue/00_provider_index.md` (provider modules are `wirekit` features)
- `README.md` (one job per package; providers are modules; crate names already taken)
- Cargo features: <https://doc.rust-lang.org/cargo/reference/features.html>
- Rust API Guidelines: <https://rust-lang.github.io/api-guidelines/checklist.html>
- HTTP semantics and `Retry-After`: <https://www.rfc-editor.org/rfc/rfc9110#field.retry-after>
- Token bucket algorithm: <https://en.wikipedia.org/wiki/Token_bucket>
- `reqwest` client builder and pooling: <https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html>
- OAuth 2.0 client credentials: <https://www.rfc-editor.org/rfc/rfc6749#section-4.4>
- HMAC: <https://www.rfc-editor.org/rfc/rfc2104>

## User Stories

### US-001: Handle every provider failure with one error type

As an external Rust developer, I want one `WirekitError` type that every provider maps into, so that I write one error handler instead of one per provider.

**Acceptance Criteria:**

- AC-001: Given a provider response with a non-2xx status, when the pipeline maps the failure, then it returns `WirekitError::Api { status, code, message }` and no provider-specific error type appears in the public API.
- AC-002: Given any `WirekitError`, when `code()` is called, then it returns a stable `&'static str` of the form `W####`, and `Display` renders the code with a message.
- AC-003: Given any `WirekitError`, when `Debug` and `Display` are printed, then no credential, token, or raw request/response body appears.
- AC-004: Given the `WirekitError` enum, when a consumer matches it with a wildcard arm, then the match compiles, proving `#[non_exhaustive]`.
- AC-005: Given `WirekitError`, when used with `?` or as `Box<dyn std::error::Error>`, then it implements `std::error::Error` and exposes `source()` for wrapped causes.

### US-002: Get the same retry behavior from every provider

As an external Rust developer, I want one retry policy that applies everywhere, so that I am not debugging why one provider retries and another does not.

**Acceptance Criteria:**

- AC-006: Given a 5xx response and the default policy, when the request is executed, then it is retried up to 3 attempts total, and the final failure is `WirekitError::Api` carrying the last status.
- AC-007: Given a 429 response with a `Retry-After` header, when the request is retried, then the delay is at least the header value, and an exhausted policy yields `WirekitError::RateLimit { retry_after }`.
- AC-008: Given a 400, 401, 403, or 404 response, when executed, then no retry occurs and the error returns immediately.
- AC-009: Given a network or timeout failure, when executed, then it is retried and ends as `WirekitError::Network` or `WirekitError::Timeout`.
- AC-010: Given the retry policy, when delays are computed, then they grow exponentially with bounded jitter and stay within the configured bounds.
- AC-011: Given a retry, when it occurs, then a tracing event records the attempt number and the delay.

### US-003: Respect per-provider rate limits

As an external Rust developer, I want rate limiting handled in one place, so that I do not build it per API.

**Acceptance Criteria:**

- AC-012: Given a configured per-provider token bucket, when requests exceed the configured rate, then the limiter throttles them so the rate is not exceeded.
- AC-013: Given two providers with different rate configurations, when both send requests, then each uses its own bucket and its own override.
- AC-014: Given a rate-limit hit, when it occurs, then a tracing metric event is emitted.
- AC-015: Given an empty bucket, when a request waits for a token, then the async task yields instead of blocking the runtime thread (no `std::thread::sleep`).

### US-004: Share one HTTP client pool

As an external Rust developer, I want one pooled client across providers, so that connections are reused and memory is bounded.

**Acceptance Criteria:**

- AC-016: Given no injected client, when two providers send requests, then they share one `reqwest::Client` and therefore one connection pool.
- AC-017: Given a caller-injected `reqwest::Client`, when a request is sent, then the injected client is used verbatim and no second pool is created.
- AC-018: Given the client builder, when timeouts, TLS, a proxy, and default headers are configured, then those settings apply to requests.
- AC-019: Given an `http://` base URL, when a request is prepared, then it is rejected with `WirekitError::InvalidConfig` before any socket is opened.
- AC-020: Given the default configuration, when the client is built, then a request timeout and connection-pool limits are applied.

### US-005: Apply authentication consistently

As an external Rust developer, I want the auth flow built once, so that every provider using the same scheme reuses it.

**Acceptance Criteria:**

- AC-021: Given a bearer token, when a request is sent, then `Authorization: Bearer <token>` is applied.
- AC-022: Given basic credentials, when a request is sent, then `Authorization: Basic <base64(user:pass)>` is applied.
- AC-023: Given an API key configured for a header, when a request is sent, then the configured header carries the key.
- AC-024: Given an API key configured for the query string, when a request is sent, then the key is added as a query parameter.
- AC-025: Given the HMAC scheme, when a request is signed, then the signature is computed over the canonical raw request bytes and compared with a constant-time routine.
- AC-026: Given OAuth2 client credentials, when a request is made without a valid token, then a token is fetched, cached, and reused, and is refreshed before expiry.
- AC-027: Given an auth or secret type, when `Debug`/`Display` are printed, then the secret renders as `***`, and no secret field implements `Serialize`.

### US-006: Observe every call

As an external Rust developer, I want tracing on every call, so that I can plug in OpenTelemetry, Datadog, or Jaeger once.

**Acceptance Criteria:**

- AC-028: Given a request, when it starts, then a tracing event with the method and the redacted URL is emitted.
- AC-029: Given a completed request, when it finishes, then a tracing event records the duration.
- AC-030: Given a retry, a rate-limit hit, or an error, when it occurs, then the matching tracing event is emitted.
- AC-031: Given any of these events, when they are recorded, then no credential or token appears in the output.

### US-007: Implement a provider against one pipeline

As a provider author, I want to declare a URL, method, body, and response type, so that I do not reimplement auth, retry, rate limiting, tracing, or error mapping.

**Acceptance Criteria:**

- AC-032: Given a provider that declares a URL, method, body, and response type, when it executes, then the pipeline applies auth → rate limit → retry → send → decode in that order.
- AC-033: Given a successful response, when decoded, then the caller's typed value is returned via `serde`.
- AC-034: Given HTTP 200 with a body-level error (Slack `ok: false` or a GraphQL `errors[]` array), when the body hook is configured, then `WirekitError::Api` is returned instead of a success (CON-213).
- AC-035: Given a malformed body, when decoding fails, then `WirekitError::Decode` is returned and no panic occurs.
- AC-036: Given missing credentials, when a request is prepared, then `WirekitError::InvalidConfig` is returned and no request is sent.

### US-008: Keep the crate safe, dependency-honest, and lint-clean

As a security-conscious developer, I want the crate to stay within its stated job, so that it can be a dependency of any Rust application.

**Acceptance Criteria:**

- AC-037: Given the crate root, when compiled, then `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` are present and every public item is documented.
- AC-038: Given the crate, when built with default features and with each provider feature independently, then it compiles, and features are additive (CON-204).
- AC-039: Given Rust 1.85, when the workspace builds, then `wirekit` compiles under edition 2021.
- AC-040: Given the crate, when inspected, then it introduces no database, tenant, billing, or application-domain types (CON-106, CON-107).
- AC-041: Given the crate's source, when scanned, then it does not read `.env` files or environment files to obtain credentials (CON-519).
- AC-042: Given the full test suite, when it runs, then every acceptance criterion has at least one test, supplied by the Test agent (CON-404).

## Requirements

### Functional Requirements

- FR-001: The repository MUST contain a `wirekit` crate as a workspace member at `crates/wirekit`, version 0.1.0, inheriting `edition`, `rust-version`, and `license` from `[workspace.package]`.
- FR-002: The crate MUST compile with `default = []`, always expose the core, and gate every provider module behind an additive feature.
- FR-003: The crate MUST declare `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`, and every public item MUST have a rustdoc comment.
- FR-004: A single `#[non_exhaustive]` `WirekitError` enum MUST be the only error type on the public API.
- FR-005: Every `WirekitError` variant MUST expose a stable `code()` string of the form `W####` and MUST implement `Display` and `std::error::Error`.
- FR-006: Provider failures MUST map into `WirekitError::Api { status, code, message }`; a provider MUST NOT introduce a second error type.
- FR-007: `WirekitError`'s `Debug`, `Display`, logs, traces, and payloads MUST NOT contain credentials, tokens, or raw bodies.
- FR-008: HTTP retries MUST use exponential backoff with jitter and 3 attempts by default.
- FR-009: Retries MUST occur on 5xx, 429, and network or timeout errors, and MUST honor `Retry-After`.
- FR-010: Retries MUST NOT occur on any other 4xx status.
- FR-011: The retry policy MUST be configurable per client (attempt count, base delay, maximum delay, jitter bounds).
- FR-012: Rate limiting MUST be a per-provider token bucket with per-provider overrides.
- FR-013: The rate limiter MUST emit a tracing event for every rate-limit hit.
- FR-014: The crate MUST share one pooled `reqwest::Client` per process unless the caller injects its own client.
- FR-015: The client builder MUST allow configuring timeouts, TLS, proxying, and default headers.
- FR-016: The client MUST reject non-HTTPS base URLs with `WirekitError::InvalidConfig`.
- FR-017: The auth layer MUST provide Bearer, Basic, API-key-in-header, API-key-in-query, HMAC-signature, and OAuth2-client-credentials schemes.
- FR-018: OAuth2 client credentials MUST fetch a token, cache it, and refresh it before expiry, with a single in-flight fetch per client.
- FR-019: Auth and secret types MUST redact secrets in `Debug`/`Display` and MUST NOT implement `Serialize` for a secret field.
- FR-020: Every outbound request MUST emit tracing events for start, finish (with duration), retry, rate-limit hit, and error.
- FR-021: The core MUST expose one request pipeline (auth → rate limit → retry → send → decode) so a provider supplies only a URL, method, body, and response type.
- FR-022: The core MUST expose a body-level error hook so that HTTP 200 with a body-reported error maps to `WirekitError::Api`.
- FR-023: Response bodies MUST decode into caller types via `serde`, and a decode failure MUST return `WirekitError::Decode`, never a panic.
- FR-024: The crate MUST treat URLs, header values, and response bytes as untrusted and MUST NOT panic on malformed input.
- FR-025: Credentials MUST come from caller-supplied configuration or secret stores; the crate MUST NOT read `.env` or environment files.
- FR-026: The crate MUST be publishable as `wirekit` 0.1.0 with complete metadata and a `docs.rs` `all-features` configuration.

### Non-Functional Requirements

- NFR-001: The crate MUST compile on Rust 1.85 with edition 2021.
- NFR-002: The crate MUST keep `forbid(unsafe_code)` and `deny(missing_docs)`; docs MUST build with warnings denied.
- NFR-003: Features MUST be additive; enabling a provider feature MUST NOT enable or disable another feature, and the default feature set MUST be documented.
- NFR-004: The crate MUST NOT force a database, tenant model, quota state, webhook state, or application-domain types on consumers.
- NFR-005: Retry backoff and token-bucket waiting MUST yield to the async runtime and MUST NOT block a runtime thread.
- NFR-006: The shared client MUST bound resource usage through default timeouts and connection-pool limits.
- NFR-007: Secrets MUST be redacted in `Debug`, `Display`, logs, traces, and error payloads, and MUST NOT be serialized.
- NFR-008: The crate MUST treat all network input as untrusted and MUST NOT panic, abort, or hang on malformed responses.
- NFR-009: Every acceptance criterion MUST be covered by at least one test, supplied by the Test agent.
- NFR-010: The crate MUST NOT expose a global mutable singleton; client and provider construction MUST be explicit.
- NFR-011: Authentication (CON-511) - the pipeline MUST apply the configured auth scheme to every outbound request and MUST fail closed with `WirekitError::InvalidConfig` when credentials are missing.
- NFR-012: Rate limiting (CON-514) - the per-provider token bucket MUST be enforced before every outbound request, with per-provider overrides.
- NFR-013: Input validation (CON-515) - URLs, header values, and response bodies MUST be validated; malformed input MUST return a typed `WirekitError` and MUST NOT panic.
- NFR-014: Environment security (CON-519) - credentials MUST come from caller-supplied configuration or secret stores; the crate MUST NOT read `.env` files and MUST NOT commit secrets.
- NFR-015: HTTPS (CON-520) - all outbound requests MUST use TLS; non-HTTPS base URLs MUST be rejected before any socket is opened.

## UI/UX & Data Requirements

- **UI/UX**: Not applicable. This repository ships no UI layer (CON-301); `wirekit` exposes typed Rust APIs only.
- **Data**: The crate stores no persistent data. In-memory state is limited to the shared client, the per-provider token buckets, and the OAuth2 token cache - all owned by the caller's process and dropped with it.

## Security Baseline

The feature touches authentication and an HTTP surface, so the CON-51x baseline is expanded below (CON-62 rule in `.sdrive/agents/02-specification.md`).

| Control | Status | Handling |
|---|---|---|
| CON-511 Authentication | REQUIRED | Expanded into NFR-011 and US-005 (FR-017 to FR-019). Auth schemes are applied by the pipeline and fail closed. |
| CON-512 Authorization | NOT APPLICABLE | `wirekit` performs no tenant isolation or capability checks; that is `platform-core` and `quota-ledger` territory (CON-107). |
| CON-513 Security Headers | NOT APPLICABLE | The crate owns no HTTP server or response pipeline (CON-513 rationale in the constitution). |
| CON-514 Rate Limiting | REQUIRED | Expanded into NFR-012 and US-003 (FR-012, FR-013). |
| CON-515 Input Validation | REQUIRED | Expanded into NFR-013 and US-007 (FR-023, FR-024). |
| CON-516 Injection Protection | NOT APPLICABLE | The crate owns no database or NoSQL backend; parameterized queries are a `quota-ledger`/`hookgate` concern. |
| CON-517 XSS Protection | NOT APPLICABLE | The crate renders no HTML or markup. |
| CON-518 CSRF Protection | NOT APPLICABLE | The crate owns no browser session or cookie state; tokens and signatures are the boundary. |
| CON-519 Environment Security | REQUIRED | Expanded into NFR-014 and US-008 (FR-025, AC-041). |
| CON-520 HTTPS / HSTS | REQUIRED | Expanded into NFR-015 and US-004 (FR-016, AC-019). |

## Edge Cases & Error States

- EC-001: 429 with a numeric `Retry-After` -> wait at least that long, then retry; if attempts are exhausted, return `WirekitError::RateLimit { retry_after }`.
- EC-002: 429 without `Retry-After` -> apply jittered backoff.
- EC-003: 5xx with attempts exhausted -> `WirekitError::Api` carrying the last status and any parsed code/message.
- EC-004: 400, 401, 403, or 404 -> no retry; return `WirekitError::Api` immediately.
- EC-005: Network failure mid-flight -> retried, then `WirekitError::Network`.
- EC-006: Request timeout -> retried, then `WirekitError::Timeout`.
- EC-007: `Retry-After` given as an HTTP date -> parsed safely; an unparseable value falls back to backoff without panicking.
- EC-008: Empty token bucket -> the request waits asynchronously and proceeds when a token is available.
- EC-009: OAuth2 token endpoint returns 4xx -> `WirekitError::Auth`; no retry on a non-429 4xx.
- EC-010: OAuth2 token expires between requests -> refreshed before the next request, not during a partially sent one.
- EC-011: HTTP 200 with Slack `ok: false` or a GraphQL `errors[]` array -> `WirekitError::Api` via the body hook (CON-213).
- EC-012: Malformed JSON body -> `WirekitError::Decode`; no panic.
- EC-013: Base URL using `http://` -> rejected before any socket is opened.
- EC-014: Caller-injected client -> used verbatim; no second pool and no overridden configuration.
- EC-015: Concurrent requests for one provider -> share one bucket, so the aggregate rate is respected.
- EC-016: Secret present in configuration -> `Debug` prints `***`; `Serialize` is not implemented for the secret.
- EC-017: Oversized response body -> bounded by a configurable limit; exceeding it returns a typed error rather than an unbounded allocation.
- EC-018: Missing credentials -> `WirekitError::InvalidConfig`; no request is sent.
- EC-019: HMAC signing -> computed over the raw request bytes; a re-serialized body would not verify.
- EC-020: Clock skew near token expiry -> refresh uses a configurable safety margin.

## Out of Scope

- Provider modules and their feature flags (`twilio`, `sendgrid`, `noaa`, `slack`, and every other `catalogue/` provider).
- OAuth2 Authorization Code + PKCE and browser redirects (BP-015 owns the PKCE helper per CON-212).
- Examples, a provider matrix, a docs site, and the changelog (BP-010).
- Publishing `wirekit` to crates.io (BP-010).
- Cross-process/shared quota state (`quota-ledger`), webhook verification and delivery state (`hookgate`), and tenant/provider registry (`platform-core`).
- Streaming uploads/downloads, WebSocket transports, and multipart file transfer beyond a single-body request.
- Application domain types, billing rules, and product workflows (CON-106).
- JavaScript/TypeScript bindings.

## Dependencies & Risks

### Dependencies

| Dependency | Type | Notes |
|---|---|---|
| `reqwest` | library | HTTP client; `rustls-tls` default; shared per process (CON-210). |
| `tokio` | library | Async runtime (`rt`, `time`, `macros`); required by the retry/rate-limit timers. |
| `tracing` | library | Observability events (CON-211). |
| `thiserror` | library | `WirekitError` derive. |
| `serde` + `serde_json` | library | Typed request/response models. |
| `http` | library | Shared HTTP types (`HeaderMap`, `StatusCode`, `Method`). |
| `rand` | library | Jitter for exponential backoff. |
| `async-trait` | library | Object-safe async auth/transport traits. |
| `url` | library | URL parsing and HTTPS validation. |
| `wiremock` | library (dev) | Local HTTP mock for provider tests; never published. |
| crates.io | service | Publication target (BP-010); name check required before publish (CON-111). |
| GitHub Actions | service | MSRV and workspace CI from BP-002. |

### Risks

- RISK-001: A single crate carrying 26+ provider features becomes a release and build-time bottleneck. Mitigation: this feature ships core only; measure build times and propose a companion crate if needed (CON-104 amendment path).
- RISK-002: 0.x churn in the error/retry/auth contract breaks provider modules. Mitigation: `#[non_exhaustive]`, stable `W####` codes, and an additive-only feature policy until the 1.0 freeze.
- RISK-003: A dependency raises its MSRV above 1.85. Mitigation: the MSRV CI job fails fast; pin or downgrade with a recorded decision.
- RISK-004: `rustls` versus `native-tls` surprises on a platform. Mitigation: `rustls` is the default and a documented feature selects `native-tls`.
- RISK-005: A secret leaks into logs, traces, or errors. Mitigation: the `Secret<T>` wrapper, no `Serialize` for secret fields, redacted tracing fields, and property tests from the Test agent.
- RISK-006: A provider needs behavior the pipeline cannot express. Mitigation: the body-level error hook, per-provider retry/rate-limit overrides, and injectable clients.
- RISK-007: The rate limiter blocks the async runtime or is unfair under concurrency. Mitigation: async token acquisition, per-provider buckets, and a concurrency edge case (EC-015).
- RISK-008: Scope creeps into provider modules or stateful crates. Mitigation: the out-of-scope list; later work becomes new features (CON-604, CON-612).

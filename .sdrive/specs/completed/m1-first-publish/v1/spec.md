---
title: M1 First publish - wirekit 0.1.0 core, four providers, and release preparation
status: backlog
version: v1
created: 2026-10-20
updated: 2026-10-20
---

# M1 First publish - wirekit 0.1.0 core, four providers, and release preparation

## Overview

### Problem Statement

The workspace holds one crate, `dado`. `wirekit` does not exist, and neither do the four providers named for the first publish. Without a shared core, each provider would grow its own error enum, retry loop, rate limiter, and auth handling - the exact fragmentation `wirekit` exists to remove and a direct violation of CON-207. Without a release-ready package, external developers cannot install anything.

The backlog records this milestone as M1 (`BP-005` to `BP-010`) and marks `BP-005` the shared model for all of M1, M2, and M3. The M1 note is explicit: *"Do not start provider modules until the error, retry, rate-limit, and auth contracts are stable enough to test."*

### Proposed Solution

Deliver M1 as **one feature on one branch**, mirroring how `m0-foundation` covered `BP-001` to `BP-004`:

1. **`wirekit` core (BP-005).** One error type, one retry policy, one per-provider rate limiter, one shared HTTP client, one auth layer, one tracing story, one request pipeline.
2. **Four provider modules (BP-006 to BP-009).** NOAA, Twilio, SendGrid, and Slack ship as feature-gated modules of the same crate (CON-104), each using the shared pipeline and introducing no second error type.
3. **Release preparation (BP-010).** Changelog, complete metadata, a provider matrix, a feature-flag table, an error reference, one runnable example per provider, a verified `cargo publish -p wirekit --dry-run`, and a `wirekit` section in `RELEASING.md`. The real `cargo publish` and the `wirekit-v0.1.0` tag stay manual post-merge, exactly like `dado`.

### Target Audience

- **Primary:** external Rust developers who will install `wirekit` 0.1.0 and expect one error type, one retry policy, and one auth layer across providers.
- **Secondary:** the M2/M3 crates (`platform-core`, `quota-ledger`, `hookgate`) that consume this contract, and the first production consumers (GIA, OmniStream).

### Assumptions

- The crate name `wirekit` is available on crates.io; it MUST be re-checked immediately before publication (CON-111). Availability is not verified by this feature.
- The chosen dependency versions are MSRV 1.85 compatible and stay so; an MSRV CI job guards this (CON-102).
- `rustls` is the preferred TLS backend; `native-tls` is a documented alternative, not the default.
- Provider integrations remain feature-gated modules of `wirekit`, never separate crates (CON-104).
- The core is always compiled; `default = []` means "no provider module enabled", not "no core".
- `dado` is a sequencing dependency only; `wirekit` does not call `dado`.
- Every provider has a design file following the `03`-`28` pattern, satisfying CON-105 before implementation.
- Provider credentials come from the caller; no live provider account is used to build or verify this feature.
- Test authoring belongs to `/sdrive:test`, not this feature's implementation tasks.
- The four providers are validated against recorded or sandbox fixtures, never against production traffic.

### Confirmed Decisions

| Decision | Choice |
|---|---|
| Milestone shape | One feature and one branch for `BP-005` to `BP-010`, like `m0-foundation` |
| Dependency posture (core) | `reqwest` (rustls-tls), `tokio` (rt, time, macros), `tracing`, `thiserror`, `serde`, `serde_json`, `http`, `rand`, `async-trait`, `url` |
| `WirekitError` shape | `#[non_exhaustive]` from 0.1 |
| Auth schemes in core | Bearer, Basic, API key (header + query), HMAC, OAuth2 client credentials (5 server-side schemes) |
| Auth-code + PKCE (Slack install, Google) | Out of scope; owned by BP-015 per CON-212; the Slack module ships bot-token usage only |
| Rate limiter | Hand-rolled per-provider token bucket (no `governor`) |
| Error codes | Stable `W####` codes, mirroring `dado`'s `E1001`/`D1001` convention |
| Provider test doubles | `wiremock` as a dev-dependency only |
| Crate baseline | `wirekit` 0.1.0, `default = []`, core always compiled, no code dependency on `dado` |
| Twilio scope | SMS, WhatsApp, voice, MFA, and lookup (resolves backlog open question BP-007) |
| Provider crypto/encoding deps | `hmac`, `sha1`, `sha2`, `base64`, `hex`, `percent-encoding`, `serde_urlencoded`, `p256` + `ecdsa` |
| Webhook verification | Each provider ships its signature verifier over raw bytes (CON-215); stateful delivery and dedupe stay in `hookgate` |
| Publish timing | Release-prep in the branch; real `cargo publish` and the tag are manual post-merge |
| BP-010 deliverables | Changelog, metadata, provider matrix, feature-flag table, error reference, four runnable examples, `RELEASING.md` section |

### Sources

- `.sdrive/constitution.md` (CON-101 to CON-111, CON-201 to CON-216, CON-501 to CON-520, CON-601 to CON-614)
- `.sdrive/context/code-standards.md` §Error, retry, and rate-limit contract; §Secret handling; §Protocol and payload handling
- `.sdrive/context/architecture-context.md` §Provider clients are modules, not crates; §Why stateful packages are separate
- `.sdrive/product/backlog.md` (BP-005 to BP-010, M1 notes, open questions)
- `.sdrive/product/overview.md` (first publish scope, candidate dependencies, JTBD-1 to JTBD-6)
- `catalogue/01_wirekit.md`, `catalogue/00_provider_index.md`
- `catalogue/07_noaa.md`, `catalogue/03_twilio.md`, `catalogue/04_sendgrid.md`, `catalogue/23_slack.md`
- `RELEASING.md` (manual release runbook pattern)
- Cargo features: <https://doc.rust-lang.org/cargo/reference/features.html>
- Rust API Guidelines: <https://rust-lang.github.io/api-guidelines/checklist.html>
- HTTP semantics and `Retry-After`: <https://www.rfc-editor.org/rfc/rfc9110#field.retry-after>
- Token bucket algorithm: <https://en.wikipedia.org/wiki/Token_bucket>
- `reqwest` client builder and pooling: <https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html>
- OAuth 2.0 client credentials: <https://www.rfc-editor.org/rfc/rfc6749#section-4.4>
- HMAC: <https://www.rfc-editor.org/rfc/rfc2104>
- NOAA Weather API documentation: <https://www.weather.gov/documentation/services-web-api>
- Twilio API error codes: <https://www.twilio.com/docs/usage/api/error-codes>
- Twilio webhook validation: <https://www.twilio.com/docs/usage/security>
- SendGrid v3 API: <https://www.twilio.com/docs/sendgrid/api-reference>
- SendGrid Event Webhook security: <https://www.twilio.com/docs/sendgrid/for-developers/tracking-events/getting-started-event-webhook-security-features>
- Slack Web API: <https://api.slack.com/web>
- Slack request signing: <https://api.slack.com/authentication/verifying-requests-from-slack>

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

### US-009: Fetch forecasts and alerts from NOAA with no API key

As a developer needing US weather data, I want typed NOAA forecasts and alerts, so that I can build a weather pipeline without managing credentials.

**Acceptance Criteria:**

- AC-043: Given a latitude and longitude or a grid point, when a point forecast is requested, then a typed response with periods, temperature, and precipitation is returned.
- AC-044: Given an area or zone, when active alerts are listed, then typed alert values with alert ID, event, severity, areas, and description are returned.
- AC-045: Given an alert ID, when it is fetched, then the single alert is returned; an unknown ID returns `WirekitError::Api` carrying the reported status.
- AC-046: Given a `problem+json` body (`type`, `title`, `status`, `detail`), when the response reports a failure, then it maps to `WirekitError::Api` with the reported status and detail.
- AC-047: Given a request built without the required descriptive `User-Agent`, when it is sent, then it is rejected with `WirekitError::InvalidConfig` (NOAA asks for an identifying agent).
- AC-048: Given caching enabled, when the same grid or zone is requested again within the TTL, then the cached value is reused instead of a new call.
- AC-049: Given a 429 or 5xx from NOAA, when the request is retried, then the shared backoff policy applies and `Retry-After` is honored.

### US-010: Send SMS, WhatsApp, voice, MFA, and lookup through Twilio

As a developer building messaging and telephony, I want one typed Twilio client, so that SMS, voice, MFA, and number lookup share the same auth, retry, and error model.

**Acceptance Criteria:**

- AC-050: Given account credentials, when the client is built, then Basic auth (Account SID and Auth Token) or API-key-plus-secret is applied to every request.
- AC-051: Given a send-SMS request, when it is sent, then the body is form-encoded to the messages endpoint and a typed response carrying the message SID is returned.
- AC-052: Given a send-WhatsApp request, when it is sent, then it is delivered through the WhatsApp sender and a typed response is returned.
- AC-053: Given a call request with TwiML, when it is sent, then a call is created and a typed status is returned; fetching status returns the current value.
- AC-054: Given an MFA challenge, when it is started and then verified, then typed challenge and verification results are returned.
- AC-055: Given a phone number, when it is looked up, then a typed lookup result with validation data is returned.
- AC-056: Given Twilio's `code`, `message`, and HTTP status, when a failure occurs, then `WirekitError::Api { status, code, message }` is returned, and an authentication failure maps to `WirekitError::Auth`.
- AC-057: Given a webhook body and `X-Twilio-Signature`, when it is verified, then the signature is computed over the request URL plus the sorted form parameters with HMAC-SHA1 and compared in constant time; a tampered body fails verification.
- AC-058: Given a per-account token bucket, when the configured rate is exceeded, then sends are throttled and the remaining quota is surfaced through tracing.

### US-011: Send transactional email through SendGrid

As a developer sending transactional email, I want typed email and template support with correct failure handling, so that delivery failures are visible and templates are reusable.

**Acceptance Criteria:**

- AC-059: Given an API key, when the client is built, then bearer authentication is applied and the key is never logged.
- AC-060: Given a send-email request, when it is sent, then a typed result carrying the `X-Message-Id` is returned.
- AC-061: Given a template request with substitutions, when it is sent, then the dynamic template is rendered server-side and the message is delivered.
- AC-062: Given sandbox mode enabled, when a send is requested, then the request is validated without delivering mail.
- AC-063: Given an `errors[]` array in the body, when the request fails, then `WirekitError::Api` carries the first code and message and the full list is retained in details.
- AC-064: Given an event-webhook body and signature, when it is verified with the configured public key, then a valid signature passes, and a re-serialized body fails verification (CON-215).
- AC-065: Given repeated events carrying the same `sg_event_id`, when they are received, then duplicates are identifiable so the caller can dedupe.
- AC-066: Given an attachment, when it is sent, then base64 and URL attachments are supported within the documented size and MIME limits.

### US-012: Post notifications to Slack and handle body-level errors

As a developer posting internal notifications, I want Slack's HTTP-200-with-`ok: false` behavior handled correctly, so that failures are not silently swallowed.

**Acceptance Criteria:**

- AC-067: Given a bot token, when the client is built, then the token is applied to requests and never appears in logs or traces.
- AC-068: Given a text message or a Block Kit payload, when it is posted, then a typed result carrying the channel and timestamp is returned.
- AC-069: Given an HTTP 200 response with `ok: false`, when the body reports an error, then `WirekitError::Api` carries the error string and the call is not treated as a success (CON-213).
- AC-070: Given `channel_not_found` or `not_in_channel`, when the request fails, then no retry occurs.
- AC-071: Given a 429 with `Retry-After`, when the request is retried, then the header is honored and cached channel data reduces repeated lookups.
- AC-072: Given a request body with `X-Slack-Signature` and `X-Slack-Request-Timestamp`, when it is verified, then the HMAC-SHA256 signature is validated over the raw bytes and a stale timestamp is rejected.
- AC-073: Given a file, when it is uploaded, then it is uploaded within the documented size and type limits.
- AC-074: Given a channel or user query, when it is requested, then typed channel and user results are returned.

### US-013: Publish `wirekit` 0.1.0 reproducibly

As a maintainer, I want a release-ready package and a documented runbook, so that the first publish is safe, verifiable, and repeatable.

**Acceptance Criteria:**

- AC-075: Given `cargo package -p wirekit --list`, when it runs, then the package includes `Cargo.toml`, `README.md`, `CHANGELOG.md`, `src/**`, `examples/**`, and the license files.
- AC-076: Given a clean tree, when `cargo publish -p wirekit --dry-run` runs, then it succeeds.
- AC-077: Given the published package metadata, when inspected, then repository, readme, keywords, categories, license, rust-version, and the docs.rs all-features setting resolve correctly.
- AC-078: Given `crates/wirekit/CHANGELOG.md`, when read, then a `0.1.0` entry describes the core and the four providers.
- AC-079: Given the repository documentation, when read, then a provider matrix lists the four providers with their auth scheme, base URL, and feature flag.
- AC-080: Given the crate documentation, when read, then a feature-flag table and an error reference (the `W####` codes) are present.
- AC-081: Given the examples directory, when `cargo build --examples --all-features` runs, then one example per provider compiles; examples needing credentials exit with a clear message when they are absent, and the NOAA example runs without a key.
- AC-082: Given `RELEASING.md`, when followed for `wirekit`, then it covers pre-flight, version and changelog, dry-run, the `wirekit-v0.1.0` tag, publish, verification, and yank or rollback.

## Requirements

### Functional Requirements

- FR-001: The repository MUST contain a `wirekit` crate as a workspace member at `crates/wirekit`, version 0.1.0, inheriting `edition`, `rust-version`, and `license` from `[workspace.package]`.
- FR-002: The crate MUST compile with `default = []`, always expose the core, and gate every provider module behind a separate additive feature.
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
- FR-027: The crate MUST expose a `noaa` feature-gated module with typed forecast, alert, and radar metadata models.
- FR-028: NOAA requests MUST carry a descriptive `User-Agent`, and a request without one MUST be rejected with `WirekitError::InvalidConfig`.
- FR-029: NOAA `problem+json` failures MUST map to `WirekitError::Api` with the reported status and detail.
- FR-030: The NOAA module SHOULD cache forecasts and alerts by grid or zone with a configurable TTL.
- FR-031: The crate MUST expose a `twilio` feature-gated module with typed SMS, WhatsApp, voice, MFA, and lookup models.
- FR-032: Twilio requests MUST be form-encoded and authenticate with Basic (Account SID plus Auth Token) or an API key and secret.
- FR-033: Twilio failures MUST map `code`, `message`, and HTTP status into `WirekitError::Api`, and authentication failures into `WirekitError::Auth`.
- FR-034: The Twilio module MUST verify `X-Twilio-Signature` over the raw request bytes with constant-time comparison.
- FR-035: The Twilio module MUST apply a per-account token bucket and surface remaining quota through tracing.
- FR-036: The crate MUST expose a `sendgrid` feature-gated module with typed email, template, attachment, and contact-list models.
- FR-037: SendGrid MUST authenticate with a bearer API key and MUST support a sandbox mode that validates without delivering.
- FR-038: SendGrid errors MUST map its `errors[]` array into `WirekitError::Api` with the first code and message, retaining the full list in details.
- FR-039: The SendGrid module MUST verify the event-webhook signature on the raw body bytes against a configured public key.
- FR-040: The crate MUST expose a `slack` feature-gated module with typed messages, Block Kit blocks, channels, users, and file uploads.
- FR-041: The Slack module MUST treat HTTP 200 with `ok: false` as an error mapped to `WirekitError::Api`, and MUST NOT treat 200 as success.
- FR-042: The Slack module MUST verify `X-Slack-Signature` over the raw bytes and MUST reject a stale `X-Slack-Request-Timestamp`.
- FR-043: Slack `channel_not_found` and `not_in_channel` MUST NOT be retried.
- FR-044: Provider modules MUST NOT introduce a second error type; every failure maps into `WirekitError`.
- FR-045: The crate MUST include one runnable example per provider, a provider matrix, a feature-flag table, and an error reference.
- FR-046: The crate MUST include a 0.1.0 changelog entry and complete publishable metadata with `docs.rs` all-features.
- FR-047: `cargo publish -p wirekit --dry-run` MUST succeed from a clean tree.
- FR-048: `RELEASING.md` MUST document the `wirekit` release with the tag `wirekit-v0.1.0`.
- FR-049: Each provider module MUST be gated by its own additive feature that MUST NOT enable or be enabled by another provider feature.
- FR-050: Every provider MUST use the shared core pipeline for auth, retry, rate limiting, tracing, and error mapping.

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
- NFR-011: Authentication (CON-511) - the pipeline MUST apply the configured auth scheme to every outbound request for every provider and MUST fail closed with `WirekitError::InvalidConfig` when credentials are missing.
- NFR-012: Rate limiting (CON-514) - the per-provider token bucket MUST be enforced before every outbound request, with per-provider overrides.
- NFR-013: Input validation (CON-515) - URLs, header values, form bodies, and response bodies MUST be validated; malformed input MUST return a typed `WirekitError` and MUST NOT panic.
- NFR-014: Environment security (CON-519) - credentials MUST come from caller-supplied configuration or secret stores; the crate MUST NOT read `.env` files and MUST NOT commit secrets.
- NFR-015: HTTPS (CON-520) - all outbound requests MUST use TLS; non-HTTPS base URLs MUST be rejected before any socket is opened.
- NFR-016: Webhook signature verification MUST operate on the raw request bytes; a body MUST NOT be re-serialized before verification (CON-215).
- NFR-017: Provider secrets (auth tokens, API keys, signing secrets, webhook public keys where sensitive) MUST be redacted in `Debug`, `Display`, logs, and traces.
- NFR-018: Each provider module MUST use the shared retry policy and MUST NOT implement its own retry loop.
- NFR-019: A provider MUST NOT treat HTTP 200 as success when the body reports an error (CON-213).
- NFR-020: The four provider features MUST be additive, independently compilable, and MSRV 1.85 compatible.
- NFR-021: Provider modules MUST NOT contain application domain types, billing rules, or vendor workflow code (CON-106).
- NFR-022: Published documentation, examples, and fixtures MUST NOT contain committed credentials or real provider tokens.

## UI/UX & Data Requirements

- **UI/UX**: Not applicable. This repository ships no UI layer (CON-301); `wirekit` and its provider modules expose typed Rust APIs only.
- **Data**: The crate stores no persistent data. In-memory state is limited to the shared client, the per-provider token buckets, the OAuth2 token cache, and the optional NOAA forecast/alert cache - all owned by the caller's process and dropped with it. The SendGrid contact-list and template models are request/response shapes, not stored records.

## Security Baseline

The feature touches authentication and an HTTP surface, so the CON-51x baseline is expanded below. The expansion applies to the core and to each of the four provider modules.

| Control | Status | Handling |
|---|---|---|
| CON-511 Authentication | REQUIRED | Expanded into NFR-011 and US-005, US-010, US-011, US-012. Every provider applies its scheme through the pipeline and fails closed. |
| CON-512 Authorization | NOT APPLICABLE | `wirekit` performs no tenant isolation or capability checks; that is `platform-core` and `quota-ledger` territory (CON-107). |
| CON-513 Security Headers | NOT APPLICABLE | The crate owns no HTTP server or response pipeline. |
| CON-514 Rate Limiting | REQUIRED | Expanded into NFR-012 and US-003, with per-provider buckets for NOAA, Twilio, SendGrid, and Slack. |
| CON-515 Input Validation | REQUIRED | Expanded into NFR-013 and US-007, US-009 to US-012: provider bodies, form parameters, and headers are validated. |
| CON-516 Injection Protection | NOT APPLICABLE | The crate owns no database or NoSQL backend. |
| CON-517 XSS Protection | NOT APPLICABLE | The crate renders no HTML or markup; Slack Block Kit and email HTML are passed through as caller data. |
| CON-518 CSRF Protection | NOT APPLICABLE | The crate owns no browser session or cookie state; signatures and tokens are the boundary. |
| CON-519 Environment Security | REQUIRED | Expanded into NFR-014 and NFR-022; credentials come from the caller, and no fixture or example commits a secret. |
| CON-520 HTTPS / HSTS | REQUIRED | Expanded into NFR-015; every provider base URL is HTTPS-only and rejected otherwise before any socket. |

## Edge Cases & Error States

- EC-001: 429 with a numeric `Retry-After` → wait at least that long, then retry; if attempts are exhausted, return `WirekitError::RateLimit { retry_after }`.
- EC-002: 429 without `Retry-After` → apply jittered backoff.
- EC-003: 5xx with attempts exhausted → `WirekitError::Api` carrying the last status and any parsed code/message.
- EC-004: 400, 401, 403, or 404 → no retry; return immediately.
- EC-005: Network failure mid-flight → retried, then `WirekitError::Network`.
- EC-006: Request timeout → retried, then `WirekitError::Timeout`.
- EC-007: `Retry-After` given as an HTTP date, or unparseable → parsed safely, falling back to backoff without panicking.
- EC-008: Empty token bucket → the request waits asynchronously and proceeds when a token is available.
- EC-009: OAuth2 token endpoint returns 4xx → `WirekitError::Auth`; no retry on a non-429 4xx.
- EC-010: OAuth2 token expires between requests → refreshed before the next request.
- EC-011: HTTP 200 with Slack `ok: false` or a GraphQL `errors[]` array → `WirekitError::Api` via the body hook (CON-213).
- EC-012: Malformed JSON body → `WirekitError::Decode`; no panic.
- EC-013: Base URL using `http://` → rejected before any socket is opened.
- EC-014: Caller-injected client → used verbatim; no second pool.
- EC-015: Concurrent requests for one provider → share one bucket so the aggregate rate is respected.
- EC-016: Secret present in configuration → `Debug` prints `***`; no `Serialize`.
- EC-017: Oversized response body → bounded by a configurable limit; exceeding it returns a typed error.
- EC-018: Missing credentials → `WirekitError::InvalidConfig`; no request is sent.
- EC-019: HMAC signing → computed over the raw request bytes; a re-serialized body would not verify.
- EC-020: Clock skew near token expiry → refresh uses a configurable safety margin.
- EC-021: NOAA request without a `User-Agent` → rejected with `WirekitError::InvalidConfig`.
- EC-022: NOAA `problem+json` with a `status` that disagrees with the HTTP status → the HTTP status is authoritative and the detail is preserved.
- EC-023: NOAA forecast for a location outside US coverage → `WirekitError::Api`; the module does not fabricate a forecast.
- EC-024: NOAA cache entry expired → refreshed; a stale entry is never returned past its TTL.
- EC-025: Twilio signature verification over a form body with repeated or re-ordered parameters → canonical ordering is applied, and a tampered parameter fails.
- EC-026: Twilio webhook with a stale or missing signature → verification fails; the raw body is never re-serialized.
- EC-027: Twilio authentication failure (`401`) → `WirekitError::Auth`, no retry.
- EC-028: Twilio rate-limit hit → throttled by the per-account bucket, and remaining quota is visible in tracing.
- EC-029: SendGrid `errors[]` with multiple entries → the first code/message is surfaced and the full list is retained.
- EC-030: SendGrid event webhook whose body was re-serialized before verification → verification fails (CON-215).
- EC-031: SendGrid attachment over the documented size limit → rejected before sending.
- EC-032: Slack HTTP 200 with `ok: false` and `channel_not_found` → `WirekitError::Api`; no retry.
- EC-033: Slack `Retry-After` on 429 → honored; channel data is cached.
- EC-034: Slack signature with an old `X-Slack-Request-Timestamp` → rejected as a replay.
- EC-035: Slack file over the documented size limit → rejected before upload.
- EC-036: A provider example run without credentials → exits with a clear message instead of sending a request.

## Out of Scope

- Providers beyond NOAA, Twilio, SendGrid, and Slack (the remaining `catalogue/` providers, including Stripe, Meta, EagleView, QuickBooks, and the Google OAuth helper).
- OAuth2 Authorization Code + PKCE, browser redirects, and multi-tenant install flows (Slack install and Google OAuth belong to BP-015 per CON-212).
- Stateful concerns: shared quota accounting (`quota-ledger`), webhook delivery state, dedupe stores, and outboxes (`hookgate`), and the tenant/provider registry (`platform-core`).
- MFA orchestration policy, messaging compliance decisions, and message templating owned by the consuming application.
- Streaming uploads/downloads, WebSocket or gateway transports (Slack Socket Mode, Twilio Media Streams), and multipart file transfer beyond a single-body request.
- Application domain types, billing rules, and product workflows (CON-106).
- Publishing `wirekit` from the feature branch and creating the `wirekit-v0.1.0` tag; both are manual post-merge steps.
- JavaScript/TypeScript bindings.

## Dependencies & Risks

### Dependencies

| Dependency | Type | Notes |
|---|---|---|
| `reqwest` | library | HTTP client; `rustls-tls` default; the shared client per process (CON-210). |
| `tokio` | library | Async runtime (`rt`, `time`, `macros`) for retry and rate-limit timers. |
| `tracing` | library | Observability events (CON-211). |
| `thiserror` | library | `WirekitError` derive. |
| `serde` + `serde_json` | library | Typed request/response models. |
| `http` | library | Shared HTTP types. |
| `rand` | library | Backoff jitter. |
| `async-trait` | library | Object-safe async auth and transport traits. |
| `url` | library | URL parsing and HTTPS validation. |
| `hmac` | library | Webhook signature computation (Twilio, Slack). |
| `sha1` | library | Twilio `X-Twilio-Signature` digest. |
| `sha2` | library | Slack `X-Slack-Signature` digest. |
| `base64` | library | Basic auth encoding and SendGrid attachments. |
| `hex` | library | Slack signature and SendGrid event encoding. |
| `percent-encoding` | library | Slack signature base string encoding. |
| `serde_urlencoded` | library | Twilio form-encoded request bodies. |
| `p256` + `ecdsa` | library | SendGrid event-webhook signature verification. |
| `wiremock` | library (dev) | Local HTTP mock for provider tests; never published. |
| crates.io | service | Publication target (BP-010); name check required before publish (CON-111). |
| GitHub Actions | service | Workspace and MSRV CI from BP-002. |

### Risks

- RISK-001: A single crate carrying 26+ provider features becomes a release and build-time bottleneck. Mitigation: M1 ships four providers; measure build times and propose a companion crate if needed (CON-104 amendment path).
- RISK-002: 0.x churn in the error, retry, or auth contract breaks provider modules. Mitigation: `#[non_exhaustive]`, stable `W####` codes, an additive-only feature policy, and shared-pipeline use until the 1.0 freeze.
- RISK-003: A dependency raises its MSRV above 1.85. Mitigation: the MSRV CI job fails fast; pin or downgrade with a recorded decision.
- RISK-004: `rustls` versus `native-tls` surprises on a platform. Mitigation: `rustls` is the default and a documented feature selects `native-tls`.
- RISK-005: A secret leaks into logs, traces, an error payload, a fixture, or an example. Mitigation: `Secret<T>`, no `Serialize` for secret fields, redacted tracing fields, placeholder-only fixtures, and property tests from the Test agent.
- RISK-006: A provider needs behavior the pipeline cannot express. Mitigation: the body-level error hook, per-provider retry and rate-limit overrides, injectable clients, and canonical-form signing hooks.
- RISK-007: The rate limiter blocks the async runtime or is unfair under concurrency. Mitigation: async token acquisition, per-provider buckets, and EC-015.
- RISK-008: Twilio A2P 10DLC or toll-free verification is a launch blocker for SMS at scale. Mitigation: the module is sender-agnostic; registration is a consumer concern and is recorded as an open question, not a stub.
- RISK-009: SendGrid sender authentication (SPF, DKIM, DMARC) or data residency limits real use. Mitigation: the module is documented as transport-only; deliverability configuration stays with the consumer.
- RISK-010: Slack app review or scopes delay real use. Mitigation: the module ships bot-token usage and documented scopes; the install flow is deferred to BP-015.
- RISK-011: NOAA polling across many workspaces is expensive in aggregate. Mitigation: the optional grid/zone cache and a documented 15-minute polling budget.
- RISK-012: Provider error-shape drift (new codes, changed bodies) silently changes behavior. Mitigation: fixtures pin the shapes, unknown bodies map to `WirekitError::Api` rather than panicking, and tests are owned by `/sdrive:test`.
- RISK-013: Examples that need credentials cannot be verified in CI. Mitigation: examples compile in CI and exit with a clear message when credentials are absent; the NOAA example runs without a key.
- RISK-014: Scope creeps into other providers or stateful crates. Mitigation: the explicit out-of-scope list; later work becomes a new version of this feature (CON-612).

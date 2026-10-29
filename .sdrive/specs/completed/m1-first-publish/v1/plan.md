---
title: M1 First publish - wirekit 0.1.0 core, four providers, and release preparation
status: backlog
version: v1
created: 2026-10-20
updated: 2026-10-20
---

# M1 First publish - technical plan

## Approach

Add `crates/wirekit` as a new workspace member (`members = ["crates/*"]` already covers it, so the root `Cargo.toml` needs no change). The crate ships the provider-independent core plus four feature-gated provider modules, then a release-ready package.

Work proceeds in dependency order: the core contract first (BP-005), then the four providers built only against that contract (BP-006 to BP-009), then release preparation (BP-010).

The core is always compiled. `default = []` means *no provider module enabled*, not *no core*. Each provider is an additive feature that adds no second error type, retry loop, or client (CON-104, CON-207, FR-050). No application domain types, no database, no tenant model, no global mutable singleton (CON-106, CON-107, NFR-010).

## Architecture

```
crates/wirekit/
  Cargo.toml         # 0.1.0, default = [], additive provider features, docs.rs all-features
  README.md          # overview, feature table, provider matrix, error reference
  CHANGELOG.md       # 0.1.0 entry
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
    noaa.rs          # feature "noaa": typed forecast, alert, radar metadata
    twilio.rs        # feature "twilio": SMS, WhatsApp, voice, MFA, lookup, signature verify
    sendgrid.rs      # feature "sendgrid": email, templates, sandbox, errors[], event webhook
    slack.rs         # feature "slack": messages, blocks, channels, users, files, signature verify
  examples/
    noaa.rs          # runs with no key
    twilio.rs        # needs credentials; exits clearly without them
    sendgrid.rs      # needs credentials
    slack.rs         # needs credentials
```

Request flow through the pipeline, used by every provider:

```
provider declares URL + method + body + response type
        |
        v
  [auth] apply scheme      -> fail closed with InvalidConfig if credentials missing
        |
        v
  [rate limit] acquire token (per provider/account) -> async wait, emits metric
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
| `crates/wirekit/src/noaa.rs` | create | NOAA module: forecast, alerts, grid/zone cache, `problem+json`, required `User-Agent` |
| `crates/wirekit/src/twilio.rs` | create | Twilio module: form bodies, SMS/WhatsApp/voice/MFA/lookup, `X-Twilio-Signature`, per-account bucket |
| `crates/wirekit/src/sendgrid.rs` | create | SendGrid module: email, templates, sandbox, `errors[]`, attachments, ECDSA event webhook |
| `crates/wirekit/src/slack.rs` | create | Slack module: messages, blocks, channels, users, files, `ok:false` hook, `X-Slack-Signature` |
| `crates/wirekit/README.md` | create | Purpose, feature table, provider matrix, error reference, minimal usage |
| `crates/wirekit/CHANGELOG.md` | create | `0.1.0` entry covering core and the four providers |
| `crates/wirekit/examples/noaa.rs` | create | Runnable NOAA example with no credentials |
| `crates/wirekit/examples/twilio.rs` | create | Twilio example; exits clearly without credentials |
| `crates/wirekit/examples/sendgrid.rs` | create | SendGrid example; exits clearly without credentials |
| `crates/wirekit/examples/slack.rs` | create | Slack example; exits clearly without credentials |
| `.github/workflows/ci.yml` | modify | Feature-matrix build (default and each provider feature) and the MSRV build |
| `RELEASING.md` | modify | Add the `wirekit` release section and the `wirekit-v0.1.0` tag |

## Data Model

Types introduced by this feature. All are `create`; none is persisted.

| Type | Fields | Notes |
|---|---|---|
| `WirekitError` | `Api`, `Auth`, `RateLimit`, `Timeout`, `Network`, `InvalidConfig`, `Decode` | `#[non_exhaustive]`; stable `W####` codes |
| `RetryPolicy` | `attempts`, `base_delay`, `max_delay`, `jitter` | Defaults: 3 attempts, exponential, bounded jitter |
| `RateLimitConfig` | `capacity`, `refill_per_second` | Per provider; overrides the default |
| `TokenBucket` | `capacity`, `available`, `refill_rate`, `last_refill` | In-memory, per provider/account |
| `Secret<T>` | `inner` | Redacted in `Debug`/`Display`; no `Serialize` |
| `AuthScheme` | `Bearer`, `Basic`, `ApiKeyHeader`, `ApiKeyQuery`, `Hmac`, `OAuth2ClientCredentials` | Applied before every request |
| `OAuth2Token` | `access_token`, `expires_at` | Cached per client; refreshed before expiry |
| `WirekitClient` | `http`, `retry`, `rate_limit`, `auth`, `base_url` | Owns the shared `reqwest::Client`; explicit construction |
| `RequestSpec` | `method`, `url`, `headers`, `body`, `response_type`, `body_error_hook` | What a provider supplies |
| `NoaaClient` | `client`, `user_agent`, `cache_ttl` | Requires a descriptive `User-Agent` |
| `ForecastRequest` / `ForecastResponse` | `lat`, `lon` / `periods`, `temperature`, `precipitation` | Typed NOAA point forecast |
| `WeatherAlert` | `id`, `event`, `severity`, `areas`, `description` | Typed NOAA alert |
| `RadarMetadata` | `station`, `updated`, `bounds` | Radar metadata |
| `TwilioClient` | `client`, `account_sid`, `auth` | Basic or API key + secret |
| `SendSmsRequest` / `MessageResponse` | `to`, `from`, `body` / `sid`, `status` | SMS and WhatsApp |
| `CallRequest` / `CallStatus` | `to`, `from`, `twiml` / `sid`, `status` | Voice |
| `MfaChallenge` / `MfaVerification` | `service_sid`, `to` / `valid` | MFA |
| `LookupResult` | `phone_number`, `validation` | Lookup |
| `SendGridClient` | `client`, `api_key` | Bearer |
| `SendEmailRequest` | `from`, `to`, `subject`, `content`, `template`, `attachments`, `sandbox_mode` | Transactional email |
| `SendGridError` | `code`, `message`, `details` | Mirrors `errors[]` for mapping only |
| `SendGridEvent` | `sg_event_id`, `event`, `timestamp`, `email` | Event webhook payload |
| `SlackClient` | `client`, `token`, `signing_secret` | Bot token; signing secret optional |
| `SlackMessage` | `channel`, `text`, `blocks`, `thread_ts` | Message or Block Kit |
| `SlackBlock` | `block_type`, `fields` | Block Kit builder |
| `SlackChannel` / `SlackUser` | `id`, `name` / `id`, `name` | Queries |
| `SlackFile` | `channels`, `content`, `filename`, `filetype` | Upload |

No data is stored or transmitted by the crate itself; all state is in-process and dropped with the caller's client.

## Feature Flags

- `default = []` - no provider module enabled; the core is always present.
- Provider features: `noaa`, `twilio`, `sendgrid`, `slack`. Each is additive and independent (FR-049, NFR-020).
- A TLS feature selects the backend: `rustls-tls` (default) or `native-tls`.
- Enabling any feature MUST NOT enable or disable another (CON-204).

## Security Considerations

The CON-51x baseline is expanded into requirements, dependencies, and tasks, and applies to the core and each provider module.

| Control | Status | Requirement | Handling |
|---|---|---|---|
| CON-511 Authentication | REQUIRED | NFR-011 | Pipeline applies the configured scheme per provider and fails closed with `InvalidConfig` |
| CON-512 Authorization | NOT APPLICABLE | - | No tenant isolation; that is `platform-core`/`quota-ledger` (CON-107) |
| CON-513 Security Headers | NOT APPLICABLE | - | No HTTP server or response pipeline |
| CON-514 Rate Limiting | REQUIRED | NFR-012 | Per-provider/account token buckets enforced before every request |
| CON-515 Input Validation | REQUIRED | NFR-013 | URLs, headers, form bodies, and response bodies validated; typed errors, never panic |
| CON-516 Injection Protection | NOT APPLICABLE | - | No database or NoSQL backend |
| CON-517 XSS Protection | NOT APPLICABLE | - | No HTML rendering; Slack blocks and email HTML are caller data |
| CON-518 CSRF Protection | NOT APPLICABLE | - | No browser session or cookie state |
| CON-519 Environment Security | REQUIRED | NFR-014, NFR-022 | Caller-supplied credentials only; no `.env` reading; no committed secrets in fixtures |
| CON-520 HTTPS / HSTS | REQUIRED | NFR-015 | Non-HTTPS base URLs rejected before any socket; HSTS documented at the deployment edge |

Additionally CON-215 governs webhook verification: every verifier operates on raw bytes (NFR-016).

## Performance Considerations

| Requirement | Strategy |
|---|---|
| NFR-005 | Token acquisition and backoff `await` a timer; no `std::thread::sleep` and no busy-wait |
| NFR-006 | Default request timeout and connection-pool limits on the shared client; one pool per process |
| FR-030 | The NOAA module caches by grid/zone with a TTL so repeated forecasts do not re-poll |
| AC-071 | Slack channel/user lookups are cached to reduce repeated calls |
| NFR-009 | One bucket lookup, one header mutation set, and one decode per request; no per-request client |

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
| `hmac` | library | add | Webhook signature computation (Twilio, Slack) |
| `sha1` | library | add | Twilio `X-Twilio-Signature` digest |
| `sha2` | library | add | Slack `X-Slack-Signature` digest |
| `base64` | library | add | Basic auth encoding, SendGrid attachments |
| `hex` | library | add | Slack signature and SendGrid event encoding |
| `percent-encoding` | library | add | Slack signature base-string encoding |
| `serde_urlencoded` | library | add | Twilio form-encoded request bodies |
| `p256` + `ecdsa` | library | add | SendGrid event-webhook signature verification |
| `wiremock` | library | add | Dev-dependency only: local HTTP mock for provider tests |
| `crates.io` | service | no_change | Publication target (BP-010); name checked before publish (CON-111) |
| `GitHub Actions` | service | no_change | CI from BP-002; add a feature-matrix and MSRV build |

## Risks

- A single crate with 26+ provider features becomes a build bottleneck -> M1 ships four; measure and propose a companion crate if needed.
- 0.x churn in the shared contract breaks providers -> `#[non_exhaustive]`, stable codes, additive-only features until the 1.0 freeze.
- A dependency raises its MSRV above 1.85 -> the MSRV CI job fails fast; pin or downgrade with a recorded decision.
- Secret leakage into logs, traces, errors, fixtures, or examples -> `Secret<T>`, no `Serialize`, redacted tracing, placeholder-only fixtures, property tests from the Test agent.
- A provider needs behavior the pipeline cannot express -> body-level error hook, per-provider overrides, injectable client, canonical-form signing hooks.
- Twilio A2P 10DLC or toll-free verification blocks SMS at scale -> sender-agnostic module; registration stays a consumer concern.
- SendGrid sender authentication (SPF, DKIM, DMARC) or data residency -> transport-only module; deliverability stays with the consumer.
- Slack app review or scopes delay real use -> bot-token usage and documented scopes; install flow deferred to BP-015.
- NOAA polling across many workspaces is expensive -> grid/zone cache and a documented 15-minute polling budget.
- Provider error-shape drift -> fixtures pin shapes; unknown bodies map to `WirekitError::Api` rather than panicking.
- Examples needing credentials cannot be verified in CI -> examples compile and exit clearly without credentials; NOAA runs keyless.
- Scope creep into other providers or stateful crates -> the explicit out-of-scope list.

## Verification

Verification tasks are limited to the formatter, linter, doc build, build/typecheck, the MSRV build, the feature-matrix build, `cargo package --list`, `cargo publish --dry-run`, `sdrive verify`, `node .sdrive/scripts/validate-governance.js`, and manual checks. Writing and running tests belongs to `/sdrive:test` (CON-404, No Test Tasks Rule).

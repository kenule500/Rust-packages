# Changelog

All notable changes to `wirekit` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-10-20

The first release of `wirekit`: the shared HTTP core plus four provider modules.

### Added

- `WirekitClient`, a cloneable, `Arc`-backed client that owns the single pooled
  `reqwest::Client`, the retry policy, the rate limiter, and the response size
  bound. Cloning it into several providers shares one connection pool.
- `WirekitError`, one `#[non_exhaustive]` enum with stable `W2001`–`W2007`
  reason codes and a `Debug` implementation that omits wrapped causes so a URL
  carrying an API key cannot leak.
- `Secret<T>`, whose `Debug` and `Display` render `***` and which does not
  implement `Serialize`.
- `RetryPolicy` with exponential backoff, bounded jitter, a `max_delay`
  ceiling, and `Retry-After` parsing in both delay-seconds and HTTP-date form.
- `RateLimitConfig` and a hand-rolled token bucket with per-provider and
  per-account overrides. A rate-limit wait yields to the runtime instead of
  blocking it.
- Auth schemes: bearer, basic, API key in a header, API key in the query string
  (percent-encoded), HMAC request signing, and OAuth2 client credentials with a
  cached, single-flight token.
- A shared request pipeline: auth, rate limit, retry, body-error hook, decode,
  bounded body reads, and redacted tracing events.
- The `noaa` feature: point forecasts, active alerts, single-alert lookup,
  radar metadata, a required descriptive `User-Agent`, and a TTL response cache.
- The `twilio` feature: SMS, WhatsApp, voice, call status, Verify v2 MFA,
  Lookups v2 phone validation, and constant-time webhook signature
  verification (HMAC-SHA1 base64 over URL plus sorted form parameters).
- The `sendgrid` feature: transactional email with templates, attachments, and
  sandbox mode; the `X-Message-Id` result; `errors[]` mapped to
  `WirekitError::Api` with the full list in `details`; contact-list models;
  Event Webhook parsing with `sg_event_id`; and ECDSA P-256 signature
  verification over `timestamp || payload`.
- The `slack` feature: `chat.postMessage`, conversation and user lookups with a
  channel cache, `files.upload`, `ok: false` handling at HTTP 200, declared
  non-retryable channel errors, and HMAC-SHA256 request signature verification
  with a clock-skew check.
- `native-tls` as an alternative TLS backend. rustls is compiled in and used by
  default.
- Examples for all four providers, a provider matrix and error reference in the
  README, and a `wirekit-v0.1.0` section in the release runbook.

### Notes

- MSRV is Rust 1.85, edition 2021.
- `default = []` means "no provider module", not "no core".
- The crate performs outbound HTTPS requests only. It never listens, never
  reads credentials from the environment, and never persists state.

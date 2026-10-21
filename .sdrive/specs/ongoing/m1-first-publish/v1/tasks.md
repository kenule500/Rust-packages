---
title: M1 First publish - execution tasks
status: backlog
version: v1
created: 2026-10-20
updated: 2026-10-20
---

# M1 First publish - execution tasks

Production-code tasks only. Writing and running tests belongs to `/sdrive:test` (CON-404, No Test Tasks Rule). Verification is limited to the formatter, linter, doc build, build/typecheck, the MSRV build, the feature-matrix build, `cargo package --list`, `cargo publish --dry-run`, the governance validator, and manual checks.

## P1 - Crate scaffold and features

- **T1.1** Create the `wirekit` crate and workspace membership - `crates/wirekit` with a `src/` tree; confirm `members = ["crates/*"]` picks it up; inherit `edition`, `rust-version`, `license`.
- **T1.2** Define features, metadata, and docs.rs configuration - version 0.1.0, `default = []`, `rustls-tls` default with a `native-tls` alternative, the four additive provider features, `package.metadata.docs.rs` all-features.
- **T1.3** Add crate-level lints and feature documentation - `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, module declarations, feature table, I/O boundary.
- **T1.4** Add the feature-matrix and MSRV CI builds - extend `.github/workflows/ci.yml`; no test files.

## P2 - Error model

- **T2.1** Define `WirekitError` and its stable codes - seven variants, `#[non_exhaustive]`, `W####` codes.
- **T2.2** Implement `Display`, `Error`, and `source` with redacted fields.
- **T2.3** Add status and body mapping helpers - into `WirekitError::Api` and `WirekitError::Auth`; no provider-specific error type.
- **T2.4** Verify additive extensibility of the error enum.

## P3 - Shared client and configuration

- **T3.1** Implement the client builder and shared `reqwest` client.
- **T3.2** Support caller-injected clients - used verbatim, no second pool.
- **T3.3** Configure timeouts, TLS, proxy, headers, and pool limits.
- **T3.4** Reject non-HTTPS base URLs - `InvalidConfig` before any socket.

## P4 - Retry policy

- **T4.1** Define `RetryPolicy` and its defaults - 3 attempts, base/max delay, jitter bounds.
- **T4.2** Implement the retry loop with jittered exponential backoff - async timer, no blocking; the single shared loop for every provider.
- **T4.3** Honor the `Retry-After` header - numeric and HTTP-date forms, safe fallback.
- **T4.4** Implement retry classification - 5xx/429/network retryable; other 4xx not; provider-declared non-retryable strings.

## P5 - Rate limiting

- **T5.1** Implement the per-provider token bucket.
- **T5.2** Add per-provider overrides (and per Twilio account).
- **T5.3** Implement non-blocking async acquire with tracing.

## P6 - Auth layer

- **T6.1** Define the `AuthScheme` trait and contract - object-safe async trait, applied on every request; NOAA declares no auth.
- **T6.2** Implement Bearer, Basic, and API key schemes - header and query.
- **T6.3** Implement HMAC request signing - over canonical raw bytes, constant-time comparison.
- **T6.4** Implement OAuth2 client-credentials flow - cache, single-flight fetch, pre-expiry refresh.
- **T6.5** Fail closed when credentials are missing.

## P7 - Observability

- **T7.1** Emit request-start and request-finish events - redacted URL, duration.
- **T7.2** Emit retry, rate-limit, and error events.
- **T7.3** Keep tracing output secret-free.

## P8 - Secret redaction

- **T8.1** Implement the `Secret` wrapper - `Debug`/`Display` render `***`.
- **T8.2** Prevent serialization of secret fields.
- **T8.3** Audit error, log, and debug paths for leaks.

## P9 - Request pipeline and decoding

- **T9.1** Define `RequestSpec` and the provider entry point.
- **T9.2** Implement pipeline ordering - auth, rate limit, retry, send, decode; every provider routes through it.
- **T9.3** Decode bodies and map decode failures - `WirekitError::Decode`, never a panic.
- **T9.4** Implement the body-level error hook - CON-213 (`ok: false`, `errors[]`).
- **T9.5** Bound response size and validate untrusted input.

## P10 - Security baseline controls

- **T10.1** CON-511: enforce authentication and fail closed.
- **T10.2** CON-514: enforce rate limiting before every request.
- **T10.3** CON-515: validate untrusted input.
- **T10.4** CON-519: source credentials from the caller only.
- **T10.5** CON-520: enforce HTTPS.

## P11 - NOAA provider

- **T11.1** Implement `NoaaClient` with a required `User-Agent`; no auth.
- **T11.2** Implement point forecast models and requests.
- **T11.3** Implement alert list, alert get, and radar metadata.
- **T11.4** Map `problem+json` failures.
- **T11.5** Add the optional grid/zone cache.

## P12 - Twilio provider

- **T12.1** Implement `TwilioClient` with Basic or API-key auth.
- **T12.2** Implement form-encoded SMS and WhatsApp sends.
- **T12.3** Implement voice call creation and status.
- **T12.4** Implement MFA challenge and verification.
- **T12.5** Implement phone number lookup.
- **T12.6** Map Twilio errors into the shared error type.
- **T12.7** Verify the `X-Twilio-Signature` webhook header over raw bytes.
- **T12.8** Add the per-account token bucket.

## P13 - SendGrid provider

- **T13.1** Implement `SendGridClient` with bearer auth.
- **T13.2** Implement transactional email send.
- **T13.3** Implement dynamic template sends.
- **T13.4** Implement sandbox mode.
- **T13.5** Map the `errors[]` array into `WirekitError`.
- **T13.6** Verify the ECDSA event-webhook signature over raw bytes; retain `sg_event_id`.
- **T13.7** Implement base64 and URL attachments.

## P14 - Slack provider

- **T14.1** Implement `SlackClient` with bot-token auth (install flow out of scope).
- **T14.2** Implement messages and Block Kit payloads.
- **T14.3** Treat `ok: false` as an error at HTTP 200.
- **T14.4** Declare non-retryable Slack errors (`channel_not_found`, `not_in_channel`).
- **T14.5** Honor `Retry-After` and cache channel data.
- **T14.6** Verify the `X-Slack-Signature` header; reject stale timestamps.
- **T14.7** Implement file uploads.
- **T14.8** Implement channel and user queries.

## P15 - Docs, examples, and release preparation

- **T15.1** Complete publishable package metadata.
- **T15.2** Write the `0.1.0` changelog entry.
- **T15.3** Write the README, provider matrix, feature table, and error reference.
- **T15.4** Add one runnable example per provider - placeholders only.
- **T15.5** Add the `wirekit` release section to `RELEASING.md` (`wirekit-v0.1.0`).
- **T15.6** Verify the package (`cargo package --list`) and the publish dry-run.

## P16 - Verification

- **T16.1** Run the formatter check.
- **T16.2** Run clippy with warnings denied.
- **T16.3** Build documentation with warnings denied.
- **T16.4** Build the crate on Rust 1.85 (build only; no test run).
- **T16.5** Build the feature matrix (default and each provider feature).
- **T16.6** Run the governance validator and manual checks (no test run; `/sdrive:test` owns tests).

## Handoff

Every acceptance criterion (AC-001 to AC-082) is linked to at least one task above and to a requirement in `traceability.json`. The Test agent fills in the `TEST-###` IDs and writes the tests.

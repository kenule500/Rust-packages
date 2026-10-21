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

- [x] **T1.1** Create the `wirekit` crate and workspace membership - `crates/wirekit` with a `src/` tree; confirm `members = ["crates/*"]` picks it up; inherit `edition`, `rust-version`, `license`.
- [x] **T1.2** Define features, metadata, and docs.rs configuration - version 0.1.0, `default = []`, `rustls-tls` default with a `native-tls` alternative, the four additive provider features, `package.metadata.docs.rs` all-features.
- [x] **T1.3** Add crate-level lints and feature documentation - `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, module declarations, feature table, I/O boundary.
- [x] **T1.4** Add the feature-matrix and MSRV CI builds - extend `.github/workflows/ci.yml`; no test files.

## P2 - Error model

- [x] **T2.1** Define `WirekitError` and its stable codes - seven variants, `#[non_exhaustive]`, `W####` codes.
- [x] **T2.2** Implement `Display`, `Error`, and `source` with redacted fields.
- [x] **T2.3** Add status and body mapping helpers - into `WirekitError::Api` and `WirekitError::Auth`; no provider-specific error type.
- [x] **T2.4** Verify additive extensibility of the error enum.

## P3 - Shared client and configuration

- [x] **T3.1** Implement the client builder and shared `reqwest` client.
- [x] **T3.2** Support caller-injected clients - used verbatim, no second pool.
- [x] **T3.3** Configure timeouts, TLS, proxy, headers, and pool limits.
- [x] **T3.4** Reject non-HTTPS base URLs - `InvalidConfig` before any socket.

## P4 - Retry policy

- [x] **T4.1** Define `RetryPolicy` and its defaults - 3 attempts, base/max delay, jitter bounds.
- [x] **T4.2** Implement the retry loop with jittered exponential backoff - async timer, no blocking; the single shared loop for every provider.
- [x] **T4.3** Honor the `Retry-After` header - numeric and HTTP-date forms, safe fallback.
- [x] **T4.4** Implement retry classification - 5xx/429/network retryable; other 4xx not; provider-declared non-retryable strings.

## P5 - Rate limiting

- [x] **T5.1** Implement the per-provider token bucket.
- [x] **T5.2** Add per-provider overrides (and per Twilio account).
- [x] **T5.3** Implement non-blocking async acquire with tracing.

## P6 - Auth layer

- [x] **T6.1** Define the `AuthScheme` trait and contract - object-safe async trait, applied on every request; NOAA declares no auth.
- [x] **T6.2** Implement Bearer, Basic, and API key schemes - header and query.
- [x] **T6.3** Implement HMAC request signing - over canonical raw bytes, constant-time comparison.
- [x] **T6.4** Implement OAuth2 client-credentials flow - cache, single-flight fetch, pre-expiry refresh.
- [x] **T6.5** Fail closed when credentials are missing.

## P7 - Observability

- [x] **T7.1** Emit request-start and request-finish events - redacted URL, duration.
- [x] **T7.2** Emit retry, rate-limit, and error events.
- [x] **T7.3** Keep tracing output secret-free.

## P8 - Secret redaction

- [x] **T8.1** Implement the `Secret` wrapper - `Debug`/`Display` render `***`.
- [x] **T8.2** Prevent serialization of secret fields.
- [x] **T8.3** Audit error, log, and debug paths for leaks.

## P9 - Request pipeline and decoding

- [x] **T9.1** Define `RequestSpec` and the provider entry point.
- [x] **T9.2** Implement pipeline ordering - auth, rate limit, retry, send, decode; every provider routes through it.
- [x] **T9.3** Decode bodies and map decode failures - `WirekitError::Decode`, never a panic.
- [x] **T9.4** Implement the body-level error hook - CON-213 (`ok: false`, `errors[]`).
- [x] **T9.5** Bound response size and validate untrusted input.

## P10 - Security baseline controls

- [x] **T10.1** CON-511: enforce authentication and fail closed.
- [x] **T10.2** CON-514: enforce rate limiting before every request.
- [x] **T10.3** CON-515: validate untrusted input.
- [x] **T10.4** CON-519: source credentials from the caller only.
- [x] **T10.5** CON-520: enforce HTTPS.

## P11 - NOAA provider

- [x] **T11.1** Implement `NoaaClient` with a required `User-Agent`; no auth.
- [x] **T11.2** Implement point forecast models and requests.
- [x] **T11.3** Implement alert list, alert get, and radar metadata.
- [x] **T11.4** Map `problem+json` failures.
- [x] **T11.5** Add the optional grid/zone cache.

## P12 - Twilio provider

- [x] **T12.1** Implement `TwilioClient` with Basic or API-key auth.
- [x] **T12.2** Implement form-encoded SMS and WhatsApp sends.
- [x] **T12.3** Implement voice call creation and status.
- [x] **T12.4** Implement MFA challenge and verification.
- [x] **T12.5** Implement phone number lookup.
- [x] **T12.6** Map Twilio errors into the shared error type.
- [x] **T12.7** Verify the `X-Twilio-Signature` webhook header over raw bytes.
- [x] **T12.8** Add the per-account token bucket.

## P13 - SendGrid provider

- [x] **T13.1** Implement `SendGridClient` with bearer auth.
- [x] **T13.2** Implement transactional email send.
- [x] **T13.3** Implement dynamic template sends.
- [x] **T13.4** Implement sandbox mode.
- [x] **T13.5** Map the `errors[]` array into `WirekitError`.
- [x] **T13.6** Verify the ECDSA event-webhook signature over raw bytes; retain `sg_event_id`.
- [x] **T13.7** Implement base64 and URL attachments.

## P14 - Slack provider

- [x] **T14.1** Implement `SlackClient` with bot-token auth (install flow out of scope).
- [x] **T14.2** Implement messages and Block Kit payloads.
- [x] **T14.3** Treat `ok: false` as an error at HTTP 200.
- [x] **T14.4** Declare non-retryable Slack errors (`channel_not_found`, `not_in_channel`).
- [x] **T14.5** Honor `Retry-After` and cache channel data.
- [x] **T14.6** Verify the `X-Slack-Signature` header; reject stale timestamps.
- [x] **T14.7** Implement file uploads.
- [x] **T14.8** Implement channel and user queries.

## P15 - Docs, examples, and release preparation

- [x] **T15.1** Complete publishable package metadata.
- [x] **T15.2** Write the `0.1.0` changelog entry.
- [x] **T15.3** Write the README, provider matrix, feature table, and error reference.
- [x] **T15.4** Add one runnable example per provider - placeholders only.
- [x] **T15.5** Add the `wirekit` release section to `RELEASING.md` (`wirekit-v0.1.0`).
- [x] **T15.6** Verify the package (`cargo package --list`) and the publish dry-run.

## P16 - Verification

- [x] **T16.1** Run the formatter check.
- [x] **T16.2** Run clippy with warnings denied.
- [x] **T16.3** Build documentation with warnings denied.
- [x] **T16.4** Build the crate on Rust 1.85 (build only; no test run).
- [x] **T16.5** Build the feature matrix (default and each provider feature).
- [x] **T16.6** Run the governance validator and manual checks (no test run; `/sdrive:test` owns tests).

## Handoff

Every acceptance criterion (AC-001 to AC-082) is linked to at least one task above and to a requirement in `traceability.json`. The Test agent fills in the `TEST-###` IDs and writes the tests.

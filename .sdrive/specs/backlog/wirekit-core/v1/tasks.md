---
title: wirekit core - execution tasks
status: backlog
version: v1
created: 2026-10-20
updated: 2026-10-20
---

# wirekit core - execution tasks

Production-code tasks only. Writing and running tests belongs to `/sdrive:test` (CON-404, No Test Tasks Rule). Verification is limited to the formatter, linter, doc build, MSRV build, feature-matrix build, the governance validator, and manual checks.

## P1 - Crate scaffold and features

- **T1.1** Create the wirekit crate and workspace membership - `crates/wirekit` with a `src/` tree; confirm `members = ["crates/*"]` picks it up; inherit `edition`, `rust-version`, `license`.
- **T1.2** Define features, metadata, and docs.rs configuration - version 0.1.0, `default = []`, `rustls-tls` default with a `native-tls` alternative, additive provider features, `package.metadata.docs.rs` all-features.
- **T1.3** Add crate-level lints and feature documentation - `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, module declarations, feature table, I/O boundary.
- **T1.4** Add the feature-matrix and MSRV CI builds - extend `.github/workflows/ci.yml`; no test files.

## P2 - Error model

- **T2.1** Define `WirekitError` and its stable codes - seven variants, `#[non_exhaustive]`, `W####` codes.
- **T2.2** Implement `Display`, `Error`, and `source` with redacted fields.
- **T2.3** Add status and body mapping helpers - into `WirekitError::Api`; no provider-specific error type.
- **T2.4** Verify additive extensibility of the error enum.

## P3 - Shared client and configuration

- **T3.1** Implement the client builder and shared `reqwest` client.
- **T3.2** Support caller-injected clients - used verbatim, no second pool.
- **T3.3** Configure timeouts, TLS, proxy, headers, and pool limits.
- **T3.4** Reject non-HTTPS base URLs - `InvalidConfig` before any socket.

## P4 - Retry policy

- **T4.1** Define `RetryPolicy` and its defaults - 3 attempts, base/max delay, jitter bounds.
- **T4.2** Implement the retry loop with jittered exponential backoff - async timer, no blocking.
- **T4.3** Honor the `Retry-After` header - numeric and HTTP-date forms, safe fallback.
- **T4.4** Implement retry classification - 5xx/429/network retryable; other 4xx not.

## P5 - Rate limiting

- **T5.1** Implement the per-provider token bucket.
- **T5.2** Add per-provider overrides.
- **T5.3** Implement non-blocking async acquire with tracing.

## P6 - Auth layer

- **T6.1** Define the `AuthScheme` trait and contract - object-safe async trait, applied on every request.
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
- **T9.2** Implement pipeline ordering - auth, rate limit, retry, send, decode.
- **T9.3** Decode bodies and map decode failures - `WirekitError::Decode`, never a panic.
- **T9.4** Implement the body-level error hook - CON-213 (`ok: false`, GraphQL `errors[]`).
- **T9.5** Bound response size and validate untrusted input.

## P10 - Security baseline controls

- **T10.1** CON-511: enforce authentication and fail closed.
- **T10.2** CON-514: enforce rate limiting before every request.
- **T10.3** CON-515: validate untrusted input.
- **T10.4** CON-519: source credentials from the caller only.
- **T10.5** CON-520: enforce HTTPS.

## P11 - Verification

- **T11.1** Run the formatter check.
- **T11.2** Run clippy with warnings denied.
- **T11.3** Build documentation with warnings denied.
- **T11.4** Build the crate on Rust 1.85 (build only; no test run).
- **T11.5** Build the feature matrix (default and each provider feature).
- **T11.6** Run the governance validator and manual checks (no test run; `/sdrive:test` owns tests).

## Handoff

Every acceptance criterion (AC-001 to AC-042) is linked to at least one task above and to a requirement in `traceability.json`. The Test agent fills in the `TEST-###` IDs and writes the tests.

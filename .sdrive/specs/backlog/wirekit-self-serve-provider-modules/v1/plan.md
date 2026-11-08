---
title: wirekit self-serve provider modules plan
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# wirekit self-serve provider modules plan

## Approach

Add six modules to the existing `wirekit` crate, each behind its own additive
feature, each implementing only the surface in its catalogue design file. All six
reuse the crate's existing machinery: `reqwest` client, `WirekitError`, retry with
`Retry-After`, per-provider token bucket, auth schemes, tracing, and secret
redaction. The default feature set stays empty. Each module is one `src/<provider>.rs`
plus its feature line, its `#[cfg]` module declaration, and a README row.

## Components

| Component | Action | Notes |
|---|---|---|
| `crates/wirekit/Cargo.toml` | modify | Six additive features: `accuweather`, `lob`, `pagerduty`, `deepgram`, `yelp`, `klarna`. |
| `crates/wirekit/src/lib.rs` | modify | Gate six modules; document them in the crate docs and feature table. |
| `crates/wirekit/src/accuweather.rs` | create | Forecast, conditions, and location lookup per `catalogue/08_accuweather.md`. |
| `crates/wirekit/src/lob.rs` | create | Letters, postcards, and verification per `catalogue/18_lob.md`. |
| `crates/wirekit/src/pagerduty.rs` | create | Incidents, alerts, and on-calls per `catalogue/22_pagerduty.md`, including `error.message` body errors. |
| `crates/wirekit/src/deepgram.rs` | create | Transcription and project calls per `catalogue/24_deepgram.md`. |
| `crates/wirekit/src/yelp.rs` | create | Business search and reviews per `catalogue/19_yelp.md`; leads only if the catalogue marks it available. |
| `crates/wirekit/src/klarna.rs` | create | Session and order calls per `catalogue/25_klarna.md`. |
| `crates/wirekit/README.md` | modify | Provider table, feature list, and the partner-gated note. |

## Data model

No new persisted entities. Each module carries the request and response structs
named in its catalogue file; all values are in-memory `serde` types. The shared
client, error, retry, rate-limit, and auth types are reused unchanged.

**Flow.** `Client::builder()` → module constructor takes the client and the
provider's credential → typed call → shared pipeline applies auth, rate limit,
retry, tracing → response decoded into a typed value or mapped into
`WirekitError`.

## Security considerations

- **CON-207** — every failure maps into `WirekitError`; no module defines a second error type.
- **CON-208/209** — retry and rate limiting are the shared policy; `Retry-After` is honoured and non-429 4xx is never retried.
- **CON-213** — a 200 carrying an error envelope is an error (PagerDuty, Yelp).
- **CON-214** — provider secrets render only through the redacting `Secret` type.
- **CON-520** — base URLs are HTTPS constants; a non-HTTPS URL is rejected.

## Performance considerations

- **CON-210** — one pooled client per process, shared by all modules.
- **NFR-004** — every outbound call emits the shared tracing spans, so latency per provider is observable without per-module instrumentation.

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `reqwest`, `serde`, `serde_json`, `hmac`, `sha2`, `sha1`, `base64`, `hex` | library | no_change | Already crate dependencies; the six modules need no new library. |

## Risks

- **RISK-001** — Yelp leads may be partner-only; if the catalogue marks it restricted, that surface is withdrawn with a citation rather than faked.
- **RISK-002** — six modules in one feature is a large review unit; each module is reviewed against its catalogue file and can be withdrawn independently.
- **RISK-003** — sandbox access may be missing; acceptance tests use recorded fixtures and the report names providers not exercised live.

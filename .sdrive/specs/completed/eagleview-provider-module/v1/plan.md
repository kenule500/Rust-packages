---
title: EagleView provider module — technical plan
status: backlog
version: v1
created: 2026-11-01
updated: 2026-11-01
---

# Technical Plan: EagleView provider module

## 1. Approach

One feature-flagged module, `crates/wirekit/src/eagleview.rs`, behind a new
`eagleview` feature, following the existing provider modules. It owns three
things the synchronous providers do not: client-credentials token acquisition
with caching, an **idempotent paid order** (`order_measurement` refuses without
an explicit cost acknowledgement and returns the existing order for a repeated
key), and an asynchronous lifecycle (`order_status` then `fetch_report`).
Transport, retry, rate limiting, tracing, and `Secret` handling are reused
unchanged; no new dependency.

The provider call sequence is order → status → report → document. Nothing in v1
blocks waiting for a report: the caller polls `order_status` and, once complete,
calls `fetch_report`. `download_pdf` returns bounded bytes after validating the
content type. Report listing is address-scoped and limit-bounded.

## 2. Components

| Component | Action | Notes |
|---|---|---|
| `crates/wirekit/src/eagleview.rs` | create | Client, token cache, order lifecycle, report and PDF, typed errors |
| `crates/wirekit/Cargo.toml` | modify | `eagleview = []` feature, additive and default-off |
| `crates/wirekit/src/lib.rs` | modify | `pub mod eagleview` behind `#[cfg(feature = "eagleview")]` |
| `crates/wirekit/src/client.rs` | reuse | pooled transport, TLS enforcement, base URL |
| `crates/wirekit/src/auth.rs` | reuse | bearer header application after the token call |
| `crates/wirekit/src/error.rs` | reuse | `WirekitError::Api` / `Auth` / `Decode` / `InvalidConfig` |
| `crates/wirekit/src/retry.rs` | reuse | 429/5xx with `Retry-After` |
| `crates/wirekit/src/rate_limit.rs` | reuse | token bucket |
| `crates/wirekit/src/secret.rs` | reuse | client secret and access token |
| `crates/wirekit/src/trace.rs` | reuse | request spans |
| `crates/wirekit/README.md` | modify | provider row and example |
| `crates/wirekit/CHANGELOG.md` | modify | unreleased entry |

Referenced requirements: FR-001 … FR-013, NFR-001 … NFR-005.

## 3. Data model

| Entity | Action | Fields |
|---|---|---|
| `EagleViewConfig` | create | `client_id`, `client_secret: Secret<String>` |
| `AccessToken` | create | `access_token: Secret<String>`, `expires_at: i64` |
| `MeasurementRequest` | create | `address_line`, `city`, `state`, `postal_code`, `product`, `options` |
| `MeasurementOrder` | create | `id`, `status: MeasurementStatus`, `cost_acknowledged: bool` |
| `MeasurementStatus` | create | `Accepted`, `Processing`, `Complete`, `Failed { reason }` |
| `MeasurementReport` | create | `order_id`, `total_area: RoofArea`, `pitch: RoofPitch`, `segments: Vec<RoofSegment>`, `document_url: Option<String>` |
| `RoofSegment` | create | `kind: SegmentKind`, `area_squares: f64`, `pitch: RoofPitch` |
| `RoofPitch` | create | `ratio: String`, `rise_over_run: f64` |
| `RoofArea` | create | `squares: f64`, `facets: u32`, `waste_factor: f64` |
| `OrderError` | create | `Rejected`, `NotMeasurable`, `ProcessingFailed` — mapped onto `WirekitError::Api` codes |

Entity names are provider schema names; none of them is an application-domain
noun, so `wirekit`'s hygiene criterion (AC-040) holds.

## 4. API contracts

| Method | Path | Purpose |
|---|---|---|
| POST | `/oauth2/token` | client-credentials token exchange, `grant_type=client_credentials` |
| POST | `/v1/orders` | create a measurement order, carrying the caller idempotency key |
| GET | `/v1/orders/{order-id}` | read the order status |
| GET | `/v1/orders/{order-id}/report` | fetch the completed report |
| GET | `/v1/orders/{order-id}/report/document` | download the PDF |
| GET | `/v1/reports` | list reports for an address, bounded by `limit` |

All calls are authenticated with the cached bearer token and use HTTPS only.

## 5. Security considerations

- Client secret and access token live in `Secret`; `Debug` never renders them (NFR-001).
- A non-loopback cleartext base URL is refused at construction (NFR-003, CON-520).
- The PDF body is bounded before it is returned, and its content type is validated (NFR-002).
- No environment reads: the caller supplies credentials (CON-519).
- A rejected order is never retried; a repeated idempotency key with a different address is refused (FR-004, EC-004).

## 6. Dependencies

| Name | Type | Action | Notes |
|---|---|---|---|
| `reqwest` | library | no_change | already compiled in with rustls-tls |
| `serde` / `serde_json` | library | no_change | payloads and typed entities |
| `base64` | library | no_change | client-credentials header encoding |

## 7. Risks

- **RISK-001** commercial access unconfirmed → build against a local mock; keep `[to be confirmed]`.
- **RISK-002** no status webhook assumed → poll with bounded backoff; webhooks only if the contract confirms them.
- **RISK-003** cost per report unknown → ordering requires an explicit cost acknowledgement.
- **RISK-004** coverage gaps → typed unmeasurable error rather than a generic failure.

## 8. Testing approach

Mock the whole lifecycle: accepted, processing, complete, failed; an
unmeasurable address; a repeated idempotency key (assert one order only); a
missing document; a 429 with `Retry-After`; and the cleartext-base refusal.
The suite lives in `crates/wirekit/tests/eagleview-provider-module/main.rs` with
one test per acceptance criterion, gated on the `eagleview` feature.

## Plan correction (2026-11-01, apply phase)

Recorded during implementation; no behaviour or acceptance criterion changed.

- **`MeasurementStatus` variants.** The four variants (`Accepted`, `Processing`,
  `Complete`, `Failed`) are implemented as an enum with `SCREAMING_SNAKE_CASE`
  serde names, but the governance model records entity *fields* as `name: type`,
  and an enum variant has no type position, so they are documented here instead
  of as pseudo-fields.
- **API contracts.** All six routes are recorded as withdrawn/superseded: the
  client builds each call from a path-template constant (`TOKEN_PATH`,
  `ORDERS_PATH`, `ORDER_STATUS_PATH`, `REPORT_PATH`, `DOCUMENT_PATH`,
  `REPORTS_PATH`) with the order id substituted at call time, so the route never
  appears as a literal beside a request builder.

`sdrive verify` after the correction: 50 matched, 0 missing, 6 withdrawn, status
`warn`.


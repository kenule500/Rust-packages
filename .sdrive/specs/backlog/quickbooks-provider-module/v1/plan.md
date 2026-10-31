---
title: QuickBooks provider module — technical plan
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Technical Plan: QuickBooks provider module

## 1. Approach

One feature-flagged module, `crates/wirekit/src/quickbooks.rs`, behind a new `quickbooks` feature, following the existing provider modules. It owns three things the other providers do not: an OAuth2 token manager (refresh before expiry, refresh once on 401, rotated refresh token surfaced), realm-scoped routing (`/v3/company/{realmId}`), and Intuit `Fault` mapping. Transport, retry, rate limiting, tracing, and `Secret` handling are reused unchanged; no new dependency.

## 2. Components

| Component | Action | Notes |
|---|---|---|
| `crates/wirekit/src/quickbooks.rs` | create | Client, token manager, entities, webhook verify, fault mapping |
| `crates/wirekit/Cargo.toml` | modify | `quickbooks = []` feature, additive and default-off |
| `crates/wirekit/src/lib.rs` | modify | `pub mod quickbooks` behind `#[cfg(feature = "quickbooks")]` |
| `wirekit::client` | reuse | pooled transport, TLS enforcement |
| `wirekit::auth` | reuse | token header application after refresh |
| `wirekit::error` | reuse | `WirekitError::Api` / `Auth` / `Decode` |
| `wirekit::retry` | reuse | 429/5xx with `Retry-After` |
| `wirekit::rate_limit` | reuse | per-realm token bucket |
| `wirekit::secret` | reuse | credentials, tokens, verifier token |
| `wirekit::trace` | reuse | request spans |
| `crates/wirekit/README.md` | modify | provider row and example |
| `crates/wirekit/CHANGELOG.md` | modify | unreleased entry |

Referenced requirements: FR-001 … FR-012, NFR-001 … NFR-006.

## 3. Data model

| Entity | Action | Fields |
|---|---|---|
| `QuickBooksConfig` | create | `client_id`, `client_secret: Secret`, `refresh_token: Secret`, `realm_id`, `verifier_token: Secret`, `minor_version` |
| `TokenSet` | create | `access_token: Secret`, `refresh_token: Secret`, `expires_at: i64` |
| `Customer` | create | `id`, `sync_token`, `display_name`, `primary_email`, `primary_phone` |
| `Invoice` | create | `id`, `sync_token`, `customer_ref`, `lines: Vec<InvoiceLine>`, `balance`, `status` |
| `InvoiceLine` | create | `amount`, `description`, `item_ref` |
| `Payment` | create | `id`, `sync_token`, `customer_ref`, `invoice_ref`, `amount` |
| `Account` | create | `id`, `name`, `account_type`, `current_balance` |
| `Item` | create | `id`, `name`, `type`, `unit_price` |
| `IntuitFault` | create | `errors: Vec<IntuitError { code, message, detail }>` |
| `WebhookNotification` | create | `event_id`, `realm_id`, `entities: Vec<ChangedEntity>` |

Flow: caller builds `QuickBooksConfig`, the module exchanges/refreshes tokens, then entity calls post to `/v3/company/{realmId}/<resource>?minorversion=...`. Failures parse `Fault.Error[]` into `WirekitError::Api`. Inbound, raw bytes plus `intuit-signature` are verified and parsed into `WebhookNotification`.

## 4. API contracts

| Method | Path | Action | Auth | Requirements |
|---|---|---|---|---|
| POST | `/oauth2/v1/tokens/bearer` | create | yes (basic) | FR-004 |
| POST | `/v3/company/{realmId}/customer` | create | yes | FR-005 |
| POST | `/v3/company/{realmId}/invoice` | create | yes | FR-006 |
| POST | `/v3/company/{realmId}/invoice?operation=void` | create | yes | FR-006 |
| POST | `/v3/company/{realmId}/invoice/{id}/send` | create | yes | FR-006 |
| POST | `/v3/company/{realmId}/payment` | create | yes | FR-007 |
| GET | `/v3/company/{realmId}/query` | create | yes | FR-008 |

## 5. Security considerations

- **NFR-001 / CON-511, CON-519:** client secret, access token, refresh token, and verifier token are `Secret`; `Debug` redacts.
- **NFR-002 / CON-512:** realm id is fixed in the config; no call parameter may override it.
- **NFR-003 / CON-515:** responses and webhook bodies are size-bounded and typed.
- **NFR-004 / CON-520:** base URL must be HTTPS.
- **FR-009:** HMAC comparison is constant time and fails closed.

## 6. Performance considerations

- One token refresh at most per request; expiry checked before the call.
- Query endpoints take a bounded result size, so listings cannot return unbounded pages.
- No extra JSON pass: entities deserialize directly into typed structs.

## 7. Dependencies

| Name | Type | Action | Notes |
|---|---|---|---|
| `reqwest` | library | no_change | rustls-tls already present |
| `sha2` | library | no_change | HMAC-SHA256 |
| `hmac` | library | no_change | constant-time comparison |
| `serde_json` | library | no_change | entities and Fault |
| `base64` | library | no_change | signature/credential encoding |

## 8. Risks

- **RISK-001** Intuit access unconfirmed → verify against a local mock; keep `[to be confirmed]`.
- **RISK-002** refresh rotation lossy → NFR-005 and an explicit `TokenSet` result.
- **RISK-003** schema drift by `minorversion` → one constant, asserted in tests.

---
title: Meta Graph provider module — technical plan
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Technical Plan: Meta Graph provider module

## 1. Approach

Add one feature-flagged module, `crates/wirekit/src/meta.rs`, behind a new `meta` Cargo feature. It follows the shape of the existing provider modules (`twilio.rs`, `slack.rs`, `sendgrid.rs`): a small client struct holding `Secret<T>` credentials and an `Arc<WirekitClient>`, typed request/response structs, and `WirekitError` for every failure. Transport, retry, rate limiting, tracing, and secret handling are reused unchanged — this feature adds no new dependency and no new runtime.

The three Meta surfaces share one request type (`SendMessageRequest`) and differ only in host and path, so the module keeps one builder per surface and one internal `send` that posts through the existing pipeline.

## 2. Components

| Component | Action | Notes |
|---|---|---|
| `crates/wirekit/src/meta.rs` | create | Module: client, three surfaces, request/attachment types, webhook verify + normalise, error mapping |
| `crates/wirekit/Cargo.toml` | modify | `meta = []` feature (default-off, additive); no new dependencies |
| `crates/wirekit/src/lib.rs` | modify | `pub mod meta` behind `#[cfg(feature = "meta")]` |
| `wirekit::client` | reuse | `WirekitClient`, pooled transport, base URL and TLS enforcement |
| `wirekit::error` | reuse | `WirekitError::Api` / `Auth` / `Decode` mapping |
| `wirekit::retry` | reuse | 429/5xx backoff policy |
| `wirekit::rate_limit` | reuse | per-provider token bucket |
| `wirekit::secret` | reuse | `Secret<T>` redaction for tokens and the app secret |
| `wirekit::auth` | reuse | bearer/token header application |
| `wirekit::trace` | reuse | request spans |
| `crates/wirekit/README.md` | modify | provider table and a `meta` example |
| `crates/wirekit/CHANGELOG.md` | modify | unreleased entry for the module |

Referenced requirements: FR-001 … FR-011, NFR-001 … NFR-005.

## 3. Data model

| Entity | Action | Fields |
|---|---|---|
| `MetaClient` | create | `app_secret: Secret<String>`, `access_token: Secret<String>`, `page_id: Option<String>`, `phone_number_id: Option<String>`, `api_version: String`, `http: Arc<WirekitClient>` |
| `SendMessageRequest` | create | `recipient: String`, `text: Option<String>`, `attachment: Option<MessageAttachment>`, `template: Option<WhatsAppTemplate>`, `messaging_type: String` |
| `MessageAttachment` | create | `kind: AttachmentKind` (image/video/file/audio), `url: Option<String>`, `attachment_id: Option<String>` |
| `WhatsAppTemplate` | create | `name: String`, `language: String`, `parameters: Vec<String>` |
| `MetaEvent` | create | `surface: MetaSurface`, `dedupe_key: String`, `sender_id: String`, `timestamp: i64`, `kind: EventKind` |
| `GraphError` | create | `message: String`, `code: i64`, `error_subcode: Option<i64>`, `fbtrace_id: Option<String>` |

Flow: caller builds `MetaClient` with credentials, picks a surface, builds `SendMessageRequest`, and the module posts through `WirekitClient`; on failure the nested Graph error is flattened into `WirekitError::Api`. Inbound, the caller passes raw bytes plus the signature header; the module verifies, parses, and returns `MetaEvent`.

## 4. Provider contracts (outbound Graph calls)

| Method | Path | Action | Auth | Requirements |
|---|---|---|---|---|
| POST | `/{page-id}/messages` | create | yes | FR-003, FR-004 |
| POST | `/{ig-user-id}/messages` | create | yes | FR-003, FR-004 |
| POST | `/{phone-number-id}/messages` | create | yes | FR-005 |
| POST | `/{phone-number-id}/media` | create | yes | FR-006 |
| POST | `/{page-id}/subscribed_apps` | create | yes | FR-010 |
| DELETE | `/{page-id}/subscribed_apps` | create | yes | FR-010 |

## 5. Security considerations

- **NFR-001 / CON-511, CON-519:** tokens and the app secret live in `Secret<T>`; `Debug` renders `***`.
- **NFR-002 / CON-512:** the page or phone-number identity comes from configuration, never from the message body, so a caller cannot address another page.
- **NFR-003 / CON-515:** inbound bodies are size-bounded and typed; unknown events become an explicit unsupported variant.
- **NFR-004 / CON-520:** base URLs are validated as HTTPS when the client is built.
- **CON-514:** `X-Business-Use-Case-Usage` is parsed and surfaced; retries stay inside the existing policy.

## 6. Performance considerations

- No new allocation-heavy path: one JSON serialisation per send, one HMAC over the raw body.
- The token bucket and pooled client already bound concurrency; the module adds no queue.

## 7. Dependencies

| Name | Type | Action | Notes |
|---|---|---|---|
| `reqwest` | library | no_change | rustls-tls already compiled in |
| `sha2` | library | no_change | HMAC-SHA256 |
| `hmac` | library | no_change | constant-time comparison |
| `serde_json` | library | no_change | request and webhook payloads |
| `wirekit::client` | internal | no_change | existing pipeline reused |

## 8. Risks

- **RISK-001** Meta app review unconfirmed → verify against a local mock, as `m5-gated-integrations` did.
- **RISK-002** Graph version churn → single `API_VERSION` constant.
- **RISK-003** Instagram/Messenger host differences → one request type, two hosts, asserted per surface.

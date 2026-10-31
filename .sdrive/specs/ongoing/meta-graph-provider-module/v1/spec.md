---
title: Meta Graph provider module
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Meta Graph provider module

## Overview

- **Problem Statement**: Developers who need Meta messaging must hand-roll Graph API calls for three different surfaces - Facebook Messenger, Instagram DM, and WhatsApp Business - each with its own token model, message payload, media upload path, webhook signature, and rate-limit signalling. `wirekit` ships NOAA, Twilio, SendGrid, Slack, Angi, Google LSA, Sage Intacct, ADP, and Google OAuth today, but no Meta module, so every consumer re-implements the same error mapping, retry decisions, and HMAC verification.
- **Proposed Solution**: Add a `meta` feature-flagged module to `wirekit` exposing `MetaClient`, `FacebookMessenger`, `InstagramDm`, `WhatsAppBusiness`, `SendMessageRequest`, `MessageAttachment`, and `WebhookPayload`, following the `catalogue/05_meta.md` surface. The module maps nested Graph errors, honours `X-Business-Use-Case-Usage` backoff, verifies `X-Hub-Signature-256` in constant time, echoes `hub.challenge` on subscription, and normalises inbound payloads. It owns transport, not conversation state.
- **Target Audience**: Rust developers integrating Meta messaging into an application, including the GIA receptionist, inbox, and social-media agents.
- **Assumptions**:
  - Graph API v18.0 remains current for the three surfaces; the catalogue names `https://graph.facebook.com/v18.0` and `https://graph.instagram.com`.
  - Callers supply an app secret, page access token, and (for WhatsApp) the phone-number ID; this crate never reads the environment (CON-519).
  - Meta app review, page-token permissions (`pages_messaging`, `instagram_manage_messages`, `whatsapp_business_messaging`), and WhatsApp Business verification are `[to be confirmed]` - the spec assumes the caller already holds approved credentials.
  - The existing `WirekitClient`, retry policy, token bucket, `Secret<T>`, and `WirekitError` are reused unchanged.
  - No external research was performed for this draft beyond the repository's own catalogue and constitution (CON-704).

## User Stories

### US-001: Send a Messenger message
As a developer, I want to send text or media to a Facebook Page-scoped recipient, so that an application can reply where the customer wrote.

**Acceptance Criteria:**
- AC-001: Given the `meta` feature is enabled and a `MetaClient` with a page token, when `FacebookMessenger::send_text` is called with a recipient PSID and text, then a `POST` is made to `/{page-id}/messages` with `messaging_type` and `recipient.id` in the body and the bearer token in the header.
- AC-002: Given a `SendMessageRequest` that carries an image attachment, when it is sent, then the request body contains an `attachment` payload of type `image` and no plain-text field.

### US-002: Send an Instagram DM
As a developer, I want the same send model for Instagram, so that one client covers both social channels.

**Acceptance Criteria:**
- AC-003: Given an Instagram-scoped token, when `InstagramDm::send_text` is called, then the request targets the Instagram Graph host from the catalogue and uses the same `SendMessageRequest` shape as Messenger.

### US-003: Send a WhatsApp message or template
As a developer, I want text and template sends on WhatsApp Cloud, so that business-initiated messages comply with Meta's template rules.

**Acceptance Criteria:**
- AC-004: Given a phone-number ID and a template name with ordered parameters, when the WhatsApp send is called, then the body carries `type: "template"` with the template name, language, and parameters in order.
- AC-005: Given a plain WhatsApp text message inside an open 24-hour window, when it is sent, then the body carries `type: "text"` and the recipient `wa_id`.

### US-004: Verify and normalise an inbound webhook
As a developer, I want to verify Meta's signature and read a normalised event, so that only authentic payloads reach my application.

**Acceptance Criteria:**
- AC-006: Given a raw body, the app secret, and a correct `X-Hub-Signature-256` header, when `WebhookPayload::verify_and_parse` is called, then the HMAC-SHA256 comparison uses constant time and returns the normalised event.
- AC-007: Given an incorrect or missing signature, when verification runs, then it fails with an authentication error and no payload is parsed.
- AC-008: Given a subscription handshake `GET` with `hub.mode=subscribe` and a challenge, when the helper is called, then the challenge is echoed verbatim and a mismatched `hub.verify_token` is rejected.
- AC-009: Given an inbound event, when it is normalised, then it carries the message id (`mid` or the WhatsApp `id`) as the dedupe key and the surface that produced it.

## Requirements

### Functional Requirements
- FR-001: The crate MUST expose a `meta` feature that is additive and default-off, and enabling it MUST NOT enable any other provider feature (CON-104, AC-001).
- FR-002: `MetaClient` MUST accept caller-supplied app secret and access tokens, MUST reject a non-HTTPS base URL, and MUST NOT read credentials from the environment (CON-515, CON-519, CON-520, AC-001, AC-013).
- FR-003: The module MUST provide `MetaClient`, `FacebookMessenger`, `InstagramDm`, `WhatsAppBusiness`, `SendMessageRequest`, `MessageAttachment`, and `WebhookPayload` as named in `catalogue/05_meta.md` (AC-001, AC-003).
- FR-004: Messenger and Instagram sends MUST reuse one `SendMessageRequest` covering text and media (AC-002, AC-003).
- FR-005: WhatsApp sends MUST support `text` and `template` message types with ordered parameters (AC-004, AC-005).
- FR-006: Media upload MUST return an attachment identifier that a send can reference (AC-002).
- FR-007: Nested Graph errors (`error.message`, `error.code`, `error.error_subcode`, `error.fbtrace_id`) MUST map to `WirekitError::Api`, preserving the code, subcode, and trace id (AC-010).
- FR-008: 429 and 5xx responses MUST flow through the existing retry policy, and `X-Business-Use-Case-Usage` MUST be surfaced so callers can back off before the limit (CON-514, AC-011).
- FR-009: Webhook verification MUST use HMAC-SHA256 over the raw bytes with a constant-time comparison and MUST fail closed (AC-006, AC-007).
- FR-010: The subscription handshake MUST echo `hub.challenge` and reject a mismatched `hub.verify_token` (AC-008).
- FR-011: Normalised inbound events MUST expose the dedupe key and the originating surface (AC-009).

### US-005: Fail safely and predictably
As a developer, I want Graph failures, rate limits, and credentials handled consistently, so that the module never hides a provider error or leaks a token.

**Acceptance Criteria:**
- AC-010: Given a Graph error carrying `code`, `error_subcode`, and `fbtrace_id`, when a send fails, then the error is `WirekitError::Api` and exposes the code, subcode, and trace id.
- AC-011: Given a 429 response with `X-Business-Use-Case-Usage`, when the request is retried, then the retry policy backs off and the usage value is surfaced to the caller.
- AC-012: Given a `MetaClient` holding a page token and an app secret, when it is formatted with `Debug`, then neither secret appears in the output.
- AC-013: Given a cleartext `http://` base URL, when the client is built, then construction fails with a validation error.

### Non-Functional Requirements
- NFR-001: Credentials MUST come from caller-supplied configuration or a secret store and MUST render redacted in `Debug` (CON-511, CON-519, AC-012).
- NFR-002: Every send MUST be scoped by the page or phone-number identity carried in the token; the module MUST NOT accept a caller-supplied identity that contradicts the token scope (CON-512, AC-001).
- NFR-003: Provider responses and webhook bodies MUST be validated before use, with bounded sizes and typed errors (CON-515, AC-007).
- NFR-004: All traffic MUST use TLS and MUST reject a cleartext base URL (CON-520, AC-013).
- NFR-005: The module MUST compile under `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` like the rest of the crate (CON-201, CON-202).

### Security Baseline
| Control | Status | Decision for this feature |
|---|---|---|
| CON-511 Authentication | REQUIRED | App secret and page/WhatsApp tokens are caller-supplied and held in `Secret<T>`; no refresh flow is added here. |
| CON-512 Authorization | REQUIRED | Sends are page/phone-number scoped; tenant isolation stays in the caller (`platform-core`). |
| CON-513 Security Headers | NOT APPLICABLE | The crate is an HTTP client and renders no responses. |
| CON-514 Rate Limiting | REQUIRED | The existing token bucket applies, plus `X-Business-Use-Case-Usage` surfacing. |
| CON-515 Input Validation | REQUIRED | Typed request shapes and bounded parsing of inbound payloads. |
| CON-516 Injection Protection | NOT APPLICABLE | This module adds no SQL or Redis backend. |
| CON-517 XSS / CON-518 CSRF | NOT APPLICABLE | No markup is rendered and there is no browser session. |
| CON-519 Environment Security | REQUIRED | No `.env` or process-environment reads; placeholders only in docs. |
| CON-520 HTTPS / HSTS | REQUIRED | Non-HTTPS base URLs are rejected in the builder. |

## Edge Cases & Error States
- EC-001: Usage header close to the limit -> the caller sees the usage value and the retry policy backs off instead of hot-looping.
- EC-002: Webhook body above the configured ceiling -> rejected with a bounds error before parsing.
- EC-003: Unknown event type or surface -> parsed into a typed "unsupported" variant rather than dropped silently.
- EC-004: WhatsApp template parameter count mismatch -> rejected locally, before any network call.

## Out of Scope
- Conversation state, inbox routing, and agent dispatch.
- Facebook Login, feed publishing, ads, and Pages management.
- WhatsApp template submission or approval workflows.
- Token acquisition or refresh (handled by the caller or `platform-core`).

## Dependencies & Risks
- **Dependencies**: `reqwest`/`rustls`, `sha2`, `hmac`, `serde`, and `serde_json` are already compiled into the crate. No new dependency is planned.
- **RISK-001**: Meta app review and page-token permissions are unconfirmed; without them live calls fail with permission errors. Mitigation: build against the documented shapes and cover behaviour with a local mock, as `m5-gated-integrations` did.
- **RISK-002**: Graph API version churn changes paths and payloads. Mitigation: one documented version constant, asserted per surface in tests.
- **RISK-003**: Instagram and Messenger differ in host and token scope despite a shared payload shape. Mitigation: one request type, two hosts, asserted per surface.



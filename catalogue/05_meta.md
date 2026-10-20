# 05 — Meta Graph

**Provider:** Meta Graph
**Package:** `wirekit` (module `wirekit::meta`)
**Status:** provider design, not a crate
**Covers:** Facebook Messenger, Instagram DM, WhatsApp Business
**Base URLs:** `https://graph.facebook.com/v18.0`, `https://graph.instagram.com`
**Auth:** App access token + Page access token; system user token for multi-tenant

## What it does

Meta is the social and messaging channel for homeowners who do not use SMS.
It covers Facebook Messenger, Instagram DMs, and WhatsApp Business. For GIA,
it is both an inbound channel (Agent 6) and an outbound one (Agent 13/14).

The module wraps the Graph API's messaging endpoints, media uploads, and
webhook payloads. It does not own the conversation state; it delivers messages
and normalizes events for the conversation service.

## GIA use

- Tool 2 (Omnichannel Messaging)
- Agents 1 (Receptionist), 6 (Inbox), 14 (Social Media Manager)
- Campaign delivery for Facebook/Instagram where consent allows
- WhatsApp for homeowners who prefer it

## Surface to build

| Type | Purpose |
|---|---|
| `MetaClient` | App/page credentials and version |
| `FacebookMessenger` | Page-scoped Messenger send/receive |
| `InstagramDm` | Instagram DM send/receive |
| `WhatsAppBusiness` | WhatsApp Cloud API send/receive |
| `SendMessageRequest` | Text, media, template |
| `MessageAttachment` | Image, video, file |
| `WebhookPayload` | Normalized inbound event |

Methods: send text, send media, send a WhatsApp template, upload media,
subscribe/unsubscribe webhooks, verify signatures.

## Wirekit contract

- **Errors:** Graph errors are nested (`error.message`, `error.code`,
  `error.error_subcode`); map to `WirekitError::Api` and preserve the trace ID.
- **Retry:** 429 and 5xx; respect `X-Business-Use-Case-Usage` headers and
  back off when approaching limits.
- **Rate limits:** per-page and per-app; token bucket; surface the usage
  header.
- **Webhooks:** verify `X-Hub-Signature-256` (HMAC-SHA256) with constant-time
  comparison; echo the `hub.challenge` on subscription.
- **Idempotency:** Meta does not provide a general key; dedupe on
  `mid`/message ID and the caller's action key.

## GIA integration

Inbound DMs route to Agent 6, which classifies intent and hands off to Agent 1
or a human. Outbound social posts and storm alerts go through Agent 14. The
same consent and opt-out rules apply; a Facebook DM is not a license to send
SMS.

## Test plan

- Meta test app and test users/pages.
- Webhook verification challenge and signature fixtures.
- Inbound text, media, quick reply, and postback payloads.
- WhatsApp template approval and rejection cases.
- Rate-limit headers and 429 retry.

## Open questions

1. **App review and business verification.** Messenger/Instagram permissions
   require review; WhatsApp requires a business account and template approval.
   This is a launch dependency.
2. **24-hour messaging window.** WhatsApp and Messenger have response-window
   rules; the agent must not attempt a free-form message outside them.
3. **Multi-tenant tokens.** Page tokens per tenant, or a system user? Decide
   the token storage and rotation model before onboarding.
4. **Instagram API version.** The Instagram Graph API and Messenger API for
   Instagram have different permissions; confirm which one is in scope.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §3.

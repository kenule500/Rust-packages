# 04 — SendGrid

**Provider:** SendGrid
**Package:** `wirekit` (module `wirekit::sendgrid`)
**Status:** provider design, not a crate
**Covers:** Transactional email, templates, contact lists
**Base URL:** `https://api.sendgrid.com/v3`
**Auth:** Bearer token (API key)

## What it does

SendGrid is GIA's email channel: invoices, estimates, receipts, report
delivery, and the fallback when SMS is undeliverable. It is also the delivery
mechanism for campaign email, though SMS and direct mail remain primary for
storm outreach.

The module wraps the v3 API with typed email, template, and contact-list
models. It does not manage deliverability; it makes the send path consistent
and observable.

## GIA use

- Tool 2 (Omnichannel Messaging) for email
- Agents 4 (Follow-Up), 9 (Reporting), 13 (Marketing Automation)
- Invoice and estimate delivery fallback when SMS fails
- Weekly/monthly report emails to the owner

## Surface to build

| Type | Purpose |
|---|---|
| `SendGridClient` | Authenticated client |
| `SendGridConfig` | API key, from address, sandbox flag |
| `SendEmailRequest` | To/from, subject, HTML/text, attachments |
| `EmailAddress` | Name + address |
| `EmailAttachment` | Base64 or URL attachment |
| `TemplateEmailRequest` | Dynamic template + substitutions |
| `ContactList` | List management |

Methods: send a single email, send with a template, add/update contacts,
fetch bounces/spam reports, verify event webhooks.

## Wirekit contract

- **Errors:** map SendGrid's `errors[]` array into `WirekitError::Api` with
  the first code/message and the full list retained in `details`.
- **Retry:** 429 and 5xx with backoff; respect `Retry-After`; do not retry
  validation errors.
- **Rate limits:** per-API-key; token bucket; surface remaining quota.
- **Webhooks:** verify the event webhook signature (ECDSA) with the public
  key; reject invalid signatures; dedupe by `sg_event_id`.
- **Idempotency:** no general idempotency key; dedupe on a caller-supplied
  message key and the returned `X-Message-Id`.

## GIA integration

Email is the fallback, not the default. If an SMS is undeliverable, Agent 4
or Agent 18 sends the same content by email and records the channel switch.
Reports use a template with workspace branding. Campaign email must respect
the same consent and unsubscribe rules as SMS.

## Test plan

- Sandbox mode (`sandbox_mode: true`) for request validation without delivery.
- Template rendering with missing/extra substitutions.
- Event webhook fixtures: delivered, bounced, dropped, spamreport, unsubscribe.
- Attachment size and MIME type limits.
- 429/5xx retry with a mock transport.

## Open questions

1. **Sender authentication.** Which domain(s) are authenticated with SPF,
   DKIM, and DMARC? A shared GIA domain will hurt deliverability.
2. **Dedicated IP.** At what volume does a dedicated IP make sense, and who
   warms it?
3. **Unsubscribe groups.** Marketing vs transactional must be separate;
   confirm the group IDs per workspace.
4. **Data residency.** SendGrid is US-based; confirm this is acceptable for
   Canadian/UK tenants before international launch.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §2.

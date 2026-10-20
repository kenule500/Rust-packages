# 18 — Lob

**Provider:** Lob
**Package:** `wirekit` (module `wirekit::lob`)
**Status:** provider design, not a crate
**Covers:** Direct mail — postcards, letters, checks
**Base URL:** `https://api.lob.com/v1`
**Auth:** Basic (API key)

## What it does

Lob is the direct-mail channel for Storm Blast. It is the TCPA-safe alternative
to cold SMS: a postcard or letter to a scraped address does not require prior
express written consent. Agent 13 uses Lob when SMS consent is absent or when
the campaign mix calls for mail.

The module wraps Lob's postcard and letter APIs with address verification,
test mode, and delivery tracking.

## GIA use

- Tool 12 (Campaign Delivery)
- Agent 13 (Marketing Automation)
- Storm Blast direct-mail fallback
- Door hangers/letters for cold prospects
- Return-receipt handling for high-value campaigns

## Surface to build

| Type | Purpose |
|---|---|
| `LobClient` | API key, test/live mode |
| `Address` | Domestic/international address |
| `PostcardRequest` | Front/back HTML, size, schedule |
| `LetterRequest` | Letter HTML, return address |
| `MailPiece` | ID, status, tracking events |

Methods: verify address, create postcard, create letter, get mail piece,
list events, cancel a scheduled piece.

## Wirekit contract

- **Auth:** Basic API key; test keys must be clearly separated from live keys.
- **Errors:** map Lob's error envelope to `WirekitError::Api`; address
  verification failures are validation errors, not retries.
- **Retry:** 429 and 5xx; do not retry an accepted mail piece — that prints
  twice and costs money.
- **Rate limits:** per-account; token bucket.
- **Webhooks:** verify the signature per Lob's docs; dedupe by event ID.
- **Idempotency:** send an idempotency key on create; reconcile after a
  timeout before retrying.

## GIA integration

Agent 13 builds a mail piece from the campaign template, verifies the address,
and submits it. The campaign recipient records the Lob ID and status. A
returned piece updates the prospect record; it does not silently retry.

## Test plan

- Lob test keys; test postcard/letter rendering.
- Address verification: deliverable, undeliverable, missing unit.
- Create and cancel a scheduled piece.
- Webhook events: mailed, in transit, delivered, returned.
- Duplicate prevention with an idempotency key.

## Open questions

1. **Cost per piece.** Postage and print are real COGS; model per Storm Blast
   before promising a price.
2. **Turnaround.** Print/mail timing varies; the campaign must not promise a
   same-week arrival.
3. **Address quality.** Scraped property addresses may be stale or incomplete;
   verification is mandatory.
4. **Attribution.** Direct mail is hard to attribute; use a unique URL/QR code
   and a promo code per campaign.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §16.

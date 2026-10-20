# 19 — Yelp

**Provider:** Yelp
**Package:** `wirekit` (module `wirekit::yelp`)
**Status:** provider design, not a crate
**Covers:** Business search, reviews, leads
**Base URL:** `https://api.yelp.com/v3`
**Auth:** Bearer token

## What it does

Yelp is a lead and reputation channel. GIA uses the Fusion API for business
search and review data, and — where the tenant has partner access — lead
webhooks for inbound requests. Yelp leads are valuable because the homeowner
is already looking for a contractor.

The module wraps business search, reviews, and lead handling. It does not
scrape Yelp; it uses the API and respects its terms.

## GIA use

- Agent 13 (Marketing Automation) — lead capture
- Agent 6 (Inbox) — route Yelp inquiries
- Agent 8 (Review & Referral) — monitor reviews
- Tenant dashboard — review volume and rating

## Surface to build

| Type | Purpose |
|---|---|
| `YelpClient` | Bearer token and locale |
| `Business` | Search result and details |
| `Review` | Review text, rating, time |
| `Lead` | Lead event, if partner access is granted |

Methods: search businesses, get business, list reviews, handle lead webhook,
verify webhook signature.

## Wirekit contract

- **Auth:** Bearer token; per-tenant or per-partner token, not a shared global
  key.
- **Errors:** map Yelp's error envelope to `WirekitError::Api`; 401 means a bad
  token, 429 means rate limit.
- **Retry:** 429 and 5xx; respect `Retry-After`.
- **Rate limits:** Fusion has a daily quota; cache business/review data and do
  not poll aggressively.
- **Webhooks:** lead webhooks may be partner-only; verify the signature if
  available, otherwise use a signed import.
- **Idempotency:** dedupe leads by Yelp lead ID; reviews by review ID.

## GIA integration

A Yelp lead webhook creates a contact, conversation, and lead, then routes to
Agent 6. If the tenant does not have lead API access, the module falls back to
manual/CSV import. Review data feeds Agent 8; GIA does not post reviews.

## Test plan

- Fusion API fixtures: search, business, reviews.
- Lead webhook fixture, if partner access exists.
- 401/429 behavior and cache TTL.
- Duplicate lead/review handling.
- Terms compliance: no review gating, no incentivized reviews.

## Open questions

1. **Lead API access.** Yelp's lead API is often partner-only. Confirm before
   promising automatic Yelp lead capture.
2. **Review display.** Yelp's terms restrict how reviews can be displayed and
   cached; confirm before showing them in GIA.
3. **Attribution.** Yelp requires attribution/branding; the UI must include it.
4. **Coverage.** Yelp is strong in urban markets and weak in rural ones; set
   expectations by service area.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §17.

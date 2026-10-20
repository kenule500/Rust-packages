# 21 — Google LSA

**Provider:** Google Local Services Ads
**Package:** `wirekit` (module `wirekit::google_lsa`)
**Status:** provider design, not a crate
**Covers:** Local Services Ads leads and bookings
**Base URL:** `https://localservices.googleapis.com`
**Auth:** OAuth2 (Google)

## What it does

Google Local Services Ads put a contractor at the top of local search with a
Google Guaranteed badge. GIA uses the LSA API to fetch leads, update lead
status, and report bookings. The homeowner is already high-intent; speed to
response matters.

The module wraps LSA lead and booking endpoints with typed models and a shared
Google OAuth2 client.

## GIA use

- Agent 13 (Marketing Automation) — lead intake
- Agent 6 (Inbox) — route the lead into a conversation
- Agent 1 (Receptionist) — call/text back
- Tenant dashboard — LSA spend and lead status

## Surface to build

| Type | Purpose |
|---|---|
| `GoogleLsaClient` | OAuth2 client, account ID |
| `Lead` | Lead type, contact, job details |
| `Booking` | Booking status and attribution |
| `LeadStatus` | New, booked, completed, etc. |

Methods: list leads, get lead, update lead status, report booking, handle
webhook/notification.

## Wirekit contract

- **Auth:** OAuth2 (Google); reuse `wirekit::google_oauth` for token refresh.
- **Errors:** Google error envelope; map to `WirekitError::Api`; 403 often
  means the Ads account is not linked or approved.
- **Retry:** 429 and 5xx; respect `Retry-After`.
- **Rate limits:** per-project; token bucket.
- **Webhooks:** LSA lead notifications may arrive via Pub/Sub or polling;
  confirm the mechanism for the account. Do not assume a simple webhook.
- **Idempotency:** dedupe by lead ID; status updates are idempotent by lead.

## GIA integration

A new LSA lead creates a contact and conversation, then triggers Agent 1 to
respond within seconds. Agent 6 classifies the job type. Status updates flow
back to Google so the contractor's profile stays accurate. Attribution feeds
the marketing dashboard.

## Test plan

- Google Ads/LSA test account; OAuth consent.
- Lead fixtures for each lead type.
- Status update and booking report.
- 403 access-not-approved and 429 behavior.
- Dedupe and attribution.

## Open questions

1. **API access.** LSA API access requires a Google Ads account and approval;
   confirm the onboarding path and timeline.
2. **Account structure.** One LSA account per location or per organization?
   Map it to the workspace model.
3. **Lead response SLA.** Google rewards fast responses; the agent must be
   able to answer 24/7 before the tenant turns on LSA.
4. **Attribution.** Confirm how LSA reports bookings and what counts as a
   qualified lead.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §19.

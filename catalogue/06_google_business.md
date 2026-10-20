# 06 — Google Business Profile

**Provider:** Google Business Profile
**Package:** `wirekit` (module `wirekit::google_business`)
**Status:** provider design, not a crate
**Covers:** Business listings, reviews, posts, insights
**Base URL:** `https://mybusinessbusinessinformation.googleapis.com`
**Auth:** OAuth2 (Google)

## What it does

Google Business Profile is where the contractor's reputation lives. GIA uses
it to read reviews, reply to them, publish posts, and keep location data
accurate. Agent 8 requests reviews; Agent 14 posts updates.

The module wraps the Business Profile APIs with typed listing, review, and
post models. It does not scrape Google; it uses the official APIs and their
approval process.

## GIA use

- Agents 8 (Review & Referral), 14 (Social Media Manager)
- Review monitoring and negative-review escalation (Agent 10)
- Local posts for storms, seasonal offers, and job showcases
- Location data for the contractor's public profile

## Surface to build

| Type | Purpose |
|---|---|
| `GoogleBusinessClient` | OAuth2 client and account/location IDs |
| `BusinessListing` | Name, address, hours, categories |
| `Review` / `ReviewReply` | Read and reply |
| `LocalPost` | Create/update posts |
| `Insights` | Views, searches, actions |

Methods: list accounts/locations, get a listing, list reviews, reply to a
review, create/update a post, fetch insights.

## Wirekit contract

- **Errors:** Google's `error.status` and `error.message`; map to
  `WirekitError::Api`, preserve `error.details` for quota/field violations.
- **Retry:** 429 and 5xx; respect `Retry-After`; quota errors are not retried
  blindly.
- **Rate limits:** per-project quotas; token bucket; surface quota usage.
- **Webhooks:** the Business Profile API does not offer a simple review
  webhook; poll reviews on a schedule or use Pub/Sub notifications where
  available. Do not promise real-time review events until verified.
- **Idempotency:** review replies are naturally idempotent by review ID; posts
  need a caller key to avoid duplicates.

## GIA integration

After payment, Agent 8 sends the review link. When a review appears, Agent 8
drafts a reply in the contractor's tone; Agent 10 escalates reviews below four
stars. Agent 14 posts storm updates and job showcases. The OAuth token belongs
to the contractor, not GIA.

## Test plan

- Google Cloud project with the Business Profile APIs enabled.
- OAuth consent and token refresh.
- Review reply: success, already replied, policy rejection.
- Post: text, media, event, offer.
- Quota exhaustion and 429 handling.

## Open questions

1. **API allowlist.** Business Profile APIs require access approval; confirm
   the project is approved before committing to a launch date.
2. **Location verification.** A location must be verified before some
   operations; onboarding must handle unverified locations.
3. **Review reply policy.** Google's content policy can reject replies; the
   agent must not argue with a reviewer or include promotional links.
4. **Multi-location.** A contractor with several locations needs per-location
   account/location IDs; confirm the mapping in the tenant model.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §4.

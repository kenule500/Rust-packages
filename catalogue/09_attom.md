# 09 — ATTOM

**Provider:** ATTOM
**Package:** `wirekit` (module `wirekit::attom`)
**Status:** provider design, not a crate
**Covers:** Property records, valuations, permits
**Base URL:** `https://api.gateway.attomdata.com`
**Auth:** API key (header)

## What it does

ATTOM is the primary property-data source for prospect scoring and lead
qualification. It answers: who owns this home, how old is it, what is it worth,
what permits exist, and has it sold recently. Agent 17 uses that to score
storm-affected homes; Agent 3 uses it to qualify inbound leads.

The module wraps ATTOM's property, sale, valuation, and permit endpoints with
typed records. It does not decide whether a lead is good; it supplies the
facts.

## GIA use

- Tool 4 (Public Property Records)
- Agents 3 (Lead Qualification), 17 (Lead Finder)
- Scoring factors: owner-occupied, roof age, property age, recent sale
- Permit history for competitor-work detection

## Surface to build

| Type | Purpose |
|---|---|
| `AttomClient` | API key and endpoint config |
| `PropertySearchRequest` | Address or APN |
| `PropertyRecord` | Characteristics, owner, lot, year built |
| `PropertyValuation` | AVM and confidence |
| `SaleHistory` | Prior sales and prices |
| `Permit` | Permit type, date, contractor |

Methods: address lookup, property detail, valuation, sale history, permit
history, expanded profile.

## Wirekit contract

- **Errors:** 401/403 auth, 404 no record, 429 quota; map to
  `WirekitError::Api` with a clear `NoRecord` variant for 404.
- **Retry:** 429 and 5xx; do not retry 404; cache no-records for a short TTL.
- **Rate limits:** plan-dependent; token bucket; surface remaining calls.
- **Webhooks:** none.
- **Idempotency:** reads are idempotent; cache by normalized address/APN with
  a TTL appropriate to the data (ownership changes slowly; permits faster).

## GIA integration

Agent 17 enriches every prospect before scoring. If ATTOM has no record, the
prospect is scored with the data that exists and flagged low-confidence — it
is not dropped. Agent 3 uses the same record for inbound leads. A missing
record must never become a silent zero score.

## Test plan

- Fixture-based tests for a full record, a partial record, and no record.
- Address normalization: unit numbers, rural routes, new construction.
- 404 vs 429 vs 401 behavior.
- Cache TTL and no-record caching.
- Field mapping into the `Property`/`ProspectScoreFactor` model.

## Open questions

1. **License and redistribution.** Can ATTOM data be shown to a homeowner or
   stored in the home record? Confirm before the Home Health Report uses it.
2. **Coverage.** ATTOM is strong in the US; confirm county coverage for target
   markets and for rural properties.
3. **Cost per lookup.** Storm Blast can enrich hundreds of homes per event;
   model the cost before pricing the campaign.
4. **Permit depth.** Permit coverage varies by jurisdiction; pair with
   BuildZoom or municipal portals where ATTOM is thin.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §7.

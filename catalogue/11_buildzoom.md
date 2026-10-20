# 11 — BuildZoom

**Provider:** BuildZoom
**Package:** `wirekit` (module `wirekit::buildzoom`)
**Status:** provider design, not a crate
**Covers:** Permit data, contractor records
**Base URL:** `https://api.buildzoom.com`
**Auth:** API key

## What it does

BuildZoom supplies permit and contractor data that ATTOM may not cover. For
GIA, the important question is: has this property had recent work, and who did
it? That informs prospect scoring (no recent permit + roof age = higher
probability) and competitor-work detection.

The module wraps permit search and contractor lookup with typed records.

## GIA use

- Tool 11 (Permit & Competitor DB)
- Agent 17 (Lead Finder) — recent permit check
- Scoring factor: no recent permit (+15)
- Competitor identification for territory intelligence

## Surface to build

| Type | Purpose |
|---|---|
| `BuildZoomClient` | API key and endpoint config |
| `PermitSearchRequest` | Address, jurisdiction, date range |
| `PermitRecord` | Type, date, status, contractor |
| `Contractor` | Name, license, location |

Methods: permit search by address, permit detail, contractor lookup, recent
activity by property.

## Wirekit contract

- **Errors:** map to `WirekitError::Api`; distinguish no-record from auth and
  quota errors.
- **Retry:** 429 and 5xx; do not retry 404.
- **Rate limits:** plan-dependent; token bucket; cache by address/jurisdiction.
- **Webhooks:** none.
- **Idempotency:** reads are idempotent; cache permit results for a short TTL
  (new permits appear over time).

## GIA integration

Agent 17 checks BuildZoom after ATTOM/CoreLogic. A recent permit lowers the
prospect score; no permit raises it. If BuildZoom has no coverage for a
jurisdiction, the factor is marked unknown, not zero. The same data feeds the
"competitor work" view in the tenant dashboard.

## Test plan

- Fixture-based tests for permit found, no permit, no coverage.
- Jurisdiction differences: city vs county, permit numbering.
- Contractor lookup and license fields.
- 404 vs 429 behavior.
- Cache TTL and stale permit handling.

## Open questions

1. **Coverage.** BuildZoom's permit coverage varies by jurisdiction; map
   coverage before using it as a national scoring factor.
2. **Access.** Confirm the API plan and terms; some permit data is
   partner-only.
3. **Attribution.** If BuildZoom data appears in a homeowner report, confirm
   the required attribution.
4. **Freshness.** Permit records can lag; do not treat "no permit" as proof
   that no work happened.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §9.

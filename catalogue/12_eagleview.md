# 12 — EagleView

**Provider:** EagleView
**Package:** `wirekit` (module `wirekit::eagleview`)
**Status:** provider design, not a crate
**Covers:** Satellite roof measurement
**Base URL:** `https://api.eagleview.com`
**Auth:** OAuth2 client credentials

## What it does

EagleView turns an address into roof measurements: area, pitch, segment
breakdown (ridges, valleys, hips, eaves), and a PDF report. Agent 19 uses it to
produce a credible preliminary estimate without sending a human to the roof.

The module wraps the order-and-report lifecycle. Measurement is asynchronous
and costs money per report; the client must make that explicit.

## GIA use

- Tool 5 (Satellite Measurement)
- Agent 19 (Estimation)
- Preliminary range for roofing and exterior jobs
- Report attachment for the homeowner and the formal quote

## Surface to build

| Type | Purpose |
|---|---|
| `EagleViewClient` | OAuth2 client and report config |
| `MeasurementRequest` | Address, product, options |
| `MeasurementReport` | Status, totals, segments, PDF |
| `RoofSegment` | Ridge, valley, hip, eave |
| `RoofPitch` | Pitch and area |
| `RoofArea` | Squares, facets, waste factor |

Methods: order measurement, get order status, fetch report, download PDF,
list reports for a property.

## Wirekit contract

- **Errors:** map to `WirekitError::Api`; distinguish order rejection,
  address not measurable, and processing failure.
- **Retry:** 429 and 5xx; do not retry a rejected order without changing it.
- **Rate limits:** contract-dependent; token bucket.
- **Webhooks:** if EagleView supports order-status callbacks, verify the
  signature; otherwise poll with backoff. Do not assume a webhook exists until
  confirmed.
- **Idempotency:** order creation needs a caller key; a duplicate order is a
  duplicate charge.

## GIA integration

Agent 19 orders a report only when the homeowner asks for a price and the
property is a roofing candidate. The report is cached on the home record and
reused for future estimates. If the report is pending, the agent gives a range
from public data and marks it low-confidence; it does not block the
conversation.

## Test plan

- Mock order lifecycle: accepted, processing, complete, failed.
- Address not measurable and multi-structure properties.
- PDF download and content-type validation.
- Duplicate-order prevention with an idempotency key.
- Cost guard: no order without a workspace budget check.

## Open questions

1. **Turnaround.** How long does a report take, and does the homeowner
   conversation wait or proceed with a range?
2. **Cost per report.** Measurement is a real COGS item; model it per estimate
   and cap orders per workspace.
3. **Coverage.** Confirm coverage for rural and new-construction addresses.
4. **Data rights.** Can the report be stored in the home record and shared
   with the homeowner? Confirm the license.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §10.

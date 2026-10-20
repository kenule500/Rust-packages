# 10 — CoreLogic

**Provider:** CoreLogic
**Package:** `wirekit` (module `wirekit::corelogic`)
**Status:** provider design, not a crate
**Covers:** Property data, valuations, ownership
**Base URL:** `https://api-prod.corelogic.com`
**Auth:** OAuth2 client credentials

## What it does

CoreLogic is the secondary property-data source and the failover for ATTOM.
It provides property characteristics, ownership, sales history, and
valuations. For GIA, redundancy matters: a storm pipeline that depends on one
data vendor stops when that vendor has an outage or a coverage gap.

The module wraps CoreLogic's property and valuation endpoints with typed
models and an OAuth2 token manager.

## GIA use

- Tool 4 (Public Property Records) — failover and cross-check
- Agents 3 (Lead Qualification), 17 (Lead Finder)
- Valuation cross-check for high-value properties
- Ownership verification before outreach

## Surface to build

| Type | Purpose |
|---|---|
| `CoreLogicClient` | OAuth2 client and token cache |
| `PropertyRequest` | Address or parcel |
| `PropertyDetails` | Characteristics and lot |
| `OwnershipInfo` | Owner name, occupancy, tenure |
| `Valuation` | Estimate and confidence |

Methods: property detail, ownership, sale history, valuation, token refresh.

## Wirekit contract

- **Auth:** OAuth2 client credentials; cache the token until expiry minus a
  safety margin; refresh on 401 once, then fail.
- **Errors:** map to `WirekitError::Api`; distinguish `NoRecord` from
  `Auth`.
- **Retry:** 429 and 5xx; do not retry 401 after a refresh; do not retry 404.
- **Rate limits:** contract-dependent; token bucket; surface quota.
- **Webhooks:** none.
- **Idempotency:** reads are idempotent; cache by address/parcel.

## GIA integration

Agent 17 tries ATTOM first; if ATTOM fails, times out, or has no record,
CoreLogic is the fallback. If both return data, a small set of fields (owner,
year built, roof age) are cross-checked; disagreement lowers confidence rather
than silently picking one. Agent 3 uses the same chain.

## Test plan

- OAuth2 token acquisition, cache, expiry, and refresh.
- Full, partial, and no-record fixtures.
- ATTOM-failover path: timeout, 5xx, 404.
- Cross-check disagreement behavior.
- 429/5xx retry and 401 refresh-once.

## Open questions

1. **Contract minimums.** CoreLogic often requires a commercial agreement;
   confirm volume commitments and per-lookup pricing.
2. **Coverage overlap.** Measure ATTOM/CoreLogic agreement in target counties;
   do not assume one is always right.
3. **Data use.** Confirm whether CoreLogic data can be stored in the home
   record and shown to homeowners.
4. **Latency.** CoreLogic may be slower than ATTOM; the fallback timeout must
   be short enough not to stall a Storm Blast.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §8.

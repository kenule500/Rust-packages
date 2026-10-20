# 07 — NOAA

**Provider:** NOAA (National Weather Service)
**Package:** `wirekit` (module `wirekit::noaa`)
**Status:** provider design, not a crate
**Covers:** Forecasts, severe weather alerts, radar metadata
**Base URL:** `https://api.weather.gov`
**Auth:** None

## What it does

NOAA is the primary weather source for Agent 16. It provides point forecasts,
active alerts, and radar metadata for a lat/long or ZIP. It is free and
authoritative for US weather, which makes it the first source in the
dual-verification pipeline.

The module wraps `api.weather.gov` with typed forecast, alert, and observation
models. It does not interpret hail size; that is a storm-service concern.

## GIA use

- Tool 10 (Weather & Event API)
- Agent 16 (Storm Detection)
- Storm Blast trigger for hail, wind, tornado, flood, and hurricane events
- Capacity checks and owner alerts

## Surface to build

| Type | Purpose |
|---|---|
| `NoaaClient` | Base client with required User-Agent |
| `ForecastRequest` | Lat/long or grid point |
| `ForecastResponse` | Periods, temperature, precipitation |
| `WeatherAlert` | Alert ID, event, severity, areas, description |
| `SevereWeatherEvent` | Normalized storm event |
| `RadarMetadata` | Radar station and timestamp |

Methods: get a point forecast, list active alerts for an area, get an alert by
ID, fetch radar metadata, fetch historical observations.

## Wirekit contract

- **Errors:** NOAA returns problem+json (`type`, `title`, `status`, `detail`);
  map to `WirekitError::Api`.
- **Retry:** 429/5xx with backoff; NOAA asks for a descriptive `User-Agent`
  and polite polling. Respect it.
- **Rate limits:** no published hard limit, but polling every 15 minutes per
  workspace is the design budget. Cache forecasts and alerts by grid/zone.
- **Webhooks:** none. Polling is the integration.
- **Idempotency:** alerts have IDs; dedupe by `alertId` + `onset`.

## GIA integration

Agent 16 polls every 15 minutes across active service areas. A qualifying
alert triggers a second source (AccuWeather or a commercial provider) before
`storm.verified`. The normalized event includes event type, severity, ZIPs,
and the raw alert. NOAA alone is not enough for hail size; pair it.

## Test plan

- Fixture-based tests for forecast, alert, and problem responses.
- A recorded alert lifecycle: issued, updated, cancelled.
- Rate-limit and 5xx retry with a mock transport.
- User-Agent validation: requests without one are rejected.
- Timezone and unit conversion (C/F, mm/inches).

## Open questions

1. **Hail size.** NOAA alerts describe severe storms but do not reliably
   report hail diameter. The storm service must combine radar, spotter
   reports, or a commercial source.
2. **Coverage.** NOAA covers the US and territories; Canada/UK need different
   providers. Do not route international tenants to NOAA.
3. **Polling budget.** 15-minute polling across many workspaces can be
   expensive in aggregate; consider a shared event bus by ZIP.
4. **Alert latency.** NWS alerts can lag the event; the pipeline must accept
   late verification and still trigger.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §5.

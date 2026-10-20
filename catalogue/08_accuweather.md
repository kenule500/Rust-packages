# 08 — AccuWeather

**Provider:** AccuWeather
**Package:** `wirekit` (module `wirekit::accuweather`)
**Status:** provider design, not a crate
**Covers:** Forecasts, severe weather, historical data
**Base URL:** `https://dataservice.accuweather.com`
**Auth:** API key (query parameter)

## What it does

AccuWeather is the second weather source for Agent 16. NOAA is authoritative
for US alerts; AccuWeather adds commercial forecasts, historical summaries,
and a second opinion before a Storm Blast launches. The dual-source rule is
what prevents a single bad alert from sending hundreds of texts.

The module wraps location lookup, forecasts, alerts, and historical daily
summaries. It does not replace NOAA; it corroborates it.

## GIA use

- Tool 10 (Weather & Event API)
- Agent 16 (Storm Detection) — second verification source
- Historical hail/wind summaries for home risk scoring
- Storm-event enrichment (wind speed, precipitation)

## Surface to build

| Type | Purpose |
|---|---|
| `AccuWeatherClient` | API key and location cache |
| `ForecastRequest` | Location key or lat/long |
| `ForecastResponse` | Daily/hourly periods |
| `WeatherAlert` | Headline, severity, areas, dates |
| `HistoricalWeather` | Daily summaries for a date range |

Methods: location search, current conditions, daily/hourly forecast, active
alerts, historical daily data.

## Wirekit contract

- **Errors:** AccuWeather returns a code/message envelope; map to
  `WirekitError::Api`. A 401 usually means a bad or expired key.
- **Retry:** 429 and 5xx with backoff; do not retry 401/403.
- **Rate limits:** plan-dependent; token bucket; cache location keys and
  forecasts aggressively.
- **Webhooks:** none. Polling is the integration.
- **Secrets:** the API key is a query parameter; redact it from logs and
  traces. This is a wirekit requirement, not an option.
- **Idempotency:** forecast reads are naturally idempotent; cache by location
  and time bucket.

## GIA integration

Agent 16 compares the NOAA alert with the AccuWeather alert for the same
ZIP/time window. Both must agree on event type and threshold before
`storm.verified`. If they disagree, the event is held for human review rather
than blasted. Historical summaries feed the property risk model.

## Test plan

- Fixture-based tests for location search, forecast, alert, and historical.
- Key-redaction test: no API key appears in logs or error messages.
- Threshold comparison: NOAA-only, AccuWeather-only, both agree, both disagree.
- 429/5xx retry with a mock transport.
- Unit conversion and timezone handling.

## Open questions

1. **Plan and quota.** Which commercial plan supports the polling volume and
   historical access GIA needs? Confirm before launch.
2. **Hail size.** AccuWeather alerts do not reliably give hail diameter;
   treat this as corroboration, not a measurement.
3. **Data licensing.** Historical data may have redistribution restrictions;
   confirm before using it in a homeowner-facing report.
4. **International coverage.** AccuWeather is global, but alert formats vary;
   test the markets GIA plans to enter.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §6.

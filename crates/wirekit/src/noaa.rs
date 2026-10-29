//! NOAA / National Weather Service: forecasts, active alerts, and radar
//! metadata.
//!
//! Scope: typed point forecasts (lat/long to grid to periods), active-alert
//! queries by area or zone, single-alert lookup, and radar-station metadata.
//! A descriptive `User-Agent` is mandatory because NWS rejects unidentified
//! clients, and an optional TTL cache keeps grid lookups off the network.
//!
//! Non-goals: interpreting hail size or storm severity (that is a
//! storm-service concern), radar imagery decoding, and any write operation —
//! NOAA's API is read-only.
//!
//! ```no_run
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! use wirekit::noaa::{ForecastRequest, NoaaClient};
//! use wirekit::WirekitClient;
//!
//! let client = WirekitClient::builder(wirekit::noaa::BASE_URL)?.build()?;
//! let noaa = NoaaClient::builder(client)
//!     .user_agent("my-service/1.0 (ops@example.com)")
//!     .build();
//!
//! let forecast = noaa.forecast(&ForecastRequest::new(40.7128, -74.0060)?).await?;
//! println!("{} periods", forecast.periods.len());
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::RequestSpec;

/// The NOAA API base URL.
pub const BASE_URL: &str = "https://api.weather.gov";

/// The default cache lifetime for a successful GET.
pub const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);

/// A quantitative value as NOAA encodes it: a unit code and a nullable number.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct QuantitativeValue {
    /// The unit, such as `wmoUnit:percent`.
    #[serde(default, rename = "unitCode")]
    pub unit_code: Option<String>,
    /// The value, when the provider supplied one.
    #[serde(default)]
    pub value: Option<f64>,
}

/// One forecast period.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ForecastPeriod {
    /// The period number, starting at 1.
    #[serde(default)]
    pub number: Option<u32>,
    /// A display name such as `Tonight` or `Monday`.
    #[serde(default)]
    pub name: String,
    /// Period start, as an RFC 3339 timestamp.
    #[serde(default, rename = "startTime")]
    pub start_time: Option<String>,
    /// Period end, as an RFC 3339 timestamp.
    #[serde(default, rename = "endTime")]
    pub end_time: Option<String>,
    /// Whether the period is daytime.
    #[serde(default, rename = "isDaytime")]
    pub is_daytime: Option<bool>,
    /// The forecast temperature in `temperature_unit`.
    #[serde(default)]
    pub temperature: Option<f64>,
    /// `F` or `C`.
    #[serde(default, rename = "temperatureUnit")]
    pub temperature_unit: Option<String>,
    /// Wind speed and direction, as NOAA renders it.
    #[serde(default, rename = "windSpeed")]
    pub wind_speed: Option<String>,
    /// A one-line forecast.
    #[serde(default, rename = "shortForecast")]
    pub short_forecast: Option<String>,
    /// The full narrative forecast.
    #[serde(default, rename = "detailedForecast")]
    pub detailed_forecast: Option<String>,
    /// Chance of precipitation.
    #[serde(default, rename = "probabilityOfPrecipitation")]
    pub probability_of_precipitation: Option<QuantitativeValue>,
}

/// A point forecast: the periods plus the first period's headline numbers.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ForecastResponse {
    /// The forecast periods, nearest first.
    pub periods: Vec<ForecastPeriod>,
    /// The first period's temperature, when present.
    pub temperature: Option<f64>,
    /// The first period's chance of precipitation, when present.
    pub precipitation: Option<f64>,
}

/// A request for a point forecast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForecastRequest {
    /// Latitude in decimal degrees, `-90.0..=90.0`.
    pub latitude: f64,
    /// Longitude in decimal degrees, `-180.0..=180.0`.
    pub longitude: f64,
}

impl ForecastRequest {
    /// Build a request, rejecting non-finite or out-of-range coordinates.
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, WirekitError> {
        if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
            return Err(WirekitError::invalid_config(
                "latitude must be a finite number between -90 and 90",
            ));
        }
        if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
            return Err(WirekitError::invalid_config(
                "longitude must be a finite number between -180 and 180",
            ));
        }
        Ok(Self {
            latitude,
            longitude,
        })
    }

    /// The NWS point path for these coordinates.
    #[must_use]
    pub fn point_path(&self) -> String {
        format!("points/{},{}", self.latitude, self.longitude)
    }

    /// Latitude in decimal degrees.
    #[must_use]
    pub fn lat(&self) -> f64 {
        self.latitude
    }

    /// Longitude in decimal degrees.
    #[must_use]
    pub fn lon(&self) -> f64 {
        self.longitude
    }
}

/// A filter for active alerts.
#[derive(Debug, Clone, Default)]
pub struct AlertQuery {
    /// Two-letter state or territory code, such as `TX`.
    pub area: Option<String>,
    /// NWS zone identifier, such as `TXZ174`.
    pub zone: Option<String>,
    /// Severity filter: `Extreme`, `Severe`, `Moderate`, `Minor`, `Unknown`.
    pub severity: Option<String>,
    /// Maximum number of alerts to return.
    pub limit: Option<u32>,
}

impl AlertQuery {
    /// An unfiltered query.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by state or territory code.
    #[must_use]
    pub fn area(mut self, area: impl Into<String>) -> Self {
        self.area = Some(area.into());
        self
    }

    /// Filter by zone identifier.
    #[must_use]
    pub fn zone(mut self, zone: impl Into<String>) -> Self {
        self.zone = Some(zone.into());
        self
    }

    /// Filter by severity.
    #[must_use]
    pub fn severity(mut self, severity: impl Into<String>) -> Self {
        self.severity = Some(severity.into());
        self
    }

    /// Cap the number of alerts.
    #[must_use]
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// An active or historical weather alert.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct WeatherAlert {
    /// The alert identifier.
    pub id: String,
    /// The event name, such as `Tornado Warning`.
    #[serde(default)]
    pub event: Option<String>,
    /// `Extreme`, `Severe`, `Moderate`, `Minor`, or `Unknown`.
    #[serde(default)]
    pub severity: Option<String>,
    /// `Observed`, `Likely`, `Possible`, `Unlikely`, or `Unknown`.
    #[serde(default)]
    pub certainty: Option<String>,
    /// `Immediate`, `Expected`, `Future`, `Past`, or `Unknown`.
    #[serde(default)]
    pub urgency: Option<String>,
    /// A one-line summary.
    #[serde(default)]
    pub headline: Option<String>,
    /// The full alert text.
    #[serde(default)]
    pub description: Option<String>,
    /// What the recipient should do.
    #[serde(default)]
    pub instruction: Option<String>,
    /// The affected areas, split from NOAA's `areaDesc`.
    #[serde(default)]
    pub areas: Vec<String>,
    /// When the alert takes effect.
    #[serde(default)]
    pub onset: Option<String>,
    /// When the alert expires.
    #[serde(default)]
    pub expires: Option<String>,
    /// `Actual`, `Exercise`, `Test`, or `Draft`.
    #[serde(default)]
    pub status: Option<String>,
}

/// A normalized severe-weather event, ready for a storm pipeline.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SevereWeatherEvent {
    /// The alert this event came from.
    pub alert_id: String,
    /// The event name.
    pub event: String,
    /// Severity, lowercased.
    pub severity: String,
    /// Whether the event is a warning.
    pub is_warning: bool,
    /// The affected areas.
    pub areas: Vec<String>,
}

impl WeatherAlert {
    /// Whether this alert is a warning.
    #[must_use]
    pub fn is_warning(&self) -> bool {
        self.event
            .as_deref()
            .is_some_and(|event| event.to_ascii_lowercase().contains("warning"))
    }

    /// Normalize this alert into a [`SevereWeatherEvent`].
    #[must_use]
    pub fn to_event(&self) -> SevereWeatherEvent {
        SevereWeatherEvent {
            alert_id: self.id.clone(),
            event: self.event.clone().unwrap_or_default(),
            severity: self
                .severity
                .clone()
                .unwrap_or_else(|| "unknown".to_string())
                .to_ascii_lowercase(),
            is_warning: self.is_warning(),
            areas: self.areas.clone(),
        }
    }
}

/// Metadata for one radar station.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RadarMetadata {
    /// The station identifier, such as `KTLX`.
    pub station: String,
    /// The station's display name.
    #[serde(default)]
    pub name: Option<String>,
    /// The last update reported by NOAA.
    #[serde(default)]
    pub updated: Option<String>,
    /// Latitude in decimal degrees.
    #[serde(default)]
    pub latitude: Option<f64>,
    /// Longitude in decimal degrees.
    #[serde(default)]
    pub longitude: Option<f64>,
}

struct CacheEntry {
    expires_at: Instant,
    body: Vec<u8>,
}

struct NoaaInner {
    client: WirekitClient,
    user_agent: Option<String>,
    cache_ttl: Option<Duration>,
    cache: Mutex<HashMap<String, CacheEntry>>,
}

/// The NOAA client. Clone it freely; the cache and transport are shared.
#[derive(Clone)]
pub struct NoaaClient {
    inner: Arc<NoaaInner>,
}

/// The builder for [`NoaaClient`].
pub struct NoaaClientBuilder {
    client: WirekitClient,
    user_agent: Option<String>,
    cache_ttl: Option<Duration>,
}

impl NoaaClientBuilder {
    /// Set the `User-Agent` NOAA requires. Identify your application and a
    /// contact address, for example `storm-service/1.0 (ops@example.com)`.
    #[must_use]
    pub fn user_agent(mut self, value: impl Into<String>) -> Self {
        self.user_agent = Some(value.into());
        self
    }

    /// Change the successful-GET cache lifetime.
    #[must_use]
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = Some(ttl);
        self
    }

    /// Turn the cache off.
    #[must_use]
    pub fn disable_cache(mut self) -> Self {
        self.cache_ttl = None;
        self
    }

    /// Build the client.
    #[must_use]
    pub fn build(self) -> NoaaClient {
        NoaaClient {
            inner: Arc::new(NoaaInner {
                client: self.client,
                user_agent: self.user_agent,
                cache_ttl: self.cache_ttl,
                cache: Mutex::new(HashMap::new()),
            }),
        }
    }
}

impl NoaaClient {
    /// Start building a NOAA client over a shared [`WirekitClient`].
    ///
    /// Build the underlying client against [`BASE_URL`].
    #[must_use]
    pub fn builder(client: WirekitClient) -> NoaaClientBuilder {
        NoaaClientBuilder {
            client,
            user_agent: None,
            cache_ttl: Some(DEFAULT_CACHE_TTL),
        }
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// The configured `User-Agent`, if any.
    #[must_use]
    pub fn user_agent(&self) -> Option<&str> {
        self.inner.user_agent.as_deref()
    }

    /// Drop every cached response.
    pub fn clear_cache(&self) {
        lock(&self.inner.cache).clear();
    }

    fn require_user_agent(&self) -> Result<&str, WirekitError> {
        match self.inner.user_agent.as_deref() {
            Some(agent) if !agent.trim().is_empty() => Ok(agent),
            _ => Err(WirekitError::invalid_config(
                "NOAA requires a descriptive User-Agent; set it with NoaaClient::builder(..).user_agent(\"app/1.0 (contact)\")",
            )),
        }
    }

    /// Fetch a point forecast.
    ///
    /// Two requests are made on a cache miss: the point lookup resolves the
    /// grid, and the forecast document supplies the periods.
    pub async fn forecast(
        &self,
        request: &ForecastRequest,
    ) -> Result<ForecastResponse, WirekitError> {
        self.require_user_agent()?;

        let point: PointEnvelope = self.get_json(&request.point_path()).await?;
        let forecast_url = point.properties.forecast.ok_or_else(|| {
            WirekitError::decode("the NOAA point response did not include a forecast URL")
        })?;

        let document: ForecastEnvelope = self.get_json(&forecast_url).await?;
        let periods = document.properties.periods;
        let temperature = periods.first().and_then(|period| period.temperature);
        let precipitation = periods
            .first()
            .and_then(|period| period.probability_of_precipitation.as_ref())
            .and_then(|value| value.value);

        Ok(ForecastResponse {
            periods,
            temperature,
            precipitation,
        })
    }

    /// Fetch active alerts matching `query`.
    pub async fn alerts_active(
        &self,
        query: &AlertQuery,
    ) -> Result<Vec<WeatherAlert>, WirekitError> {
        self.require_user_agent()?;

        let mut url = self.inner.client.url("alerts/active")?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(area) = query.area.as_deref() {
                pairs.append_pair("area", area);
            }
            if let Some(zone) = query.zone.as_deref() {
                pairs.append_pair("zone", zone);
            }
            if let Some(severity) = query.severity.as_deref() {
                pairs.append_pair("severity", severity);
            }
            if let Some(limit) = query.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
        }

        let envelope: AlertEnvelope = self.get_json(url.as_str()).await?;
        Ok(envelope
            .features
            .into_iter()
            .map(|feature| feature.properties.into_alert())
            .collect())
    }

    /// Fetch one alert by identifier.
    pub async fn alert(&self, id: &str) -> Result<WeatherAlert, WirekitError> {
        self.require_user_agent()?;
        if id.trim().is_empty() {
            return Err(WirekitError::invalid_config("an alert id is required"));
        }
        let path = format!("alerts/{}", id.trim());
        let envelope: AlertFeatureEnvelope = self.get_json(&path).await?;
        Ok(envelope.properties.into_alert())
    }

    /// Fetch metadata for a radar station, such as `KTLX`.
    pub async fn radar_metadata(&self, station: &str) -> Result<RadarMetadata, WirekitError> {
        self.require_user_agent()?;
        let station = station.trim();
        if station.is_empty() {
            return Err(WirekitError::invalid_config("a radar station is required"));
        }

        let path = format!("radar/stations/{station}");
        let envelope: RadarEnvelope = self.get_json(&path).await?;
        let properties = envelope.properties;

        Ok(RadarMetadata {
            station: properties
                .station_identifier
                .unwrap_or_else(|| station.to_string()),
            name: properties.name,
            updated: properties.updated,
            latitude: properties.latitude.and_then(|value| value.value),
            longitude: properties.longitude.and_then(|value| value.value),
        })
    }

    async fn get_json<T: DeserializeOwned>(&self, path_or_url: &str) -> Result<T, WirekitError> {
        let url = self.inner.client.url(path_or_url)?;
        let cache_key = url.as_str().to_string();

        if self.inner.cache_ttl.is_some() {
            if let Some(body) = self.cache_get(&cache_key) {
                return decode(&body);
            }
        }

        let agent = self.require_user_agent()?;
        let request = RequestSpec::get(url).try_header("user-agent", agent)?;
        let raw = self.inner.client.send(request).await?;

        self.cache_put(cache_key, &raw.body);

        decode(&raw.body)
    }

    fn cache_get(&self, key: &str) -> Option<Vec<u8>> {
        let mut cache = lock(&self.inner.cache);
        let entry = cache.get(key)?;
        if entry.expires_at <= Instant::now() {
            cache.remove(key);
            return None;
        }
        Some(entry.body.clone())
    }

    fn cache_put(&self, key: String, body: &[u8]) {
        let Some(ttl) = self.inner.cache_ttl else {
            return;
        };
        // A cached error body is never stored, so only successful GETs reach
        // here.
        lock(&self.inner.cache).insert(
            key,
            CacheEntry {
                expires_at: Instant::now() + ttl,
                body: body.to_vec(),
            },
        );
    }
}

fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, WirekitError> {
    serde_json::from_slice(body).map_err(|error| {
        WirekitError::decode_with("the NOAA response did not match the expected type", error)
    })
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Deserialize)]
struct PointEnvelope {
    properties: PointProperties,
}

#[derive(Deserialize)]
struct PointProperties {
    #[serde(default)]
    forecast: Option<String>,
}

#[derive(Deserialize)]
struct ForecastEnvelope {
    properties: ForecastProperties,
}

#[derive(Deserialize)]
struct ForecastProperties {
    #[serde(default)]
    periods: Vec<ForecastPeriod>,
}

#[derive(Deserialize)]
struct AlertEnvelope {
    #[serde(default)]
    features: Vec<AlertFeature>,
}

#[derive(Deserialize)]
struct AlertFeatureEnvelope {
    properties: AlertProperties,
}

#[derive(Deserialize)]
struct AlertFeature {
    properties: AlertProperties,
}

#[derive(Deserialize)]
struct AlertProperties {
    #[serde(default)]
    id: String,
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    certainty: Option<String>,
    #[serde(default)]
    urgency: Option<String>,
    #[serde(default)]
    headline: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    instruction: Option<String>,
    #[serde(default, rename = "areaDesc")]
    area_desc: Option<String>,
    #[serde(default)]
    onset: Option<String>,
    #[serde(default)]
    expires: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

impl AlertProperties {
    fn into_alert(self) -> WeatherAlert {
        let areas = self
            .area_desc
            .as_deref()
            .map(|desc| {
                desc.split(';')
                    .map(str::trim)
                    .filter(|area| !area.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        WeatherAlert {
            id: self.id,
            event: self.event,
            severity: self.severity,
            certainty: self.certainty,
            urgency: self.urgency,
            headline: self.headline,
            description: self.description,
            instruction: self.instruction,
            areas,
            onset: self.onset,
            expires: self.expires,
            status: self.status,
        }
    }
}

#[derive(Deserialize)]
struct RadarEnvelope {
    properties: RadarProperties,
}

#[derive(Deserialize)]
struct RadarProperties {
    #[serde(default, rename = "stationIdentifier")]
    station_identifier: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(default)]
    latitude: Option<QuantitativeValue>,
    #[serde(default)]
    longitude: Option<QuantitativeValue>,
}

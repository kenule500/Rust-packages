//! AccuWeather — location lookup, current conditions, forecasts, alerts, and
//! historical daily summaries.
//!
//! # Scope
//!
//! Built from `catalogue/08_accuweather.md`: search locations, read current
//! conditions, read daily and hourly forecasts, list active alerts, and
//! summarise the recent past into per-day values. Every call travels through
//! the shared [`WirekitClient`], so the API-key auth scheme, the token bucket,
//! the retry policy with `Retry-After`, the tracing events, and the response
//! size limit behave exactly as they do for every other provider.
//!
//! # Non-goals
//!
//! - **Not a NOAA replacement.** NOAA stays authoritative for United States
//!   alerts; this module exists so a caller can corroborate one source against
//!   the other before acting.
//! - **No verification logic.** Deciding that two sources agree, and holding a
//!   storm event for human review when they disagree, belongs to the caller.
//! - **No paid historical tiers.** AccuWeather's self-serve surface exposes the
//!   past 24 hours of observations, so [`AccuWeatherClient::historical_daily`]
//!   aggregates those observations per calendar day. It does not call an
//!   endpoint this feature does not document.
//! - **No forecast cache keyed by time bucket.** Caching a forecast is a
//!   caller policy; only location keys are cached here.
//!
//! # Errors, retry, and secrets
//!
//! AccuWeather reports failures as a `Code`/`Message` envelope, which maps to
//! [`WirekitError::Api`] with the provider code preserved. A 401 or 403 usually
//! means the key is wrong or expired, so it is reported as an authentication
//! failure and never retried. Retries cover 429 and 5xx only. The API key is a
//! query parameter, so it is held in a [`Secret`], applied by
//! [`ApiKeyQuery`], and redacted from tracing and error output.
//!
//! ```no_run
//! use wirekit::{AccuWeatherClient, AccuWeatherConfig, Secret, WirekitClient};
//!
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! let transport = WirekitClient::new(wirekit::accuweather::BASE_URL)?;
//! let client = AccuWeatherClient::new(
//!     transport,
//!     AccuWeatherConfig::new(Secret::new(String::from("your-api-key"))),
//! )?;
//!
//! let locations = client.search_locations("San Francisco", 1).await?;
//! let conditions = client.current_conditions(&locations[0].key).await?;
//! println!("{}: {}", locations[0].localized_name, conditions.weather_text);
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Deserialize;

use crate::auth::{ApiKeyQuery, AuthScheme};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The AccuWeather API base URL.
pub const BASE_URL: &str = "https://dataservice.accuweather.com";

/// The location keys the client remembers before the oldest entry is dropped.
pub const LOCATION_CACHE_CAPACITY: usize = 128;

/// How the client is configured.
#[derive(Debug, Clone)]
pub struct AccuWeatherConfig {
    /// The API key. It is sent as the `apikey` query parameter and never
    /// rendered by `Debug`.
    pub api_key: Secret<String>,
    /// Report temperatures in Celsius rather than Fahrenheit.
    pub metric: bool,
}

impl AccuWeatherConfig {
    /// Build a metric configuration from an API key.
    #[must_use]
    pub fn new(api_key: Secret<String>) -> Self {
        Self {
            api_key,
            metric: true,
        }
    }

    /// Report temperatures in Fahrenheit instead of Celsius.
    #[must_use]
    pub fn with_imperial_units(mut self) -> Self {
        self.metric = false;
        self
    }
}

/// One temperature reading with the unit the provider reported it in.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct TemperatureValue {
    /// The numeric value.
    #[serde(rename = "Value", default)]
    pub value: f64,
    /// The unit, `C` or `F`.
    #[serde(rename = "Unit", default)]
    pub unit: String,
}

/// A location AccuWeather recognises, and the key every other call needs.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Location {
    /// The location key, used by every other method.
    #[serde(rename = "Key", default)]
    pub key: String,
    /// The location type, for example `City`.
    #[serde(rename = "Type", default)]
    pub kind: String,
    /// The provider's relevance rank.
    #[serde(rename = "Rank", default)]
    pub rank: u32,
    /// The display name, for example `San Francisco`.
    #[serde(rename = "LocalizedName", default)]
    pub localized_name: String,
    /// The administrative area, for example `California`.
    #[serde(rename = "AdministrativeArea", default)]
    pub administrative_area: Option<AdministrativeArea>,
    /// The country.
    #[serde(rename = "Country", default)]
    pub country: Option<Country>,
}

/// The administrative area of a [`Location`].
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct AdministrativeArea {
    /// The area name.
    #[serde(rename = "LocalizedName", default)]
    pub localized_name: String,
}

/// The country of a [`Location`].
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Country {
    /// The ISO country code.
    #[serde(rename = "ID", default)]
    pub id: String,
    /// The country name.
    #[serde(rename = "LocalizedName", default)]
    pub localized_name: String,
}

/// Current conditions for one location.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CurrentConditions {
    /// When the provider observed the conditions, as it reported them.
    pub observed_at: String,
    /// The same instant as an epoch second.
    pub epoch_time: i64,
    /// A short description, for example `Partly cloudy`.
    pub weather_text: String,
    /// The provider's icon code.
    pub weather_icon: i32,
    /// Whether precipitation was falling.
    pub has_precipitation: bool,
    /// The precipitation type, when the provider names one.
    pub precipitation_type: Option<String>,
    /// Whether it was daylight.
    pub is_day_time: bool,
    /// The temperature in the configured unit.
    pub temperature: TemperatureValue,
}

/// One day of a daily forecast.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DailyForecast {
    /// The local date, `YYYY-MM-DD`.
    pub date: String,
    /// The same day as an epoch second.
    pub epoch_date: i64,
    /// The minimum temperature of the day.
    pub minimum: TemperatureValue,
    /// The maximum temperature of the day.
    pub maximum: TemperatureValue,
    /// The daytime phrase, for example `Intermittent clouds`.
    pub day_phrase: String,
    /// The overnight phrase.
    pub night_phrase: String,
}

/// One hour of an hourly forecast.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HourlyForecast {
    /// The local date and time, as the provider reported it.
    pub date_time: String,
    /// The same hour as an epoch second.
    pub epoch_date_time: i64,
    /// The temperature for the hour.
    pub temperature: TemperatureValue,
    /// A short description, for example `Mostly cloudy`.
    pub icon_phrase: String,
    /// The provider's precipitation probability, when it reports one.
    pub precipitation_probability: Option<u8>,
}

/// An active weather alert.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeatherAlert {
    /// The provider's alert identifier.
    pub alert_id: String,
    /// The headline a human reads first.
    pub headline: String,
    /// The severity, for example `Moderate`.
    pub severity: String,
    /// The category, for example `rain`.
    pub category: String,
    /// The affected area names.
    pub areas: Vec<String>,
    /// When the alert takes effect, as the provider reported it.
    pub effective_at: String,
    /// When the alert expires, as the provider reported it.
    pub expires_at: String,
}

/// A per-day summary aggregated from the provider's recent observations.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoricalWeather {
    /// The local date, `YYYY-MM-DD`.
    pub date: String,
    /// The lowest temperature observed on that date.
    pub minimum: TemperatureValue,
    /// The highest temperature observed on that date.
    pub maximum: TemperatureValue,
    /// How many observations the summary was computed from.
    pub observations: usize,
}

/// What a forecast call asks for: a location key or a coordinate pair, and how
/// many days or hours to return.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ForecastRequest {
    /// A location key from [`AccuWeatherClient::search_locations`].
    pub location_key: Option<String>,
    /// The latitude, when the caller prefers coordinates.
    pub latitude: Option<f64>,
    /// The longitude, when the caller prefers coordinates.
    pub longitude: Option<f64>,
    /// How many daily periods to return. Zero requests no daily periods.
    pub days: u8,
    /// How many hourly periods to return. Zero requests no hourly periods.
    pub hours: u8,
}

impl ForecastRequest {
    /// Ask for a forecast by location key.
    #[must_use]
    pub fn for_location_key(key: impl Into<String>, days: u8, hours: u8) -> Self {
        Self {
            location_key: Some(key.into()),
            latitude: None,
            longitude: None,
            days,
            hours,
        }
    }

    /// Ask for a forecast by coordinate.
    #[must_use]
    pub fn for_coordinates(latitude: f64, longitude: f64, days: u8, hours: u8) -> Self {
        Self {
            location_key: None,
            latitude: Some(latitude),
            longitude: Some(longitude),
            days,
            hours,
        }
    }
}

/// The periods a forecast returned.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ForecastResponse {
    /// The provider's headline for the period, when it sends one.
    pub headline: Option<String>,
    /// The daily periods, in provider order.
    pub daily: Vec<DailyForecast>,
    /// The hourly periods, in provider order.
    pub hourly: Vec<HourlyForecast>,
}

struct AccuWeatherInner {
    client: WirekitClient,
    config: AccuWeatherConfig,
    auth: Arc<dyn AuthScheme>,
    locations: Mutex<HashMap<String, String>>,
}

/// The AccuWeather client. Clone it freely; the transport and the location
/// cache are shared.
#[derive(Clone)]
pub struct AccuWeatherClient {
    inner: Arc<AccuWeatherInner>,
}

impl AccuWeatherClient {
    /// Build a client over a shared [`WirekitClient`].
    ///
    /// Build the underlying client against [`BASE_URL`]. An empty API key is
    /// [`WirekitError::InvalidConfig`].
    pub fn new(
        client: WirekitClient,
        config: AccuWeatherConfig,
    ) -> Result<Self, WirekitError> {
        if config.api_key.expose().trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "an AccuWeather API key is required",
            ));
        }

        let auth: Arc<dyn AuthScheme> = Arc::new(ApiKeyQuery::new(
            "apikey",
            config.api_key.expose().clone(),
        ));
        Ok(Self {
            inner: Arc::new(AccuWeatherInner {
                client,
                config,
                auth,
                locations: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// The client configuration.
    #[must_use]
    pub fn config(&self) -> &AccuWeatherConfig {
        &self.inner.config
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// Search for a location by name and return the best `limit` matches.
    ///
    /// A blank query is [`WirekitError::InvalidConfig`] rather than a request
    /// the provider would reject.
    pub async fn search_locations(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Location>, WirekitError> {
        if query.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a location search needs a query",
            ));
        }
        if limit == 0 {
            return Err(WirekitError::invalid_config(
                "a location search needs a limit of at least one",
            ));
        }

        let mut url = self.inner.client.url("locations/v1/cities/search")?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", query.trim());
            pairs.append_pair("details", "true");
        }

        let request = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("accuweather");
        let response: Response<Vec<Location>> = self
            .inner
            .client
            .execute(request)
            .await
            .map_err(map_accuweather_error)?;

        Ok(response.value.into_iter().take(limit).collect())
    }

    /// Read the current conditions for a location key.
    pub async fn current_conditions(
        &self,
        location_key: &str,
    ) -> Result<CurrentConditions, WirekitError> {
        ensure_location_key(location_key)?;

        let url = self
            .inner
            .client
            .url(&format!("currentconditions/v1/{location_key}"))?;
        let request = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("accuweather")
            .body_error_hook(reject_error_envelope);
        let response: Response<Vec<WireCurrentConditions>> = self
            .inner
            .client
            .execute(request)
            .await
            .map_err(map_accuweather_error)?;

        response
            .value
            .into_iter()
            .next()
            .map(|wire| wire.into_conditions(self.inner.config.metric))
            .ok_or_else(|| {
                WirekitError::decode(
                    "AccuWeather returned no current conditions for that location",
                )
            })
    }

    /// Read the daily and hourly periods a request asks for.
    ///
    /// The location key in the request is used as given; coordinates are
    /// resolved through [`AccuWeatherClient::search_locations`]'s geoposition
    /// endpoint and cached, so repeated calls for one coordinate pair cost one
    /// lookup.
    pub async fn forecast(
        &self,
        request: &ForecastRequest,
    ) -> Result<ForecastResponse, WirekitError> {
        if request.days == 0 && request.hours == 0 {
            return Err(WirekitError::invalid_config(
                "a forecast needs at least one daily or hourly period",
            ));
        }

        let key = self.resolve_location_key(request).await?;
        let mut response = ForecastResponse::default();

        if request.days > 0 {
            let days = request.days.min(15);
            let url = self
                .inner
                .client
                .url(&format!("forecasts/v1/daily/{days}day/{key}"))?;
            let spec = RequestSpec::get(url)
                .auth(self.inner.auth.clone())
                .rate_limit_key("accuweather")
                .body_error_hook(reject_error_envelope);
            let daily: Response<WireDailyForecast> = self
                .inner
                .client
                .execute(spec)
                .await
                .map_err(map_accuweather_error)?;

            response.headline = daily.value.headline.and_then(|headline| headline.text);
            response.daily = daily
                .value
                .daily_forecasts
                .into_iter()
                .map(WireDayForecast::into_daily)
                .collect();
        }

        if request.hours > 0 {
            let hours = request.hours.min(12);
            let url = self
                .inner
                .client
                .url(&format!("forecasts/v1/hourly/{hours}hour/{key}"))?;
            let spec = RequestSpec::get(url)
                .auth(self.inner.auth.clone())
                .rate_limit_key("accuweather")
                .body_error_hook(reject_error_envelope);
            let hourly: Response<Vec<WireHourlyForecast>> = self
                .inner
                .client
                .execute(spec)
                .await
                .map_err(map_accuweather_error)?;

            response.hourly = hourly
                .value
                .into_iter()
                .map(WireHourlyForecast::into_hourly)
                .collect();
        }

        Ok(response)
    }

    /// List the active alerts for a location key.
    pub async fn alerts(&self, location_key: &str) -> Result<Vec<WeatherAlert>, WirekitError> {
        ensure_location_key(location_key)?;

        let url = self
            .inner
            .client
            .url(&format!("alerts/v1/{location_key}"))?;
        let request = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("accuweather")
            .body_error_hook(reject_error_envelope);
        let response: Response<Vec<WireAlert>> = self
            .inner
            .client
            .execute(request)
            .await
            .map_err(map_accuweather_error)?;

        Ok(response.value.into_iter().map(WireAlert::into_alert).collect())
    }

    /// Summarise the last `hours` of observations into per-day values.
    ///
    /// The self-serve surface exposes the past 24 hours of observations, so
    /// `hours` is clamped to 24 and the days come from the observations the
    /// provider actually returns. A day with no observation is absent from the
    /// result rather than invented.
    pub async fn historical_daily(
        &self,
        location_key: &str,
        hours: u8,
    ) -> Result<Vec<HistoricalWeather>, WirekitError> {
        ensure_location_key(location_key)?;
        if hours == 0 {
            return Err(WirekitError::invalid_config(
                "a historical summary needs at least one hour",
            ));
        }

        let url = self
            .inner
            .client
            .url(&format!("currentconditions/v1/{location_key}/historical/24"))?;
        let request = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("accuweather")
            .body_error_hook(reject_error_envelope);
        let response: Response<Vec<WireCurrentConditions>> = self
            .inner
            .client
            .execute(request)
            .await
            .map_err(map_accuweather_error)?;

        let metric = self.inner.config.metric;
        let observations: Vec<CurrentConditions> = response
            .value
            .into_iter()
            .take(usize::from(hours))
            .map(|wire| wire.into_conditions(metric))
            .collect();

        Ok(summarise_by_day(&observations))
    }

    /// Resolve a request to a location key, caching coordinate lookups.
    async fn resolve_location_key(
        &self,
        request: &ForecastRequest,
    ) -> Result<String, WirekitError> {
        if let Some(key) = request.location_key.as_deref() {
            ensure_location_key(key)?;
            return Ok(key.to_owned());
        }

        let (latitude, longitude) = match (request.latitude, request.longitude) {
            (Some(latitude), Some(longitude)) => (latitude, longitude),
            _ => {
                return Err(WirekitError::invalid_config(
                    "a forecast needs a location key or a latitude and longitude pair",
                ));
            }
        };
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return Err(WirekitError::invalid_config(
                "a forecast coordinate is outside the valid range",
            ));
        }

        let cache_key = format!("{latitude:.4},{longitude:.4}");
        if let Some(key) = lock(&self.inner.locations).get(&cache_key) {
            return Ok(key.clone());
        }

        let mut url = self.inner.client.url("locations/v1/cities/geoposition/search")?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", &format!("{latitude},{longitude}"));
        }
        let spec = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("accuweather");
        let response: Response<Location> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_accuweather_error)?;

        let key = response.value.key;
        if key.is_empty() {
            return Err(WirekitError::decode(
                "AccuWeather returned no location for that coordinate",
            ));
        }

        let mut cache = lock(&self.inner.locations);
        if cache.len() >= LOCATION_CACHE_CAPACITY {
            if let Some(oldest) = cache.keys().next().cloned() {
                cache.remove(&oldest);
            }
        }
        cache.insert(cache_key, key.clone());

        Ok(key)
    }
}

fn ensure_location_key(location_key: &str) -> Result<(), WirekitError> {
    if location_key.trim().is_empty() {
        return Err(WirekitError::invalid_config(
            "an AccuWeather location key is required",
        ));
    }
    Ok(())
}

/// Group observations by local date and take the extreme temperature of each.
fn summarise_by_day(observations: &[CurrentConditions]) -> Vec<HistoricalWeather> {
    let mut days: Vec<HistoricalWeather> = Vec::new();

    for observation in observations {
        let date: String = observation.observed_at.chars().take(10).collect();
        if date.is_empty() {
            continue;
        }

        match days.iter_mut().find(|day| day.date == date) {
            Some(day) => {
                if observation.temperature.value < day.minimum.value {
                    day.minimum = observation.temperature.clone();
                }
                if observation.temperature.value > day.maximum.value {
                    day.maximum = observation.temperature.clone();
                }
                day.observations += 1;
            }
            None => days.push(HistoricalWeather {
                date,
                minimum: observation.temperature.clone(),
                maximum: observation.temperature.clone(),
                observations: 1,
            }),
        }
    }

    days
}

/// Reject a 2xx response whose body carries AccuWeather's error envelope.
fn reject_error_envelope(status: u16, body: &[u8]) -> Option<WirekitError> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if value.is_array() {
        return None;
    }
    let code = value.get("Code").and_then(serde_json::Value::as_str)?;
    let message = value
        .get("Message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("AccuWeather reported a failure");
    Some(WirekitError::api_with_details(
        status,
        code,
        message,
        value.clone(),
    ))
}

/// Map an AccuWeather rejection onto the shared error type.
///
/// AccuWeather capitalises its envelope keys, so the `Code` and `Message` are
/// lifted out of the retained body rather than read from the lowercase keys the
/// shared parser looks for. A 401 or 403 becomes an authentication failure.
fn map_accuweather_error(error: WirekitError) -> WirekitError {
    let WirekitError::Api {
        status,
        details: Some(details),
        ..
    } = &error
    else {
        return error;
    };

    if *status == 401 || *status == 403 {
        return WirekitError::auth("AccuWeather rejected the API key");
    }

    let code = details.get("Code").and_then(serde_json::Value::as_str);
    let message = details.get("Message").and_then(serde_json::Value::as_str);

    match (code, message) {
        (Some(code), Some(message)) => {
            WirekitError::api_with_details(*status, code, message, details.clone())
        }
        (Some(code), None) => {
            WirekitError::api_with_details(*status, code, "AccuWeather reported a failure", details.clone())
        }
        _ => error,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Default, Deserialize)]
struct WireTemperatureFlat {
    #[serde(rename = "Value", default)]
    value: f64,
    #[serde(rename = "Unit", default)]
    unit: String,
}

#[derive(Debug, Default, Deserialize)]
struct WireTemperatureVariants {
    #[serde(rename = "Metric", default)]
    metric: Option<WireTemperatureFlat>,
    #[serde(rename = "Imperial", default)]
    imperial: Option<WireTemperatureFlat>,
}

impl WireTemperatureVariants {
    fn pick(&self, metric: bool) -> TemperatureValue {
        let chosen = if metric {
            self.metric.as_ref()
        } else {
            self.imperial.as_ref()
        }
        .or(self.metric.as_ref())
        .or(self.imperial.as_ref());

        match chosen {
            Some(value) => TemperatureValue {
                value: value.value,
                unit: value.unit.clone(),
            },
            None => TemperatureValue::default(),
        }
    }
}

fn flat_to_value(flat: &WireTemperatureFlat) -> TemperatureValue {
    TemperatureValue {
        value: flat.value,
        unit: flat.unit.clone(),
    }
}

#[derive(Debug, Deserialize)]
struct WireCurrentConditions {
    #[serde(rename = "LocalObservationDateTime", default)]
    local_observation_date_time: String,
    #[serde(rename = "EpochTime", default)]
    epoch_time: i64,
    #[serde(rename = "WeatherText", default)]
    weather_text: String,
    #[serde(rename = "WeatherIcon", default)]
    weather_icon: i32,
    #[serde(rename = "HasPrecipitation", default)]
    has_precipitation: bool,
    #[serde(rename = "PrecipitationType", default)]
    precipitation_type: Option<String>,
    #[serde(rename = "IsDayTime", default)]
    is_day_time: bool,
    #[serde(rename = "Temperature", default)]
    temperature: WireTemperatureVariants,
}

impl WireCurrentConditions {
    fn into_conditions(self, metric: bool) -> CurrentConditions {
        CurrentConditions {
            observed_at: self.local_observation_date_time,
            epoch_time: self.epoch_time,
            weather_text: self.weather_text,
            weather_icon: self.weather_icon,
            has_precipitation: self.has_precipitation,
            precipitation_type: self.precipitation_type,
            is_day_time: self.is_day_time,
            temperature: self.temperature.pick(metric),
        }
    }
}

#[derive(Debug, Deserialize)]
struct WireHeadline {
    #[serde(rename = "Text", default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireDailyForecast {
    #[serde(rename = "Headline", default)]
    headline: Option<WireHeadline>,
    #[serde(rename = "DailyForecasts", default)]
    daily_forecasts: Vec<WireDayForecast>,
}

#[derive(Debug, Deserialize)]
struct WireDayForecast {
    #[serde(rename = "Date", default)]
    date: String,
    #[serde(rename = "EpochDate", default)]
    epoch_date: i64,
    #[serde(rename = "Temperature", default)]
    temperature: WireDayTemperature,
    #[serde(rename = "Day", default)]
    day: WireDaypart,
    #[serde(rename = "Night", default)]
    night: WireDaypart,
}

#[derive(Debug, Default, Deserialize)]
struct WireDayTemperature {
    #[serde(rename = "Minimum", default)]
    minimum: Option<WireTemperatureFlat>,
    #[serde(rename = "Maximum", default)]
    maximum: Option<WireTemperatureFlat>,
}

#[derive(Debug, Default, Deserialize)]
struct WireDaypart {
    #[serde(rename = "IconPhrase", default)]
    icon_phrase: String,
}

impl WireDayForecast {
    fn into_daily(self) -> DailyForecast {
        DailyForecast {
            date: self.date,
            epoch_date: self.epoch_date,
            minimum: self
                .temperature
                .minimum
                .as_ref()
                .map(flat_to_value)
                .unwrap_or_default(),
            maximum: self
                .temperature
                .maximum
                .as_ref()
                .map(flat_to_value)
                .unwrap_or_default(),
            day_phrase: self.day.icon_phrase,
            night_phrase: self.night.icon_phrase,
        }
    }
}

#[derive(Debug, Deserialize)]
struct WireHourlyForecast {
    #[serde(rename = "DateTime", default)]
    date_time: String,
    #[serde(rename = "EpochDateTime", default)]
    epoch_date_time: i64,
    #[serde(rename = "Temperature", default)]
    temperature: Option<WireTemperatureFlat>,
    #[serde(rename = "IconPhrase", default)]
    icon_phrase: String,
    #[serde(rename = "PrecipitationProbability", default)]
    precipitation_probability: Option<u8>,
}

impl WireHourlyForecast {
    fn into_hourly(self) -> HourlyForecast {
        HourlyForecast {
            date_time: self.date_time,
            epoch_date_time: self.epoch_date_time,
            temperature: self
                .temperature
                .as_ref()
                .map(flat_to_value)
                .unwrap_or_default(),
            icon_phrase: self.icon_phrase,
            precipitation_probability: self.precipitation_probability,
        }
    }
}

#[derive(Debug, Deserialize)]
struct WireAlert {
    #[serde(rename = "AlertID", default)]
    alert_id: String,
    #[serde(rename = "Headline", default)]
    headline: String,
    #[serde(rename = "Severity", default)]
    severity: String,
    #[serde(rename = "Category", default)]
    category: String,
    #[serde(rename = "Area", default)]
    areas: Vec<WireAlertArea>,
    #[serde(rename = "EffectiveTime", default)]
    effective_time: String,
    #[serde(rename = "ExpiresTime", default)]
    expires_time: String,
}

#[derive(Debug, Default, Deserialize)]
struct WireAlertArea {
    #[serde(rename = "Name", default)]
    name: String,
}

impl WireAlert {
    fn into_alert(self) -> WeatherAlert {
        WeatherAlert {
            alert_id: self.alert_id,
            headline: self.headline,
            severity: self.severity,
            category: self.category,
            areas: self
                .areas
                .into_iter()
                .map(|area| area.name)
                .filter(|name| !name.is_empty())
                .collect(),
            effective_at: self.effective_time,
            expires_at: self.expires_time,
        }
    }
}

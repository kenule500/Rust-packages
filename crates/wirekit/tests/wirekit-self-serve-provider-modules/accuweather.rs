//! Acceptance tests for the AccuWeather module (AC-001 to AC-005).

use std::time::{Duration, Instant};

use serde_json::json;
use wirekit::accuweather::{
    AccuWeatherClient, AccuWeatherConfig, ForecastRequest, TemperatureValue,
};
use wirekit::{Secret, WirekitError};
use wiremock::MockServer;

use crate::common::{self, Fixture};

fn client(server: &MockServer) -> AccuWeatherClient {
    AccuWeatherClient::new(
        common::transport(server),
        AccuWeatherConfig::new(Secret::new(String::from(common::API_KEY))),
    )
    .expect("the client builds with a credential")
}

fn location_json() -> serde_json::Value {
    json!([{
        "Key": "349727",
        "Type": "City",
        "Rank": 35,
        "LocalizedName": "Austin",
        "AdministrativeArea": { "LocalizedName": "Texas" },
        "Country": { "ID": "US", "LocalizedName": "United States" }
    }])
}

fn conditions_json() -> serde_json::Value {
    json!([{
        "LocalObservationDateTime": "2026-11-24T09:00:00-06:00",
        "EpochTime": 1_800_000_000,
        "WeatherText": "Partly cloudy",
        "WeatherIcon": 3,
        "HasPrecipitation": false,
        "IsDayTime": true,
        "Temperature": { "Metric": { "Value": 21.5, "Unit": "C" } }
    }])
}

#[tokio::test]
async fn ac_001_accuweather_surface_matches_the_catalogue() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/locations/v1/cities/search",
        Fixture::json(200, location_json()),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/currentconditions/v1/349727",
        Fixture::json(200, conditions_json()),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/forecasts/v1/daily/1day/349727",
        Fixture::json(
            200,
            json!({
                "Headline": { "Text": "Warm through the week" },
                "DailyForecasts": [{
                    "Date": "2026-11-24T07:00:00-06:00",
                    "EpochDate": 1_800_000_100,
                    "Temperature": {
                        "Minimum": { "Value": 12.0, "Unit": "C" },
                        "Maximum": { "Value": 24.0, "Unit": "C" }
                    },
                    "Day": { "IconPhrase": "Sunny" },
                    "Night": { "IconPhrase": "Clear" }
                }]
            }),
        ),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/forecasts/v1/hourly/1hour/349727",
        Fixture::json(
            200,
            json!([{
                "DateTime": "2026-11-24T10:00:00-06:00",
                "EpochDateTime": 1_800_000_200,
                "Temperature": { "Value": 20.0, "Unit": "C" },
                "IconPhrase": "Mostly cloudy",
                "PrecipitationProbability": 15
            }]),
        ),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/alerts/v1/349727",
        Fixture::json(
            200,
            json!([{
                "AlertID": "alert-1",
                "Headline": "Flood warning",
                "Severity": "Severe",
                "Category": "rain",
                "Area": [{ "Name": "Travis" }],
                "EffectiveTime": "2026-11-24T08:00:00-06:00",
                "ExpiresTime": "2026-11-24T20:00:00-06:00"
            }]),
        ),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/currentconditions/v1/349727/historical/24",
        Fixture::json(
            200,
            json!([
                {
                    "LocalObservationDateTime": "2026-11-24T01:00:00-06:00",
                    "EpochTime": 1_800_000_300,
                    "WeatherText": "Clear",
                    "Temperature": { "Metric": { "Value": 9.0, "Unit": "C" } }
                },
                {
                    "LocalObservationDateTime": "2026-11-24T13:00:00-06:00",
                    "EpochTime": 1_800_000_400,
                    "WeatherText": "Sunny",
                    "Temperature": { "Metric": { "Value": 26.0, "Unit": "C" } }
                }
            ]),
        ),
    )
    .await;

    let client = client(&server);

    let locations = client
        .search_locations("Austin", 1)
        .await
        .expect("location search succeeds");
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].key, "349727");
    assert_eq!(locations[0].localized_name, "Austin");
    assert_eq!(
        locations[0]
            .administrative_area
            .as_ref()
            .map(|area| area.localized_name.as_str()),
        Some("Texas")
    );

    let conditions = client
        .current_conditions("349727")
        .await
        .expect("conditions succeed");
    assert_eq!(conditions.weather_text, "Partly cloudy");
    assert_eq!(
        conditions.temperature,
        TemperatureValue {
            value: 21.5,
            unit: String::from("C")
        }
    );

    let forecast = client
        .forecast(&ForecastRequest::for_location_key("349727", 1, 1))
        .await
        .expect("the forecast succeeds");
    assert_eq!(forecast.headline.as_deref(), Some("Warm through the week"));
    assert_eq!(forecast.daily.len(), 1);
    assert_eq!(forecast.daily[0].minimum.value, 12.0);
    assert_eq!(forecast.daily[0].maximum.value, 24.0);
    assert_eq!(forecast.daily[0].day_phrase, "Sunny");
    assert_eq!(forecast.hourly.len(), 1);
    assert_eq!(forecast.hourly[0].icon_phrase, "Mostly cloudy");
    assert_eq!(forecast.hourly[0].precipitation_probability, Some(15));

    let alerts = client.alerts("349727").await.expect("alerts succeed");
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_id, "alert-1");
    assert_eq!(alerts[0].severity, "Severe");
    assert_eq!(alerts[0].areas, vec![String::from("Travis")]);

    let historical = client
        .historical_daily("349727", 24)
        .await
        .expect("historical data succeeds");
    assert_eq!(historical.len(), 1);
    assert_eq!(historical[0].minimum.value, 9.0);
    assert_eq!(historical[0].maximum.value, 26.0);
    assert_eq!(historical[0].observations, 2);

    // Every call carries the API key as the documented query parameter and no
    // endpoint outside the catalogue was requested.
    let requests = common::requests(&server).await;
    assert_eq!(requests.len(), 6);
    for request in &requests {
        assert!(
            common::query(request).contains("apikey=test_credential_do_not_log_1234"),
            "the API key travels as a query parameter: {}",
            request.url
        );
    }
}

#[tokio::test]
async fn ac_002_accuweather_failures_are_wirekit_errors() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/currentconditions/v1/349727",
        Fixture::json(
            401,
            json!({ "Code": "Unauthorized", "Message": "Invalid API key" }),
        ),
    )
    .await;

    let error = client(&server)
        .current_conditions("349727")
        .await
        .expect_err("a 401 is an error");
    assert!(matches!(error, WirekitError::Auth { .. }), "got {error:?}");
    assert_eq!(error.code(), "W2002");

    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/currentconditions/v1/349727",
        Fixture::raw(500, "<html>upstream failure</html>"),
    )
    .await;
    let error = client(&server)
        .current_conditions("349727")
        .await
        .expect_err("a 500 is an error");
    assert!(
        matches!(error, WirekitError::Api { status: 500, .. }),
        "got {error:?}"
    );
}

#[tokio::test]
async fn ac_003_accuweather_rejects_an_error_body_at_http_200() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/currentconditions/v1/349727",
        Fixture::json(
            200,
            json!({ "Code": "ServiceUnavailable", "Message": "Try later" }),
        ),
    )
    .await;

    let error = client(&server)
        .current_conditions("349727")
        .await
        .expect_err("a 200 with an error envelope is not a success");
    assert!(
        matches!(error, WirekitError::Api { status: 200, .. }),
        "got {error:?}"
    );
    assert_eq!(error.api_code(), Some("ServiceUnavailable"));
}

#[tokio::test]
async fn ac_004_accuweather_honours_retry_after_and_never_retries_another_4xx() {
    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "GET",
        "/currentconditions/v1/349727",
        Fixture::json(200, conditions_json())
            .first_status(429)
            .header("Retry-After", "1"),
    )
    .await;

    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let retrying = AccuWeatherClient::new(
        transport,
        AccuWeatherConfig::new(Secret::new(String::from(common::API_KEY))),
    )
    .expect("the client builds");

    let started = Instant::now();
    let conditions = retrying
        .current_conditions("349727")
        .await
        .expect("the retry succeeds");
    assert_eq!(conditions.weather_text, "Partly cloudy");
    assert_eq!(fixture.hits(), 2);
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "Retry-After: 1 must delay the retry, elapsed {:?}",
        started.elapsed()
    );

    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "GET",
        "/currentconditions/v1/349727",
        Fixture::json(
            400,
            json!({ "Code": "BadRequest", "Message": "The location key is malformed" }),
        ),
    )
    .await;
    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let plain = AccuWeatherClient::new(
        transport,
        AccuWeatherConfig::new(Secret::new(String::from(common::API_KEY))),
    )
    .expect("the client builds");
    let error = plain
        .current_conditions("349727")
        .await
        .expect_err("a 400 is an error");
    assert!(
        matches!(error, WirekitError::Api { status: 400, .. }),
        "got {error:?}"
    );
    assert_eq!(fixture.hits(), 1, "a 4xx other than 429 is not retried");
}

#[tokio::test]
async fn ac_005_accuweather_secret_is_redacted() {
    let config = AccuWeatherConfig::new(Secret::new(String::from(common::API_KEY)));
    let rendered = format!("{config:?}");
    assert!(rendered.contains("***"));
    assert!(!rendered.contains(common::API_KEY));
}

//! Acceptance tests for the NOAA provider (AC-043 to AC-049).

use std::time::Duration;

use serde_json::json;
use wirekit::noaa::{AlertQuery, ForecastRequest, NoaaClient};
use wirekit::{JitterBounds, RetryPolicy};

use crate::common;

const USER_AGENT: &str = "wirekit-tests/0.1 (qa@example.com)";

fn noaa(server: &wiremock::MockServer) -> NoaaClient {
    NoaaClient::builder(common::client(server))
        .user_agent(USER_AGENT)
        .build()
}

#[tokio::test]
async fn ac_043_a_point_forecast_returns_typed_periods_temperature_and_precipitation() {
    let server = common::server().await;
    let forecast_url = format!("{}/forecast", server.uri());
    let point = common::mount(
        &server,
        "GET",
        "/points/40.7128,-74.006",
        common::Recorder::json(200, json!({ "properties": { "forecast": forecast_url } })),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/forecast",
        common::Recorder::json(
            200,
            json!({
                "properties": {
                    "periods": [
                        {
                            "number": 1,
                            "name": "Today",
                            "isDaytime": true,
                            "temperature": 72,
                            "temperatureUnit": "F",
                            "windSpeed": "5 mph",
                            "shortForecast": "Sunny",
                            "detailedForecast": "Sunny with a light breeze.",
                            "probabilityOfPrecipitation": { "unitCode": "wmoUnit:percent", "value": 30 }
                        },
                        {
                            "number": 2,
                            "name": "Tonight",
                            "temperature": 55,
                            "temperatureUnit": "F"
                        }
                    ]
                }
            }),
        ),
    )
    .await;

    let request = ForecastRequest::new(40.7128, -74.006).expect("a valid coordinate pair");
    let response = noaa(&server)
        .forecast(&request)
        .await
        .expect("the forecast is returned");

    assert_eq!(response.periods.len(), 2);
    assert_eq!(response.temperature, Some(72.0));
    assert_eq!(response.precipitation, Some(30.0));
    assert_eq!(response.periods[0].name, "Today");
    assert_eq!(response.periods[0].temperature_unit.as_deref(), Some("F"));
    assert_eq!(response.periods[0].short_forecast.as_deref(), Some("Sunny"));
    assert_eq!(point.hits(), 1);
    assert_eq!(
        point.last().header("user-agent"),
        Some(USER_AGENT),
        "NOAA requires the descriptive User-Agent on every request"
    );

    assert!(
        ForecastRequest::new(91.0, 0.0).is_err(),
        "an out-of-range latitude is rejected"
    );
    assert!(
        ForecastRequest::new(0.0, 181.0).is_err(),
        "an out-of-range longitude is rejected"
    );
}

#[tokio::test]
async fn ac_044_active_alerts_return_typed_values() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/alerts/active",
        common::Recorder::json(
            200,
            json!({
                "features": [
                    {
                        "properties": {
                            "id": "urn:oid:2.49.0.1.840.0.1",
                            "event": "Tornado Warning",
                            "severity": "Extreme",
                            "certainty": "Observed",
                            "urgency": "Immediate",
                            "headline": "Tornado Warning issued for the area",
                            "description": "A confirmed tornado was reported.",
                            "instruction": "Take shelter now.",
                            "areaDesc": "Cook County; Lake County",
                            "onset": "2026-10-20T18:00:00-05:00",
                            "expires": "2026-10-20T19:00:00-05:00",
                            "status": "Actual"
                        }
                    }
                ]
            }),
        ),
    )
    .await;

    let query = AlertQuery::new()
        .area("TX")
        .zone("TXZ174")
        .severity("Severe")
        .limit(5);
    let alerts = noaa(&server)
        .alerts_active(&query)
        .await
        .expect("the alerts are returned");

    assert_eq!(alerts.len(), 1);
    let alert = &alerts[0];
    assert_eq!(alert.id, "urn:oid:2.49.0.1.840.0.1");
    assert_eq!(alert.event.as_deref(), Some("Tornado Warning"));
    assert_eq!(alert.severity.as_deref(), Some("Extreme"));
    assert_eq!(
        alert.areas,
        vec!["Cook County".to_string(), "Lake County".to_string()]
    );
    assert_eq!(
        alert.description.as_deref(),
        Some("A confirmed tornado was reported.")
    );
    assert!(alert.is_warning());
    let event = alert.to_event();
    assert_eq!(event.event, "Tornado Warning");
    assert_eq!(event.severity, "extreme");
    assert!(event.is_warning);

    let request = recorder.last();
    assert_eq!(request.query_param("area").as_deref(), Some("TX"));
    assert_eq!(request.query_param("zone").as_deref(), Some("TXZ174"));
    assert_eq!(request.query_param("severity").as_deref(), Some("Severe"));
    assert_eq!(request.query_param("limit").as_deref(), Some("5"));
}

#[tokio::test]
async fn ac_045_one_alert_is_fetched_by_id_and_an_unknown_id_is_an_api_error() {
    let server = common::server().await;
    let known = common::mount(
        &server,
        "GET",
        "/alerts/known",
        common::Recorder::json(
            200,
            json!({ "properties": { "id": "known", "event": "Flood Advisory" } }),
        ),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/alerts/missing",
        common::Recorder::json(
            404,
            json!({
                "type": "https://api.weather.gov/errors",
                "title": "Not Found",
                "status": 404,
                "detail": "The alert 'missing' does not exist."
            }),
        ),
    )
    .await;

    let client = noaa(&server);
    let alert = client
        .alert(" known ")
        .await
        .expect("the alert is returned");
    assert_eq!(alert.id, "known");
    // The mock server rebuilds the URL from the Host header, so assert the
    // path rather than the host.
    assert!(
        known.last().url.ends_with("/alerts/known"),
        "the alert path was requested: {}",
        known.last().url
    );

    let error = client
        .alert("missing")
        .await
        .expect_err("an unknown alert id fails");
    match error {
        wirekit::WirekitError::Api { status, .. } => assert_eq!(status, 404),
        other => panic!("expected Api, got {other:?}"),
    }

    let error = client
        .alert("   ")
        .await
        .expect_err("a blank id is invalid");
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_046_a_problem_json_body_maps_to_an_api_error() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/alerts/active",
        common::Recorder::json(
            500,
            json!({
                "type": "https://api.weather.gov/errors",
                "title": "Service Unavailable",
                "status": 500,
                "detail": "The upstream grid data is unavailable."
            }),
        )
        .header("content-type", "application/problem+json"),
    )
    .await;

    let error = noaa(&server)
        .alerts_active(&AlertQuery::new())
        .await
        .expect_err("a problem+json failure is not success");

    match error {
        wirekit::WirekitError::Api {
            status,
            ref code,
            ref message,
            ref details,
        } => {
            assert_eq!(status, 500);
            assert_eq!(
                code, "http_500",
                "no provider code exists, so the status is used"
            );
            assert_eq!(message, "The upstream grid data is unavailable.");
            let details = details
                .as_ref()
                .expect("the parsed problem document is kept");
            assert_eq!(details["title"], json!("Service Unavailable"));
        }
        other => panic!("expected Api, got {other:?}"),
    }
}

#[tokio::test]
async fn ac_047_a_missing_user_agent_is_rejected_before_any_call() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/points/1,1",
        common::Recorder::json(200, json!({})),
    )
    .await;
    let recorder_alerts = common::mount(
        &server,
        "GET",
        "/alerts/active",
        common::Recorder::json(200, json!({ "features": [] })),
    )
    .await;

    let anonymous = NoaaClient::builder(common::client(&server)).build();
    let blank = NoaaClient::builder(common::client(&server))
        .user_agent("   ")
        .build();

    let request = ForecastRequest::new(1.0, 1.0).expect("a valid coordinate pair");
    for client in [&anonymous, &blank] {
        assert_eq!(
            client
                .forecast(&request)
                .await
                .expect_err("a missing User-Agent is rejected")
                .code(),
            "W2006"
        );
        assert_eq!(
            client
                .alerts_active(&AlertQuery::new())
                .await
                .expect_err("a missing User-Agent is rejected")
                .code(),
            "W2006"
        );
    }

    assert_eq!(recorder.hits(), 0, "no forecast request was sent");
    assert_eq!(recorder_alerts.hits(), 0, "no alerts request was sent");
}

#[tokio::test]
async fn ac_048_a_repeated_request_within_the_ttl_is_served_from_cache() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/alerts/active",
        common::Recorder::json(
            200,
            json!({ "features": [{ "properties": { "id": "cached", "event": "Flood Watch" } }] }),
        ),
    )
    .await;

    let client = noaa(&server);
    let first = client
        .alerts_active(&AlertQuery::new().area("TX"))
        .await
        .expect("the first call succeeds");
    let second = client
        .alerts_active(&AlertQuery::new().area("TX"))
        .await
        .expect("the second call succeeds");

    assert_eq!(first.len(), 1);
    assert_eq!(second[0].id, "cached");
    assert_eq!(recorder.hits(), 1, "the second call is served from cache");

    client.clear_cache();
    client
        .alerts_active(&AlertQuery::new().area("TX"))
        .await
        .expect("the third call succeeds");
    assert_eq!(recorder.hits(), 2, "clearing the cache forces a new call");

    let uncached = NoaaClient::builder(common::client(&server))
        .user_agent(USER_AGENT)
        .disable_cache()
        .build();
    uncached
        .alerts_active(&AlertQuery::new().area("TX"))
        .await
        .expect("the call succeeds");
    uncached
        .alerts_active(&AlertQuery::new().area("TX"))
        .await
        .expect("the call succeeds");
    assert_eq!(recorder.hits(), 4, "a disabled cache sends every request");
}

#[tokio::test]
async fn ac_049_noaa_retries_share_the_backoff_policy_and_honour_retry_after() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    let limited = common::mount(
        &server,
        "GET",
        "/alerts/active",
        common::Recorder::json(429, json!({ "detail": "slow down" })).header("retry-after", "1"),
    )
    .await;
    let unavailable = common::mount(
        &server,
        "GET",
        "/radar/stations/KTLX",
        common::Recorder::json(503, json!({ "detail": "maintenance" })),
    )
    .await;

    let client = NoaaClient::builder(
        common::builder(&server)
            .retry(RetryPolicy {
                attempts: 2,
                base_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(5),
                jitter: JitterBounds::new(1.0, 1.0).expect("valid jitter bounds"),
            })
            .build()
            .expect("the client builds"),
    )
    .user_agent(USER_AGENT)
    .build();

    let error = client
        .alerts_active(&AlertQuery::new())
        .await
        .expect_err("the 429 is not hidden");
    assert_eq!(error.code(), "W2003");
    assert_eq!(limited.hits(), 2, "the shared policy retried once");
    let retry = capture
        .find("retrying request")
        .expect("the retry is traced");
    assert_eq!(
        retry.field("delay_ms"),
        Some("5"),
        "the Retry-After value is capped by max_delay"
    );

    let error = client
        .radar_metadata("KTLX")
        .await
        .expect_err("the 503 is not hidden");
    assert_eq!(error.code(), "W2001");
    assert_eq!(unavailable.hits(), 2, "a 5xx is retried once");
}

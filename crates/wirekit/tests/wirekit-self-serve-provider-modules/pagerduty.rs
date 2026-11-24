//! Acceptance tests for the PagerDuty module (AC-001 to AC-005).

use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::Sha256;
use wirekit::pagerduty::{
    IncidentEvent, IncidentQuery, IncidentStatus, PagerDutyClient, PagerDutyConfig, Severity,
    Urgency,
};
use wirekit::{Secret, WirekitError};
use wiremock::MockServer;

use crate::common::{self, Fixture};

fn config(server: &MockServer) -> PagerDutyConfig {
    PagerDutyConfig::new(
        Secret::new(String::from(common::API_KEY)),
        Secret::new(String::from("routing_key_do_not_log")),
    )
    .with_events_base_url(server.uri())
    .with_rest_base_url(server.uri())
    .with_from_email("ops@example.com")
    .with_webhook_secret(Secret::new(String::from(common::WEBHOOK_SECRET)))
}

fn client(server: &MockServer) -> PagerDutyClient {
    PagerDutyClient::new(common::transport(server), config(server)).expect("the client builds")
}

fn event_ack() -> serde_json::Value {
    json!({
        "status": "success",
        "message": "Event processed",
        "dedup_key": "escalation-42"
    })
}

fn incident_json() -> serde_json::Value {
    json!({
        "incidents": [{
            "id": "P1",
            "incident_number": 7,
            "title": "Roof crew hazard",
            "status": "triggered",
            "urgency": "high",
            "created_at": "2026-11-24T09:00:00Z",
            "html_url": "https://example.pagerduty.com/incidents/P1",
            "escalation_policy": {
                "id": "EP1",
                "summary": "Default",
                "html_url": "https://example.pagerduty.com/escalation_policies/EP1"
            },
            "alerts": [{
                "id": "A1",
                "status": "triggered",
                "summary": "Hazard reported",
                "created_at": "2026-11-24T09:00:00Z",
                "html_url": "https://example.pagerduty.com/alerts/A1"
            }]
        }],
        "more": false,
        "limit": 25,
        "offset": 0
    })
}

fn signature(secret: &str, body: &[u8]) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).expect("a valid key");
    mac.update(body);
    let bytes = mac.finalize().into_bytes();
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("v1={hex}")
}

#[tokio::test]
async fn ac_001_pagerduty_surface_matches_the_catalogue() {
    let server = common::server().await;
    common::mount(&server, "POST", "/enqueue", Fixture::json(202, event_ack())).await;
    common::mount(
        &server,
        "GET",
        "/incidents",
        Fixture::json(200, incident_json()),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/oncalls",
        Fixture::json(
            200,
            json!({
                "oncalls": [{
                    "id": "OC1",
                    "escalation_policy": { "id": "EP1", "summary": "Default" },
                    "schedule": { "id": "S1" },
                    "user": { "id": "U1" },
                    "start": "2026-11-24T08:00:00Z",
                    "end": "2026-11-24T20:00:00Z"
                }]
            }),
        ),
    )
    .await;

    let client = client(&server);

    let event = IncidentEvent::new("Roof crew hazard reported", "storm-blast")
        .with_severity(Severity::Critical)
        .with_dedup_key("escalation-42")
        .with_component("dispatch")
        .with_group("field-ops")
        .with_class("safety")
        .with_custom_details(json!({ "ticket": "T-9" }));
    let ack = client.trigger(&event).await.expect("the trigger succeeds");
    assert_eq!(ack.status, "success");
    assert_eq!(ack.dedup_key.as_deref(), Some("escalation-42"));

    let ack = client
        .acknowledge("escalation-42")
        .await
        .expect("the acknowledgement succeeds");
    assert_eq!(ack.status, "success");

    let ack = client
        .resolve("escalation-42")
        .await
        .expect("the resolution succeeds");
    assert_eq!(ack.status, "success");

    let page = client
        .list_incidents(
            &IncidentQuery::new()
                .with_status(IncidentStatus::Triggered)
                .with_urgency(Urgency::High)
                .with_limit(25),
        )
        .await
        .expect("the incident list succeeds");
    assert_eq!(page.incidents.len(), 1);
    assert_eq!(page.incidents[0].id, "P1");
    assert_eq!(page.incidents[0].incident_number, Some(7));
    assert_eq!(page.incidents[0].status.as_deref(), Some("triggered"));
    assert_eq!(
        page.incidents[0]
            .escalation_policy
            .as_ref()
            .map(|policy| policy.summary.as_deref()),
        Some(Some("Default"))
    );
    assert_eq!(page.incidents[0].alerts[0].id, "A1");
    assert!(!page.more);

    let on_calls = client
        .on_calls(Some("S1"))
        .await
        .expect("the on-call lookup succeeds");
    assert_eq!(on_calls.len(), 1);
    assert_eq!(on_calls[0].id, "OC1");
    assert_eq!(on_calls[0].schedule_id.as_deref(), Some("S1"));
    assert_eq!(on_calls[0].user_id.as_deref(), Some("U1"));
    assert_eq!(
        on_calls[0]
            .escalation_policy
            .as_ref()
            .map(|policy| policy.id.as_str()),
        Some("EP1")
    );

    // Webhook verification over the exact bytes.
    let raw = br#"{"event":{"id":"evt-1"}}"#;
    client
        .verify_webhook(raw, &signature(common::WEBHOOK_SECRET, raw))
        .expect("a valid signature verifies");
    let mismatch = client
        .verify_webhook(raw, "v1=deadbeef")
        .expect_err("a wrong signature fails");
    assert!(matches!(mismatch, WirekitError::Auth { .. }));

    let requests = common::requests(&server).await;
    assert_eq!(requests.len(), 5);

    let trigger = &requests[0];
    let body = common::json(trigger);
    assert_eq!(body["routing_key"], "routing_key_do_not_log");
    assert_eq!(body["event_action"], "trigger");
    assert_eq!(body["dedup_key"], "escalation-42");
    assert_eq!(body["payload"]["summary"], "Roof crew hazard reported");
    assert_eq!(body["payload"]["source"], "storm-blast");
    assert_eq!(body["payload"]["severity"], "critical");
    assert_eq!(body["payload"]["component"], "dispatch");
    assert_eq!(body["payload"]["custom_details"]["ticket"], "T-9");

    let acknowledge = common::json(&requests[1]);
    assert_eq!(acknowledge["event_action"], "acknowledge");
    assert_eq!(acknowledge["dedup_key"], "escalation-42");
    assert!(acknowledge.get("payload").is_none());
    let resolve = common::json(&requests[2]);
    assert_eq!(resolve["event_action"], "resolve");

    let list = &requests[3];
    assert_eq!(
        common::header(list, "authorization"),
        Some("Token token=test_credential_do_not_log_1234")
    );
    assert_eq!(common::header(list, "from"), Some("ops@example.com"));
    assert_eq!(
        common::header(list, "accept"),
        Some("application/vnd.pagerduty+json;version=2")
    );
    assert!(common::query(list).contains("statuses%5B%5D=triggered"));
    assert!(common::query(list).contains("urgencies%5B%5D=high"));
    assert!(common::query(list).contains("limit=25"));

    assert!(common::query(&requests[4]).contains("earliest=true"));
    assert!(common::query(&requests[4]).contains("schedule_ids%5B%5D=S1"));
}

#[tokio::test]
async fn ac_002_pagerduty_failures_are_wirekit_errors() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/enqueue",
        Fixture::json(
            400,
            json!({
                "status": "invalid event",
                "message": "Event object is invalid",
                "errors": ["payload is missing"]
            }),
        ),
    )
    .await;

    let error = client(&server)
        .trigger(&IncidentEvent::new("summary", "source"))
        .await
        .expect_err("a malformed event is an error");
    assert!(
        matches!(error, WirekitError::Api { status: 400, .. }),
        "got {error:?}"
    );
    assert!(
        format!("{error}").contains("Event object is invalid"),
        "the provider text is surfaced: {error}"
    );

    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/incidents",
        Fixture::json(
            401,
            json!({ "error": { "message": "Unauthorized", "code": 2006 } }),
        ),
    )
    .await;
    let error = client(&server)
        .list_incidents(&IncidentQuery::new())
        .await
        .expect_err("a 401 is an error");
    assert!(matches!(error, WirekitError::Auth { .. }), "got {error:?}");
}

#[tokio::test]
async fn ac_003_pagerduty_rejects_a_body_level_error_at_http_200() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/enqueue",
        Fixture::json(
            200,
            json!({ "error": { "message": "Dedup key invalid", "code": 2001 } }),
        ),
    )
    .await;
    let error = client(&server)
        .trigger(&IncidentEvent::new("summary", "source"))
        .await
        .expect_err("a 200 with error.message is not a success");
    assert!(
        matches!(error, WirekitError::Api { status: 200, .. }),
        "got {error:?}"
    );
    assert_eq!(error.api_code(), Some("2001"));

    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/enqueue",
        Fixture::json(
            200,
            json!({ "status": "invalid event", "message": "Payload is invalid" }),
        ),
    )
    .await;
    let error = client(&server)
        .trigger(&IncidentEvent::new("summary", "source"))
        .await
        .expect_err("a 200 with an invalid-event status is not a success");
    assert!(
        matches!(error, WirekitError::Api { status: 200, .. }),
        "got {error:?}"
    );
    assert_eq!(error.api_code(), Some("invalid_event"));
}

#[tokio::test]
async fn ac_004_pagerduty_honours_retry_after() {
    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "POST",
        "/enqueue",
        Fixture::json(202, event_ack())
            .first_status(429)
            .header("Retry-After", "1"),
    )
    .await;

    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let client = PagerDutyClient::new(transport, config(&server)).expect("the client builds");

    let started = Instant::now();
    let ack = client
        .trigger(&IncidentEvent::new("summary", "source").with_dedup_key("dedup-1"))
        .await
        .expect("the retry succeeds");
    assert_eq!(ack.status, "success");
    assert_eq!(fixture.hits(), 2);
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "Retry-After: 1 must delay the retry, elapsed {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn ac_005_pagerduty_secrets_are_redacted() {
    let server = common::server().await;
    let rendered = format!("{:?}", config(&server));
    assert!(rendered.contains("***"));
    assert!(!rendered.contains(common::API_KEY));
    assert!(!rendered.contains("routing_key_do_not_log"));
    assert!(!rendered.contains(common::WEBHOOK_SECRET));

    // A rejected credential never echoes the secret.
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/incidents",
        Fixture::json(401, json!({ "error": { "message": "Unauthorized" } })),
    )
    .await;
    let error = client(&server)
        .list_incidents(&IncidentQuery::new())
        .await
        .expect_err("a 401 is an error");
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains(common::API_KEY));
}

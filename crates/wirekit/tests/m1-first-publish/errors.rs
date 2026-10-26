//! Acceptance tests for `WirekitError` (AC-001 to AC-005).

use std::error::Error as _;

use serde_json::json;
use wirekit::WirekitError;

use crate::common;

/// A cause whose `Debug` renders a URL carrying a credential, so the
/// error `Debug` implementation can be checked for redaction.
#[derive(Debug)]
struct SecretiveCause(
    // The payload is deliberately only observable through `Debug`, which is
    // what the redaction assertions inspect.
    #[allow(dead_code)] &'static str,
);

impl std::fmt::Display for SecretiveCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the underlying transport failed")
    }
}

impl std::error::Error for SecretiveCause {}

#[tokio::test]
async fn ac_001_non_2xx_maps_to_api_with_status_code_and_message() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/v1/items",
        common::Recorder::json(
            503,
            json!({ "code": "service_unavailable", "message": "try again later" }),
        ),
    )
    .await;

    let error = common::client(&server)
        .get_json::<serde_json::Value>("v1/items")
        .await
        .expect_err("a 503 must not be reported as success");

    match error {
        WirekitError::Api {
            status,
            ref code,
            ref message,
            ref details,
        } => {
            assert_eq!(status, 503);
            assert_eq!(code, "service_unavailable");
            assert_eq!(message, "try again later");
            assert_eq!(
                details.as_ref().and_then(|value| value.get("code")),
                Some(&json!("service_unavailable")),
                "the provider payload is retained in details"
            );
        }
        other => panic!("expected WirekitError::Api, got {other:?}"),
    }
}

#[test]
fn ac_002_every_variant_reports_a_stable_w_code_in_debug_and_display() {
    let cases = [
        (WirekitError::api(400, "bad_request", "nope"), "W2001"),
        (WirekitError::auth("rejected"), "W2002"),
        (
            WirekitError::RateLimit {
                retry_after: None,
            },
            "W2003",
        ),
        (WirekitError::Timeout, "W2004"),
        (WirekitError::network("unreachable"), "W2005"),
        (WirekitError::invalid_config("no token"), "W2006"),
        (WirekitError::decode("not json"), "W2007"),
    ];

    for (error, expected) in cases {
        let code = error.code();
        assert_eq!(code, expected);
        assert_eq!(code.len(), 5, "{code} must be W followed by four digits");
        assert!(code.starts_with('W'));
        assert!(
            code[1..].chars().all(|character| character.is_ascii_digit()),
            "{code} must be W followed by four digits"
        );
        assert!(
            error.to_string().contains(code),
            "Display for {code} was {}",
            error
        );
        assert!(
            !format!("{error:?}").is_empty(),
            "Debug must render something for {code}"
        );
    }
}

#[test]
fn ac_003_debug_and_display_never_render_a_credential_or_a_body() {
    let secret_url = "http://api.example.com/v1?api_key=sk_live_SECRET";
    let network = WirekitError::network_with("could not connect", SecretiveCause(secret_url));
    for rendered in [format!("{network}"), format!("{network:?}")] {
        assert!(
            !rendered.contains("sk_live_SECRET"),
            "a wrapped cause leaked into the rendered error: {rendered}"
        );
        assert!(!rendered.contains("api_key"), "a URL leaked: {rendered}");
    }

    let decode = WirekitError::decode_with("the body did not match", SecretiveCause("RAW_BODY"));
    for rendered in [format!("{decode}"), format!("{decode:?}")] {
        assert!(
            !rendered.contains("RAW_BODY"),
            "a raw body leaked into the rendered error: {rendered}"
        );
    }

    // A provider message is rendered, but the credential is never part of it:
    // the pipeline only stores what the provider reported.
    let api = WirekitError::api_from_body(401, br#"{"code":"unauthorized","message":"invalid token"}"#);
    let rendered = format!("{api:?}");
    assert!(rendered.contains("unauthorized"));
    assert!(!rendered.contains("Bearer"));
}

#[test]
fn ac_004_the_enum_is_non_exhaustive_so_a_wildcard_arm_compiles() {
    fn classify(error: &WirekitError) -> &'static str {
        match error {
            WirekitError::Api { .. } => "api",
            WirekitError::Auth { .. } => "auth",
            WirekitError::RateLimit { .. } => "rate-limit",
            WirekitError::Timeout => "timeout",
            WirekitError::Network { .. } => "network",
            WirekitError::InvalidConfig { .. } => "invalid-config",
            WirekitError::Decode { .. } => "decode",
            _ => "unknown",
        }
    }

    assert_eq!(classify(&WirekitError::api(500, "boom", "boom")), "api");
    assert_eq!(classify(&WirekitError::Timeout), "timeout");
    assert_eq!(classify(&WirekitError::network("x")), "network");
}

#[test]
fn ac_005_error_trait_and_source_are_available() {
    fn fail() -> Result<serde_json::Value, WirekitError> {
        Err(WirekitError::decode("not json"))
    }

    fn propagate() -> Result<(), Box<dyn std::error::Error>> {
        // The `?` operator must convert `WirekitError` into the boxed trait
        // object, which proves the `std::error::Error` implementation.
        let _ = fail()?;
        Ok(())
    }

    let error = propagate().expect_err("the decode error must propagate");
    assert!(
        error.to_string().contains("W2007"),
        "the boxed message was {error}"
    );

    let boxed: Box<dyn std::error::Error> = Box::new(WirekitError::auth("rejected"));
    assert!(boxed.source().is_none());

    let with_cause = WirekitError::decode_with("the body did not match", SecretiveCause("cause"));
    let source = with_cause.source().expect("the wrapped cause is exposed");
    assert_eq!(source.to_string(), "the underlying transport failed");

    let without_cause = WirekitError::network("unreachable");
    assert!(without_cause.source().is_none());
}

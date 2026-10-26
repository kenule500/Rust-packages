//! Acceptance tests for the Twilio provider (AC-050 to AC-058).
//!
//! The Verify and Lookups hosts are absolute HTTPS URLs inside the provider,
//! so a loopback mock cannot intercept them. AC-054 and AC-055 therefore
//! assert the input validation plus the typed result models, which is
//! recorded as a deliberate coverage gap in the test report.

use std::time::{Duration, Instant};

use serde_json::json;
use wirekit::twilio::{
    CallRequest, LookupResult, MfaChallenge, MfaChallengeResponse, MfaChannel, MfaVerification,
    MfaVerificationResponse, SendSmsRequest, SendWhatsAppRequest, TwilioClient,
};
use wirekit::RateLimitConfig;

use crate::common;

const ACCOUNT_SID: &str = "AC123";
const AUTH_TOKEN: &str = "secret";
const MESSAGES_PATH: &str = "/Accounts/AC123/Messages.json";

fn twilio(server: &wiremock::MockServer) -> TwilioClient {
    TwilioClient::builder(common::client(server))
        .account_sid(ACCOUNT_SID)
        .auth_token(AUTH_TOKEN)
        .build()
        .expect("the Twilio client builds")
}

#[tokio::test]
async fn ac_050_account_credentials_authenticate_every_request() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        MESSAGES_PATH,
        common::Recorder::json(200, json!({ "sid": "SM1", "status": "queued" })),
    )
    .await;

    twilio(&server)
        .send_sms(&SendSmsRequest::new("+15550002222", "+15550001111", "hello"))
        .await
        .expect("the message is accepted");

    assert_eq!(
        recorder.last().header("authorization").map(str::to_string),
        Some(format!("Basic {}", common::TWILIO_BASIC_AUTH))
    );

    // The API-key pair is sent the same way, with the key SID as the username.
    let keyed_server = common::server().await;
    let keyed = common::mount(
        &keyed_server,
        "POST",
        MESSAGES_PATH,
        common::Recorder::json(200, json!({ "sid": "SM2", "status": "queued" })),
    )
    .await;
    let client = TwilioClient::builder(common::client(&keyed_server))
        .account_sid(ACCOUNT_SID)
        .api_key("SK123", "secret2")
        .build()
        .expect("the API-key client builds");
    client
        .send_sms(&SendSmsRequest::new("+15550002222", "+15550001111", "hello"))
        .await
        .expect("the message is accepted");
    assert_eq!(
        keyed.last().header("authorization").map(str::to_string),
        Some(format!("Basic {}", common::TWILIO_API_KEY_AUTH))
    );

    let missing_credential = TwilioClient::builder(common::client(&server))
        .account_sid(ACCOUNT_SID)
        .build()
        .err()
        .expect("a credential is required");
    assert_eq!(missing_credential.code(), "W2006");
    assert!(missing_credential.to_string().contains("auth token"));

    let missing_sid = TwilioClient::builder(common::client(&server))
        .auth_token(AUTH_TOKEN)
        .build()
        .err()
        .expect("an account SID is required");
    assert_eq!(missing_sid.code(), "W2006");
}

#[tokio::test]
async fn ac_051_send_sms_posts_a_form_and_returns_the_message() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        MESSAGES_PATH,
        common::Recorder::json(
            201,
            json!({
                "sid": "SM123",
                "status": "queued",
                "to": "+15550002222",
                "from": "+15550001111",
                "body": "hello",
                "num_segments": "1"
            }),
        ),
    )
    .await;

    let message = twilio(&server)
        .send_sms(
            &SendSmsRequest::new("+15550002222", "+15550001111", "hello")
                .status_callback("https://example.com/status"),
        )
        .await
        .expect("the message is accepted");

    assert_eq!(message.sid, "SM123");
    assert_eq!(message.status, "queued");
    assert_eq!(message.num_segments.as_deref(), Some("1"));

    let request = recorder.last();
    assert_eq!(
        request.method, "POST",
        "the message endpoint is posted to"
    );
    assert_eq!(
        request.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    let body = request.text();
    assert!(body.contains("To=%2B15550002222"), "body was {body}");
    assert!(body.contains("From=%2B15550001111"), "body was {body}");
    assert!(body.contains("Body=hello"), "body was {body}");
    assert!(
        body.contains("StatusCallback=https%3A%2F%2Fexample.com%2Fstatus"),
        "body was {body}"
    );

    let error = twilio(&server)
        .send_sms(&SendSmsRequest::new("", "+15550001111", "hello"))
        .await
        .expect_err("a destination is required");
    assert_eq!(error.code(), "W2006");

    let mut senderless = SendSmsRequest::new("+15550002222", "+15550001111", "hello");
    senderless.from = None;
    let error = twilio(&server)
        .send_sms(&senderless)
        .await
        .expect_err("a sender is required");
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_052_whatsapp_messages_use_the_whatsapp_sender() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        MESSAGES_PATH,
        common::Recorder::json(200, json!({ "sid": "SM_WHATSAPP", "status": "queued" })),
    )
    .await;

    let message = twilio(&server)
        .send_whatsapp(&SendWhatsAppRequest::new(
            "+15550002222",
            "whatsapp:+15550001111",
            "hello from WhatsApp",
        ))
        .await
        .expect("the message is accepted");

    assert_eq!(message.sid, "SM_WHATSAPP");
    let body = recorder.last().text();
    assert!(
        body.contains("To=whatsapp%3A%2B15550002222"),
        "the destination is prefixed: {body}"
    );
    assert!(
        body.contains("From=whatsapp%3A%2B15550001111"),
        "an existing prefix is not doubled: {body}"
    );
    assert!(body.contains("Body=hello+from+WhatsApp"), "body was {body}");
}

#[tokio::test]
async fn ac_053_calls_are_created_with_twiml_and_status_is_fetchable() {
    let server = common::server().await;
    let calls = common::mount(
        &server,
        "POST",
        "/Accounts/AC123/Calls.json",
        common::Recorder::json(
            201,
            json!({ "sid": "CA1", "status": "queued", "to": "+1555", "from": "+1666" }),
        ),
    )
    .await;
    let status = common::mount(
        &server,
        "GET",
        "/Accounts/AC123/Calls/CA1.json",
        common::Recorder::json(
            200,
            json!({ "sid": "CA1", "status": "in-progress", "duration": "12" }),
        ),
    )
    .await;

    let client = twilio(&server);
    let created = client
        .place_call(&CallRequest::new(
            "+15550002222",
            "+15550001111",
            "<Response><Say>hello</Say></Response>",
        ))
        .await
        .expect("the call is created");
    assert_eq!(created.sid, "CA1");
    let body = calls.last().text();
    assert!(body.contains("Twiml="), "the TwiML is posted: {body}");

    let current = client.call_status(" CA1 ").await.expect("the status is read");
    assert_eq!(current.sid, "CA1");
    assert_eq!(current.status, "in-progress");
    assert_eq!(current.duration.as_deref(), Some("12"));
    assert_eq!(status.hits(), 1);

    let error = client.call_status("").await.expect_err("a SID is required");
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_054_mfa_challenges_and_verifications_are_typed() {
    let server = common::server().await;
    let client = twilio(&server);

    let error = client
        .start_verification(&MfaChallenge::new("", "+15550002222", MfaChannel::Sms))
        .await
        .expect_err("a Verify service SID is required");
    assert_eq!(error.code(), "W2006");

    let error = client
        .start_verification(&MfaChallenge::new("VA1", "  ", MfaChannel::Sms))
        .await
        .expect_err("a destination is required");
    assert_eq!(error.code(), "W2006");

    let error = client
        .check_verification(&MfaVerification::new("VA1", "+15550002222", ""))
        .await
        .expect_err("a code is required");
    assert_eq!(error.code(), "W2006");

    assert_eq!(MfaChannel::Sms.as_str(), "sms");
    assert_eq!(MfaChannel::Call.as_str(), "call");
    assert_eq!(MfaChannel::Email.as_str(), "email");
    assert_eq!(MfaChannel::WhatsApp.as_str(), "whatsapp");

    // The typed results Twilio's Verify v2 returns.
    let challenge: MfaChallengeResponse = serde_json::from_str(
        r#"{"sid":"VE1","status":"pending","to":"+15550002222","channel":"sms"}"#,
    )
    .expect("the challenge response decodes");
    assert_eq!(challenge.sid, "VE1");
    assert_eq!(challenge.status, "pending");
    assert_eq!(challenge.channel.as_deref(), Some("sms"));

    let verification: MfaVerificationResponse =
        serde_json::from_str(r#"{"status":"approved","valid":true,"to":"+15550002222"}"#)
            .expect("the verification response decodes");
    assert_eq!(verification.status, "approved");
    assert!(verification.valid);
}

#[tokio::test]
async fn ac_055_a_lookup_returns_typed_validation_data() {
    let server = common::server().await;
    let client = twilio(&server);

    let error = client
        .lookup("   ", &[])
        .await
        .expect_err("a phone number is required");
    assert_eq!(error.code(), "W2006");

    let result: LookupResult = serde_json::from_str(
        r#"{
            "phone_number": "+15550002222",
            "valid": true,
            "country_code": "US",
            "national_format": "(555) 000-2222",
            "line_type": "mobile"
        }"#,
    )
    .expect("the lookup result decodes");

    assert_eq!(result.phone_number, "+15550002222");
    assert_eq!(result.valid, Some(true));
    assert_eq!(result.country_code.as_deref(), Some("US"));
    assert_eq!(result.national_format.as_deref(), Some("(555) 000-2222"));
    assert_eq!(result.line_type.as_deref(), Some("mobile"));
}

#[tokio::test]
async fn ac_056_provider_errors_map_to_api_and_rejected_credentials_to_auth() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        MESSAGES_PATH,
        common::Recorder::json(
            400,
            json!({
                "code": 21211,
                "message": "The 'To' number is not a valid phone number.",
                "status": 400,
                "more_info": "https://www.twilio.com/docs/errors/21211"
            }),
        ),
    )
    .await;

    let error = twilio(&server)
        .send_sms(&SendSmsRequest::new("+1555", "+1666", "hello"))
        .await
        .expect_err("a 400 is a failure");
    match error {
        wirekit::WirekitError::Api {
            status,
            ref code,
            ref message,
            ..
        } => {
            assert_eq!(status, 400);
            assert_eq!(code, "21211");
            assert!(message.contains("not a valid phone number"));
        }
        other => panic!("expected Api, got {other:?}"),
    }

    let auth_server = common::server().await;
    common::mount(
        &auth_server,
        "POST",
        MESSAGES_PATH,
        common::Recorder::json(
            401,
            json!({ "code": 20003, "message": "Authenticate", "status": 401 }),
        ),
    )
    .await;
    let error = twilio(&auth_server)
        .send_sms(&SendSmsRequest::new("+1555", "+1666", "hello"))
        .await
        .expect_err("a 401 is a failure");
    assert_eq!(error.code(), "W2002");
    assert!(error.is_auth());

    let forbidden_server = common::server().await;
    common::mount(
        &forbidden_server,
        "POST",
        MESSAGES_PATH,
        common::Recorder::json(
            403,
            json!({ "code": 20003, "message": "Authentication Error", "status": 403 }),
        ),
    )
    .await;
    let error = twilio(&forbidden_server)
        .send_sms(&SendSmsRequest::new("+1555", "+1666", "hello"))
        .await
        .expect_err("a credential rejection is a failure");
    assert_eq!(error.code(), "W2002");
}

#[tokio::test]
async fn ac_057_webhook_signatures_are_computed_and_compared_in_constant_time() {
    let server = common::server().await;
    // The fixture signature was produced with Twilio's documented token, so
    // the client under test uses the same token.
    let client = TwilioClient::builder(common::client(&server))
        .account_sid(ACCOUNT_SID)
        .auth_token(common::TWILIO_TOKEN)
        .build()
        .expect("the Twilio client builds");

    let computed = client
        .compute_webhook_signature(common::TWILIO_WEBHOOK_URL, common::TWILIO_WEBHOOK_BODY.as_bytes())
        .expect("the signature is computed");
    assert_eq!(
        computed,
        common::TWILIO_WEBHOOK_SIGNATURE,
        "the signature matches the fixture produced outside the crate"
    );

    client
        .verify_webhook(
            common::TWILIO_WEBHOOK_URL,
            common::TWILIO_WEBHOOK_BODY.as_bytes(),
            common::TWILIO_WEBHOOK_SIGNATURE,
        )
        .expect("the fixture signature verifies");

    let error = client
        .verify_webhook(
            common::TWILIO_WEBHOOK_URL,
            b"CallSid=CA999&From=%2B15550001111&To=%2B15550002222",
            common::TWILIO_WEBHOOK_SIGNATURE,
        )
        .expect_err("a tampered body must fail");
    assert_eq!(error.code(), "W2002");

    let error = client
        .verify_webhook(
            "https://example.com/other",
            common::TWILIO_WEBHOOK_BODY.as_bytes(),
            common::TWILIO_WEBHOOK_SIGNATURE,
        )
        .expect_err("a different URL must fail");
    assert_eq!(error.code(), "W2002");

    let keyed = TwilioClient::builder(common::client(&server))
        .account_sid(ACCOUNT_SID)
        .api_key("SK123", "secret2")
        .build()
        .expect("the API-key client builds");
    let error = keyed
        .verify_webhook(
            common::TWILIO_WEBHOOK_URL,
            common::TWILIO_WEBHOOK_BODY.as_bytes(),
            common::TWILIO_WEBHOOK_SIGNATURE,
        )
        .expect_err("webhook verification needs the auth token");
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_058_sends_are_throttled_per_account() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        MESSAGES_PATH,
        common::Recorder::json(200, json!({ "sid": "SM1", "status": "queued" })),
    )
    .await;

    let throttled = TwilioClient::builder(
        common::builder(&server)
            .rate_limit_for(
                "twilio:AC123",
                RateLimitConfig::new(1, 5.0).expect("a valid bucket"),
            )
            .build()
            .expect("the client builds"),
    )
    .account_sid(ACCOUNT_SID)
    .auth_token(AUTH_TOKEN)
    .build()
    .expect("the Twilio client builds");

    let started = Instant::now();
    for _ in 0..3 {
        throttled
            .send_sms(&SendSmsRequest::new("+1555", "+1666", "hello"))
            .await
            .expect("the message is accepted");
    }
    let elapsed = started.elapsed();
    assert_eq!(recorder.hits(), 3);
    assert!(
        elapsed >= Duration::from_millis(350),
        "the per-account bucket throttles the sends: {elapsed:?}"
    );

    let rate_events = capture.matching("rate limit hit; waiting for a token");
    assert_eq!(rate_events.len(), 2, "each wait is traced");
    for event in rate_events {
        assert_eq!(event.field("capacity"), Some("1"));
        assert!(
            event.field("wait_ms").is_some(),
            "the traced event reports the wait"
        );
    }

    // A different account has its own bucket, so it is not throttled by
    // AC123's override.
    let other_server = common::server().await;
    let other = common::mount(
        &other_server,
        "POST",
        MESSAGES_PATH,
        common::Recorder::json(200, json!({ "sid": "SM2", "status": "queued" })),
    )
    .await;
    let unthrottled = TwilioClient::builder(
        common::builder(&other_server)
            .rate_limit_for(
                "twilio:AC999",
                RateLimitConfig::new(1, 1.0).expect("a valid bucket"),
            )
            .build()
            .expect("the client builds"),
    )
    .account_sid(ACCOUNT_SID)
    .auth_token(AUTH_TOKEN)
    .build()
    .expect("the Twilio client builds");

    let started = Instant::now();
    for _ in 0..3 {
        unthrottled
            .send_sms(&SendSmsRequest::new("+1555", "+1666", "hello"))
            .await
            .expect("the message is accepted");
    }
    assert!(
        started.elapsed() < Duration::from_millis(300),
        "an override for another account must not apply"
    );
    assert_eq!(other.hits(), 3);
}


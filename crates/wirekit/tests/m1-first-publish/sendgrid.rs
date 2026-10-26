//! Acceptance tests for the SendGrid provider (AC-059 to AC-066).
//!
//! AC-066 requires base64 *and* URL attachments; the provider only models
//! base64 content, so the URL half is recorded as a divergence in the test
//! report rather than asserted here.

use serde_json::json;
use wirekit::sendgrid::{
    verify_event, EmailAddress, EmailAttachment, SendEmailRequest, SendGridClient, SendGridConfig,
    SendGridEvent, MAX_ATTACHMENT_BYTES,
};

use crate::common;

const API_KEY: &str = "SG.api_KEY_VALUE";

fn sendgrid(server: &wiremock::MockServer) -> SendGridClient {
    SendGridClient::new(
        common::client(server),
        SendGridConfig::new(API_KEY, EmailAddress::new("from@example.com")),
    )
    .expect("the SendGrid client builds")
}

fn email() -> SendEmailRequest {
    SendEmailRequest::new(EmailAddress::new("to@example.com")).subject("hello")
}

#[tokio::test]
async fn ac_059_the_api_key_is_applied_as_a_bearer_token_and_never_printed() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        "/mail/send",
        common::Recorder::new(202, ""),
    )
    .await;

    let client = sendgrid(&server);
    client
        .send_email(&email().text("body"))
        .await
        .expect("the mail is accepted");

    assert_eq!(
        recorder.last().header("authorization").map(str::to_string),
        Some(format!("Bearer {API_KEY}"))
    );
    assert!(
        !format!("{:?}", client.config()).contains(API_KEY),
        "the configuration renders the key as ***"
    );

    let no_key = SendGridClient::new(
        common::client(&server),
        SendGridConfig::new("   ", EmailAddress::new("from@example.com")),
    )
    .err()
    .expect("an empty key is rejected");
    assert_eq!(no_key.code(), "W2006");

    let no_sender = SendGridClient::new(
        common::client(&server),
        SendGridConfig::new(API_KEY, EmailAddress::new("")),
    )
    .err()
    .expect("an empty sender is rejected");
    assert_eq!(no_sender.code(), "W2006");
}

#[tokio::test]
async fn ac_060_a_sent_email_returns_the_message_id_header() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/mail/send",
        common::Recorder::new(202, "").header("x-message-id", "msg_ABC123"),
    )
    .await;

    let response = sendgrid(&server)
        .send_email(&email().text("body"))
        .await
        .expect("the mail is accepted");

    assert_eq!(response.status, 202);
    assert_eq!(response.message_id.as_deref(), Some("msg_ABC123"));

    let missing_subject = sendgrid(&server)
        .send_email(&SendEmailRequest::new(EmailAddress::new("to@example.com")).text("body"))
        .await
        .expect_err("a non-template email needs a subject");
    assert_eq!(missing_subject.code(), "W2006");

    let missing_content = sendgrid(&server)
        .send_email(&SendEmailRequest::new(EmailAddress::new("to@example.com")).subject("hi"))
        .await
        .expect_err("a non-template email needs content");
    assert_eq!(missing_content.code(), "W2006");

    let no_recipient = sendgrid(&server)
        .send_email(&SendEmailRequest::new(EmailAddress::new("")).subject("hi").text("body"))
        .await
        .expect_err("a recipient is required");
    assert_eq!(no_recipient.code(), "W2006");
}

#[tokio::test]
async fn ac_061_a_template_request_carries_the_dynamic_data() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        "/mail/send",
        common::Recorder::new(202, "").header("x-message-id", "msg_TEMPLATE"),
    )
    .await;

    let request = SendEmailRequest::new(EmailAddress::new("to@example.com")).template(
        "d-0123456789",
        json!({ "name": "Ada", "order": 42 }),
    );
    let response = sendgrid(&server)
        .send_email(&request)
        .await
        .expect("the template mail is accepted");
    assert_eq!(response.message_id.as_deref(), Some("msg_TEMPLATE"));

    let body = recorder.last().json();
    assert_eq!(body["template_id"], json!("d-0123456789"));
    assert_eq!(
        body["personalizations"][0]["dynamic_template_data"]["name"],
        json!("Ada")
    );
    assert_eq!(
        body["personalizations"][0]["dynamic_template_data"]["order"],
        json!(42)
    );
    assert!(
        body.get("subject").is_none(),
        "a template supplies its own subject"
    );
    assert!(
        body.get("content").is_none(),
        "a template supplies its own content"
    );
}

#[tokio::test]
async fn ac_062_sandbox_mode_validates_without_delivering() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        "/mail/send",
        common::Recorder::new(200, "").header("x-message-id", "msg_SANDBOX"),
    )
    .await;

    let client = SendGridClient::new(
        common::client(&server),
        SendGridConfig::new(API_KEY, EmailAddress::new("from@example.com")).sandbox(true),
    )
    .expect("the client builds");

    client
        .send_email(&email().text("body"))
        .await
        .expect("the sandbox send is accepted");
    assert_eq!(
        recorder.last().json()["mail_settings"]["sandbox_mode"]["enable"],
        json!(true),
        "the configured sandbox flag is sent"
    );

    client
        .send_email(&email().text("body").sandbox(false))
        .await
        .expect("the sandbox send is accepted");
    assert!(
        recorder.last().json().get("mail_settings").is_none(),
        "a per-request override disables sandbox mode"
    );
}

#[tokio::test]
async fn ac_063_an_errors_array_becomes_an_api_error_with_details() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/mail/send",
        common::Recorder::json(
            400,
            json!({
                "errors": [
                    { "message": "The to address is invalid.", "field": "to", "help": null },
                    { "message": "The subject is missing.", "field": "subject", "help": null }
                ]
            }),
        ),
    )
    .await;

    let error = sendgrid(&server)
        .send_email(&email().text("body"))
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
            assert_eq!(code, "to", "SendGrid's field becomes the error code");
            assert_eq!(message, "The to address is invalid.");
        }
        other => panic!("expected Api, got {other:?}"),
    }

    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/mail/send",
        common::Recorder::json(
            400,
            json!({ "errors": [{ "message": "The to address is invalid.", "field": "to" }] }),
        ),
    )
    .await;
    let error = sendgrid(&server)
        .send_email(&email().text("body"))
        .await
        .expect_err("a 400 is a failure");
    let details = error.details().expect("the error list is retained");
    let errors = details["errors"]
        .as_array()
        .expect("the details keep the errors array");
    assert!(!errors.is_empty());
    assert_eq!(errors[0]["field"], json!("to"));
}

#[tokio::test]
async fn ac_064_an_event_webhook_signature_is_verified_over_the_raw_bytes() {
    verify_event(
        common::SENDGRID_EVENT_KEY,
        common::SENDGRID_EVENT_PAYLOAD.as_bytes(),
        common::SENDGRID_EVENT_SIGNATURE,
        common::SENDGRID_EVENT_TIMESTAMP,
    )
    .expect("the fixture signature verifies");

    let tampered = common::SENDGRID_EVENT_PAYLOAD.replace("\"delivered\"", "\"bounce\"");
    let error = verify_event(
        common::SENDGRID_EVENT_KEY,
        tampered.as_bytes(),
        common::SENDGRID_EVENT_SIGNATURE,
        common::SENDGRID_EVENT_TIMESTAMP,
    )
    .expect_err("a modified payload must not verify");
    assert_eq!(error.code(), "W2002");
    assert!(error.to_string().contains("did not match"));

    let error = verify_event(
        common::SENDGRID_EVENT_KEY,
        common::SENDGRID_EVENT_PAYLOAD.as_bytes(),
        common::SENDGRID_EVENT_SIGNATURE,
        "1700000001",
    )
    .expect_err("a different timestamp must not verify");
    assert_eq!(error.code(), "W2002");

    let error = verify_event(
        "not-a-key",
        common::SENDGRID_EVENT_PAYLOAD.as_bytes(),
        common::SENDGRID_EVENT_SIGNATURE,
        common::SENDGRID_EVENT_TIMESTAMP,
    )
    .expect_err("an unusable key is rejected");
    assert_eq!(error.code(), "W2002");
    assert!(error.to_string().contains("public key"));
}

#[tokio::test]
async fn ac_065_repeated_events_are_identifiable_for_deduplication() {
    let payload = json!([
        {
            "sg_event_id": "event-1",
            "sg_message_id": "msg-1",
            "event": "delivered",
            "timestamp": 1700000000,
            "email": "example@test.com"
        },
        {
            "sg_event_id": "event-1",
            "sg_message_id": "msg-1",
            "event": "delivered",
            "timestamp": 1700000000,
            "email": "example@test.com"
        },
        {
            "sg_event_id": "event-2",
            "event": "bounce",
            "timestamp": 1700000001,
            "reason": "mailbox full"
        }
    ]);

    let raw = serde_json::to_vec(&payload).expect("the fixture is serializable");
    let events: Vec<SendGridEvent> =
        SendGridClient::parse_events(&raw).expect("the payload parses");

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].dedupe_key(), "event-1");
    assert_eq!(events[1].dedupe_key(), events[0].dedupe_key());
    assert_ne!(events[2].dedupe_key(), events[0].dedupe_key());
    assert_eq!(events[2].event, "bounce");
    assert_eq!(events[2].reason.as_deref(), Some("mailbox full"));

    let malformed = SendGridClient::parse_events(b"not json")
        .expect_err("a malformed payload is a decode error");
    assert_eq!(malformed.code(), "W2007");
}

#[tokio::test]
async fn ac_066_attachments_are_base64_encoded_within_the_size_limit() {
    assert_eq!(MAX_ATTACHMENT_BYTES, 10 * 1024 * 1024);

    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        "/mail/send",
        common::Recorder::new(202, ""),
    )
    .await;

    let attachment = EmailAttachment::new("report.pdf", "JVBERi0xLjQK")
        .content_type("application/pdf")
        .inline("cid-1");
    sendgrid(&server)
        .send_email(&email().text("body").attach(attachment))
        .await
        .expect("the attachment is accepted");

    let body = recorder.last().json();
    let sent = &body["attachments"][0];
    assert_eq!(sent["content"], json!("JVBERi0xLjQK"));
    assert_eq!(sent["filename"], json!("report.pdf"));
    assert_eq!(sent["type"], json!("application/pdf"));
    assert_eq!(sent["disposition"], json!("inline"));
    assert_eq!(sent["content_id"], json!("cid-1"));

    // 14,000,000 base64 characters decode to 10.5 MB, past the 10 MiB limit.
    let oversized = "A".repeat(14_000_000);
    let attachment = EmailAttachment::new("huge.bin", oversized);
    let error = sendgrid(&server)
        .send_email(&email().text("body").attach(attachment))
        .await
        .expect_err("an oversized attachment is rejected before sending");
    assert_eq!(error.code(), "W2006");
    assert!(error.to_string().contains("limit"));
    assert_eq!(recorder.hits(), 1, "the oversized send never left the process");
}

//! Acceptance tests for `meta-graph-provider-module` (AC-001 to AC-013).
//!
//! The Graph host is mocked, so no Meta app review is needed to prove the
//! contract. Every test name carries its AC id (CON-405).

use hmac::{Hmac, Mac};
use sha2::Sha256;
use wirekit::meta::{
    verify_handshake, AttachmentKind, EventKind, MessageAttachment, MetaClient, MetaSurface,
    SendMessageRequest, WebhookPayload, WhatsAppTemplate,
};
use wirekit::WirekitClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn meta(server: &MockServer) -> MetaClient {
    let http = WirekitClient::builder(server.uri())
        .expect("a loopback base URL")
        .allow_insecure_localhost(true)
        .build()
        .expect("the client builds");

    MetaClient::builder(http)
        .access_token("page-token")
        .app_secret("app-secret")
        .page_id("page-1")
        .phone_number_id("phone-1")
        .build()
        .expect("the Meta client builds")
}

async fn respond_json(server: &MockServer, body: &str) {
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

fn sign(secret: &str, raw: &[u8]) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).expect("key");
    mac.update(raw);
    mac.finalize()
        .into_bytes()
        .iter()
        .fold(String::from("sha256="), |mut acc, byte| {
            use std::fmt::Write as _;
            let _ = write!(acc, "{byte:02x}");
            acc
        })
}

const MESSENGER_EVENT: &str = r#"{"object":"page","entry":[{"id":"1","time":1730000000,"messaging":[{"sender":{"id":"psid-1"},"message":{"mid":"mid.1","text":"hi"}}]}]}"#;

#[tokio::test]
async fn ac_001_messenger_send_posts_the_page_scoped_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v18.0/page-1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"message_id":"m1"}"#))
        .expect(1)
        .mount(&server)
        .await;

    let meta = meta(&server).await;
    let outcome = meta
        .messenger()
        .send_text("psid-1", "Hello")
        .await
        .expect("the send succeeds");

    assert_eq!(outcome.message_id.as_deref(), Some("m1"));

    let request = &server.received_requests().await.expect("recorded")[0];
    let body: serde_json::Value = serde_json::from_slice(&request.body).expect("json");
    assert_eq!(body["recipient"]["id"], "psid-1");
    assert_eq!(body["messaging_type"], "RESPONSE");
    assert_eq!(body["message"]["text"], "Hello");
    assert_eq!(request.headers["authorization"], "Bearer page-token");
}

#[tokio::test]
async fn ac_002_a_media_send_carries_an_image_attachment() {
    let server = MockServer::start().await;
    respond_json(&server, r#"{"message_id":"m2"}"#).await;

    let meta = meta(&server).await;
    meta.messenger()
        .send(&SendMessageRequest::media(
            "psid-1",
            MessageAttachment::from_url(AttachmentKind::Image, "https://example.com/a.png"),
        ))
        .await
        .expect("the send succeeds");

    let request = &server.received_requests().await.expect("recorded")[0];
    let body: serde_json::Value = serde_json::from_slice(&request.body).expect("json");
    assert_eq!(body["message"]["attachment"]["type"], "image");
    assert_eq!(
        body["message"]["attachment"]["payload"]["url"],
        "https://example.com/a.png"
    );
    assert!(body["message"].get("text").is_none());
}

#[tokio::test]
async fn ac_003_instagram_posts_to_the_instagram_shape() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v18.0/me/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"message_id":"ig-1"}"#))
        .expect(1)
        .mount(&server)
        .await;

    let meta = meta(&server).await;
    meta.instagram()
        .send_text("ig-user", "Hello")
        .await
        .expect("the send succeeds");

    let request = &server.received_requests().await.expect("recorded")[0];
    let body: serde_json::Value = serde_json::from_slice(&request.body).expect("json");
    assert_eq!(body["recipient"]["id"], "ig-user");
    assert_eq!(body["message"]["text"], "Hello");
}

#[tokio::test]
async fn ac_004_whatsapp_template_send_orders_its_parameters() {
    let server = MockServer::start().await;
    respond_json(&server, r#"{"messages":[{"id":"wamid.1"}]}"#).await;

    let meta = meta(&server).await;
    let outcome = meta
        .whatsapp()
        .send_template(
            "15550001",
            WhatsAppTemplate {
                name: String::from("appointment"),
                language: String::from("en_US"),
                parameters: vec![String::from("Dana"), String::from("10am")],
            },
        )
        .await
        .expect("the send succeeds");

    assert_eq!(outcome.message_id.as_deref(), Some("wamid.1"));

    let request = &server.received_requests().await.expect("recorded")[0];
    let body: serde_json::Value = serde_json::from_slice(&request.body).expect("json");
    assert_eq!(body["type"], "template");
    assert_eq!(body["template"]["name"], "appointment");
    assert_eq!(body["template"]["language"]["code"], "en_US");
    assert_eq!(
        body["template"]["components"][0]["parameters"][0]["text"],
        "Dana"
    );
    assert_eq!(
        body["template"]["components"][0]["parameters"][1]["text"],
        "10am"
    );
}

#[tokio::test]
async fn ac_005_whatsapp_text_send_uses_the_text_type() {
    let server = MockServer::start().await;
    respond_json(&server, r#"{"messages":[{"id":"wamid.2"}]}"#).await;

    let meta = meta(&server).await;
    meta.whatsapp()
        .send_text("15550001", "Hello")
        .await
        .expect("the send succeeds");

    let request = &server.received_requests().await.expect("recorded")[0];
    let body: serde_json::Value = serde_json::from_slice(&request.body).expect("json");
    assert_eq!(body["type"], "text");
    assert_eq!(body["to"], "15550001");
    assert_eq!(body["text"]["body"], "Hello");
}

#[test]
fn ac_006_a_correct_signature_parses_the_event() {
    let payload = WebhookPayload::new("app-secret");
    let raw = MESSENGER_EVENT.as_bytes();
    let event = payload
        .verify_and_parse(raw, &sign("app-secret", raw))
        .expect("a correct signature verifies");

    assert_eq!(event.surface, MetaSurface::Messenger);
    assert_eq!(event.kind, EventKind::Message);
}

#[test]
fn ac_007_a_wrong_signature_fails_closed() {
    let payload = WebhookPayload::new("app-secret");
    let error = payload
        .verify_and_parse(MESSENGER_EVENT.as_bytes(), "sha256=deadbeef")
        .expect_err("a wrong signature is rejected");

    assert_eq!(error.code(), "W2002");
}

#[test]
fn ac_008_the_handshake_echoes_the_challenge() {
    let echoed =
        verify_handshake("subscribe", "token", "token", "challenge-1").expect("handshake succeeds");
    assert_eq!(echoed, "challenge-1");

    let error = verify_handshake("subscribe", "wrong", "token", "challenge-1")
        .expect_err("a mismatched token is rejected");
    assert_eq!(error.code(), "W2002");
}

#[test]
fn ac_009_the_event_carries_its_dedupe_key_and_surface() {
    let payload = WebhookPayload::new("app-secret");
    let raw = MESSENGER_EVENT.as_bytes();
    let event = payload
        .verify_and_parse(raw, &sign("app-secret", raw))
        .expect("verified");

    assert_eq!(event.dedupe_key, "mid.1");
    assert_eq!(event.sender_id, "psid-1");
    assert_eq!(event.timestamp, 1_730_000_000);
}

#[tokio::test]
async fn ac_010_a_graph_error_body_becomes_an_api_error_with_its_code() {
    let server = MockServer::start().await;
    respond_json(
        &server,
        r#"{"error":{"message":"Invalid parameter","code":100,"error_subcode":2018001,"fbtrace_id":"trace-1"}}"#,
    )
    .await;

    let meta = meta(&server).await;
    let error = meta
        .messenger()
        .send_text("psid-1", "Hello")
        .await
        .expect_err("a Graph error is not success");

    assert_eq!(error.code(), "W2001");
    assert_eq!(error.api_code(), Some("100"));
    assert!(error.to_string().contains("Invalid parameter"));
}

#[tokio::test]
async fn ac_011_the_usage_header_is_surfaced() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-business-use-case-usage", "page-usage")
                .set_body_string(r#"{"message_id":"m3"}"#),
        )
        .mount(&server)
        .await;

    let meta = meta(&server).await;
    let outcome = meta
        .messenger()
        .send_text("psid-1", "Hello")
        .await
        .expect("the send succeeds");

    assert_eq!(outcome.usage.as_deref(), Some("page-usage"));
}

#[test]
fn ac_012_credentials_never_render() {
    let http = WirekitClient::builder("https://graph.facebook.com")
        .expect("a base URL")
        .build()
        .expect("the client builds");
    let meta = MetaClient::builder(http)
        .access_token("page-token")
        .app_secret("app-secret")
        .page_id("page-1")
        .build()
        .expect("the client builds");

    let rendered = format!("{meta:?}");
    assert!(!rendered.contains("page-token"));
    assert!(!rendered.contains("app-secret"));
    assert!(rendered.contains("***"));
}

#[test]
fn ac_013_a_cleartext_base_url_is_rejected() {
    // The transport rejects cleartext when it builds; Meta keeps its own guard
    // for the absolute URLs it constructs.
    let builder =
        WirekitClient::builder("http://graph.facebook.com").expect("the builder accepts any URL");
    let error = match builder.build() {
        Ok(_) => panic!("a cleartext base URL must be rejected"),
        Err(error) => error,
    };

    assert_eq!(error.code(), "W2006");
}

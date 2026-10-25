//! AC-022 through AC-025: raw-byte verification, replay windows, and every
//! shipped HMAC preset.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use bytes::Bytes;
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use hookgate::presets;
use hookgate::{
    HmacPreset, InboundRequest, ProviderId, TenantId, Verifier, VerifyError,
};
use http::{HeaderMap, HeaderName, HeaderValue};
use sha1::Sha1;
use sha2::Sha256;

fn hmac_sha256(secret: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts keys of any length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

fn hmac_sha1(secret: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac =
        Hmac::<Sha1>::new_from_slice(secret).expect("HMAC accepts keys of any length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

fn header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    headers.insert(
        HeaderName::from_static(name),
        HeaderValue::from_str(value).expect("the header value must be visible ASCII"),
    );
}

fn request_for(verifier: &dyn Verifier, headers: HeaderMap, body: &'static [u8]) -> InboundRequest {
    InboundRequest::new(
        TenantId::generate(),
        verifier.provider(),
        headers,
        Bytes::from_static(body),
    )
}

/// AC-022: a valid signature over the raw bytes is accepted.
#[test]
fn ac_022_a_valid_signature_over_raw_bytes_is_accepted() {
    let secret = b"super-secret";
    let body: &'static [u8] =
        br#"{"id":"evt_022","type":"message.created","data":{"text":"hello"}}"#;
    let signature = hex::encode(hmac_sha256(secret, body));

    let mut headers = HeaderMap::new();
    header(&mut headers, "x-signature", &signature);

    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");
    let request = InboundRequest::new(tenant, provider, headers, Bytes::from_static(body));
    let event = HmacPreset::new(provider, secret)
        .verify(&request)
        .expect("AC-022: a valid signature over the raw body must be accepted");

    assert_eq!(
        event.event_id.as_deref(),
        Some("evt_022"),
        "AC-022: the verified event must expose the provider event id"
    );
    assert_eq!(
        event.event_type, "message.created",
        "AC-022: the verified event must expose the provider event type"
    );
    assert_eq!(
        event.raw.as_ref(),
        body,
        "AC-022: the verified event must retain the exact raw bytes"
    );
    assert_eq!(event.tenant, tenant, "AC-022: the event must stay tenant-scoped");
    assert_eq!(event.provider, provider, "AC-022: the event must record its provider");
}

/// AC-023: re-serializing the body after signing invalidates the signature.
#[test]
fn ac_023_reserialized_json_fails_verification() {
    let secret = b"super-secret";
    let raw: &'static [u8] = br#"{"id": "evt_023", "type": "message.created"}"#;
    let signature = hex::encode(hmac_sha256(secret, raw));

    let parsed: serde_json::Value =
        serde_json::from_slice(raw).expect("the fixture must be valid JSON");
    let reserialized = serde_json::to_vec(&parsed).expect("the fixture must re-serialize");
    assert_ne!(
        raw,
        reserialized.as_slice(),
        "the fixture must change bytes when re-serialized"
    );

    let mut headers = HeaderMap::new();
    header(&mut headers, "x-signature", &signature);

    let request = InboundRequest::new(
        TenantId::generate(),
        ProviderId::new("acme"),
        headers,
        Bytes::from(reserialized),
    );
    let error = HmacPreset::new(ProviderId::new("acme"), secret)
        .verify(&request)
        .expect_err("AC-023: a re-serialized body must fail verification");

    assert!(
        matches!(error, VerifyError::InvalidSignature),
        "AC-023: verification must fail on the byte change, got {error:?}"
    );
}

/// AC-024: a timestamp outside the tolerance is rejected as stale.
#[test]
fn ac_024_a_timestamp_outside_the_tolerance_is_rejected() {
    let secret = b"super-secret";
    let body: &'static [u8] = br#"{"id":"evt_024","type":"message.created"}"#;
    let signature = hex::encode(hmac_sha256(secret, body));

    let mut headers = HeaderMap::new();
    header(&mut headers, "x-signature", &signature);

    let provider = ProviderId::new("acme");
    let verifier = HmacPreset::new(provider, secret);
    let request = InboundRequest::new(
        TenantId::generate(),
        provider,
        headers,
        Bytes::from_static(body),
    );

    let stale = request
        .clone()
        .with_received_at(Utc::now() - Duration::minutes(10));
    let error = verifier
        .verify(&stale)
        .expect_err("AC-024: a stale request must be rejected");
    match error {
        VerifyError::Stale { age_secs } => assert!(
            age_secs >= 600,
            "AC-024: the rejection must report the request age, got {age_secs}"
        ),
        other => panic!("AC-024: a stale request must be rejected as stale, got {other:?}"),
    }

    let future = request.with_received_at(Utc::now() + Duration::minutes(10));
    let error = verifier
        .verify(&future)
        .expect_err("AC-024: a future timestamp must be rejected");
    assert!(
        matches!(error, VerifyError::FutureTimestamp { .. }),
        "AC-024: a future timestamp must be rejected as invalid, got {error:?}"
    );
}

/// Verifies a preset accepts its documented signature and rejects one byte of
/// tampering.
fn assert_preset(label: &str, verifier: &dyn Verifier, request: InboundRequest) {
    verifier.verify(&request).unwrap_or_else(|error| {
        panic!("AC-025: the {label} preset must accept its documented signature: {error}")
    });

    let mut tampered = request.body.to_vec();
    let last = tampered
        .last_mut()
        .unwrap_or_else(|| panic!("AC-025: the {label} fixture must not be empty"));
    *last ^= 0xff;

    let tampered_request = InboundRequest {
        body: Bytes::from(tampered),
        ..request
    };
    let error = verifier
        .verify(&tampered_request)
        .expect_err("AC-025: a one-byte body change must fail verification");
    assert!(
        matches!(error, VerifyError::InvalidSignature),
        "AC-025: the {label} preset must reject a tampered body, got {error:?}"
    );
}

/// AC-025: every shipped HMAC preset accepts its documented signature and
/// rejects a one-byte body change.
#[test]
fn ac_025_every_hmac_preset_accepts_valid_and_rejects_tampered_bodies() {
    let secret = b"preset-secret";

    // Meta Graph: X-Hub-Signature-256, SHA-256, hex after `sha256=`.
    let meta = presets::meta_graph(secret);
    let meta_body: &'static [u8] =
        br#"{"object":"page","entry":[{"id":"evt_meta_025"}]}"#;
    let mut meta_headers = HeaderMap::new();
    header(
        &mut meta_headers,
        "x-hub-signature-256",
        &format!("sha256={}", hex::encode(hmac_sha256(secret, meta_body))),
    );
    assert_preset(
        "Meta Graph",
        &meta,
        request_for(&meta, meta_headers, meta_body),
    );

    // Stripe: Stripe-Signature, SHA-256, hex over `{t}.{body}`.
    let stripe = presets::stripe(secret);
    let stripe_body: &'static [u8] =
        br#"{"id":"evt_stripe_025","type":"payment_intent.succeeded"}"#;
    let stripe_timestamp = Utc::now().timestamp();
    let mut stripe_message = stripe_timestamp.to_string().into_bytes();
    stripe_message.push(b'.');
    stripe_message.extend_from_slice(stripe_body);
    let mut stripe_headers = HeaderMap::new();
    header(
        &mut stripe_headers,
        "stripe-signature",
        &format!(
            "t={stripe_timestamp},v1={}",
            hex::encode(hmac_sha256(secret, &stripe_message))
        ),
    );
    assert_preset(
        "Stripe",
        &stripe,
        request_for(&stripe, stripe_headers, stripe_body),
    );

    // Shopify: X-Shopify-Hmac-Sha256, SHA-256, base64 over the raw body.
    let shopify = presets::shopify(secret);
    let shopify_body: &'static [u8] = br#"{"id":"evt_shopify_025"}"#;
    let mut shopify_headers = HeaderMap::new();
    header(
        &mut shopify_headers,
        "x-shopify-hmac-sha256",
        &BASE64.encode(hmac_sha256(secret, shopify_body)),
    );
    header(&mut shopify_headers, "x-shopify-event-id", "evt_shopify_025");
    header(&mut shopify_headers, "x-shopify-topic", "orders/create");
    assert_preset(
        "Shopify",
        &shopify,
        request_for(&shopify, shopify_headers, shopify_body),
    );

    // Twilio: X-Twilio-Signature, SHA-1, base64 over URL plus sorted fields.
    let twilio = presets::twilio(secret);
    let twilio_body: &'static [u8] =
        b"MessageSid=SM025&MessageStatus=delivered&Body=hello";
    let twilio_url = "https://example.test/webhooks/twilio";
    let mut twilio_message = twilio_url.as_bytes().to_vec();
    twilio_message.extend_from_slice(b"Bodyhello");
    twilio_message.extend_from_slice(b"MessageSidSM025");
    twilio_message.extend_from_slice(b"MessageStatusdelivered");
    let mut twilio_headers = HeaderMap::new();
    header(
        &mut twilio_headers,
        "x-twilio-signature",
        &BASE64.encode(hmac_sha1(secret, &twilio_message)),
    );
    header(&mut twilio_headers, "x-forwarded-url", twilio_url);
    assert_preset(
        "Twilio",
        &twilio,
        request_for(&twilio, twilio_headers, twilio_body),
    );

    // Slack: X-Slack-Signature, `v0=`, SHA-256 over `v0:{timestamp}:{body}`.
    let slack = presets::slack(secret);
    let slack_body: &'static [u8] =
        br#"{"event_id":"Ev025","type":"event_callback"}"#;
    let slack_timestamp = Utc::now().timestamp();
    let mut slack_message = b"v0:".to_vec();
    slack_message.extend_from_slice(slack_timestamp.to_string().as_bytes());
    slack_message.push(b':');
    slack_message.extend_from_slice(slack_body);
    let mut slack_headers = HeaderMap::new();
    header(
        &mut slack_headers,
        "x-slack-signature",
        &format!("v0={}", hex::encode(hmac_sha256(secret, &slack_message))),
    );
    header(
        &mut slack_headers,
        "x-slack-request-timestamp",
        &slack_timestamp.to_string(),
    );
    assert_preset(
        "Slack",
        &slack,
        request_for(&slack, slack_headers, slack_body),
    );

    // GitHub: X-Hub-Signature-256, SHA-256, hex after `sha256=`.
    let github = presets::github(secret);
    let github_body: &'static [u8] = br#"{"action":"opened"}"#;
    let mut github_headers = HeaderMap::new();
    header(
        &mut github_headers,
        "x-hub-signature-256",
        &format!("sha256={}", hex::encode(hmac_sha256(secret, github_body))),
    );
    header(&mut github_headers, "x-github-delivery", "delivery-025");
    header(&mut github_headers, "x-github-event", "issues");
    assert_preset(
        "GitHub",
        &github,
        request_for(&github, github_headers, github_body),
    );

    // Generic: caller-supplied header, SHA-256, hex over the raw body.
    let generic = presets::generic_sha256(secret, HeaderName::from_static("x-custom-signature"));
    let generic_body: &'static [u8] =
        br#"{"id":"evt_generic_025","type":"message.created"}"#;
    let mut generic_headers = HeaderMap::new();
    header(
        &mut generic_headers,
        "x-custom-signature",
        &hex::encode(hmac_sha256(secret, generic_body)),
    );
    assert_preset(
        "generic",
        &generic,
        request_for(&generic, generic_headers, generic_body),
    );
}

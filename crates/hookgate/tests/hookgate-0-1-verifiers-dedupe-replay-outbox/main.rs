//! Acceptance tests for hookgate 0.1.
//!
//! One test per acceptance criterion (AC-001 to AC-014). Every signature is
//! computed here over the same bytes the request carries, so a verifier that
//! re-serializes the body before checking fails these tests.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hmac::{Hmac, Mac};
use http::{HeaderMap, HeaderName, HeaderValue};
use sha1::Sha1;
use sha2::Sha256;

use hookgate::{
    presets, redact_error, DeliveryRecord, DeliveryState, DeliveryStore, InboundRequest,
    InsertOutcome, MemoryDeliveryStore, Outbox, OutboxMessage, ProviderId, TenantId, VerifiedEvent,
    Verifier, VerifyError,
};

const SECRET: &[u8] = b"whsec_test_secret_value";

fn tenant() -> TenantId {
    TenantId::generate()
}

fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(
            HeaderName::from_static(name),
            HeaderValue::from_str(value).expect("a test header is valid"),
        );
    }
    map
}

fn request(
    tenant: TenantId,
    provider: &'static str,
    headers: HeaderMap,
    body: &[u8],
) -> InboundRequest {
    InboundRequest::new(tenant, ProviderId::new(provider), headers, body.to_vec())
}

fn hmac_sha256(secret: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

fn hmac_sha1(secret: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

fn signature_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after the epoch")
        .as_secs()
}

/// A correctly signed request through the generic preset, used by the store tests.
fn signed_event(tenant: TenantId, event_id: &str) -> VerifiedEvent {
    let body = format!(r#"{{"id":"{event_id}","type":"test.event"}}"#);
    let signature = signature_hex(&hmac_sha256(SECRET, body.as_bytes()));
    let preset = presets::generic_sha256(SECRET, HeaderName::from_static("x-signature"));
    let inbound = request(
        tenant,
        "generic",
        headers(&[("x-signature", &signature)]),
        body.as_bytes(),
    );
    preset.verify(&inbound).expect("the request is signed")
}

// AC-001
#[test]
fn ac_001_verification_uses_the_exact_received_bytes() {
    let preset = presets::generic_sha256(SECRET, HeaderName::from_static("x-signature"));
    let received = br#"{"id":"evt_1","type":"ping","note":"two  spaces"}"#;

    let signature = signature_hex(&hmac_sha256(SECRET, received));
    let signed = request(
        tenant(),
        "generic",
        headers(&[("x-signature", &signature)]),
        received,
    );
    assert!(
        preset.verify(&signed).is_ok(),
        "a signature over the received bytes is accepted"
    );

    // EC-001: signing the re-serialized body must not verify the raw one.
    let reserialized = br#"{"id":"evt_1","type":"ping","note":"two spaces"}"#;
    let wrong = signature_hex(&hmac_sha256(SECRET, reserialized));
    let tampered_provenance = request(
        tenant(),
        "generic",
        headers(&[("x-signature", &wrong)]),
        received,
    );
    assert!(
        matches!(
            preset.verify(&tampered_provenance),
            Err(VerifyError::InvalidSignature)
        ),
        "a signature over a re-serialized body is rejected"
    );
}

// AC-002
#[test]
fn ac_002_a_valid_signature_returns_the_verified_event() {
    let body = br#"{"id":"evt_42","type":"invoice.paid","data":{"amount":1200}}"#;
    let signature = signature_hex(&hmac_sha256(SECRET, body));
    let preset = presets::generic_sha256(SECRET, HeaderName::from_static("x-signature"));
    let owner = tenant();
    let inbound = request(
        owner,
        "generic",
        headers(&[("x-signature", &signature)]),
        body,
    );

    let event = preset.verify(&inbound).expect("valid signature");
    assert_eq!(event.tenant, owner);
    assert_eq!(event.provider, ProviderId::new("generic"));
    assert_eq!(event.event_id.as_deref(), Some("evt_42"));
    assert_eq!(event.event_type, "invoice.paid");
    assert_eq!(
        event
            .payload
            .pointer("/data/amount")
            .and_then(|v| v.as_i64()),
        Some(1200)
    );
    assert_eq!(&event.raw[..], &body[..], "the raw bytes are preserved");

    let again = preset.verify(&inbound).expect("valid signature");
    assert_ne!(
        event.delivery, again.delivery,
        "each accepted request carries its own delivery id"
    );
}

// AC-003
#[test]
fn ac_003_a_tampered_body_fails_verification() {
    let body = br#"{"id":"evt_3","type":"ping"}"#;
    let signature = signature_hex(&hmac_sha256(SECRET, body));
    let preset = presets::generic_sha256(SECRET, HeaderName::from_static("x-signature"));

    let mut tampered = body.to_vec();
    tampered[9] ^= 0x01;
    let inbound = request(
        tenant(),
        "generic",
        headers(&[("x-signature", &signature)]),
        &tampered,
    );

    let outcome = preset.verify(&inbound);
    assert!(
        matches!(outcome, Err(VerifyError::InvalidSignature)),
        "a changed byte is a typed signature failure, not an event: {outcome:?}"
    );
}

// AC-004
#[test]
fn ac_004_a_stale_timestamp_is_rejected() {
    let body = br#"{"id":"evt_4","type":"charge.succeeded"}"#;
    let preset = presets::stripe(SECRET);

    let stale_seconds = unix_now().saturating_sub(600);
    let mut stale_message = format!("{stale_seconds}.").into_bytes();
    stale_message.extend_from_slice(body);
    let stale_header = format!(
        "t={stale_seconds},v1={}",
        signature_hex(&hmac_sha256(SECRET, &stale_message))
    );
    let stale = request(
        tenant(),
        "stripe",
        headers(&[("stripe-signature", &stale_header)]),
        body,
    );
    assert!(
        matches!(preset.verify(&stale), Err(VerifyError::Stale { .. })),
        "a ten-minute-old signature is stale"
    );

    let fresh_seconds = unix_now();
    let mut fresh_message = format!("{fresh_seconds}.").into_bytes();
    fresh_message.extend_from_slice(body);
    let fresh_header = format!(
        "t={fresh_seconds},v1={}",
        signature_hex(&hmac_sha256(SECRET, &fresh_message))
    );
    let fresh = request(
        tenant(),
        "stripe",
        headers(&[("stripe-signature", &fresh_header)]),
        body,
    );
    assert!(
        preset.verify(&fresh).is_ok(),
        "a fresh signature is inside the window"
    );
}

// AC-005
#[tokio::test]
async fn ac_005_a_repeated_delivery_is_a_duplicate() {
    let store = MemoryDeliveryStore::new();
    let event = signed_event(tenant(), "evt_dup");
    let record = DeliveryRecord::from_event(&event).expect("the event has an id");
    let message = OutboxMessage::from_event(&event);

    assert_eq!(
        store
            .insert_if_new(record.clone(), message.clone())
            .await
            .expect("the first insert succeeds"),
        InsertOutcome::Inserted
    );
    assert_eq!(
        store
            .insert_if_new(record, message)
            .await
            .expect("the repeat is an outcome, not an error"),
        InsertOutcome::Duplicate
    );
    assert_eq!(store.len().await.expect("outbox length"), 1);
}

// AC-006
#[tokio::test]
async fn ac_006_the_same_event_id_in_two_tenants_is_not_a_duplicate() {
    let store = MemoryDeliveryStore::new();
    let first = tenant();
    let second = tenant();

    let event_a = signed_event(first, "evt_shared");
    let event_b = signed_event(second, "evt_shared");

    assert_eq!(
        store
            .insert_if_new(
                DeliveryRecord::from_event(&event_a).expect("id"),
                OutboxMessage::from_event(&event_a),
            )
            .await
            .expect("first tenant insert"),
        InsertOutcome::Inserted
    );
    assert_eq!(
        store
            .insert_if_new(
                DeliveryRecord::from_event(&event_b).expect("id"),
                OutboxMessage::from_event(&event_b),
            )
            .await
            .expect("second tenant insert"),
        InsertOutcome::Inserted
    );
    assert_eq!(store.len().await.expect("outbox length"), 2);
}

// AC-007
#[tokio::test]
async fn ac_007_the_delivery_and_outbox_row_are_written_before_success() {
    let store = MemoryDeliveryStore::new();
    let owner = tenant();
    let event = signed_event(owner, "evt_outbox");

    let inserted = store
        .insert_if_new(
            DeliveryRecord::from_event(&event).expect("id"),
            OutboxMessage::from_event(&event),
        )
        .await
        .expect("insert");

    assert_eq!(inserted, InsertOutcome::Inserted);
    assert!(
        store
            .get(owner, event.delivery)
            .await
            .expect("read")
            .is_some(),
        "the delivery record exists before success is reported"
    );
    assert_eq!(store.len().await.expect("outbox length"), 1);
    let queued = store.pop().await.expect("pop").expect("one message");
    assert_eq!(queued.delivery, event.delivery);
}

// AC-008
#[tokio::test]
async fn ac_008_terminal_states_and_dead_letter_capture() {
    let store = MemoryDeliveryStore::with_max_attempts(2);
    let owner = tenant();

    let processed = signed_event(owner, "evt_processed");
    store
        .insert_if_new(
            DeliveryRecord::from_event(&processed).expect("id"),
            OutboxMessage::from_event(&processed),
        )
        .await
        .expect("insert");
    store
        .mark_processed(owner, processed.delivery)
        .await
        .expect("mark processed");
    assert_eq!(
        store
            .get(owner, processed.delivery)
            .await
            .expect("read")
            .expect("present")
            .state,
        DeliveryState::Processed
    );

    let failing = signed_event(owner, "evt_failing");
    store
        .insert_if_new(
            DeliveryRecord::from_event(&failing).expect("id"),
            OutboxMessage::from_event(&failing),
        )
        .await
        .expect("insert");
    let first = store
        .fail(owner, failing.delivery, "handler exploded")
        .await
        .expect("first failure");
    assert!(
        first.is_none(),
        "below the threshold there is no dead letter"
    );

    let dead = store
        .fail(owner, failing.delivery, "handler exploded")
        .await
        .expect("second failure")
        .expect("the threshold produces a dead letter");
    assert_eq!(dead.delivery, failing.delivery);
    assert_eq!(dead.reason, "handler exploded");
    assert_eq!(&dead.raw[..], &failing.raw[..], "the raw body is kept");
    assert_eq!(
        store
            .get(owner, failing.delivery)
            .await
            .expect("read")
            .expect("present")
            .state,
        DeliveryState::DeadLettered
    );
}

// AC-009
#[tokio::test]
async fn ac_009_dead_letters_are_readable_and_replayable_once() {
    let store = MemoryDeliveryStore::with_max_attempts(1);
    let owner = tenant();
    let event = signed_event(owner, "evt_replay");

    store
        .insert_if_new(
            DeliveryRecord::from_event(&event).expect("id"),
            OutboxMessage::from_event(&event),
        )
        .await
        .expect("insert");
    store
        .fail(owner, event.delivery, "boom")
        .await
        .expect("dead letter")
        .expect("at the threshold");

    let letters = store.dead_letters(owner).await.expect("dead letters");
    assert_eq!(letters.len(), 1);
    assert_eq!(&letters[0].raw[..], &event.raw[..]);
    assert_eq!(letters[0].reason, "boom");

    assert_eq!(
        store.replay(owner, event.delivery).await.expect("replay"),
        InsertOutcome::Inserted,
        "a dead letter is re-enqueued"
    );

    store
        .mark_processed(owner, event.delivery)
        .await
        .expect("mark processed");
    assert_eq!(
        store
            .replay(owner, event.delivery)
            .await
            .expect("replay after success"),
        InsertOutcome::Duplicate,
        "a processed delivery is not re-enqueued"
    );
}

// AC-010
#[test]
fn ac_010_every_provider_preset_verifies_a_signed_request() {
    // Meta
    let body = br#"{"object":"page","entry":[{"id":"evt_meta"}]}"#;
    let signature = format!("sha256={}", signature_hex(&hmac_sha256(SECRET, body)));
    let meta = presets::meta_graph(SECRET);
    let event = meta
        .verify(&request(
            tenant(),
            "meta",
            headers(&[("x-hub-signature-256", &signature)]),
            body,
        ))
        .expect("Meta signature");
    assert_eq!(event.event_id.as_deref(), Some("evt_meta"));
    assert_eq!(event.event_type, "page");

    // Stripe
    let body = br#"{"id":"evt_stripe","type":"payment_intent.succeeded"}"#;
    let seconds = unix_now();
    let mut message = format!("{seconds}.").into_bytes();
    message.extend_from_slice(body);
    let signature = format!(
        "t={seconds},v1={}",
        signature_hex(&hmac_sha256(SECRET, &message))
    );
    let stripe = presets::stripe(SECRET);
    let event = stripe
        .verify(&request(
            tenant(),
            "stripe",
            headers(&[("stripe-signature", &signature)]),
            body,
        ))
        .expect("Stripe signature");
    assert_eq!(event.event_id.as_deref(), Some("evt_stripe"));
    assert_eq!(event.event_type, "payment_intent.succeeded");

    // Shopify
    let body = br#"{"id":123}"#;
    let signature = BASE64.encode(hmac_sha256(SECRET, body));
    let shopify = presets::shopify(SECRET);
    let event = shopify
        .verify(&request(
            tenant(),
            "shopify",
            headers(&[
                ("x-shopify-hmac-sha256", &signature),
                ("x-shopify-event-id", "evt_shop"),
                ("x-shopify-topic", "orders/create"),
            ]),
            body,
        ))
        .expect("Shopify signature");
    assert_eq!(event.event_id.as_deref(), Some("evt_shop"));
    assert_eq!(event.event_type, "orders/create");

    // Twilio: HMAC-SHA1 over the URL plus sorted form fields.
    let body = b"MessageSid=SM123&MessageStatus=delivered";
    let url = "https://example.test/hook";
    let mut message = url.as_bytes().to_vec();
    message.extend_from_slice(b"MessageSidSM123MessageStatusdelivered");
    let signature = BASE64.encode(hmac_sha1(SECRET, &message));
    let twilio = presets::twilio(SECRET);
    let event = twilio
        .verify(&request(
            tenant(),
            "twilio",
            headers(&[("x-twilio-signature", &signature), ("x-forwarded-url", url)]),
            body,
        ))
        .expect("Twilio signature");
    assert_eq!(event.event_id.as_deref(), Some("SM123"));
    assert_eq!(event.event_type, "delivered");

    // Slack
    let body = br#"{"event_id":"evt_slack","type":"event_callback"}"#;
    let seconds = unix_now().to_string();
    let mut message = format!("v0:{seconds}:").into_bytes();
    message.extend_from_slice(body);
    let signature = format!("v0={}", signature_hex(&hmac_sha256(SECRET, &message)));
    let slack = presets::slack(SECRET);
    let event = slack
        .verify(&request(
            tenant(),
            "slack",
            headers(&[
                ("x-slack-signature", &signature),
                ("x-slack-request-timestamp", &seconds),
            ]),
            body,
        ))
        .expect("Slack signature");
    assert_eq!(event.event_id.as_deref(), Some("evt_slack"));

    // GitHub
    let body = br#"{"zen":"keep it simple"}"#;
    let signature = format!("sha256={}", signature_hex(&hmac_sha256(SECRET, body)));
    let github = presets::github(SECRET);
    let event = github
        .verify(&request(
            tenant(),
            "github",
            headers(&[
                ("x-hub-signature-256", &signature),
                ("x-github-delivery", "evt_gh"),
                ("x-github-event", "push"),
            ]),
            body,
        ))
        .expect("GitHub signature");
    assert_eq!(event.event_id.as_deref(), Some("evt_gh"));
    assert_eq!(event.event_type, "push");

    // Generic
    let body = br#"{"id":"evt_generic","type":"ping"}"#;
    let signature = signature_hex(&hmac_sha256(SECRET, body));
    let generic = presets::generic_sha256(SECRET, HeaderName::from_static("x-custom-signature"));
    let event = generic
        .verify(&request(
            tenant(),
            "generic",
            headers(&[("x-custom-signature", &signature)]),
            body,
        ))
        .expect("generic signature");
    assert_eq!(event.event_id.as_deref(), Some("evt_generic"));
}

// AC-011
#[test]
fn ac_011_the_presets_feature_ships_every_provider_verifier() {
    // With `presets` enabled (the default) every documented verifier exists and
    // accepts a request. Absence in the core-only build is proven separately by
    // `cargo check -p hookgate --no-default-features`, because a runtime test
    // cannot assert that a module is missing.
    let body = br#"{"object":"page","entry":[{"id":"evt"}]}"#;
    let signature = format!("sha256={}", signature_hex(&hmac_sha256(SECRET, body)));
    let verifiers: Vec<Box<dyn Verifier>> = vec![
        Box::new(presets::meta_graph(SECRET)),
        Box::new(presets::stripe(SECRET)),
        Box::new(presets::shopify(SECRET)),
        Box::new(presets::twilio(SECRET)),
        Box::new(presets::slack(SECRET)),
        Box::new(presets::github(SECRET)),
        Box::new(presets::generic_sha256(
            SECRET,
            HeaderName::from_static("x-signature"),
        )),
    ];
    assert_eq!(
        verifiers.len(),
        7,
        "six providers plus the generic verifier"
    );
    assert!(
        verifiers
            .iter()
            .any(|verifier| verifier.provider() == ProviderId::new("meta")),
        "the Meta verifier is part of the preset surface"
    );

    let meta = presets::meta_graph(SECRET);
    assert!(meta
        .verify(&request(
            tenant(),
            "meta",
            headers(&[("x-hub-signature-256", &signature)]),
            body,
        ))
        .is_ok());

    // The gate itself: lib.rs exposes `presets` only under the feature.
    let lib = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("lib.rs is readable");
    assert!(
        lib.contains("#[cfg(feature = \"presets\")]"),
        "the presets re-export is feature-gated"
    );
}

// AC-012
#[tokio::test]
async fn ac_012_one_tenant_cannot_read_another_tenants_delivery() {
    let store = MemoryDeliveryStore::new();
    let owner = tenant();
    let stranger = tenant();
    let event = signed_event(owner, "evt_private");

    store
        .insert_if_new(
            DeliveryRecord::from_event(&event).expect("id"),
            OutboxMessage::from_event(&event),
        )
        .await
        .expect("insert");

    assert!(
        store
            .get(stranger, event.delivery)
            .await
            .expect("read")
            .is_none(),
        "a foreign tenant cannot read the delivery"
    );
    assert!(
        store
            .dead_letters(stranger)
            .await
            .expect("dead letters")
            .is_empty(),
        "a foreign tenant has no dead letters"
    );
    assert!(
        store
            .get(owner, event.delivery)
            .await
            .expect("read")
            .is_some(),
        "the owner still reads it"
    );
}

// AC-013
#[test]
fn ac_013_errors_and_debug_never_render_secret_material() {
    let redacted = redact_error("secret=topsecret token=abc123 signature=deadbeef");
    assert!(!redacted.contains("topsecret"), "the secret is masked");
    assert!(!redacted.contains("abc123"), "the token is masked");
    assert!(!redacted.contains("deadbeef"), "the signature is masked");
    assert!(redacted.contains("[REDACTED]"));

    let preset = presets::generic_sha256(SECRET, HeaderName::from_static("x-signature"));
    let debug = format!("{preset:?}");
    assert!(
        !debug.contains("whsec_test_secret_value"),
        "Debug does not render the shared secret"
    );
    assert!(debug.contains("[REDACTED]"), "Debug marks the redaction");

    let error = VerifyError::Config("no secret material".to_owned());
    assert!(!format!("{error}").contains("whsec_test_secret_value"));
}

// AC-014
#[test]
fn ac_014_crate_hygiene_and_published_metadata() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(root.join("src/lib.rs")).expect("lib.rs is readable");
    assert!(lib.contains("#![forbid(unsafe_code)]"));
    assert!(lib.contains("#![deny(missing_docs)]"));

    for licence in ["LICENSE-MIT", "LICENSE-APACHE"] {
        assert!(root.join(licence).exists(), "{licence} is shipped");
    }

    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("manifest is readable");
    assert!(manifest.contains("edition.workspace = true"));
    assert!(manifest.contains("rust-version.workspace = true"));
}

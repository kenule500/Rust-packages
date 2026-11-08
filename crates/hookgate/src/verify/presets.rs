//! Provider HMAC presets.
//!
//! Each preset is an [`HmacPreset`] with the provider's documented header,
//! encoding, prefix, digest, and signed payload shape. Callers can override any
//! field with the builder methods.

use http::header::HeaderName;

use crate::verify::{
    FieldSource, HmacDigest, HmacPreset, SignatureEncoding, SignedPayload, VerifyError,
};
use crate::ProviderId;

/// Meta Graph webhooks: `X-Hub-Signature-256`, hex after `sha256=`, SHA-256
/// over the raw body.
pub fn meta_graph(secret: impl AsRef<[u8]>) -> HmacPreset {
    HmacPreset::new(ProviderId::new("meta"), secret)
        .header(HeaderName::from_static("x-hub-signature-256"))
        .encoding(SignatureEncoding::Hex)
        .prefix("sha256=")
        .digest(HmacDigest::Sha256)
        .signed_payload(SignedPayload::Body)
        .event_id(FieldSource::JsonPointer("/entry/0/id"))
        .event_type(FieldSource::JsonPointer("/object"))
}

/// Stripe webhooks: `Stripe-Signature` with `t` and one or more `v1` values,
/// SHA-256 over `{t}.{body}`.
pub fn stripe(secret: impl AsRef<[u8]>) -> HmacPreset {
    HmacPreset::new(ProviderId::new("stripe"), secret)
        .header(HeaderName::from_static("stripe-signature"))
        .encoding(SignatureEncoding::Hex)
        .digest(HmacDigest::Sha256)
        .signed_payload(SignedPayload::Stripe)
        .event_id(FieldSource::JsonPointer("/id"))
        .event_type(FieldSource::JsonPointer("/type"))
}

/// Shopify webhooks: `X-Shopify-Hmac-Sha256`, standard base64, SHA-256 over
/// the raw body.
pub fn shopify(secret: impl AsRef<[u8]>) -> HmacPreset {
    HmacPreset::new(ProviderId::new("shopify"), secret)
        .header(HeaderName::from_static("x-shopify-hmac-sha256"))
        .encoding(SignatureEncoding::Base64)
        .digest(HmacDigest::Sha256)
        .signed_payload(SignedPayload::Body)
        .event_id(FieldSource::Header(HeaderName::from_static(
            "x-shopify-event-id",
        )))
        .event_type(FieldSource::Header(HeaderName::from_static(
            "x-shopify-topic",
        )))
}

/// Twilio webhooks: `X-Twilio-Signature`, standard base64, HMAC-SHA1 over the
/// externally visible URL plus sorted form fields.
///
/// The caller must forward the public URL in `X-Forwarded-Url` (override it
/// with [`HmacPreset::signed_payload`] and a different
/// [`SignedPayload::TwilioForm`] header).
pub fn twilio(secret: impl AsRef<[u8]>) -> HmacPreset {
    HmacPreset::new(ProviderId::new("twilio"), secret)
        .header(HeaderName::from_static("x-twilio-signature"))
        .encoding(SignatureEncoding::Base64)
        .digest(HmacDigest::Sha1)
        .signed_payload(SignedPayload::TwilioForm {
            url_header: HeaderName::from_static("x-forwarded-url"),
        })
        .event_id(FieldSource::FirstFormField(&[
            "MessageSid",
            "CallSid",
            "SmsSid",
        ]))
        .event_type(FieldSource::FirstFormField(&[
            "EventType",
            "MessageStatus",
            "CallStatus",
        ]))
}

/// Slack events: `X-Slack-Signature` with a `v0=` prefix, hex, SHA-256 over
/// `v0:{timestamp}:{body}`. The timestamp is read from
/// `X-Slack-Request-Timestamp`.
pub fn slack(secret: impl AsRef<[u8]>) -> HmacPreset {
    HmacPreset::new(ProviderId::new("slack"), secret)
        .header(HeaderName::from_static("x-slack-signature"))
        .encoding(SignatureEncoding::Hex)
        .prefix("v0=")
        .digest(HmacDigest::Sha256)
        .signed_payload(SignedPayload::Slack {
            timestamp_header: HeaderName::from_static("x-slack-request-timestamp"),
        })
        .event_id(FieldSource::JsonPointer("/event_id"))
        .event_type(FieldSource::JsonPointer("/type"))
}

/// GitHub webhooks: `X-Hub-Signature-256`, hex after `sha256=`, SHA-256 over
/// the raw body.
pub fn github(secret: impl AsRef<[u8]>) -> HmacPreset {
    HmacPreset::new(ProviderId::new("github"), secret)
        .header(HeaderName::from_static("x-hub-signature-256"))
        .encoding(SignatureEncoding::Hex)
        .prefix("sha256=")
        .digest(HmacDigest::Sha256)
        .signed_payload(SignedPayload::Body)
        .event_id(FieldSource::Header(HeaderName::from_static(
            "x-github-delivery",
        )))
        .event_type(FieldSource::Header(HeaderName::from_static(
            "x-github-event",
        )))
}

/// A generic HMAC-SHA256 verifier over the raw body.
///
/// The caller supplies the signature header. Use the builder to change the
/// encoding, prefix, digest, payload shape, event sources, or replay window.
pub fn generic_sha256(secret: impl AsRef<[u8]>, header: HeaderName) -> HmacPreset {
    HmacPreset::new(ProviderId::new("generic"), secret)
        .header(header)
        .encoding(SignatureEncoding::Hex)
        .digest(HmacDigest::Sha256)
        .signed_payload(SignedPayload::Body)
        .event_id(FieldSource::JsonPointer("/id"))
        .event_type(FieldSource::JsonPointer("/type"))
}

/// Returns the preset name and a short description, for diagnostics.
pub fn describe(name: &str) -> Result<&'static str, VerifyError> {
    match name {
        "meta" => Ok("Meta Graph X-Hub-Signature-256, SHA-256, hex"),
        "stripe" => Ok("Stripe t/v1 signature, SHA-256, hex"),
        "shopify" => Ok("Shopify X-Shopify-Hmac-Sha256, SHA-256, base64"),
        "twilio" => Ok("Twilio X-Twilio-Signature, SHA-1, base64"),
        "slack" => Ok("Slack v0 signature, SHA-256, hex"),
        "github" => Ok("GitHub X-Hub-Signature-256, SHA-256, hex"),
        "generic" => Ok("Generic HMAC-SHA256 over the raw body"),
        other => Err(VerifyError::Config(format!("unknown preset `{other}`"))),
    }
}

//! Raw-byte HMAC verification, replay windows, and signature presets.

use std::fmt;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use http::{HeaderMap, HeaderName};
use serde_json::Value;
use sha1::Sha1;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use thiserror::Error;

use crate::{DeliveryId, InboundRequest, ProviderId, VerifiedEvent};

#[cfg(feature = "presets")]
pub mod presets;

/// A signature byte encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignatureEncoding {
    /// Lowercase or uppercase hexadecimal.
    Hex,
    /// Standard base64 with padding.
    Base64,
}

/// The HMAC digest used by a preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HmacDigest {
    /// HMAC-SHA1. Used by the Twilio preset.
    Sha1,
    /// HMAC-SHA256.
    Sha256,
}

/// The bytes a provider signs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignedPayload {
    /// The raw request body.
    Body,
    /// Stripe's `t` timestamp followed by a dot and the raw body.
    Stripe,
    /// Slack's `v0:{timestamp}:{body}` shape.
    Slack {
        /// The header carrying the Unix timestamp.
        timestamp_header: HeaderName,
    },
    /// Twilio's externally visible URL plus sorted form fields.
    TwilioForm {
        /// The header carrying the externally visible request URL.
        url_header: HeaderName,
    },
}

/// Where a verified event's id or type comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldSource {
    /// A JSON pointer, for example `/data/id`.
    JsonPointer(&'static str),
    /// A request header.
    Header(HeaderName),
    /// A form field in the parsed payload.
    FormField(&'static str),
    /// The first present form field.
    FirstFormField(&'static [&'static str]),
    /// A fixed value.
    Fixed(&'static str),
    /// The field is absent.
    Missing,
}

/// A configurable HMAC verifier.
///
/// The preset captures the header, encoding, optional prefix, digest, signed
/// payload shape, replay window, and where the event id and type live. All
/// presets share this implementation.
pub struct HmacPreset {
    provider: ProviderId,
    secret: Vec<u8>,
    header: HeaderName,
    encoding: SignatureEncoding,
    prefix: Option<String>,
    digest: HmacDigest,
    signed_payload: SignedPayload,
    replay: ReplayWindow,
    event_id: FieldSource,
    event_type: FieldSource,
}

impl HmacPreset {
    /// Creates a generic HMAC-SHA256 verifier over the raw body.
    ///
    /// Defaults: hex encoding, no prefix, `x-signature` header, five-minute
    /// tolerance, one MiB body limit, and `/id` plus `/type` JSON pointers.
    pub fn new(provider: ProviderId, secret: impl AsRef<[u8]>) -> Self {
        Self {
            provider,
            secret: secret.as_ref().to_vec(),
            header: HeaderName::from_static("x-signature"),
            encoding: SignatureEncoding::Hex,
            prefix: None,
            digest: HmacDigest::Sha256,
            signed_payload: SignedPayload::Body,
            replay: ReplayWindow::default(),
            event_id: FieldSource::JsonPointer("/id"),
            event_type: FieldSource::JsonPointer("/type"),
        }
    }

    /// Sets the signature header.
    pub fn header(mut self, header: HeaderName) -> Self {
        self.header = header;
        self
    }

    /// Sets the signature encoding.
    pub fn encoding(mut self, encoding: SignatureEncoding) -> Self {
        self.encoding = encoding;
        self
    }

    /// Sets a required signature prefix such as `sha256=` or `v0=`.
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Sets the HMAC digest.
    pub fn digest(mut self, digest: HmacDigest) -> Self {
        self.digest = digest;
        self
    }

    /// Sets the signed payload shape.
    pub fn signed_payload(mut self, signed_payload: SignedPayload) -> Self {
        self.signed_payload = signed_payload;
        self
    }

    /// Sets the replay window and body limit.
    pub fn replay_window(mut self, replay: ReplayWindow) -> Self {
        self.replay = replay;
        self
    }

    /// Sets where the provider event id is read.
    pub fn event_id(mut self, source: FieldSource) -> Self {
        self.event_id = source;
        self
    }

    /// Sets where the provider event type is read.
    pub fn event_type(mut self, source: FieldSource) -> Self {
        self.event_type = source;
        self
    }

    fn decode(&self, signature: &str) -> Result<Vec<u8>, VerifyError> {
        let signature = match self.prefix.as_deref() {
            Some(prefix) => signature
                .strip_prefix(prefix)
                .ok_or(VerifyError::InvalidSignature)?,
            None => signature,
        };
        let signature = signature.trim();

        match self.encoding {
            SignatureEncoding::Hex => hex::decode(signature).map_err(|_| {
                VerifyError::MalformedHeader("signature is not valid hexadecimal".to_owned())
            }),
            SignatureEncoding::Base64 => BASE64.decode(signature).map_err(|_| {
                VerifyError::MalformedHeader("signature is not valid base64".to_owned())
            }),
        }
    }

    fn verify_standard(&self, signature: &str, message: &[u8]) -> Result<(), VerifyError> {
        let provided = self.decode(signature)?;
        let expected = compute(self.digest, &self.secret, message)?;
        if constant_time_eq(&provided, &expected) {
            Ok(())
        } else {
            Err(VerifyError::InvalidSignature)
        }
    }
}

impl fmt::Debug for HmacPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HmacPreset")
            .field("provider", &self.provider)
            .field("header", &self.header)
            .field("encoding", &self.encoding)
            .field("prefix", &self.prefix)
            .field("digest", &self.digest)
            .field("signed_payload", &self.signed_payload)
            .field("replay", &self.replay)
            .field("event_id", &self.event_id)
            .field("event_type", &self.event_type)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

/// A request verifier.
pub trait Verifier: Send + Sync {
    /// Returns the provider this verifier accepts.
    fn provider(&self) -> ProviderId;

    /// Verifies a raw request and returns the accepted event.
    fn verify(&self, request: &InboundRequest) -> Result<VerifiedEvent, VerifyError>;
}

impl Verifier for HmacPreset {
    fn provider(&self) -> ProviderId {
        self.provider
    }

    fn verify(&self, request: &InboundRequest) -> Result<VerifiedEvent, VerifyError> {
        if request.provider != self.provider {
            return Err(VerifyError::ProviderMismatch {
                expected: self.provider.to_string(),
                actual: request.provider.to_string(),
            });
        }

        self.replay.check_body(request.body.len())?;

        let form_pairs = match &self.signed_payload {
            SignedPayload::TwilioForm { .. } => Some(parse_form(&request.body)),
            _ => None,
        };

        let timestamp = match &self.signed_payload {
            SignedPayload::Stripe => {
                let header = header_str(&request.headers, &self.header)?;
                let (timestamp, _) = parse_stripe_header(header)?;
                timestamp
            }
            SignedPayload::Slack { timestamp_header } => {
                let header = header_str(&request.headers, timestamp_header)?;
                parse_unix(header)?
            }
            SignedPayload::Body | SignedPayload::TwilioForm { .. } => request.received_at,
        };
        self.replay.check_timestamp(timestamp, Utc::now())?;

        match &self.signed_payload {
            SignedPayload::Body => {
                let signature = header_str(&request.headers, &self.header)?;
                self.verify_standard(signature, &request.body)?;
            }
            SignedPayload::Stripe => {
                let header = header_str(&request.headers, &self.header)?;
                let (timestamp, signatures) = parse_stripe_header(header)?;

                let mut message = timestamp.timestamp().to_string().into_bytes();
                message.push(b'.');
                message.extend_from_slice(&request.body);
                let expected = compute(self.digest, &self.secret, &message)?;

                let matched = signatures.iter().any(|signature| {
                    self.decode(signature)
                        .map(|provided| constant_time_eq(&provided, &expected))
                        .unwrap_or(false)
                });
                if !matched {
                    return Err(VerifyError::InvalidSignature);
                }
            }
            SignedPayload::Slack { timestamp_header } => {
                let signature = header_str(&request.headers, &self.header)?;
                let timestamp_header = header_str(&request.headers, timestamp_header)?;

                let mut message = b"v0:".to_vec();
                message.extend_from_slice(timestamp_header.as_bytes());
                message.push(b':');
                message.extend_from_slice(&request.body);

                self.verify_standard(signature, &message)?;
            }
            SignedPayload::TwilioForm { url_header } => {
                let signature = header_str(&request.headers, &self.header)?;
                let url = header_str(&request.headers, url_header)?;
                let pairs = form_pairs
                    .as_ref()
                    .ok_or_else(|| VerifyError::Config("form payload is missing".to_owned()))?;

                let mut sorted = pairs.clone();
                sorted.sort_by(|left, right| left.0.cmp(&right.0));

                let mut message = url.as_bytes().to_vec();
                for (key, value) in &sorted {
                    message.extend_from_slice(key.as_bytes());
                    message.extend_from_slice(value.as_bytes());
                }

                self.verify_standard(signature, &message)?;
            }
        }

        let payload = match form_pairs {
            Some(pairs) => form_to_json(&pairs),
            None => serde_json::from_slice(&request.body).map_err(|error| {
                VerifyError::InvalidPayload(format!("body is not valid JSON: {error}"))
            })?,
        };

        let event_id = resolve_field(&self.event_id, &request.headers, &payload);
        let event_type = resolve_field(&self.event_type, &request.headers, &payload)
            .ok_or_else(|| VerifyError::InvalidPayload("event type is missing".to_owned()))?;

        Ok(VerifiedEvent {
            delivery: DeliveryId::generate(),
            tenant: request.tenant,
            provider: request.provider,
            event_id,
            event_type,
            payload,
            raw: request.body.clone(),
        })
    }
}

/// A configurable timestamp tolerance and body-size limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayWindow {
    tolerance: Duration,
    max_body_bytes: usize,
}

impl ReplayWindow {
    /// Creates a replay window.
    pub const fn new(tolerance: Duration, max_body_bytes: usize) -> Self {
        Self {
            tolerance,
            max_body_bytes,
        }
    }

    /// Returns the timestamp tolerance.
    pub const fn tolerance(&self) -> Duration {
        self.tolerance
    }

    /// Returns the maximum body size in bytes.
    pub const fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
    }

    /// Rejects an oversized body before it is parsed.
    pub fn check_body(&self, actual: usize) -> Result<(), VerifyError> {
        if actual > self.max_body_bytes {
            Err(VerifyError::BodyTooLarge {
                limit: self.max_body_bytes,
                actual,
            })
        } else {
            Ok(())
        }
    }

    /// Rejects a timestamp outside the tolerance in either direction.
    pub fn check_timestamp(
        &self,
        timestamp: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<(), VerifyError> {
        let tolerance = self.tolerance.num_seconds().max(0);
        let age = now.timestamp().saturating_sub(timestamp.timestamp());
        if age > tolerance {
            return Err(VerifyError::Stale { age_secs: age });
        }

        let skew = timestamp.timestamp().saturating_sub(now.timestamp());
        if skew > tolerance {
            return Err(VerifyError::FutureTimestamp { skew_secs: skew });
        }

        Ok(())
    }
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self {
            tolerance: Duration::minutes(5),
            max_body_bytes: 1024 * 1024,
        }
    }
}

/// An error returned by a verifier.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum VerifyError {
    /// A required header is missing.
    #[error("HG-VERIFY-MISSING-HEADER: missing header `{0}`")]
    MissingHeader(String),
    /// A header is present but cannot be parsed.
    #[error("HG-VERIFY-MALFORMED-HEADER: {0}")]
    MalformedHeader(String),
    /// The signature does not match the raw bytes.
    #[error("HG-VERIFY-SIGNATURE: signature did not match")]
    InvalidSignature,
    /// The request is older than the replay window.
    #[error("HG-VERIFY-STALE: request is {age_secs} seconds old")]
    Stale {
        /// The request age in seconds.
        age_secs: i64,
    },
    /// The request timestamp is too far in the future.
    #[error("HG-VERIFY-FUTURE: request is {skew_secs} seconds in the future")]
    FutureTimestamp {
        /// The future skew in seconds.
        skew_secs: i64,
    },
    /// The body exceeds the configured limit.
    #[error("HG-VERIFY-BODY-TOO-LARGE: {actual} bytes exceeds {limit}")]
    BodyTooLarge {
        /// The configured limit.
        limit: usize,
        /// The actual body size.
        actual: usize,
    },
    /// The body is not a payload this preset understands.
    #[error("HG-VERIFY-PAYLOAD: {0}")]
    InvalidPayload(String),
    /// The request belongs to a different provider.
    #[error("HG-VERIFY-PROVIDER-MISMATCH: expected `{expected}`, received `{actual}`")]
    ProviderMismatch {
        /// The verifier's provider.
        expected: String,
        /// The request's provider.
        actual: String,
    },
    /// The verifier configuration is invalid.
    #[error("HG-VERIFY-CONFIG: {0}")]
    Config(String),
}

fn compute(digest: HmacDigest, key: &[u8], message: &[u8]) -> Result<Vec<u8>, VerifyError> {
    match digest {
        HmacDigest::Sha1 => {
            let mut mac = Hmac::<Sha1>::new_from_slice(key)
                .map_err(|_| VerifyError::Config("invalid HMAC key".to_owned()))?;
            mac.update(message);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        HmacDigest::Sha256 => {
            let mut mac = Hmac::<Sha256>::new_from_slice(key)
                .map_err(|_| VerifyError::Config("invalid HMAC key".to_owned()))?;
            mac.update(message);
            Ok(mac.finalize().into_bytes().to_vec())
        }
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && bool::from(left.ct_eq(right))
}

fn header_str<'a>(headers: &'a HeaderMap, name: &HeaderName) -> Result<&'a str, VerifyError> {
    let value = headers
        .get(name)
        .ok_or_else(|| VerifyError::MissingHeader(name.as_str().to_owned()))?;
    value.to_str().map_err(|_| {
        VerifyError::MalformedHeader(format!("header `{}` is not valid ASCII", name.as_str()))
    })
}

fn parse_stripe_header(value: &str) -> Result<(DateTime<Utc>, Vec<String>), VerifyError> {
    let mut timestamp = None;
    let mut signatures = Vec::new();

    for part in value.split(',') {
        if let Some((key, value)) = part.trim().split_once('=') {
            match key {
                "t" => timestamp = Some(parse_unix(value)?),
                "v1" => signatures.push(value.to_owned()),
                _ => {}
            }
        }
    }

    let timestamp = timestamp.ok_or_else(|| {
        VerifyError::MalformedHeader("Stripe-Signature has no t value".to_owned())
    })?;
    if signatures.is_empty() {
        return Err(VerifyError::MalformedHeader(
            "Stripe-Signature has no v1 value".to_owned(),
        ));
    }

    Ok((timestamp, signatures))
}

fn parse_unix(value: &str) -> Result<DateTime<Utc>, VerifyError> {
    let seconds: i64 = value
        .trim()
        .parse()
        .map_err(|_| VerifyError::MalformedHeader(format!("`{value}` is not a Unix timestamp")))?;
    DateTime::from_timestamp(seconds, 0).ok_or_else(|| {
        VerifyError::MalformedHeader(format!("`{value}` is outside the supported time range"))
    })
}

fn resolve_field(source: &FieldSource, headers: &HeaderMap, payload: &Value) -> Option<String> {
    match source {
        FieldSource::Missing => None,
        FieldSource::Fixed(value) => Some((*value).to_owned()),
        FieldSource::JsonPointer(pointer) => payload.pointer(pointer).and_then(value_to_string),
        FieldSource::Header(name) => headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        FieldSource::FormField(name) => payload.get(*name).and_then(value_to_string),
        FieldSource::FirstFormField(names) => names
            .iter()
            .find_map(|name| payload.get(*name).and_then(value_to_string)),
    }
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn parse_form(body: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(body);
    text.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

fn form_to_json(pairs: &[(String, String)]) -> Value {
    let mut object = serde_json::Map::new();
    for (key, value) in pairs {
        object.insert(key.clone(), Value::String(value.clone()));
    }
    Value::Object(object)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                match (hex_value(bytes[index + 1]), hex_value(bytes[index + 2])) {
                    (Some(high), Some(low)) => {
                        output.push((high << 4) | low);
                        index += 3;
                    }
                    _ => {
                        output.push(b'%');
                        index += 1;
                    }
                }
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

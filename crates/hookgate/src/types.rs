use std::fmt;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::HeaderMap;
use serde_json::Value;
use uuid::Uuid;

pub use platform_core::{ProviderId, TenantId};

/// The identity of one inbound delivery.
///
/// A delivery identifier is generated when the request is verified and stays
/// stable through retries, dead-lettering, and replay. Consumers must be
/// idempotent on this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeliveryId(Uuid);

impl DeliveryId {
    /// Generates a new time-ordered delivery identifier.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wraps an existing UUID.
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Returns the underlying UUID.
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for DeliveryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A caller-supplied idempotency key for an inbound operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Creates an idempotency key.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One raw inbound webhook request.
///
/// The body is owned as [`Bytes`] and is never re-serialized before
/// verification. `received_at` is the instant the application accepted the
/// request; providers without a signed timestamp are checked against it.
#[derive(Clone)]
pub struct InboundRequest {
    /// The tenant that owns the connection.
    pub tenant: TenantId,
    /// The provider family that sent the request.
    pub provider: ProviderId,
    /// The request headers.
    pub headers: HeaderMap,
    /// The raw request body.
    pub body: Bytes,
    /// The instant the application accepted the request.
    pub received_at: DateTime<Utc>,
}

impl InboundRequest {
    /// Creates a request stamped at the current instant.
    pub fn new(
        tenant: TenantId,
        provider: ProviderId,
        headers: HeaderMap,
        body: impl Into<Bytes>,
    ) -> Self {
        Self {
            tenant,
            provider,
            headers,
            body: body.into(),
            received_at: Utc::now(),
        }
    }

    /// Replaces the receive instant.
    pub fn with_received_at(mut self, received_at: DateTime<Utc>) -> Self {
        self.received_at = received_at;
        self
    }
}

impl fmt::Debug for InboundRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InboundRequest")
            .field("tenant", &self.tenant)
            .field("provider", &self.provider)
            .field("headers", &self.headers)
            .field("body_len", &self.body.len())
            .field("received_at", &self.received_at)
            .finish()
    }
}

/// A request whose signature and replay window passed.
#[derive(Clone)]
pub struct VerifiedEvent {
    /// The stable delivery identifier for this event.
    pub delivery: DeliveryId,
    /// The tenant that owns the connection.
    pub tenant: TenantId,
    /// The provider family that sent the request.
    pub provider: ProviderId,
    /// The provider's event identifier, when the payload supplies one.
    pub event_id: Option<String>,
    /// The provider's event type.
    pub event_type: String,
    /// The parsed JSON payload. Twilio form bodies are converted to an object.
    pub payload: Value,
    /// The exact bytes that were verified.
    pub raw: Bytes,
}

impl fmt::Debug for VerifiedEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedEvent")
            .field("delivery", &self.delivery)
            .field("tenant", &self.tenant)
            .field("provider", &self.provider)
            .field("event_id", &self.event_id)
            .field("event_type", &self.event_type)
            .field("payload", &"<redacted>")
            .field("raw_len", &self.raw.len())
            .finish()
    }
}

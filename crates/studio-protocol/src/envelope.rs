//! Envelope, identifiers, and JSON encoding.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ProtocolError;
use crate::message::Message;
use crate::version::PROTOCOL_VERSION;

/// A UUID v7 message identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(Uuid);

impl MessageId {
    /// Generates a new time-ordered message ID.
    #[must_use]
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }

    /// Creates an ID from an existing UUID.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A UUID v7 participant identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParticipantId(Uuid);

impl ParticipantId {
    /// Generates a new time-ordered participant ID.
    #[must_use]
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }

    /// Creates an ID from an existing UUID.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ParticipantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A validated tenant identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TenantId(String);

impl TenantId {
    /// Validates and creates a tenant identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if !is_safe_identifier(&value) {
            return Err(ProtocolError::Decode {
                detail: "tenant id must be 1-64 ASCII alphanumerics, '-' or '_'".to_string(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
}

/// One protocol message and its routing metadata.
///
/// Unknown fields are ignored so a higher minor version can add envelope
/// fields without breaking older readers. Payload structs are stricter and use
/// `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    /// Protocol version integer.
    pub v: u16,
    /// Unique message identifier.
    pub id: MessageId,
    /// Creation time in UTC.
    pub at: DateTime<Utc>,
    /// Sending participant.
    pub from: ParticipantId,
    /// Message payload.
    pub body: Message,
}

impl Envelope {
    /// Creates a current-version envelope with a fresh ID and timestamp.
    #[must_use]
    pub fn new(from: ParticipantId, body: Message) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: MessageId::new_v7(),
            at: Utc::now(),
            from,
            body,
        }
    }

    /// Replaces the message ID, for deterministic logs.
    #[must_use]
    pub fn with_id(mut self, id: MessageId) -> Self {
        self.id = id;
        self
    }

    /// Replaces the timestamp, for deterministic logs.
    #[must_use]
    pub fn with_at(mut self, at: DateTime<Utc>) -> Self {
        self.at = at;
        self
    }

    /// Replaces the protocol version, for negotiation tests.
    #[must_use]
    pub fn with_version(mut self, version: u16) -> Self {
        self.v = version;
        self
    }

    /// Encodes the envelope as JSON.
    pub fn to_json(&self) -> Result<String, ProtocolError> {
        serde_json::to_string(self).map_err(|error| ProtocolError::Encode {
            detail: error.to_string(),
        })
    }

    /// Decodes an envelope from JSON.
    pub fn from_json(text: &str) -> Result<Self, ProtocolError> {
        serde_json::from_str(text).map_err(|error| ProtocolError::Decode {
            detail: error.to_string(),
        })
    }
}

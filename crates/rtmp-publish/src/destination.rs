//! Destination model, identifiers, and the redacted view.
//!
//! The stream key is a [`SecretString`]: it has no `Serialize` implementation
//! and both `Debug` and `Display` print `***`. [`RedactedDestination`] is the
//! only serialisable view.

use std::fmt;

use url::Url;

use crate::error::PublishError;
use crate::secret::SecretString;
use crate::validate::{self, is_safe_identifier, redacted_endpoint};

/// A validated destination identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DestinationId(String);

impl DestinationId {
    /// Validates and creates a destination identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, PublishError> {
        let value = value.into();
        if !is_safe_identifier(&value) {
            return Err(PublishError::InvalidId {
                detail: "destination id must be 1-64 ASCII alphanumerics, '-' or '_'".to_string(),
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

impl fmt::Display for DestinationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A supported publishing protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Protocol {
    /// Plain RTMP.
    Rtmp,
    /// RTMP over TLS.
    Rtmps,
    /// SRT.
    Srt,
    /// WHIP over HTTPS.
    Whip,
}

impl Protocol {
    /// Returns the stable lower-case name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rtmp => "rtmp",
            Self::Rtmps => "rtmps",
            Self::Srt => "srt",
            Self::Whip => "whip",
        }
    }

    /// Returns whether this protocol needs an HTTP probe rather than a TCP
    /// connect.
    #[must_use]
    pub const fn is_http(self) -> bool {
        matches!(self, Self::Whip)
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A publishing destination with an optional backup chain.
pub struct Destination {
    id: DestinationId,
    label: String,
    protocol: Protocol,
    url: Url,
    key: SecretString,
    region: Option<String>,
    backup: Option<Box<Destination>>,
}

impl Destination {
    /// Parses and validates a destination.
    ///
    /// The URL must carry no credentials; the key is a separate
    /// [`SecretString`]. Validation runs before any connection attempt.
    pub fn new(
        id: DestinationId,
        label: impl Into<String>,
        protocol: Protocol,
        url: &str,
        key: SecretString,
    ) -> Result<Self, PublishError> {
        let url = Url::parse(url).map_err(|error| PublishError::InvalidUrl {
            detail: error.to_string(),
        })?;
        let destination = Self {
            id,
            label: label.into(),
            protocol,
            url,
            key,
            region: None,
            backup: None,
        };
        validate::validate(&destination)?;
        Ok(destination)
    }

    /// Re-validates the destination after construction.
    pub fn validate(&self) -> Result<(), PublishError> {
        validate::validate(self)
    }

    /// Adds or replaces the region.
    pub fn with_region(mut self, region: impl Into<String>) -> Result<Self, PublishError> {
        self.region = Some(region.into());
        validate::validate(&self)?;
        Ok(self)
    }

    /// Adds a backup destination and re-validates the chain.
    pub fn with_backup(mut self, backup: Destination) -> Result<Self, PublishError> {
        self.backup = Some(Box::new(backup));
        validate::validate(&self)?;
        Ok(self)
    }

    /// Returns the identifier.
    #[must_use]
    pub fn id(&self) -> &DestinationId {
        &self.id
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the protocol.
    #[must_use]
    pub const fn protocol(&self) -> Protocol {
        self.protocol
    }

    /// Returns the parsed URL. The URL contains no credentials.
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Returns the stream key. Do not log the result.
    #[must_use]
    pub fn key(&self) -> &SecretString {
        &self.key
    }

    /// Returns the region.
    #[must_use]
    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }

    /// Returns the backup destination.
    #[must_use]
    pub fn backup(&self) -> Option<&Destination> {
        self.backup.as_deref()
    }

    /// Returns the effective host.
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        self.url.host_str()
    }

    /// Returns the effective port.
    #[must_use]
    pub fn port(&self) -> u16 {
        validate::effective_port(&self.url, self.url.scheme()).unwrap_or(0)
    }

    /// Returns a credential-free endpoint string.
    #[must_use]
    pub fn redacted_endpoint(&self) -> String {
        redacted_endpoint(&self.url)
    }

    /// Returns the serialisable, credential-free view.
    #[must_use]
    pub fn redacted(&self) -> RedactedDestination {
        RedactedDestination {
            id: self.id.to_string(),
            label: self.label.clone(),
            protocol: self.protocol,
            endpoint: self.redacted_endpoint(),
            region: self.region.clone(),
            has_key: !self.key.is_empty(),
            backup: self
                .backup
                .as_ref()
                .map(|backup| Box::new(backup.redacted())),
        }
    }
}

impl fmt::Debug for Destination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Destination")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("protocol", &self.protocol)
            .field("endpoint", &self.redacted_endpoint())
            .field("key", &self.key)
            .field("region", &self.region)
            .field("backup", &self.backup)
            .finish()
    }
}

impl fmt::Display for Destination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}] ({})",
            self.label,
            self.id,
            self.redacted_endpoint()
        )
    }
}

/// The only serialisable destination view.
///
/// It carries the endpoint, label, region, and backup chain, but never the
/// stream key. `has_key` only reports presence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct RedactedDestination {
    /// Destination identifier.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Protocol.
    pub protocol: Protocol,
    /// Credential-free endpoint.
    pub endpoint: String,
    /// Optional region.
    pub region: Option<String>,
    /// Whether a key is configured.
    pub has_key: bool,
    /// Redacted backup chain.
    pub backup: Option<Box<RedactedDestination>>,
}

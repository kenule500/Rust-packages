//! Typed publishing errors with stable codes.
//!
//! Error payloads never contain a stream key, a secret, or a full destination
//! URL. Only redacted identifiers and reasons are exposed.

use std::fmt;

/// A rejected destination, policy, probe, or hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishError {
    /// A destination or host identifier is malformed.
    InvalidId {
        /// Human-readable detail.
        detail: String,
    },
    /// The URL could not be parsed.
    InvalidUrl {
        /// Human-readable detail.
        detail: String,
    },
    /// The URL scheme is not supported.
    UnsupportedScheme {
        /// The rejected scheme.
        scheme: String,
    },
    /// The protocol and URL scheme disagree.
    SchemeMismatch {
        /// The declared protocol.
        protocol: &'static str,
        /// The URL scheme.
        scheme: String,
    },
    /// The URL has no host.
    MissingHost,
    /// The URL has no explicit port and no known default.
    MissingPort,
    /// The stream key is absent.
    MissingKey,
    /// The stream key has an invalid shape.
    InvalidKey {
        /// Human-readable detail.
        detail: String,
    },
    /// The URL carries credentials in userinfo or a query parameter.
    CredentialsInUrl,
    /// A backup repeats a destination already present in the chain.
    BackupCycle {
        /// The repeated destination identifier.
        id: String,
    },
    /// The backup chain is too deep.
    BackupDepth {
        /// Maximum accepted depth.
        max: usize,
    },
    /// A retry or breaker policy is invalid.
    InvalidPolicy {
        /// Human-readable detail.
        detail: String,
    },
    /// A bitrate hint is invalid.
    InvalidHint {
        /// Human-readable detail.
        detail: String,
    },
    /// This build has no probe implementation for the protocol.
    ProbeUnsupported {
        /// The protocol that cannot be probed.
        protocol: &'static str,
    },
    /// A label is empty or too long.
    InvalidLabel {
        /// Human-readable detail.
        detail: String,
    },
}

impl PublishError {
    /// Returns the stable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "RTMP-001",
            Self::InvalidUrl { .. } => "RTMP-002",
            Self::UnsupportedScheme { .. } => "RTMP-003",
            Self::SchemeMismatch { .. } => "RTMP-004",
            Self::MissingHost => "RTMP-005",
            Self::MissingPort => "RTMP-006",
            Self::MissingKey => "RTMP-007",
            Self::InvalidKey { .. } => "RTMP-008",
            Self::CredentialsInUrl => "RTMP-009",
            Self::BackupCycle { .. } => "RTMP-010",
            Self::BackupDepth { .. } => "RTMP-011",
            Self::InvalidPolicy { .. } => "RTMP-012",
            Self::InvalidHint { .. } => "RTMP-013",
            Self::ProbeUnsupported { .. } => "RTMP-014",
            Self::InvalidLabel { .. } => "RTMP-015",
        }
    }
}

impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.code())?;
        match self {
            Self::InvalidId { detail } => write!(f, "invalid destination id: {detail}"),
            Self::InvalidUrl { detail } => write!(f, "invalid destination URL: {detail}"),
            Self::UnsupportedScheme { scheme } => write!(f, "unsupported URL scheme {scheme}"),
            Self::SchemeMismatch { protocol, scheme } => {
                write!(f, "{protocol} cannot use the {scheme} scheme")
            }
            Self::MissingHost => f.write_str("destination URL has no host"),
            Self::MissingPort => f.write_str("destination URL has no port"),
            Self::MissingKey => f.write_str("destination has no stream key"),
            Self::InvalidKey { detail } => write!(f, "invalid stream key: {detail}"),
            Self::CredentialsInUrl => {
                f.write_str("credentials must not appear in the destination URL")
            }
            Self::BackupCycle { id } => write!(f, "backup chain repeats destination {id}"),
            Self::BackupDepth { max } => {
                write!(f, "backup chain exceeds the maximum depth of {max}")
            }
            Self::InvalidPolicy { detail } => write!(f, "invalid policy: {detail}"),
            Self::InvalidHint { detail } => write!(f, "invalid bitrate hint: {detail}"),
            Self::ProbeUnsupported { protocol } => {
                write!(f, "no probe is available for {protocol} in this build")
            }
            Self::InvalidLabel { detail } => write!(f, "invalid label: {detail}"),
        }
    }
}

impl std::error::Error for PublishError {}

//! Typed link errors with stable codes.
//!
//! No variant carries a signing key or a token.

use std::fmt;

/// A signing, claim, store, or redirect failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// The signing key is absent or too short.
    InvalidKey {
        /// Human-readable detail.
        detail: String,
    },
    /// The signing policy is invalid.
    InvalidPolicy {
        /// Human-readable detail.
        detail: String,
    },
    /// The target URL is missing, unparsable, or uses a forbidden scheme.
    InvalidTarget {
        /// Human-readable detail.
        detail: String,
    },
    /// The tenant identifier is malformed.
    InvalidTenant,
    /// A claim text field is empty or exceeds its bound.
    InvalidText {
        /// Field name.
        field: &'static str,
    },
    /// A UTM key is outside the closed set.
    InvalidUtmKey {
        /// The rejected key.
        key: String,
    },
    /// A UTM value is empty or exceeds its bound.
    InvalidUtmValue {
        /// The rejected key.
        key: &'static str,
    },
    /// OS entropy was unavailable.
    Entropy,
    /// Every generated code collided.
    CodeExhausted {
        /// Number of attempts.
        attempts: u32,
    },
    /// A short code is malformed.
    InvalidCode {
        /// Human-readable detail.
        detail: String,
    },
    /// A token is malformed, not base64url, or not a known version.
    InvalidToken {
        /// Human-readable detail.
        detail: String,
    },
    /// The payload or MAC was modified.
    Tampered,
    /// The link is past its expiry plus skew.
    Expired {
        /// Expiry instant in Unix seconds.
        expires_at: i64,
        /// Verification instant in Unix seconds.
        now: i64,
    },
    /// The link is before its issue instant minus skew.
    NotYetValid {
        /// Issue instant in Unix seconds.
        issued_at: i64,
        /// Verification instant in Unix seconds.
        now: i64,
    },
    /// The store reports the code as revoked.
    Revoked,
    /// The single-use nonce was already consumed.
    Consumed,
    /// The store has no active link for the code.
    NotFound,
    /// The store failed.
    Store {
        /// Human-readable detail.
        detail: String,
    },
}

impl LinkError {
    /// Returns the stable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidKey { .. } => "SL-001",
            Self::InvalidPolicy { .. } => "SL-002",
            Self::InvalidTarget { .. } => "SL-003",
            Self::InvalidTenant => "SL-004",
            Self::InvalidText { .. } => "SL-005",
            Self::InvalidUtmKey { .. } => "SL-006",
            Self::InvalidUtmValue { .. } => "SL-007",
            Self::Entropy => "SL-008",
            Self::CodeExhausted { .. } => "SL-009",
            Self::InvalidCode { .. } => "SL-010",
            Self::InvalidToken { .. } => "SL-011",
            Self::Tampered => "SL-012",
            Self::Expired { .. } => "SL-013",
            Self::NotYetValid { .. } => "SL-014",
            Self::Revoked => "SL-015",
            Self::Consumed => "SL-016",
            Self::NotFound => "SL-017",
            Self::Store { .. } => "SL-018",
        }
    }
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.code())?;
        match self {
            Self::InvalidKey { detail } => write!(f, "invalid signing key: {detail}"),
            Self::InvalidPolicy { detail } => write!(f, "invalid policy: {detail}"),
            Self::InvalidTarget { detail } => write!(f, "invalid target: {detail}"),
            Self::InvalidTenant => f.write_str("invalid tenant identifier"),
            Self::InvalidText { field } => write!(f, "invalid {field}"),
            Self::InvalidUtmKey { key } => write!(f, "unknown UTM key {key}"),
            Self::InvalidUtmValue { key } => write!(f, "invalid UTM value for {key}"),
            Self::Entropy => f.write_str("OS entropy is unavailable"),
            Self::CodeExhausted { attempts } => {
                write!(f, "no unique code after {attempts} attempts")
            }
            Self::InvalidCode { detail } => write!(f, "invalid code: {detail}"),
            Self::InvalidToken { detail } => write!(f, "invalid token: {detail}"),
            Self::Tampered => f.write_str("signature or payload was modified"),
            Self::Expired { expires_at, now } => {
                write!(f, "link expired at {expires_at}, now {now}")
            }
            Self::NotYetValid { issued_at, now } => {
                write!(f, "link is not valid until {issued_at}, now {now}")
            }
            Self::Revoked => f.write_str("link is revoked"),
            Self::Consumed => f.write_str("link is already consumed"),
            Self::NotFound => f.write_str("link does not exist"),
            Self::Store { detail } => write!(f, "link store failure: {detail}"),
        }
    }
}

impl std::error::Error for LinkError {}

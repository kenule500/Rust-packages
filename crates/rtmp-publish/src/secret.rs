//! A caller-supplied secret with redacted formatting.
//!
//! `SecretString` deliberately implements neither `Serialize` nor equality.
//! The only way to read the value is [`SecretString::expose_secret`], and both
//! `Debug` and `Display` print `***`.

use std::fmt;

use crate::error::PublishError;

/// A non-empty, shape-checked stream key.
///
/// The crate never persists the key. It lives only in memory for the lifetime
/// of the value and is never formatted.
pub struct SecretString(String);

impl SecretString {
    /// Validates and wraps a stream key.
    ///
    /// Valid keys are 1-512 characters, contain no ASCII control characters or
    /// whitespace, and are not only `*`.
    pub fn new(value: impl Into<String>) -> Result<Self, PublishError> {
        let value = value.into();
        if value.is_empty() {
            return Err(PublishError::MissingKey);
        }
        if value.chars().count() > 512 {
            return Err(PublishError::InvalidKey {
                detail: "stream key exceeds 512 characters".to_string(),
            });
        }
        if value.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(PublishError::InvalidKey {
                detail: "stream key contains control characters or whitespace".to_string(),
            });
        }
        if value.chars().all(|c| c == '*') {
            return Err(PublishError::InvalidKey {
                detail: "stream key is a redaction marker".to_string(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the key. Callers must not log or serialize the result.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Returns the key length in characters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.chars().count()
    }

    /// Returns whether the key is empty. It never is for a constructed value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString(***)")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

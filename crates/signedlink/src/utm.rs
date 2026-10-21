//! Closed-set UTM normalisation.
//!
//! Only `utm_source`, `utm_medium`, `utm_campaign`, `utm_term`, and
//! `utm_content` are accepted. Keys are matched case-insensitively and stored in
//! lower case. Values are trimmed but keep their case. Canonical ordering is
//! by key, so encoding is deterministic.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::error::LinkError;

/// The five accepted UTM keys, in canonical order.
pub const UTM_KEYS: [&str; 5] = [
    "utm_campaign",
    "utm_content",
    "utm_medium",
    "utm_source",
    "utm_term",
];

const MAX_UTM_VALUE: usize = 256;

/// Attribution parameters attached to a link.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Utm {
    /// `utm_source`
    pub source: Option<String>,
    /// `utm_medium`
    pub medium: Option<String>,
    /// `utm_campaign`
    pub campaign: Option<String>,
    /// `utm_term`
    pub term: Option<String>,
    /// `utm_content`
    pub content: Option<String>,
}

impl Utm {
    /// Creates empty UTM parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts one parameter, rejecting unknown keys and invalid values.
    ///
    /// The key is case-insensitive; the value is trimmed and must be 1-256
    /// characters with no control characters.
    pub fn set(&mut self, key: &str, value: impl Into<String>) -> Result<(), LinkError> {
        let normalized = key.to_ascii_lowercase();
        let slot = match normalized.as_str() {
            "utm_source" => &mut self.source,
            "utm_medium" => &mut self.medium,
            "utm_campaign" => &mut self.campaign,
            "utm_term" => &mut self.term,
            "utm_content" => &mut self.content,
            _ => {
                return Err(LinkError::InvalidUtmKey { key: normalized });
            }
        };
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.chars().count() > MAX_UTM_VALUE {
            return Err(LinkError::InvalidUtmValue {
                key: canonical_key(&normalized),
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(LinkError::InvalidUtmValue {
                key: canonical_key(&normalized),
            });
        }
        *slot = Some(trimmed.to_string());
        Ok(())
    }

    /// Builds UTM parameters from pairs.
    pub fn from_pairs<I, K, V>(pairs: I) -> Result<Self, LinkError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: Into<String>,
    {
        let mut utm = Self::new();
        for (key, value) in pairs {
            utm.set(key.as_ref(), value)?;
        }
        Ok(utm)
    }

    /// Returns one value by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        match key.to_ascii_lowercase().as_str() {
            "utm_source" => self.source.as_deref(),
            "utm_medium" => self.medium.as_deref(),
            "utm_campaign" => self.campaign.as_deref(),
            "utm_term" => self.term.as_deref(),
            "utm_content" => self.content.as_deref(),
            _ => None,
        }
    }

    /// Iterates present parameters in canonical key order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [
            ("utm_campaign", self.campaign.as_deref()),
            ("utm_content", self.content.as_deref()),
            ("utm_medium", self.medium.as_deref()),
            ("utm_source", self.source.as_deref()),
            ("utm_term", self.term.as_deref()),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
    }

    /// Returns whether no parameter is present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Returns the canonical percent-encoded suffix.
    ///
    /// The suffix is empty when no parameter is present. Otherwise it begins
    /// with `&` and contains the parameters in canonical order.
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in self.iter() {
            serializer.append_pair(key, value);
        }
        let encoded = serializer.finish();
        if encoded.is_empty() {
            encoded
        } else {
            format!("&{encoded}")
        }
    }

    /// Re-validates every present value.
    pub fn validate(&self) -> Result<(), LinkError> {
        for (key, value) in self.iter() {
            if value.is_empty()
                || value.chars().count() > MAX_UTM_VALUE
                || value.chars().any(char::is_control)
            {
                return Err(LinkError::InvalidUtmValue {
                    key: canonical_key(key),
                });
            }
        }
        Ok(())
    }
}

fn canonical_key(key: &str) -> &'static str {
    match key {
        "utm_source" => "utm_source",
        "utm_medium" => "utm_medium",
        "utm_campaign" => "utm_campaign",
        "utm_term" => "utm_term",
        "utm_content" => "utm_content",
        _ => "utm",
    }
}

//! Claims, tenant identifiers, and the canonical payload encoding.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use base64::Engine;
use url::Url;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::error::LinkError;
use crate::utm::{Utm, UTM_KEYS};

/// Canonical payload format version.
pub const CLAIM_FORMAT_VERSION: &str = "1";

/// Maximum accepted target URL length.
pub const MAX_TARGET_BYTES: usize = 2_048;

/// Maximum attribution window: one year.
pub const MAX_ATTRIBUTION_WINDOW_SECS: u64 = 31_536_000;

const NONCE_BYTES: usize = 16;

/// A validated tenant identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct TenantId(String);

impl TenantId {
    /// Validates and creates a tenant identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, LinkError> {
        let value = value.into();
        if !is_safe_identifier(&value) {
            return Err(LinkError::InvalidTenant);
        }
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The signed payload of a short link.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Claims {
    /// Destination URL.
    pub target: Url,
    /// Optional campaign label.
    pub campaign: Option<String>,
    /// Tenant that owns the link.
    pub tenant: TenantId,
    /// Attribution window explicit in the claims.
    pub attribution_window: Duration,
    /// Whether verification consumes the link.
    pub single_use: bool,
    /// Random nonce, generated once.
    pub nonce: String,
    /// Issued-at Unix seconds. Set by the signer.
    pub issued_at: i64,
    /// Expiry Unix seconds. Set by the signer.
    pub expires_at: i64,
    /// Attribution parameters.
    pub utm: Utm,
}

impl Claims {
    /// Creates claims with a fresh nonce.
    ///
    /// `attribution_window` is explicit; this constructor never invents one.
    pub fn new(
        target: Url,
        tenant: TenantId,
        attribution_window: Duration,
    ) -> Result<Self, LinkError> {
        let claims = Self {
            target,
            campaign: None,
            tenant,
            attribution_window,
            single_use: false,
            nonce: random_nonce()?,
            issued_at: 0,
            expires_at: 0,
            utm: Utm::new(),
        };
        claims.validate()?;
        Ok(claims)
    }

    /// Sets the campaign label.
    pub fn with_campaign(mut self, campaign: impl Into<String>) -> Result<Self, LinkError> {
        let campaign = campaign.into();
        validate_text("campaign", &campaign, 128)?;
        self.campaign = Some(campaign);
        Ok(self)
    }

    /// Enables or disables single use.
    #[must_use]
    pub fn with_single_use(mut self, single_use: bool) -> Self {
        self.single_use = single_use;
        self
    }

    /// Replaces the UTM parameters.
    pub fn with_utm(mut self, utm: Utm) -> Result<Self, LinkError> {
        utm.validate()?;
        self.utm = utm;
        Ok(self)
    }

    /// Returns the attribution window in seconds.
    #[must_use]
    pub fn attribution_window_secs(&self) -> u64 {
        self.attribution_window.as_secs()
    }

    /// Re-validates every field.
    pub fn validate(&self) -> Result<(), LinkError> {
        if !matches!(self.target.scheme(), "http" | "https") {
            return Err(LinkError::InvalidTarget {
                detail: "target scheme must be http or https".to_string(),
            });
        }
        if self.target.as_str().len() > MAX_TARGET_BYTES {
            return Err(LinkError::InvalidTarget {
                detail: "target URL exceeds the size bound".to_string(),
            });
        }
        if self.target.host_str().is_none() {
            return Err(LinkError::InvalidTarget {
                detail: "target has no host".to_string(),
            });
        }
        if !self.target.username().is_empty() || self.target.password().is_some() {
            return Err(LinkError::InvalidTarget {
                detail: "target must not contain credentials".to_string(),
            });
        }
        if let Some(campaign) = &self.campaign {
            validate_text("campaign", campaign, 128)?;
        }
        if !is_nonce(&self.nonce) {
            return Err(LinkError::InvalidText { field: "nonce" });
        }
        let window = self.attribution_window_secs();
        if window == 0 || window > MAX_ATTRIBUTION_WINDOW_SECS {
            return Err(LinkError::InvalidPolicy {
                detail: "attribution window must be 1 second to 1 year".to_string(),
            });
        }
        if self.expires_at < self.issued_at {
            return Err(LinkError::InvalidText { field: "expiry" });
        }
        self.utm.validate()
    }

    /// Returns the canonical payload bytes.
    ///
    /// Field order is fixed and UTM parameters are sorted, so equal claims
    /// always produce identical bytes.
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        serializer.append_pair("v", CLAIM_FORMAT_VERSION);
        serializer.append_pair("target", self.target.as_str());
        serializer.append_pair("tenant", self.tenant.as_str());
        serializer.append_pair(
            "campaign_set",
            if self.campaign.is_some() { "1" } else { "0" },
        );
        serializer.append_pair("campaign", self.campaign.as_deref().unwrap_or(""));
        serializer.append_pair(
            "attribution_window",
            &self.attribution_window_secs().to_string(),
        );
        serializer.append_pair("single_use", if self.single_use { "1" } else { "0" });
        serializer.append_pair("nonce", &self.nonce);
        serializer.append_pair("issued_at", &self.issued_at.to_string());
        serializer.append_pair("expires_at", &self.expires_at.to_string());
        format!("{}{}", serializer.finish(), self.utm.canonical())
    }

    /// Parses a canonical payload.
    ///
    /// Unknown or duplicate fields fail closed. The parser is used only after
    /// the MAC has been verified.
    pub fn parse_canonical(payload: &str) -> Result<Self, LinkError> {
        let mut fields: BTreeMap<String, String> = BTreeMap::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (key, value) in url::form_urlencoded::parse(payload.as_bytes()) {
            let key = key.into_owned();
            if !is_known_field(&key) {
                return Err(LinkError::InvalidToken {
                    detail: format!("unknown field {key}"),
                });
            }
            if !seen.insert(key.clone()) {
                return Err(LinkError::InvalidToken {
                    detail: format!("duplicate field {key}"),
                });
            }
            fields.insert(key, value.into_owned());
        }

        let version = take(&mut fields, "v")?;
        if version != CLAIM_FORMAT_VERSION {
            return Err(LinkError::InvalidToken {
                detail: format!("unsupported format version {version}"),
            });
        }
        let target =
            Url::parse(&take(&mut fields, "target")?).map_err(|_| LinkError::InvalidTarget {
                detail: "target is not a URL".to_string(),
            })?;
        let tenant = TenantId::new(take(&mut fields, "tenant")?)?;
        let campaign_set = take(&mut fields, "campaign_set")?;
        let campaign = match campaign_set.as_str() {
            "0" => {
                if !take(&mut fields, "campaign")?.is_empty() {
                    return Err(LinkError::InvalidToken {
                        detail: "campaign is present without campaign_set".to_string(),
                    });
                }
                None
            }
            "1" => Some(take(&mut fields, "campaign")?),
            _ => {
                return Err(LinkError::InvalidToken {
                    detail: "campaign_set must be 0 or 1".to_string(),
                });
            }
        };
        let window = take(&mut fields, "attribution_window")?
            .parse::<u64>()
            .map_err(|_| LinkError::InvalidToken {
                detail: "invalid attribution window".to_string(),
            })?;
        let single_use = match take(&mut fields, "single_use")?.as_str() {
            "0" => false,
            "1" => true,
            _ => {
                return Err(LinkError::InvalidToken {
                    detail: "single_use must be 0 or 1".to_string(),
                });
            }
        };
        let nonce = take(&mut fields, "nonce")?;
        let issued_at = take(&mut fields, "issued_at")?
            .parse::<i64>()
            .map_err(|_| LinkError::InvalidToken {
                detail: "invalid issued_at".to_string(),
            })?;
        let expires_at = take(&mut fields, "expires_at")?
            .parse::<i64>()
            .map_err(|_| LinkError::InvalidToken {
                detail: "invalid expires_at".to_string(),
            })?;

        let mut utm = Utm::new();
        for key in UTM_KEYS {
            if let Some(value) = fields.remove(key) {
                utm.set(key, value)?;
            }
        }

        let claims = Self {
            target,
            campaign,
            tenant,
            attribution_window: Duration::from_secs(window),
            single_use,
            nonce,
            issued_at,
            expires_at,
            utm,
        };
        claims.validate()?;
        Ok(claims)
    }
}

fn take(fields: &mut BTreeMap<String, String>, key: &str) -> Result<String, LinkError> {
    fields.remove(key).ok_or_else(|| LinkError::InvalidToken {
        detail: format!("missing field {key}"),
    })
}

fn is_known_field(key: &str) -> bool {
    matches!(
        key,
        "v" | "target"
            | "tenant"
            | "campaign_set"
            | "campaign"
            | "attribution_window"
            | "single_use"
            | "nonce"
            | "issued_at"
            | "expires_at"
    ) || UTM_KEYS.contains(&key)
}

fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), LinkError> {
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(LinkError::InvalidText { field });
    }
    Ok(())
}

fn is_safe_identifier(value: &str) -> bool {
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

fn is_nonce(value: &str) -> bool {
    (16..=64).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn random_nonce() -> Result<String, LinkError> {
    let mut bytes = [0_u8; NONCE_BYTES];
    getrandom::getrandom(&mut bytes).map_err(|_| LinkError::Entropy)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

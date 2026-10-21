//! HMAC-SHA256 signing and verification.
//!
//! The token is `base64url(payload).base64url(mac)` with no padding. The MAC
//! covers a domain separator and the canonical payload, and is checked in
//! constant time. The signing key has no `Serialize` implementation and both
//! `Debug` and `Display` print `***`.

use std::fmt;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use url::Url;

use crate::claims::Claims;
use crate::code::Code;
use crate::error::LinkError;
use crate::store::LinkStore;

type HmacSha256 = Hmac<Sha256>;

const DOMAIN: &[u8] = b"m4signedlink.v1\0";

/// Minimum accepted signing-key length in bytes.
pub const MIN_KEY_BYTES: usize = 16;

/// Maximum accepted signing-key length in bytes.
pub const MAX_KEY_BYTES: usize = 1024;

/// Maximum accepted clock skew in seconds.
pub const MAX_CLOCK_SKEW_SECS: u64 = 300;

/// Maximum accepted token length.
pub const MAX_TOKEN_BYTES: usize = 8_192;

/// Maximum link lifetime: one year.
pub const MAX_TTL_SECS: u64 = 31_536_000;

/// A caller-supplied HMAC key.
#[derive(Clone)]
pub struct SigningKey(Vec<u8>);

impl SigningKey {
    /// Validates and wraps a key of 16-1024 bytes.
    pub fn new(secret: impl AsRef<[u8]>) -> Result<Self, LinkError> {
        let secret = secret.as_ref();
        if secret.len() < MIN_KEY_BYTES || secret.len() > MAX_KEY_BYTES {
            return Err(LinkError::InvalidKey {
                detail: format!("key must be {MIN_KEY_BYTES}-{MAX_KEY_BYTES} bytes"),
            });
        }
        Ok(Self(secret.to_vec()))
    }

    /// Returns the key bytes. Do not log or serialize the result.
    #[must_use]
    pub fn expose_secret(&self) -> &[u8] {
        &self.0
    }

    /// Returns the key length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether the key is empty. It never is for a constructed value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SigningKey(***)")
    }
}

impl fmt::Display for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// HMAC-SHA256 signer with an explicit lifetime and clock skew.
#[derive(Debug, Clone)]
pub struct Signer {
    key: SigningKey,
    ttl: Duration,
    clock_skew: Duration,
}

impl Signer {
    /// Validates and creates a signer.
    ///
    /// The lifetime must be 1 second to 1 year; the skew must be at most 300
    /// seconds.
    pub fn new(key: SigningKey, ttl: Duration, clock_skew: Duration) -> Result<Self, LinkError> {
        let ttl_secs = ttl.as_secs();
        if ttl_secs == 0 || ttl_secs > MAX_TTL_SECS {
            return Err(LinkError::InvalidPolicy {
                detail: "ttl must be 1 second to 1 year".to_string(),
            });
        }
        if clock_skew > Duration::from_secs(MAX_CLOCK_SKEW_SECS) {
            return Err(LinkError::InvalidPolicy {
                detail: format!("clock skew must be at most {MAX_CLOCK_SKEW_SECS} seconds"),
            });
        }
        Ok(Self {
            key,
            ttl,
            clock_skew,
        })
    }

    /// Returns the configured lifetime.
    #[must_use]
    pub const fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Returns the configured clock skew.
    #[must_use]
    pub const fn clock_skew(&self) -> Duration {
        self.clock_skew
    }

    /// Signs claims at the supplied instant.
    ///
    /// The signer sets `issued_at` and `expires_at`; any caller-supplied values
    /// are replaced. The returned token is redacted by `Debug` and `Display`.
    pub fn sign(&self, claims: &Claims, now: i64) -> Result<SignedToken, LinkError> {
        claims.validate()?;
        let mut signed = claims.clone();
        signed.issued_at = now;
        signed.expires_at = now.saturating_add(self.ttl.as_secs() as i64);
        signed.validate()?;
        let payload = signed.canonical();
        let mac = self.mac(payload.as_bytes())?;
        let token = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(payload.as_bytes()),
            URL_SAFE_NO_PAD.encode(mac)
        );
        if token.len() > MAX_TOKEN_BYTES {
            return Err(LinkError::InvalidPolicy {
                detail: "signed token exceeds the size bound".to_string(),
            });
        }
        Ok(SignedToken {
            token,
            claims: signed,
        })
    }

    /// Verifies a token at the supplied instant.
    ///
    /// This is stateless: it checks the MAC, format, issue instant, and expiry,
    /// but not revocation or single use. Use [`Self::verify_with_store`] for
    /// those.
    pub fn verify(&self, token: &str, now: i64) -> Result<Claims, LinkError> {
        if token.is_empty() || token.len() > MAX_TOKEN_BYTES {
            return Err(LinkError::InvalidToken {
                detail: "token is empty or too long".to_string(),
            });
        }
        let (payload_b64, mac_b64) =
            token
                .split_once('.')
                .ok_or_else(|| LinkError::InvalidToken {
                    detail: "token must contain one separator".to_string(),
                })?;
        if mac_b64.contains('.') || payload_b64.is_empty() || mac_b64.is_empty() {
            return Err(LinkError::InvalidToken {
                detail: "token must contain exactly two parts".to_string(),
            });
        }
        let payload = URL_SAFE_NO_PAD
            .decode(payload_b64)
            .map_err(|_| LinkError::InvalidToken {
                detail: "payload is not base64url".to_string(),
            })?;
        let mac = URL_SAFE_NO_PAD
            .decode(mac_b64)
            .map_err(|_| LinkError::InvalidToken {
                detail: "mac is not base64url".to_string(),
            })?;
        if mac.len() != 32 {
            return Err(LinkError::Tampered);
        }

        let mut verifier = HmacSha256::new_from_slice(self.key.expose_secret()).map_err(|_| {
            LinkError::InvalidKey {
                detail: "HMAC rejected the key".to_string(),
            }
        })?;
        verifier.update(DOMAIN);
        verifier.update(&payload);
        verifier
            .verify_slice(&mac)
            .map_err(|_| LinkError::Tampered)?;

        let payload = String::from_utf8(payload).map_err(|_| LinkError::InvalidToken {
            detail: "payload is not UTF-8".to_string(),
        })?;
        let claims = Claims::parse_canonical(&payload)?;

        let skew = self.clock_skew.as_secs() as i64;
        if now.saturating_add(skew) < claims.issued_at {
            return Err(LinkError::NotYetValid {
                issued_at: claims.issued_at,
                now,
            });
        }
        if now > claims.expires_at.saturating_add(skew) {
            return Err(LinkError::Expired {
                expires_at: claims.expires_at,
                now,
            });
        }
        Ok(claims)
    }

    /// Verifies a token and enforces store-backed revocation and single use.
    ///
    /// The stored claims must be the same signed claims. A single-use link is
    /// consumed atomically; only the first caller receives the claims.
    pub async fn verify_with_store<S: LinkStore>(
        &self,
        token: &str,
        code: &Code,
        now: i64,
        store: &S,
    ) -> Result<Claims, LinkError> {
        let claims = self.verify(token, now)?;
        if store.is_revoked(code).await? {
            return Err(LinkError::Revoked);
        }
        if store.is_consumed(code).await? {
            return Err(LinkError::Consumed);
        }
        let Some(stored) = store.get(code).await? else {
            return Err(LinkError::NotFound);
        };
        if stored.canonical() != claims.canonical() {
            return Err(LinkError::Tampered);
        }
        if claims.single_use && !store.consume(code).await? {
            return Err(LinkError::Consumed);
        }
        Ok(claims)
    }

    fn mac(&self, payload: &[u8]) -> Result<Vec<u8>, LinkError> {
        let mut mac = HmacSha256::new_from_slice(self.key.expose_secret()).map_err(|_| {
            LinkError::InvalidKey {
                detail: "HMAC rejected the key".to_string(),
            }
        })?;
        mac.update(DOMAIN);
        mac.update(payload);
        Ok(mac.finalize().into_bytes().to_vec())
    }
}

/// A signed token whose formatting is redacted.
pub struct SignedToken {
    token: String,
    claims: Claims,
}

impl SignedToken {
    /// Returns the token. Do not log, serialize, or include it in an error.
    #[must_use]
    pub fn expose_token(&self) -> &str {
        &self.token
    }

    /// Returns the signed claims, including the signer-set instants.
    #[must_use]
    pub fn claims(&self) -> &Claims {
        &self.claims
    }
}

impl fmt::Debug for SignedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignedToken")
            .field("token", &"***")
            .field("claims", &self.claims)
            .finish()
    }
}

impl fmt::Display for SignedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignedToken(***)")
    }
}

/// Builds the public short URL from a base URL, code, and token.
///
/// The base must be HTTP(S), have a host, and carry no query or fragment. The
/// token is appended as the last path segment in one piece; the scheme is never
/// changed.
fn build_public_url(base: &Url, code: &Code, token: &SignedToken) -> Result<Url, LinkError> {
    if !matches!(base.scheme(), "http" | "https")
        || base.host_str().is_none()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err(LinkError::InvalidTarget {
            detail: "base URL must be HTTP(S) with a host and no query or fragment".to_string(),
        });
    }
    let mut url = base.clone();
    let prefix = url.path().trim_end_matches('/');
    url.set_path(&format!(
        "{prefix}/{}/{}",
        code.as_str(),
        token.expose_token()
    ));
    Ok(url)
}

/// A code and its public URL.
pub struct SignedUrl {
    /// Short code.
    pub code: Code,
    url: Url,
}

impl SignedUrl {
    /// Builds a signed URL.
    pub fn new(base: &Url, code: &Code, token: &SignedToken) -> Result<Self, LinkError> {
        Ok(Self {
            code: code.clone(),
            url: build_public_url(base, code, token)?,
        })
    }

    /// Returns the public URL. It contains the token; do not log it.
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Returns the public URL string. It contains the token; do not log it.
    #[must_use]
    pub fn expose_url(&self) -> &str {
        self.url.as_str()
    }

    /// Returns the URL with the token segment redacted for display.
    #[must_use]
    pub fn redacted(&self) -> String {
        let mut url = self.url.clone();
        let path = url.path();
        if let Some((prefix, _)) = path.rsplit_once('/') {
            url.set_path(&format!("{prefix}/***"));
        }
        url.to_string()
    }
}

impl fmt::Debug for SignedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignedUrl")
            .field("code", &self.code)
            .field("url", &self.redacted())
            .finish()
    }
}

impl fmt::Display for SignedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.code, self.redacted())
    }
}

//! Base62 short codes generated from OS entropy.
//!
//! A code is an identifier, not an authorisation. The HMAC token is the
//! authorisation; the code only selects the stored claims and the revocation
//! state.

use crate::error::LinkError;
use crate::store::LinkStore;

/// The base62 alphabet.
pub const CODE_ALPHABET: &[u8; 62] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Default generated code length.
pub const DEFAULT_CODE_LENGTH: usize = 10;

/// Default number of collision retries.
pub const DEFAULT_CODE_ATTEMPTS: u32 = 8;

/// Minimum accepted code length.
pub const MIN_CODE_LENGTH: usize = 4;

/// Maximum accepted code length.
pub const MAX_CODE_LENGTH: usize = 32;

/// A validated base62 short code.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code(String);

impl Code {
    /// Validates a base62 code of 4-32 characters.
    pub fn new(value: impl Into<String>) -> Result<Self, LinkError> {
        let value = value.into();
        if !(MIN_CODE_LENGTH..=MAX_CODE_LENGTH).contains(&value.len())
            || !value.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return Err(LinkError::InvalidCode {
                detail: "code must be 4-32 base62 characters".to_string(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the code as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Generates a unique code with the default length and attempt bound.
pub async fn generate<S: LinkStore>(store: &S) -> Result<Code, LinkError> {
    generate_with(store, DEFAULT_CODE_LENGTH, DEFAULT_CODE_ATTEMPTS).await
}

/// Generates a unique code, retrying collisions through the store.
///
/// `attempts` bounds the loop; a revoked code is never reused.
pub async fn generate_with<S: LinkStore>(
    store: &S,
    length: usize,
    attempts: u32,
) -> Result<Code, LinkError> {
    if !(MIN_CODE_LENGTH..=MAX_CODE_LENGTH).contains(&length) {
        return Err(LinkError::InvalidCode {
            detail: "code length must be 4-32".to_string(),
        });
    }
    if attempts == 0 || attempts > 64 {
        return Err(LinkError::InvalidPolicy {
            detail: "code attempts must be 1..=64".to_string(),
        });
    }
    for _ in 0..attempts {
        let code = Code(random_code(length)?);
        let exists = store.get(&code).await?.is_some();
        let revoked = store.is_revoked(&code).await?;
        let consumed = store.is_consumed(&code).await?;
        if !exists && !revoked && !consumed {
            return Ok(code);
        }
    }
    Err(LinkError::CodeExhausted { attempts })
}

fn random_code(length: usize) -> Result<String, LinkError> {
    let mut code = String::with_capacity(length);
    let mut buffer = [0_u8; 64];
    let mut refills = 0_u8;
    while code.len() < length {
        if refills >= 8 {
            return Err(LinkError::Entropy);
        }
        refills += 1;
        getrandom::getrandom(&mut buffer).map_err(|_| LinkError::Entropy)?;
        for byte in buffer {
            // 248 is the largest multiple of 62 below 256, so the modulo is
            // unbiased.
            if byte < 248 {
                code.push(char::from(CODE_ALPHABET[usize::from(byte % 62)]));
                if code.len() == length {
                    break;
                }
            }
        }
    }
    Ok(code)
}

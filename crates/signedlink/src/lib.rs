//! Signed, revocable short links.
//!
//! `signedlink` signs a compact claim payload with HMAC-SHA256, mints base62
//! codes, normalises UTM parameters, and enforces revocation and single use
//! through a caller-supplied [`LinkStore`].
//!
//! # Token format
//!
//! ```text
//! base64url(canonical_payload).base64url(HMAC-SHA256(key, domain || payload))
//! ```
//!
//! Both parts use URL-safe base64 without padding. The payload is a versioned,
//! fixed-order form encoding with sorted UTM parameters.
//!
//! # Store requirement
//!
//! Stateless [`Signer::verify`] checks the signature, issue instant, and expiry.
//! It cannot revoke or consume. Revocation and single use require
//! [`Signer::verify_with_store`] and a [`LinkStore`] implementation. A code is an
//! identifier; the signature is the authorisation.
//!
//! # Redaction
//!
//! [`SigningKey`], [`SignedToken`], and [`SignedUrl`] redact secrets in `Debug`
//! and `Display`. No type serialises a key or a token.
//!
//! # Scope
//!
//! - HMAC-SHA256 signing and constant-time verification.
//! - Base62 code generation with bounded collision retry.
//! - Closed-set UTM normalisation and an explicit attribution window.
//! - Store-backed revocation and atomic single use.
//! - An optional `axum` redirect handler with a no-referrer policy.
//!
//! # Non-goals
//!
//! - A database, Redis client, HTTP client, or network server.
//! - Link analytics storage; the click hook is a caller callback.
//! - HTML, cookies, or a global header pipeline.
//! - Persisting keys. Keys are caller-supplied and stay in memory only.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod claims;
mod code;
mod error;
mod signer;
mod store;
mod utm;

#[cfg(feature = "axum")]
pub mod redirect;

pub use claims::{
    Claims, TenantId, CLAIM_FORMAT_VERSION, MAX_ATTRIBUTION_WINDOW_SECS, MAX_TARGET_BYTES,
};
pub use code::{
    generate, generate_with, Code, CODE_ALPHABET, DEFAULT_CODE_ATTEMPTS, DEFAULT_CODE_LENGTH,
    MAX_CODE_LENGTH, MIN_CODE_LENGTH,
};
pub use error::LinkError;
pub use signer::{
    SignedToken, SignedUrl, Signer, SigningKey, MAX_CLOCK_SKEW_SECS, MAX_KEY_BYTES,
    MAX_TOKEN_BYTES, MAX_TTL_SECS, MIN_KEY_BYTES,
};
pub use store::{LinkStore, MemoryLinkStore};
pub use utm::{Utm, UTM_KEYS};

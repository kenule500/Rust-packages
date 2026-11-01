//! Tenant and provider identity, capabilities, an explicit registry, and a
//! token-store contract for multi-tenant Rust services.
//!
//! `platform-core` is a trait crate. It deliberately ships no product
//! vocabulary, no OAuth implementation, no quota client, and no webhook client.
//! Those belong to the consuming application's adapter crate.
//!
//! # Feature flags
//!
//! - `registry` (default) - the `Registry` and `Provider` trait
//! - `memory` (default) - `MemoryTokenStore` for tests and single-binary tools
//!
//! # Non-goals
//!
//! - No global mutable provider registry.
//! - No token encryption; [`TokenStore`] leaves storage and KMS integration to
//!   the adapter.
//! - No database or HTTP framework features in 0.1.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(feature = "registry")]
mod capability;
mod error;
mod id;
#[cfg(feature = "registry")]
mod provider;
#[cfg(feature = "registry")]
mod registry;
mod token;

#[cfg(feature = "registry")]
pub use capability::{Capability, ProvisioningMode};
pub use error::{ErrorCode, IntegrationError};
pub use id::{ProviderId, TenantId};
#[cfg(feature = "registry")]
pub use provider::{Provider, ProviderRef};
#[cfg(feature = "registry")]
pub use registry::Registry;
#[cfg(feature = "memory")]
pub use token::MemoryTokenStore;
pub use token::{Secret, TokenError, TokenStore};

/// The crate version, for build info and diagnostics headers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

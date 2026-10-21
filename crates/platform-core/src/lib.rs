//! Tenant and provider identity, capabilities, an explicit registry, and a
//! token-store contract for multi-tenant Rust services.
//!
//! `platform-core` is a trait crate. It deliberately ships no product
//! vocabulary, no OAuth implementation, no quota client, and no webhook client.
//! Those belong to the consuming application's adapter crate.
//!
//! # Feature flags
//!
//! - `registry` (default) - the [`Registry`] and [`Provider`] trait
//! - `memory` (default) - [`MemoryTokenStore`] for tests and single-binary tools
//!
//! # Non-goals
//!
//! - No global mutable provider registry.
//! - No token encryption; the [`TokenStore`] contract leaves storage and KMS
//!   integration to the adapter.
//! - No database or HTTP framework features in 0.1.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// The crate version, for build info and diagnostics headers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

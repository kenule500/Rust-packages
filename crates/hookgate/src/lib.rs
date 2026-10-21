//! Inbound webhook verification on raw bytes, replay windows, durable
//! deduplication, an outbox, and dead-letter replay.
//!
//! `hookgate` owns the part of webhook handling that must survive a restart:
//! the delivery record, the idempotency key, the replay window, and the handoff
//! to a queue.
//!
//! # Feature flags
//!
//! - `presets` (default) - HMAC verifiers for Meta Graph, Stripe, Shopify,
//!   Twilio, Slack, GitHub, and a configurable generic HMAC-SHA256 verifier.
//!
//! # Non-goals
//!
//! - No provider client and no outbound HTTP.
//! - No HTTP server or `axum` integration in 0.1.
//! - No SendGrid ECDSA verifier in 0.1.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// The crate version, for build info and diagnostics headers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

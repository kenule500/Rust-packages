//! Inbound webhook verification on raw bytes, replay windows, durable
//! deduplication, an outbox, and dead-letter replay.
//!
//! `hookgate` owns the part of webhook handling that must survive a restart:
//! the delivery record, the idempotency key, the replay window, and the handoff
//! to a queue.
//!
//! # Flow
//!
//! 1. Build an [`InboundRequest`] that owns the raw [`bytes::Bytes`] body.
//! 2. Run a [`Verifier`] over the raw bytes. The body is never re-serialized
//!    before verification.
//! 3. [`DeliveryStore::insert_if_new`] deduplicates on tenant, provider, and
//!    provider event id.
//! 4. The delivery and the outbox row commit together.
//! 5. Failures beyond the configured maximum become a [`DeadLetterRecord`]
//!    that can be replayed with the same delivery identity.
//!
//! The contract is at-least-once. Consumers must be idempotent on the delivery
//! id.
//!
//! # Presets
//!
//! | Preset | Header | Digest | Encoding | Signed payload |
//! |---|---|---|---|---|
//! | Meta Graph | `X-Hub-Signature-256` | SHA-256 | hex after `sha256=` | raw body |
//! | Stripe | `Stripe-Signature` | SHA-256 | hex `v1` | `{t}.{body}` |
//! | Shopify | `X-Shopify-Hmac-Sha256` | SHA-256 | base64 | raw body |
//! | Twilio | `X-Twilio-Signature` | SHA-1 | base64 | URL plus sorted form fields |
//! | Slack | `X-Slack-Signature` | SHA-256 | hex after `v0=` | `v0:{timestamp}:{body}` |
//! | GitHub | `X-Hub-Signature-256` | SHA-256 | hex after `sha256=` | raw body |
//! | Generic | caller-supplied | SHA-256 | hex | raw body |
//!
//! # Feature flags
//!
//! - `presets` (default) - the `presets` module. Core types, the verifier
//!   trait, and the in-memory store are always available.
//!
//! # Non-goals
//!
//! - No provider client and no outbound HTTP.
//! - No HTTP server or `axum` integration in 0.1.
//! - No SendGrid ECDSA verifier in 0.1.
//! - No standalone binary in 0.1.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod memory;
mod store;
mod types;
mod verify;

pub use error::{redact_error, DeadLetterRecord, HookgateError};
pub use memory::MemoryDeliveryStore;
pub use store::{
    DeliveryRecord, DeliveryStatus, DeliveryStore, InsertOutcome, Outbox, OutboxError,
    OutboxMessage, StoreError,
};
pub use types::{DeliveryId, IdempotencyKey, InboundRequest, ProviderId, TenantId, VerifiedEvent};
#[cfg(feature = "presets")]
pub use verify::presets;
pub use verify::{
    FieldSource, HmacDigest, HmacPreset, ReplayWindow, SignatureEncoding, SignedPayload, Verifier,
    VerifyError,
};

/// The crate version, for build info and diagnostics headers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

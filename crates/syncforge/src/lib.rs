//! A durable action queue with UUID v7 idempotency, per-action batch results,
//! and explicit conflict strategies for offline-first Rust clients.
//!
//! The action is persisted before it is acknowledged, replayed in order, and
//! settled per action so one poison or retrying action cannot block the rest.
//!
//! # Feature flags
//!
//! This crate has no optional features in 0.1.
//!
//! # Non-goals
//!
//! - No SQLite or HTTP implementation in 0.1; [`Storage`] and [`Transport`] are
//!   traits with in-memory and mock implementations.
//! - No connectivity detection and no WASM packaging.
//! - At-least-once delivery, not exactly-once; consumers must be idempotent on
//!   the action id.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// The crate version, for build info and diagnostics headers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

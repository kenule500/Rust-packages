//! A durable action queue with UUID v7 idempotency, per-action batch results,
//! and explicit conflict strategies for offline-first Rust clients.
//!
//! The action is persisted before it is acknowledged, replayed in order, and
//! settled per action so one poison or retrying action cannot block the rest.
//!
//! # Replay model
//!
//! 1. [`SyncEngine::enqueue`] appends the action and returns only after the
//!    storage write succeeds.
//! 2. [`SyncEngine::replay`] snapshot-drains pending actions in bounded batches.
//! 3. Each attempted action receives exactly one [`ActionResult`] in enqueue
//!    order: `Applied`, `Duplicate`, `Conflict`, `Rejected`, or `Retry`.
//! 4. `Applied` and `Duplicate` settle the action. A resolved `Conflict` also
//!    settles it. `Rejected` marks it failed.
//! 5. A `Retry` increments the attempt count and leaves the action pending. At
//!    the attempt ceiling it is marked failed and reported as `Rejected`; later
//!    actions still replay.
//!
//! The delivery contract is at-least-once, not exactly-once. Storage writes and
//! transport calls are not transactional, so a crash between a successful send
//! and the local settle can replay an action. Consumers must be idempotent on
//! the action id or idempotency key.
//!
//! # Conflict strategies
//!
//! | Strategy | Decision |
//! | --- | --- |
//! | [`ConflictStrategy::LastWriteWins`] | The later client or server timestamp wins; ties favour the server. |
//! | [`ConflictStrategy::ServerWins`] | The server state is kept unchanged. |
//! | [`ConflictStrategy::ClientWins`] | The client payload is kept. |
//! | [`ConflictStrategy::Custom`] | The caller's [`CustomMerger`] returns the resolution. |
//!
//! A conflict must carry the server state; otherwise the engine returns
//! [`SyncError::ConflictWithoutServerState`]. A custom merger can return
//! [`Resolution::Merged`] to record a merged value.
//!
//! # Timestamps and retries
//!
//! The client timestamp is preserved exactly; replay never rewrites it. The
//! server timestamp is recorded in [`ActionResult::server_timestamp`] and in
//! [`ConflictInfo`]. [`SyncEngine::timestamp_decision`] exposes the clock-skew
//! audit, including whether the pair is within the configured tolerance.
//!
//! Retries use a capped exponential delay exposed by
//! [`SyncEngine::retry_delay`]. The engine never sleeps or blocks; the host
//! schedules the next replay. The first delay is 250 ms and the ceiling is 30
//! seconds.
//!
//! # Tracing
//!
//! With a `tracing` subscriber installed, the engine emits `syncforge.enqueue`,
//! `syncforge.replay.start`, `syncforge.replay.finish`, `syncforge.action.retry`,
//! `syncforge.action.dead_letter`, `syncforge.action.outcome`, and
//! `syncforge.conflict` events. Replay summaries include queue depth, result
//! count, conflict count, and duration.
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
//! - No automatic application of a merged value to the server; the resolution
//!   is recorded for the caller.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod action;
mod batch;
mod conflict;
mod engine;
mod error;
mod storage;
mod timestamp;
mod transport;

pub use action::{Action, ActionId, EntityType, Operation};
pub use batch::{ActionResult, ActionStatus, Batch, BatchResult};
pub use conflict::{ConflictInfo, ConflictStrategy, CustomMerger, Resolution};
pub use engine::{
    SyncEngine, BASE_BACKOFF_MILLIS, DEFAULT_BATCH_SIZE, DEFAULT_MAX_ATTEMPTS,
    DEFAULT_TOLERANCE_MINUTES, MAX_BACKOFF_MILLIS,
};
pub use error::SyncError;
pub use storage::{MemoryStorage, Storage};
pub use timestamp::{decide, TimestampDecision};
pub use transport::{MockTransport, Transport};

/// The crate version, for build info and diagnostics headers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

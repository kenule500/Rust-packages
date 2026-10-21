//! Packet-in replay recording with isolated tracks and a replay ring.
//!
//! `replay-recorder` accepts already-encoded packets, writes per-track segments
//! into a spool, maintains an atomic JSON index, enforces a disk budget, and
//! extracts clips from a bounded ring.
//!
//! # Scope
//!
//! - Per-track start/stop, monotonic per-track timestamps, and segment rotation
//!   by duration or size.
//! - `Sink::Segment`, `Sink::Tee`, and `Sink::Ring`.
//! - Validated ID-derived paths, traversal rejection, and atomic `index.json`
//!   writes.
//! - Deterministic crash recovery and clip extraction without stopping writes.
//!
//! # Non-goals
//!
//! - Encoding, decoding, muxing, or remuxing. Packets are opaque payloads.
//! - Uploading, CDN orchestration, or a database.
//! - Owning a network server or a global recorder.
//! - Querying the operating system for free space; the caller supplies the
//!   current free-byte value so the policy is testable and portable.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod budget;
mod error;
mod index;
mod recorder;
mod ring;
mod segment;
mod sink;
mod spool;
mod track;

pub use budget::{DiskBudget, PressureEvent, PressurePolicy};
pub use error::RecorderError;
pub use index::{SegmentEntry, SegmentIndex};
pub use recorder::{Recorder, RecorderConfig, RecoveryReport};
pub use ring::ClipHandle;
pub use sink::{RingSink, SegmentSink, Sink, TeeSink};
pub use spool::Spool;
pub use track::{Container, Packet, SessionId, SourceId, Track, TrackCodec, TrackId};

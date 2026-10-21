//! Deterministic scene-to-FFmpeg compilation.
//!
//! `ffscene` turns an in-memory scene description into a stable FFmpeg argument
//! vector, a `filter_complex` graph, and per-layer output labels. It also models
//! encoder ladders, encoder selection, and output descriptors.
//!
//! # Scope
//!
//! - Pure validation and compilation: no clock, randomness, network, process, or
//!   filesystem access.
//! - Encoder probing against an injected list of available encoders.
//! - Output descriptors as data, including explicit validation of their shape.
//!
//! # Non-goals
//!
//! - Running FFmpeg, opening sockets, resolving DNS, or spawning processes.
//! - Storing the scene schema in an application document format.
//! - Incremental recompilation or a live preview engine.
//! - Connection health, retries, or failover; those belong to `rtmp-publish`.
//!
//! # Determinism
//!
//! The graph is derived from a normalised `(z, layer id, source id)` order and
//! from sorted source identifiers. No timestamp, process identity, address, or
//! hash iteration order reaches the output (FR-001, FR-006, NFR-001).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod compile;
mod error;
mod ladder;
mod output;
mod probe;
mod scene;
mod validate;

pub use compile::{compile, CompiledGraph, LayerLabelMap};
pub use error::CompileError;
pub use ladder::{Codec, EncoderProfile, Ladder, LadderExpansion, RateControl, Rung};
pub use output::Output;
pub use probe::{EncoderProbe, EncoderSelection, KNOWN_ENCODERS};
pub use scene::{
    Background, BlendMode, Canvas, Layer, LayerId, Rational, Scene, SourceId, Transform,
};
pub use validate::validate;

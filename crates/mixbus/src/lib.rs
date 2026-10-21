//! Real-time-safe audio mixing.
//!
//! `mixbus` processes caller-owned interleaved stereo blocks through gain,
//! constant-power pan, gate, compressor, sidechain ducking, loudness metering,
//! and true-peak limiting. It owns no device, decoder, thread, or async runtime.
//!
//! # Scope
//!
//! - Pre-allocated processing state, a bounded control queue, and block-boundary
//!   plan swaps.
//! - Lock-free meter publication through atomics.
//! - EBU R128-style K-weighting, gated integrated loudness, and a true-peak
//!   ceiling.
//!
//! # Non-goals
//!
//! - Opening audio devices, decoding files, or resampling.
//! - Hosting VST3/CLAP plugins.
//! - Owning the audio thread; the caller drives [`Mixer::process`].
//! - Application routing UI or persistence.
//!
//! # Real-time contract
//!
//! [`Mixer::process`] performs no allocation, locking, logging, or awaiting. All
//! buffers and queues are sized during construction or [`MixPlan::build`]. The
//! control surface ([`Mixer::install_plan`] and the `set_*` methods) must be
//! called between blocks by the thread that owns the mixer.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod block;
mod compressor;
mod ducker;
mod error;
mod gain;
mod gate;
mod loudness;
mod meter;
mod mixer;
mod pan;
mod plan;

pub use block::{BlockSize, Frame};
pub use compressor::CompressorConfig;
pub use ducker::Duck;
pub use error::MixError;
pub use gain::{db_to_linear, linear_to_db};
pub use gate::GateConfig;
pub use loudness::{LimiterConfig, LoudnessMeter};
pub use meter::{MeterBank, MeterFlags, MeterSnapshot};
pub use mixer::{Mixer, MixerConfig};
pub use pan::constant_power;
pub use plan::{Bus, BusId, Insert, MixPlan, Send, Source, SourceId};

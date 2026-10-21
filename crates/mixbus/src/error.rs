//! Typed mixing errors with stable codes.

use std::fmt;

/// A configuration, routing, or buffer-contract failure.
#[derive(Debug, Clone, PartialEq)]
pub enum MixError {
    /// The sample rate is outside the supported range.
    InvalidSampleRate {
        /// The rejected rate.
        sample_rate: u32,
    },
    /// The block size is zero or exceeds the maximum.
    InvalidBlockSize {
        /// The rejected size in frames.
        frames: u32,
    },
    /// No buses or no sources were configured.
    EmptyGraph,
    /// A bus identifier is empty or malformed.
    InvalidBusId {
        /// The rejected identifier.
        value: String,
    },
    /// A source identifier is empty or malformed.
    InvalidSourceId {
        /// The rejected identifier.
        value: String,
    },
    /// Two buses or two sources share an identifier.
    DuplicateId {
        /// The duplicated identifier.
        value: String,
    },
    /// A gain is not finite or is outside the supported range.
    InvalidGain {
        /// The rejected gain in dB.
        gain_db: f32,
    },
    /// A pan value is not finite or is outside `-1.0..=1.0`.
    InvalidPan {
        /// The rejected pan.
        pan: f32,
    },
    /// A send names an unknown bus.
    UnknownBus {
        /// The missing bus.
        bus: String,
    },
    /// An insert or duck configuration is invalid.
    InvalidDynamics {
        /// Human-readable detail.
        detail: String,
    },
    /// The input buffer does not match the configured source layout.
    InvalidInput {
        /// Expected sample count.
        expected: usize,
        /// Actual sample count.
        actual: usize,
    },
    /// The output is too small or not stereo.
    InvalidOutput {
        /// Expected sample count.
        expected: usize,
        /// Actual sample count.
        actual: usize,
    },
    /// The block exceeds the configured maximum.
    BlockTooLarge {
        /// Configured maximum frames.
        max_frames: u32,
        /// Requested frames.
        frames: usize,
    },
    /// The bounded control queue is full.
    QueueFull,
    /// An installed plan does not match the mixer's sources and buses.
    PlanMismatch,
}

impl MixError {
    /// Returns the stable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidSampleRate { .. } => "MIX-001",
            Self::InvalidBlockSize { .. } => "MIX-002",
            Self::EmptyGraph => "MIX-003",
            Self::InvalidBusId { .. } => "MIX-004",
            Self::InvalidSourceId { .. } => "MIX-005",
            Self::DuplicateId { .. } => "MIX-006",
            Self::InvalidGain { .. } => "MIX-007",
            Self::InvalidPan { .. } => "MIX-008",
            Self::UnknownBus { .. } => "MIX-009",
            Self::InvalidDynamics { .. } => "MIX-010",
            Self::InvalidInput { .. } => "MIX-011",
            Self::InvalidOutput { .. } => "MIX-012",
            Self::BlockTooLarge { .. } => "MIX-013",
            Self::QueueFull => "MIX-014",
            Self::PlanMismatch => "MIX-015",
        }
    }
}

impl fmt::Display for MixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.code())?;
        match self {
            Self::InvalidSampleRate { sample_rate } => {
                write!(f, "sample rate {sample_rate} is outside 8000..=384000")
            }
            Self::InvalidBlockSize { frames } => {
                write!(f, "block size {frames} is outside 1..=8192")
            }
            Self::EmptyGraph => f.write_str("at least one bus and one source are required"),
            Self::InvalidBusId { value } => write!(f, "invalid bus id {value:?}"),
            Self::InvalidSourceId { value } => write!(f, "invalid source id {value:?}"),
            Self::DuplicateId { value } => write!(f, "duplicate id {value:?}"),
            Self::InvalidGain { gain_db } => write!(f, "invalid gain {gain_db} dB"),
            Self::InvalidPan { pan } => write!(f, "invalid pan {pan}"),
            Self::UnknownBus { bus } => write!(f, "unknown bus {bus:?}"),
            Self::InvalidDynamics { detail } => write!(f, "invalid dynamics: {detail}"),
            Self::InvalidInput { expected, actual } => {
                write!(
                    f,
                    "input length {actual} does not match expected {expected}"
                )
            }
            Self::InvalidOutput { expected, actual } => {
                write!(
                    f,
                    "output length {actual} does not match expected {expected}"
                )
            }
            Self::BlockTooLarge { max_frames, frames } => {
                write!(f, "block of {frames} frames exceeds {max_frames}")
            }
            Self::QueueFull => f.write_str("control queue is full"),
            Self::PlanMismatch => f.write_str("plan does not match mixer routing"),
        }
    }
}

impl std::error::Error for MixError {}

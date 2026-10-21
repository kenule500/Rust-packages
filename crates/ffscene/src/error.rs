//! Typed compilation errors with stable codes.

use std::fmt;

/// A validation or compilation failure.
///
/// Every variant carries a stable code (CON-205). Codes are part of the public
/// contract and must never be reused with a different meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// The canvas width or height is zero.
    EmptyCanvas,
    /// The frame rate has a zero denominator or numerator.
    InvalidRational {
        /// The invalid numerator.
        numerator: u32,
        /// The invalid denominator.
        denominator: u32,
    },
    /// A layer identifier was empty.
    EmptyLayerId,
    /// A layer identifier contained unsupported characters.
    InvalidLayerId {
        /// The rejected value.
        value: String,
    },
    /// A source identifier was empty.
    EmptySourceId,
    /// A source identifier contained unsupported characters.
    InvalidSourceId {
        /// The rejected value.
        value: String,
    },
    /// Two layers share an identifier.
    DuplicateLayer {
        /// The duplicated layer identifier.
        layer: String,
    },
    /// A layer names a parent that does not exist.
    UnknownParent {
        /// The child layer.
        layer: String,
        /// The missing parent.
        parent: String,
    },
    /// A parent chain forms a cycle.
    Cycle {
        /// A layer in the cycle.
        layer: String,
    },
    /// A transform extends outside the canvas or has a zero size.
    TransformOutOfBounds {
        /// The offending layer.
        layer: String,
    },
    /// Opacity is NaN or infinite.
    NonFiniteOpacity {
        /// The offending layer.
        layer: String,
    },
    /// Opacity is outside `0.0..=1.0`.
    OpacityOutOfRange {
        /// The offending layer.
        layer: String,
    },
    /// A blend mode is not supported by the compiler.
    UnsupportedBlend {
        /// The offending layer.
        layer: String,
    },
    /// A ladder has no rungs.
    EmptyLadder,
    /// A rung has a zero dimension or an empty name.
    InvalidRung {
        /// The offending rung name.
        rung: String,
    },
    /// Two rungs share a name.
    DuplicateRung {
        /// The duplicated rung name.
        rung: String,
    },
    /// A requested encoder is not a known encoder name.
    UnsupportedEncoder {
        /// The requested encoder.
        encoder: String,
    },
    /// No requested or fallback encoder is available.
    NoEncoderAvailable {
        /// The requested encoder.
        requested: String,
    },
    /// An output descriptor is malformed.
    InvalidOutput {
        /// Human-readable detail with no secret material.
        detail: String,
    },
}

impl CompileError {
    /// Returns the stable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EmptyCanvas => "FFS-001",
            Self::InvalidRational { .. } => "FFS-002",
            Self::EmptyLayerId => "FFS-003",
            Self::InvalidLayerId { .. } => "FFS-004",
            Self::EmptySourceId => "FFS-005",
            Self::InvalidSourceId { .. } => "FFS-006",
            Self::DuplicateLayer { .. } => "FFS-007",
            Self::UnknownParent { .. } => "FFS-008",
            Self::Cycle { .. } => "FFS-009",
            Self::TransformOutOfBounds { .. } => "FFS-010",
            Self::NonFiniteOpacity { .. } => "FFS-011",
            Self::OpacityOutOfRange { .. } => "FFS-012",
            Self::UnsupportedBlend { .. } => "FFS-013",
            Self::EmptyLadder => "FFS-014",
            Self::InvalidRung { .. } => "FFS-015",
            Self::DuplicateRung { .. } => "FFS-016",
            Self::UnsupportedEncoder { .. } => "FFS-017",
            Self::NoEncoderAvailable { .. } => "FFS-018",
            Self::InvalidOutput { .. } => "FFS-019",
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.code())?;
        match self {
            Self::EmptyCanvas => f.write_str("canvas width and height must be non-zero"),
            Self::InvalidRational {
                numerator,
                denominator,
            } => write!(f, "invalid rational {numerator}/{denominator}"),
            Self::EmptyLayerId => f.write_str("layer id must not be empty"),
            Self::InvalidLayerId { value } => {
                write!(
                    f,
                    "layer id {value:?} must be 1-64 ASCII alphanumeric, '-' or '_'"
                )
            }
            Self::EmptySourceId => f.write_str("source id must not be empty"),
            Self::InvalidSourceId { value } => {
                write!(
                    f,
                    "source id {value:?} must be 1-64 ASCII alphanumeric, '-' or '_'"
                )
            }
            Self::DuplicateLayer { layer } => write!(f, "duplicate layer id {layer:?}"),
            Self::UnknownParent { layer, parent } => {
                write!(f, "layer {layer:?} names unknown parent {parent:?}")
            }
            Self::Cycle { layer } => write!(f, "layer {layer:?} participates in a cycle"),
            Self::TransformOutOfBounds { layer } => {
                write!(f, "layer {layer:?} has a transform outside the canvas")
            }
            Self::NonFiniteOpacity { layer } => {
                write!(f, "layer {layer:?} has a non-finite opacity")
            }
            Self::OpacityOutOfRange { layer } => {
                write!(f, "layer {layer:?} opacity must be within 0.0..=1.0")
            }
            Self::UnsupportedBlend { layer } => {
                write!(f, "layer {layer:?} uses an unsupported blend mode")
            }
            Self::EmptyLadder => f.write_str("ladder must contain at least one rung"),
            Self::InvalidRung { rung } => {
                write!(
                    f,
                    "rung {rung:?} must have a non-empty name and non-zero dimensions"
                )
            }
            Self::DuplicateRung { rung } => write!(f, "duplicate rung name {rung:?}"),
            Self::UnsupportedEncoder { encoder } => write!(f, "unknown encoder {encoder:?}"),
            Self::NoEncoderAvailable { requested } => {
                write!(f, "no encoder available for requested {requested:?}")
            }
            Self::InvalidOutput { detail } => write!(f, "invalid output descriptor: {detail}"),
        }
    }
}

impl std::error::Error for CompileError {}

//! Encoder profiles and deterministic ladder expansion.

use std::collections::BTreeSet;

use crate::error::CompileError;
use crate::scene::is_safe_identifier;

/// A video codec supported by the compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    /// H.264 / AVC.
    H264,
    /// H.265 / HEVC.
    Hevc,
    /// AV1.
    Av1,
}

impl Codec {
    /// Returns the FFmpeg encoder-agnostic codec name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::Hevc => "hevc",
            Self::Av1 => "av1",
        }
    }
}

/// Rate-control strategy for an encoder profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateControl {
    /// Constant rate factor.
    Crf(u8),
    /// Constant bitrate in kilobits per second.
    Cbr(u32),
    /// Variable bitrate with a target and a maximum, in kilobits per second.
    Vbr {
        /// Target bitrate.
        target_kbps: u32,
        /// Maximum bitrate.
        max_kbps: u32,
    },
}

/// One encoder configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderProfile {
    /// Codec family.
    pub codec: Codec,
    /// Rate-control strategy.
    pub rate_control: RateControl,
    /// Encoder preset name, such as `medium`.
    pub preset: String,
    /// Keyframe interval in frames; zero means encoder default.
    pub keyint: u32,
    /// Optional codec profile, such as `high`.
    pub profile: Option<String>,
}

impl EncoderProfile {
    /// Creates a profile without changing the fields.
    #[must_use]
    pub fn new(codec: Codec, rate_control: RateControl, preset: impl Into<String>) -> Self {
        Self {
            codec,
            rate_control,
            preset: preset.into(),
            keyint: 0,
            profile: None,
        }
    }
}

/// One ladder rung: a named rendition size and bitrate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rung {
    /// Stable rung name, used in output labels.
    pub name: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Target bitrate in kilobits per second.
    pub bitrate_kbps: u32,
}

impl Rung {
    /// Creates a rung.
    #[must_use]
    pub fn new(name: impl Into<String>, width: u32, height: u32, bitrate_kbps: u32) -> Self {
        Self {
            name: name.into(),
            width,
            height,
            bitrate_kbps,
        }
    }
}

/// An ordered set of renditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ladder {
    /// Rungs in declaration order.
    pub rungs: Vec<Rung>,
}

/// The result of expanding a ladder into filter-graph fragments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderExpansion {
    /// Filter fragments, joined with `;`.
    pub filter: String,
    /// One output label per rung, without brackets and in rung order.
    pub outputs: Vec<String>,
    /// One encoder profile per rung, in rung order.
    pub profiles: Vec<EncoderProfile>,
}

impl Ladder {
    /// Creates a ladder.
    #[must_use]
    pub fn new(rungs: Vec<Rung>) -> Self {
        Self { rungs }
    }

    /// Validates the ladder and returns one encoder profile per rung.
    pub fn expand(&self, base_label: &str) -> Result<LadderExpansion, CompileError> {
        if self.rungs.is_empty() {
            return Err(CompileError::EmptyLadder);
        }
        let mut names = BTreeSet::new();
        for rung in &self.rungs {
            if rung.width == 0 || rung.height == 0 || !is_safe_identifier(&rung.name) {
                return Err(CompileError::InvalidRung {
                    rung: rung.name.clone(),
                });
            }
            if !names.insert(rung.name.as_str()) {
                return Err(CompileError::DuplicateRung {
                    rung: rung.name.clone(),
                });
            }
        }

        let base = base_label.trim_matches(|c| c == '[' || c == ']');
        let mut fragments = Vec::with_capacity(self.rungs.len() + 1);
        let mut split = format!("[{base}]split={}", self.rungs.len());
        for index in 0..self.rungs.len() {
            split.push_str(&format!("[sp{index}]"));
        }
        fragments.push(split);

        let mut outputs = Vec::with_capacity(self.rungs.len());
        let mut profiles = Vec::with_capacity(self.rungs.len());
        for (index, rung) in self.rungs.iter().enumerate() {
            let label = format!("out_{}", rung.name);
            fragments.push(format!(
                "[sp{index}]scale={}:{}:flags=bicubic[{label}]",
                rung.width, rung.height
            ));
            outputs.push(label);
            profiles.push(EncoderProfile {
                codec: Codec::H264,
                rate_control: RateControl::Cbr(rung.bitrate_kbps),
                preset: "medium".to_string(),
                keyint: 0,
                profile: None,
            });
        }

        Ok(LadderExpansion {
            filter: fragments.join(";"),
            outputs,
            profiles,
        })
    }
}

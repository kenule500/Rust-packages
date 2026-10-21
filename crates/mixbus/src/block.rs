//! Block sizing and frame helpers.

use crate::error::MixError;

/// Maximum block size in frames.
pub const MAX_BLOCK_FRAMES: u32 = 8192;

/// A validated block size in frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockSize(u32);

impl BlockSize {
    /// Creates a block size between 1 and 8192 frames.
    pub fn new(frames: u32) -> Result<Self, MixError> {
        if frames == 0 || frames > MAX_BLOCK_FRAMES {
            return Err(MixError::InvalidBlockSize { frames });
        }
        Ok(Self(frames))
    }

    /// Returns the size in frames.
    #[must_use]
    pub const fn frames(self) -> u32 {
        self.0
    }

    /// Returns the size in interleaved stereo samples.
    #[must_use]
    pub const fn samples(self) -> usize {
        self.0 as usize * 2
    }
}

/// One interleaved stereo frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Left sample.
    pub left: f32,
    /// Right sample.
    pub right: f32,
}

impl Frame {
    /// Creates a frame.
    #[must_use]
    pub const fn new(left: f32, right: f32) -> Self {
        Self { left, right }
    }
}

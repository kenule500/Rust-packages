//! Bounded bitrate hints.
//!
//! The calculation is a pure clamp: observed throughput is raised to the floor
//! and lowered to the ceiling. It never returns zero for a valid hint.

use crate::error::PublishError;

/// A floor, ceiling, and observed throughput in bits per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitrateHint {
    /// Minimum accepted bitrate.
    pub floor: u64,
    /// Maximum accepted bitrate.
    pub ceiling: u64,
    /// Observed throughput.
    pub observed: u64,
}

impl BitrateHint {
    /// Validates and creates a hint.
    ///
    /// The floor must be non-zero and the ceiling must be at or above it.
    pub fn new(floor: u64, ceiling: u64, observed: u64) -> Result<Self, PublishError> {
        if floor == 0 {
            return Err(PublishError::InvalidHint {
                detail: "bitrate floor must be non-zero".to_string(),
            });
        }
        if ceiling < floor {
            return Err(PublishError::InvalidHint {
                detail: "bitrate ceiling must be at least the floor".to_string(),
            });
        }
        Ok(Self {
            floor,
            ceiling,
            observed,
        })
    }

    /// Converts observed bytes per second to bits and builds a hint.
    pub fn from_throughput(
        floor: u64,
        ceiling: u64,
        bytes_per_second: u64,
    ) -> Result<Self, PublishError> {
        Self::new(floor, ceiling, bytes_per_second.saturating_mul(8))
    }

    /// Returns the bounded target bitrate.
    #[must_use]
    pub fn target(&self) -> u64 {
        self.observed.clamp(self.floor, self.ceiling)
    }

    /// Returns whether the observed value already sits inside the bounds.
    #[must_use]
    pub fn is_within(&self) -> bool {
        (self.floor..=self.ceiling).contains(&self.observed)
    }
}

//! Sidechain ducker keyed by another source.

use crate::error::MixError;
use crate::gain::db_to_linear;
use crate::plan::SourceId;

/// Ducker parameters attached to a source.
#[derive(Debug, Clone, PartialEq)]
pub struct Duck {
    /// Source whose level drives the reduction.
    pub key: SourceId,
    /// Threshold in dBFS.
    pub threshold_db: f32,
    /// Reduction ratio, at least 1.0.
    pub ratio: f32,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
}

impl Duck {
    /// Validates the parameters.
    pub fn validate(&self) -> Result<(), MixError> {
        if !self.threshold_db.is_finite()
            || !self.ratio.is_finite()
            || self.ratio < 1.0
            || !self.attack_ms.is_finite()
            || self.attack_ms <= 0.0
            || !self.release_ms.is_finite()
            || self.release_ms <= 0.0
        {
            return Err(MixError::InvalidDynamics {
                detail: "duck threshold, ratio, and times must be finite".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DuckState {
    reduction_db: f32,
}

impl DuckState {
    pub(crate) fn new() -> Self {
        Self { reduction_db: 0.0 }
    }

    /// Updates the reduction for one block and returns the linear gain.
    pub(crate) fn update(
        &mut self,
        key_peak_db: f32,
        config: &Duck,
        sample_rate: u32,
        frames: usize,
    ) -> f32 {
        let over = (key_peak_db - config.threshold_db).max(0.0);
        let target = over * (1.0 - 1.0 / config.ratio);
        let block_seconds = frames as f32 / sample_rate as f32;
        let attack = 1.0 - (-block_seconds / (config.attack_ms / 1000.0).max(0.000_01)).exp();
        let release = 1.0 - (-block_seconds / (config.release_ms / 1000.0).max(0.000_01)).exp();
        let coefficient = if target > self.reduction_db {
            attack
        } else {
            release
        };
        self.reduction_db += (target - self.reduction_db) * coefficient;
        db_to_linear(-self.reduction_db)
    }
}

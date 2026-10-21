//! Feed-forward compressor with attack, release, and makeup gain.

use crate::error::MixError;
use crate::gain::{db_to_linear, linear_to_db, sanitize};

/// Compressor parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct CompressorConfig {
    /// Threshold in dBFS.
    pub threshold_db: f32,
    /// Compression ratio, at least 1.0.
    pub ratio: f32,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
    /// Makeup gain in dB.
    pub makeup_db: f32,
}

impl CompressorConfig {
    /// Validates the parameters.
    pub fn validate(&self) -> Result<(), MixError> {
        if !self.threshold_db.is_finite()
            || !self.ratio.is_finite()
            || self.ratio < 1.0
            || !self.attack_ms.is_finite()
            || self.attack_ms <= 0.0
            || !self.release_ms.is_finite()
            || self.release_ms <= 0.0
            || !self.makeup_db.is_finite()
        {
            return Err(MixError::InvalidDynamics {
                detail: "compressor threshold, ratio, times, and makeup must be finite".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Compressor {
    threshold_db: f32,
    ratio: f32,
    attack_coefficient: f32,
    release_coefficient: f32,
    makeup_linear: f32,
    reduction_db: f32,
}

impl Compressor {
    pub(crate) fn new(config: &CompressorConfig, sample_rate: u32) -> Result<Self, MixError> {
        config.validate()?;
        Ok(Self {
            threshold_db: config.threshold_db,
            ratio: config.ratio,
            attack_coefficient: time_coefficient(config.attack_ms, sample_rate),
            release_coefficient: time_coefficient(config.release_ms, sample_rate),
            makeup_linear: db_to_linear(config.makeup_db),
            reduction_db: 0.0,
        })
    }

    pub(crate) fn process(&mut self, sample: f32) -> f32 {
        let (sample, _) = sanitize(sample);
        let level_db = linear_to_db(sample.abs());
        let over = (level_db - self.threshold_db).max(0.0);
        let target = over * (1.0 - 1.0 / self.ratio);
        let coefficient = if target > self.reduction_db {
            self.attack_coefficient
        } else {
            self.release_coefficient
        };
        self.reduction_db += (target - self.reduction_db) * coefficient;
        sample * db_to_linear(-self.reduction_db) * self.makeup_linear
    }

    pub(crate) fn gain_reduction_db(&self) -> f32 {
        self.reduction_db
    }
}

fn time_coefficient(milliseconds: f32, sample_rate: u32) -> f32 {
    let seconds = (milliseconds / 1000.0).max(0.000_01);
    1.0 - (-1.0 / (seconds * sample_rate as f32)).exp()
}

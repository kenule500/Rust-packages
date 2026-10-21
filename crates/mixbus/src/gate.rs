//! Noise gate with hysteresis, hold, attack, and release.

use crate::error::MixError;
use crate::gain::{db_to_linear, sanitize};

/// Gate parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct GateConfig {
    /// Open threshold in dBFS.
    pub threshold_db: f32,
    /// Additional attenuation before closing, in dB.
    pub hysteresis_db: f32,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Minimum open time after the signal falls, in milliseconds.
    pub hold_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
}

impl GateConfig {
    /// Validates the parameters.
    pub fn validate(&self) -> Result<(), MixError> {
        if !self.threshold_db.is_finite()
            || !self.hysteresis_db.is_finite()
            || self.hysteresis_db < 0.0
            || !self.attack_ms.is_finite()
            || self.attack_ms <= 0.0
            || !self.hold_ms.is_finite()
            || self.hold_ms < 0.0
            || !self.release_ms.is_finite()
            || self.release_ms <= 0.0
        {
            return Err(MixError::InvalidDynamics {
                detail: "gate thresholds and times must be finite and non-negative".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Gate {
    open_linear: f32,
    close_linear: f32,
    attack_coefficient: f32,
    release_coefficient: f32,
    hold_frames: u32,
    hold_remaining: u32,
    gain: f32,
    open: bool,
}

impl Gate {
    pub(crate) fn new(config: &GateConfig, sample_rate: u32) -> Result<Self, MixError> {
        config.validate()?;
        Ok(Self {
            open_linear: db_to_linear(config.threshold_db),
            close_linear: db_to_linear(config.threshold_db - config.hysteresis_db),
            attack_coefficient: time_coefficient(config.attack_ms, sample_rate),
            release_coefficient: time_coefficient(config.release_ms, sample_rate),
            hold_frames: (config.hold_ms * sample_rate as f32 / 1000.0).round() as u32,
            hold_remaining: 0,
            gain: 0.0,
            open: false,
        })
    }

    pub(crate) fn process(&mut self, sample: f32) -> f32 {
        let (sample, _) = sanitize(sample);
        let level = sample.abs();
        if level >= self.open_linear {
            self.open = true;
            self.hold_remaining = self.hold_frames;
        } else if level <= self.close_linear {
            if self.hold_remaining > 0 {
                self.hold_remaining -= 1;
            } else {
                self.open = false;
            }
        }
        let target = if self.open { 1.0 } else { 0.0 };
        let coefficient = if target > self.gain {
            self.attack_coefficient
        } else {
            self.release_coefficient
        };
        self.gain += (target - self.gain) * coefficient;
        sample * self.gain
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }
}

fn time_coefficient(milliseconds: f32, sample_rate: u32) -> f32 {
    let seconds = (milliseconds / 1000.0).max(0.000_01);
    1.0 - (-1.0 / (seconds * sample_rate as f32)).exp()
}

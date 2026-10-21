//! EBU R128-style loudness measurement and true-peak limiting.

use crate::error::MixError;
use crate::gain::{db_to_linear, sanitize};

const BLOCK_HISTORY: usize = 64;

/// True-peak limiter parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct LimiterConfig {
    /// Ceiling in dBTP.
    pub ceiling_db: f32,
    /// Look-ahead in milliseconds; also the reported latency.
    pub lookahead_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
}

impl LimiterConfig {
    /// Validates the parameters.
    pub fn validate(&self) -> Result<(), MixError> {
        if !self.ceiling_db.is_finite()
            || !self.lookahead_ms.is_finite()
            || self.lookahead_ms < 0.0
            || !self.release_ms.is_finite()
            || self.release_ms <= 0.0
        {
            return Err(MixError::InvalidDynamics {
                detail: "limiter ceiling and times must be finite".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.z1;
        self.z1 = self.b1 * input - self.a1 * output + self.z2;
        self.z2 = self.b2 * input - self.a2 * output;
        output
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// BS.1770 K-weighting filters, normalised at 48 kHz.
#[derive(Debug, Clone)]
struct KWeighting {
    shelf: Biquad,
    high_pass: Biquad,
}

impl KWeighting {
    fn new() -> Self {
        Self {
            shelf: Biquad {
                b0: 1.535_124_9,
                b1: -2.691_696_2,
                b2: 1.198_392_8,
                a1: -1.690_659_3,
                a2: 0.732_480_77,
                ..Biquad::default()
            },
            high_pass: Biquad {
                b0: 1.0,
                b1: -2.0,
                b2: 1.0,
                a1: -1.990_047_5,
                a2: 0.990_072_25,
                ..Biquad::default()
            },
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        self.high_pass.process(self.shelf.process(sample))
    }

    fn reset(&mut self) {
        self.shelf.reset();
        self.high_pass.reset();
    }
}

/// Gated integrated loudness in LUFS.
#[derive(Debug, Clone)]
pub struct LoudnessMeter {
    filters: KWeighting,
    block_frames: u32,
    frames_in_block: u32,
    sum_left: f64,
    sum_right: f64,
    energies: Vec<f32>,
    write: usize,
    used: usize,
    lufs: f32,
}

impl LoudnessMeter {
    /// Creates a meter for the given sample rate.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            filters: KWeighting::new(),
            block_frames: (sample_rate / 5).max(1),
            frames_in_block: 0,
            sum_left: 0.0,
            sum_right: 0.0,
            energies: vec![0.0; BLOCK_HISTORY],
            write: 0,
            used: 0,
            lufs: -120.0,
        }
    }

    /// Feeds one stereo frame.
    pub fn process(&mut self, left: f32, right: f32) {
        let (left, _) = sanitize(left);
        let (right, _) = sanitize(right);
        let left = self.filters.process(left);
        let right = self.filters.process(right);
        self.sum_left += f64::from(left) * f64::from(left);
        self.sum_right += f64::from(right) * f64::from(right);
        self.frames_in_block += 1;
        if self.frames_in_block >= self.block_frames {
            self.close_block();
        }
    }

    fn close_block(&mut self) {
        let frames = f64::from(self.frames_in_block.max(1));
        let energy = (self.sum_left + self.sum_right) / frames;
        self.energies[self.write] = energy as f32;
        self.write = (self.write + 1) % BLOCK_HISTORY;
        self.used = (self.used + 1).min(BLOCK_HISTORY);
        self.sum_left = 0.0;
        self.sum_right = 0.0;
        self.frames_in_block = 0;
        self.lufs = self.integrated();
    }

    fn integrated(&self) -> f32 {
        if self.used == 0 {
            return -120.0;
        }
        let mut above_absolute = 0.0_f64;
        let mut absolute_count = 0_u32;
        for energy in self.energies[..self.used].iter() {
            let loudness = energy_to_lufs(*energy);
            if loudness > -70.0 {
                above_absolute += f64::from(*energy);
                absolute_count += 1;
            }
        }
        if absolute_count == 0 {
            return -120.0;
        }
        let relative_gate =
            energy_to_lufs((above_absolute / f64::from(absolute_count)) as f32) - 10.0;
        let mut gated = 0.0_f64;
        let mut gated_count = 0_u32;
        for energy in self.energies[..self.used].iter() {
            let loudness = energy_to_lufs(*energy);
            if loudness > -70.0 && loudness > relative_gate {
                gated += f64::from(*energy);
                gated_count += 1;
            }
        }
        if gated_count == 0 {
            -120.0
        } else {
            energy_to_lufs((gated / f64::from(gated_count)) as f32)
        }
    }

    /// Returns the current gated integrated loudness.
    #[must_use]
    pub fn lufs(&self) -> f32 {
        self.lufs
    }

    /// Resets filters, blocks, and the integrated value.
    pub fn reset(&mut self) {
        self.filters.reset();
        self.frames_in_block = 0;
        self.sum_left = 0.0;
        self.sum_right = 0.0;
        self.energies.fill(0.0);
        self.write = 0;
        self.used = 0;
        self.lufs = -120.0;
    }
}

fn energy_to_lufs(energy: f32) -> f32 {
    if !energy.is_finite() || energy <= 0.0 {
        return -120.0;
    }
    -0.691 + 10.0 * energy.log10()
}

#[derive(Debug, Clone)]
pub(crate) struct Limiter {
    ceiling: f32,
    buffer: Vec<f32>,
    write: usize,
    envelope: f32,
    release_coefficient: f32,
}

impl Limiter {
    pub(crate) fn new(config: &LimiterConfig, sample_rate: u32) -> Result<Self, MixError> {
        config.validate()?;
        let lookahead = (config.lookahead_ms * sample_rate as f32 / 1000.0).round() as usize;
        Ok(Self {
            ceiling: db_to_linear(config.ceiling_db),
            buffer: vec![0.0; lookahead.max(1)],
            write: 0,
            envelope: 1.0,
            release_coefficient: 1.0
                - (-1.0 / ((config.release_ms / 1000.0).max(0.000_01) * sample_rate as f32)).exp(),
        })
    }

    pub(crate) fn process(&mut self, sample: f32) -> f32 {
        let (sample, _) = sanitize(sample);
        let delayed = self.buffer[self.write];
        self.buffer[self.write] = sample;
        self.write = (self.write + 1) % self.buffer.len();
        let peak = sample.abs().max(delayed.abs());
        let target = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        if target < self.envelope {
            self.envelope = target;
        } else {
            self.envelope += (target - self.envelope) * self.release_coefficient;
        }
        (delayed * self.envelope).clamp(-self.ceiling, self.ceiling)
    }

    pub(crate) fn ceiling(&self) -> f32 {
        self.ceiling
    }
}

impl Default for LoudnessMeter {
    fn default() -> Self {
        Self::new(48_000)
    }
}

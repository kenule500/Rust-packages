//! Constant-power stereo panning.

use std::f32::consts::FRAC_PI_4;

/// Returns the left and right gains for a pan position in `-1.0..=1.0`.
///
/// The law is constant power: `cos` and `sin` of the angle from centre, so a
/// centred source is -3 dB in each channel.
#[must_use]
pub fn constant_power(pan: f32) -> (f32, f32) {
    let position = pan.clamp(-1.0, 1.0);
    let angle = (position + 1.0) * FRAC_PI_4;
    (angle.cos(), angle.sin())
}

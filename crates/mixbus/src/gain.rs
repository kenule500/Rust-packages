//! Decibel conversion and finite-value sanitisation.

/// Converts a gain in decibels to a linear multiplier.
///
/// Negative infinity maps to zero; NaN maps to zero so a bad control value can
/// never poison the audio path.
#[must_use]
pub fn db_to_linear(db: f32) -> f32 {
    if !db.is_finite() {
        return 0.0;
    }
    10.0_f32.powf(db / 20.0)
}

/// Converts a linear amplitude to decibels, flooring at -120 dB.
#[must_use]
pub fn linear_to_db(linear: f32) -> f32 {
    if !linear.is_finite() || linear <= 1.0e-6 {
        return -120.0;
    }
    (20.0 * linear.log10()).max(-120.0)
}

/// Replaces NaN and infinite samples with zero.
///
/// Returns the sanitised sample and whether a change was made.
#[must_use]
pub fn sanitize(sample: f32) -> (f32, bool) {
    if sample.is_finite() {
        (sample, false)
    } else {
        (0.0, true)
    }
}

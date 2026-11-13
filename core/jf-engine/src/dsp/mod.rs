//! Signal processing: biquad filters, the parametric equalizer, the limiter and ReplayGain.
//!
//! These are plain, allocation-free-per-sample building blocks; the engine thread runs them
//! in its pipeline, and they are public so tests and tools can use them directly.

mod biquad;
mod eq;
mod limiter;
mod replaygain;

pub use biquad::{Biquad, BiquadCoeffs};
pub use eq::{BandKind, EqBand, EqSettings, Equalizer, MAX_BANDS};
pub use limiter::Limiter;
pub use replaygain::replaygain_scale;

/// Decibels to a linear amplitude factor.
pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// A linear amplitude factor to decibels.
pub fn linear_to_db(linear: f32) -> f32 {
    20.0 * linear.log10()
}

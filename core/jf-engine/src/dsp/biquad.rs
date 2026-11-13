//! Second-order IIR filters from the RBJ Audio EQ Cookbook.

use std::f64::consts::PI;

use super::eq::{BandKind, EqBand};

/// Normalised biquad coefficients (`a0 = 1`):
/// `y[n] = b0 x[n] + b1 x[n-1] + b2 x[n-2] - a1 y[n-1] - a2 y[n-2]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiquadCoeffs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl BiquadCoeffs {
    /// Passes the signal through unchanged.
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// Peaking EQ: `gain_db` at `freq_hz`, bandwidth set by `q`.
    pub fn peaking(sample_rate: f64, freq_hz: f64, q: f64, gain_db: f64) -> Self {
        let (a, cos, alpha) = Self::common(sample_rate, freq_hz, q, gain_db);
        Self::normalise(
            [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
        )
    }

    /// Low shelf: `gain_db` below `freq_hz` (half of it at `freq_hz`), slope set by `q`
    /// (0.707 is the steepest without overshoot).
    pub fn low_shelf(sample_rate: f64, freq_hz: f64, q: f64, gain_db: f64) -> Self {
        let (a, cos, alpha) = Self::common(sample_rate, freq_hz, q, gain_db);
        let k = 2.0 * a.sqrt() * alpha;
        Self::normalise(
            [
                a * ((a + 1.0) - (a - 1.0) * cos + k),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                a * ((a + 1.0) - (a - 1.0) * cos - k),
            ],
            [
                (a + 1.0) + (a - 1.0) * cos + k,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                (a + 1.0) + (a - 1.0) * cos - k,
            ],
        )
    }

    /// High shelf: `gain_db` above `freq_hz` (half of it at `freq_hz`).
    pub fn high_shelf(sample_rate: f64, freq_hz: f64, q: f64, gain_db: f64) -> Self {
        let (a, cos, alpha) = Self::common(sample_rate, freq_hz, q, gain_db);
        let k = 2.0 * a.sqrt() * alpha;
        Self::normalise(
            [
                a * ((a + 1.0) + (a - 1.0) * cos + k),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                a * ((a + 1.0) + (a - 1.0) * cos - k),
            ],
            [
                (a + 1.0) - (a - 1.0) * cos + k,
                2.0 * ((a - 1.0) - (a + 1.0) * cos),
                (a + 1.0) - (a - 1.0) * cos - k,
            ],
        )
    }

    /// Coefficients for an EQ band at `sample_rate`, with its parameters clamped to sane
    /// ranges (frequency below 0.45 × the sample rate, Q 0.1–24, gain ±24 dB).
    pub fn for_band(band: &EqBand, sample_rate: f64) -> Self {
        if !band.enabled || band.gain_db == 0.0 || !band.gain_db.is_finite() {
            return Self::IDENTITY;
        }
        let freq = f64::from(band.freq_hz).clamp(10.0, 0.45 * sample_rate);
        let q = if band.q.is_finite() {
            f64::from(band.q).clamp(0.1, 24.0)
        } else {
            0.707
        };
        let gain = f64::from(band.gain_db).clamp(-24.0, 24.0);
        match band.kind {
            BandKind::Peaking => Self::peaking(sample_rate, freq, q, gain),
            BandKind::LowShelf => Self::low_shelf(sample_rate, freq, q, gain),
            BandKind::HighShelf => Self::high_shelf(sample_rate, freq, q, gain),
        }
    }

    /// Magnitude response at `freq_hz`, in dB.
    pub fn magnitude_db(&self, sample_rate: f64, freq_hz: f64) -> f64 {
        let w = 2.0 * PI * freq_hz / sample_rate;
        // H(e^jw) with z^-1 = e^-jw, evaluated with plain complex arithmetic.
        let (c1, s1) = (w.cos(), -w.sin());
        let (c2, s2) = ((2.0 * w).cos(), -(2.0 * w).sin());
        let num_re = self.b0 + self.b1 * c1 + self.b2 * c2;
        let num_im = self.b1 * s1 + self.b2 * s2;
        let den_re = 1.0 + self.a1 * c1 + self.a2 * c2;
        let den_im = self.a1 * s1 + self.a2 * s2;
        let mag2 = (num_re * num_re + num_im * num_im) / (den_re * den_re + den_im * den_im);
        10.0 * mag2.log10()
    }

    fn common(sample_rate: f64, freq_hz: f64, q: f64, gain_db: f64) -> (f64, f64, f64) {
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        (a, w0.cos(), w0.sin() / (2.0 * q))
    }

    fn normalise(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b0: b[0] / a[0],
            b1: b[1] / a[0],
            b2: b[2] / a[0],
            a1: a[1] / a[0],
            a2: a[2] / a[0],
        }
    }
}

impl Default for BiquadCoeffs {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// One biquad channel in transposed direct form II with `f64` state.
#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    coeffs: BiquadCoeffs,
    z1: f64,
    z2: f64,
}

impl Biquad {
    pub fn new(coeffs: BiquadCoeffs) -> Self {
        Self {
            coeffs,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn coeffs(&self) -> BiquadCoeffs {
        self.coeffs
    }

    /// Changes the response, keeping the state so a running signal does not click.
    pub fn set_coeffs(&mut self, coeffs: BiquadCoeffs) {
        self.coeffs = coeffs;
    }

    /// Clears the filter memory.
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let c = &self.coeffs;
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }

    /// Flushes denormal-range state to zero (call once per block).
    pub(crate) fn settle(&mut self) {
        if self.z1.abs() < 1e-30 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1e-30 {
            self.z2 = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference values from an independent implementation of the RBJ cookbook formulas
    // (Python, double precision).
    fn assert_coeffs(actual: BiquadCoeffs, expected: [f64; 5]) {
        let got = [actual.b0, actual.b1, actual.b2, actual.a1, actual.a2];
        for (g, e) in got.iter().zip(expected) {
            assert!((g - e).abs() < 1e-12, "got {got:?}, expected {expected:?}");
        }
    }

    #[test]
    fn peaking_matches_reference() {
        let c = BiquadCoeffs::peaking(48_000.0, 1_000.0, 1.0, 6.0);
        assert_coeffs(
            c,
            [
                1.043953086990335,
                -1.895320723936596,
                0.867722284759857,
                -1.895320723936596,
                0.911675371750192,
            ],
        );
        assert!((c.magnitude_db(48_000.0, 1_000.0) - 6.0).abs() < 1e-9);
        assert!(c.magnitude_db(48_000.0, 10.0).abs() < 0.01);
        assert!(c.magnitude_db(48_000.0, 20_000.0).abs() < 0.05);
    }

    #[test]
    fn low_shelf_matches_reference() {
        let c = BiquadCoeffs::low_shelf(44_100.0, 100.0, 0.707, 6.0);
        assert_coeffs(
            c,
            [
                1.003503469490367,
                -1.982973508002929,
                0.979754363678846,
                -1.983044420514301,
                0.983186920657841,
            ],
        );
        assert!((c.magnitude_db(44_100.0, 100.0) - 3.0).abs() < 1e-9);
        assert!((c.magnitude_db(44_100.0, 5.0) - 6.0).abs() < 0.05);
        assert!(c.magnitude_db(44_100.0, 10_000.0).abs() < 0.01);
    }

    #[test]
    fn high_shelf_matches_reference() {
        let c = BiquadCoeffs::high_shelf(44_100.0, 8_000.0, 0.707, -4.0);
        assert_coeffs(
            c,
            [
                0.749211986908622,
                -0.285611589854050,
                0.147828046987551,
                -0.631474031746343,
                0.242902475788467,
            ],
        );
        assert!((c.magnitude_db(44_100.0, 8_000.0) + 2.0).abs() < 1e-9);
        assert!((c.magnitude_db(44_100.0, 22_000.0) + 4.0).abs() < 0.1);
        assert!(c.magnitude_db(44_100.0, 50.0).abs() < 0.01);
    }

    #[test]
    fn zero_gain_and_disabled_bands_are_identity() {
        let mut band = EqBand::peaking(1_000.0, 0.0, 1.0);
        assert_eq!(BiquadCoeffs::for_band(&band, 48_000.0), BiquadCoeffs::IDENTITY);
        band.gain_db = 3.0;
        band.enabled = false;
        assert_eq!(BiquadCoeffs::for_band(&band, 48_000.0), BiquadCoeffs::IDENTITY);
    }

    #[test]
    fn filtering_a_sine_matches_the_magnitude_response() {
        let fs = 48_000.0;
        let coeffs = BiquadCoeffs::peaking(fs, 1_000.0, 1.0, 6.0);
        let mut filter = Biquad::new(coeffs);
        let (mut peak_in, mut peak_out) = (0.0f64, 0.0f64);
        for n in 0..48_000 {
            let x = (2.0 * PI * 1_000.0 * f64::from(n) / fs).sin() * 0.25;
            let y = filter.process(x);
            if n > 24_000 {
                peak_in = peak_in.max(x.abs());
                peak_out = peak_out.max(y.abs());
            }
        }
        let gain_db = 20.0 * (peak_out / peak_in).log10();
        assert!((gain_db - 6.0).abs() < 0.01, "{gain_db}");
    }
}

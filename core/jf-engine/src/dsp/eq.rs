//! Parametric equalizer: up to ten biquad bands and a preamp.

use serde::{Deserialize, Serialize};

use super::biquad::{Biquad, BiquadCoeffs};
use super::db_to_linear;

/// Most bands an [`EqSettings`] may hold; extra bands are ignored.
pub const MAX_BANDS: usize = 10;

/// Band centres of the presets: the ISO octave frequencies of a 10-band graphic EQ.
const PRESET_FREQS: [f32; MAX_BANDS] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];
/// One octave wide.
const PRESET_Q: f32 = 1.41;

/// Filter shape of an EQ band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BandKind {
    Peaking,
    LowShelf,
    HighShelf,
}

/// One EQ band.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EqBand {
    pub kind: BandKind,
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub enabled: bool,
}

impl EqBand {
    pub fn peaking(freq_hz: f32, gain_db: f32, q: f32) -> Self {
        Self {
            kind: BandKind::Peaking,
            freq_hz,
            gain_db,
            q,
            enabled: true,
        }
    }

    pub fn low_shelf(freq_hz: f32, gain_db: f32, q: f32) -> Self {
        Self {
            kind: BandKind::LowShelf,
            freq_hz,
            gain_db,
            q,
            enabled: true,
        }
    }

    pub fn high_shelf(freq_hz: f32, gain_db: f32, q: f32) -> Self {
        Self {
            kind: BandKind::HighShelf,
            freq_hz,
            gain_db,
            q,
            enabled: true,
        }
    }
}

/// Equalizer settings: a preamp and up to [`MAX_BANDS`] bands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqSettings {
    pub enabled: bool,
    pub preamp_db: f32,
    pub bands: Vec<EqBand>,
}

impl Default for EqSettings {
    /// The flat preset, switched off.
    fn default() -> Self {
        Self {
            enabled: false,
            ..Self::flat()
        }
    }
}

impl EqSettings {
    /// Ten one-octave peaking bands at 0 dB.
    pub fn flat() -> Self {
        Self::graphic([0.0; MAX_BANDS])
    }

    /// Lifts the lowest octaves.
    pub fn bass_boost() -> Self {
        Self::graphic([6.0, 5.0, 4.0, 2.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0])
    }

    /// Brings voices forward: presence up, lows and the top end a little down.
    pub fn vocal() -> Self {
        Self::graphic([-2.0, -2.0, -1.0, 0.0, 1.5, 3.0, 3.0, 2.0, 0.0, -1.0])
    }

    /// Lifts the highest octaves.
    pub fn treble() -> Self {
        Self::graphic([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.5, 3.0, 4.5, 6.0])
    }

    /// The presets every app offers, in their order, by name: `flat`, `bass_boost`, `vocal`,
    /// `treble` (the apps show "Flat", "Bass Boost", "Vocal" and "Treble").
    pub fn presets() -> Vec<(&'static str, EqSettings)> {
        vec![
            ("flat", Self::flat()),
            ("bass_boost", Self::bass_boost()),
            ("vocal", Self::vocal()),
            ("treble", Self::treble()),
        ]
    }

    fn graphic(gains: [f32; MAX_BANDS]) -> Self {
        let bands = PRESET_FREQS
            .iter()
            .zip(gains)
            .map(|(&f, g)| EqBand::peaking(f, g, PRESET_Q))
            .collect();
        Self {
            enabled: true,
            preamp_db: 0.0,
            bands,
        }
    }

    /// The bands in use: enabled, within the first [`MAX_BANDS`].
    fn active_bands(&self) -> impl Iterator<Item = &EqBand> {
        self.bands.iter().take(MAX_BANDS).filter(|b| b.enabled)
    }

    /// Whether these settings change the signal at all.
    pub fn is_active(&self) -> bool {
        self.enabled && (self.preamp_db != 0.0 || self.active_bands().any(|b| b.gain_db != 0.0))
    }

    /// An upper bound on the gain these settings can apply at any frequency, in dB: the preamp
    /// plus every band's boost (bands can overlap). The engine engages its limiter when this
    /// is above 0 dB.
    pub fn max_boost_db(&self) -> f32 {
        if !self.is_active() {
            return 0.0;
        }
        self.preamp_db
            + self
                .active_bands()
                .map(|b| b.gain_db.clamp(0.0, 24.0))
                .sum::<f32>()
    }
}

/// Applies [`EqSettings`] to interleaved audio, one filter per band and channel.
#[derive(Debug, Clone)]
pub struct Equalizer {
    settings: EqSettings,
    sample_rate: u32,
    channels: usize,
    preamp: f32,
    /// Filters per band; each band holds one filter per channel.
    bands: Vec<Vec<Biquad>>,
    active: bool,
}

impl Equalizer {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        let mut eq = Self {
            settings: EqSettings::default(),
            sample_rate: sample_rate.max(1),
            channels: channels.max(1),
            preamp: 1.0,
            bands: Vec::new(),
            active: false,
        };
        eq.rebuild();
        eq
    }

    pub fn settings(&self) -> &EqSettings {
        &self.settings
    }

    /// Changes the settings. Filters of bands that still exist keep their state, so a change
    /// during playback does not click.
    pub fn set_settings(&mut self, settings: &EqSettings) {
        self.settings = settings.clone();
        self.rebuild();
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        if sample_rate.max(1) != self.sample_rate {
            self.sample_rate = sample_rate.max(1);
            self.rebuild();
        }
    }

    /// Whether processing changes the signal (when not, [`process`](Self::process) does nothing).
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Clears the filter memories (after a seek).
    pub fn reset(&mut self) {
        self.bands.iter_mut().flatten().for_each(Biquad::reset);
    }

    /// Filters interleaved audio in place.
    pub fn process(&mut self, samples: &mut [f32]) {
        if !self.active {
            return;
        }
        let channels = self.channels;
        for frame in samples.chunks_exact_mut(channels) {
            for (ch, sample) in frame.iter_mut().enumerate() {
                let mut x = f64::from(*sample * self.preamp);
                for band in &mut self.bands {
                    x = band[ch].process(x);
                }
                *sample = x as f32;
            }
        }
        self.bands.iter_mut().flatten().for_each(Biquad::settle);
    }

    fn rebuild(&mut self) {
        self.active = self.settings.is_active();
        self.preamp = if self.settings.enabled {
            db_to_linear(self.settings.preamp_db.clamp(-24.0, 24.0))
        } else {
            1.0
        };
        let rate = f64::from(self.sample_rate);
        let coeffs: Vec<BiquadCoeffs> = self
            .settings
            .active_bands()
            .map(|band| BiquadCoeffs::for_band(band, rate))
            .filter(|c| *c != BiquadCoeffs::IDENTITY)
            .collect();
        self.bands.truncate(coeffs.len());
        for (i, c) in coeffs.into_iter().enumerate() {
            match self.bands.get_mut(i) {
                Some(filters) => filters.iter_mut().for_each(|f| f.set_coeffs(c)),
                None => self.bands.push(vec![Biquad::new(c); self.channels]),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_ten_bands() {
        for preset in [
            EqSettings::flat(),
            EqSettings::bass_boost(),
            EqSettings::vocal(),
            EqSettings::treble(),
        ] {
            assert_eq!(preset.bands.len(), MAX_BANDS);
            assert!(preset.enabled);
        }
        assert!(!EqSettings::flat().is_active());
        assert!(EqSettings::bass_boost().is_active());
        assert!(!EqSettings::default().is_active());
    }

    #[test]
    fn max_boost_sums_preamp_and_boosts() {
        let mut eq = EqSettings::flat();
        assert_eq!(eq.max_boost_db(), 0.0);
        eq.bands[5].gain_db = 6.0;
        eq.bands[2].gain_db = -3.0;
        eq.preamp_db = -2.0;
        assert_eq!(eq.max_boost_db(), 4.0);
        eq.enabled = false;
        assert_eq!(eq.max_boost_db(), 0.0);
    }

    #[test]
    fn inactive_equalizer_is_bit_transparent() {
        let mut eq = Equalizer::new(44_100, 2);
        eq.set_settings(&EqSettings::flat());
        let mut samples = [0.1f32, -0.2, 0.3, -0.4];
        eq.process(&mut samples);
        assert_eq!(samples, [0.1, -0.2, 0.3, -0.4]);
    }
}

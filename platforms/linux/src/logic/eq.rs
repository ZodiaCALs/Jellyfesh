//! The equalizer page: presets, band labels and edits.
//!
//! The presets are the core's own (`EqSettings::flat()`, …); the page only
//! recognises which one the current bands match and edits gains.

use jf_core::EqSettings;

/// Band gains the sliders allow, in dB.
pub const GAIN_RANGE_DB: f32 = 12.0;

/// The core's presets, in the order the page lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Flat,
    BassBoost,
    Vocal,
    Treble,
    /// Anything else.
    Custom,
}

impl Preset {
    pub const NAMED: [Preset; 4] = [Preset::Flat, Preset::BassBoost, Preset::Vocal, Preset::Treble];

    /// The preset's settings (`Custom` has none).
    pub fn settings(self) -> Option<EqSettings> {
        match self {
            Preset::Flat => Some(EqSettings::flat()),
            Preset::BassBoost => Some(EqSettings::bass_boost()),
            Preset::Vocal => Some(EqSettings::vocal()),
            Preset::Treble => Some(EqSettings::treble()),
            Preset::Custom => None,
        }
    }

    /// Position in the preset list (Custom last).
    pub fn index(self) -> u32 {
        match self {
            Preset::Flat => 0,
            Preset::BassBoost => 1,
            Preset::Vocal => 2,
            Preset::Treble => 3,
            Preset::Custom => 4,
        }
    }

    pub fn from_index(index: u32) -> Preset {
        Preset::NAMED
            .get(index as usize)
            .copied()
            .unwrap_or(Preset::Custom)
    }
}

/// The preset whose bands `eq` matches (gains within 0.05 dB, same
/// frequencies), or `Custom`.
pub fn detect(eq: &EqSettings) -> Preset {
    for preset in Preset::NAMED {
        let Some(p) = preset.settings() else { continue };
        let same = p.bands.len() == eq.bands.len()
            && p.bands.iter().zip(&eq.bands).all(|(a, b)| {
                (a.freq_hz - b.freq_hz).abs() < 0.5
                    && (a.gain_db - b.gain_db).abs() < 0.05
                    && a.kind == b.kind
            })
            && (p.preamp_db - eq.preamp_db).abs() < 0.05;
        if same {
            return preset;
        }
    }
    Preset::Custom
}

/// Applies a preset, keeping whether the equalizer is on.
pub fn apply_preset(current: &EqSettings, preset: Preset) -> EqSettings {
    match preset.settings() {
        Some(mut eq) => {
            eq.enabled = current.enabled;
            eq
        }
        None => current.clone(),
    }
}

/// Sets band `index` to `gain_db` (clamped to the slider range).
pub fn set_gain(current: &EqSettings, index: usize, gain_db: f32) -> EqSettings {
    let mut eq = current.clone();
    if let Some(band) = eq.bands.get_mut(index) {
        band.gain_db = (gain_db.clamp(-GAIN_RANGE_DB, GAIN_RANGE_DB) * 10.0).round() / 10.0;
    }
    eq
}

/// A short frequency label: `31`, `250`, `1k`, `16k`.
pub fn band_label(freq_hz: f32) -> String {
    if freq_hz >= 1_000.0 {
        let k = freq_hz / 1_000.0;
        if (k - k.round()).abs() < 0.05 {
            format!("{}k", k.round() as u32)
        } else {
            format!("{k:.1}k")
        }
    } else {
        format!("{}", freq_hz.round() as u32)
    }
}

/// A gain for a label or accessible value: `+3 dB`, `0 dB`, `−2.5 dB`.
pub fn gain_label(db: f32) -> String {
    let rounded = (db * 10.0).round() / 10.0;
    let body = if rounded.fract().abs() < 0.05 {
        format!("{}", rounded.abs().round() as i32)
    } else {
        format!("{:.1}", rounded.abs())
    };
    if rounded > 0.0 {
        format!("+{body} dB")
    } else if rounded < 0.0 {
        format!("\u{2212}{body} dB")
    } else {
        "0 dB".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_presets() {
        assert_eq!(detect(&EqSettings::flat()), Preset::Flat);
        assert_eq!(detect(&EqSettings::bass_boost()), Preset::BassBoost);
        assert_eq!(detect(&EqSettings::vocal()), Preset::Vocal);
        assert_eq!(detect(&EqSettings::treble()), Preset::Treble);
        let custom = set_gain(&EqSettings::flat(), 3, 2.0);
        assert_eq!(detect(&custom), Preset::Custom);
    }

    #[test]
    fn presets_keep_the_switch() {
        let mut off = EqSettings::flat();
        off.enabled = false;
        let bass = apply_preset(&off, Preset::BassBoost);
        assert!(!bass.enabled);
        assert_eq!(detect(&bass), Preset::BassBoost);
        assert_eq!(apply_preset(&off, Preset::Custom), off);
        for p in Preset::NAMED {
            assert_eq!(Preset::from_index(p.index()), p);
        }
        assert_eq!(Preset::from_index(99), Preset::Custom);
    }

    #[test]
    fn gains_are_clamped_and_rounded() {
        let eq = set_gain(&EqSettings::flat(), 0, 30.0);
        assert_eq!(eq.bands[0].gain_db, GAIN_RANGE_DB);
        let eq = set_gain(&eq, 1, -2.345);
        assert_eq!(eq.bands[1].gain_db, -2.3);
        assert_eq!(set_gain(&eq, 99, 1.0), eq);
    }

    #[test]
    fn labels() {
        assert_eq!(band_label(31.0), "31");
        assert_eq!(band_label(1_000.0), "1k");
        assert_eq!(band_label(16_000.0), "16k");
        assert_eq!(band_label(2_500.0), "2.5k");
        assert_eq!(gain_label(3.0), "+3 dB");
        assert_eq!(gain_label(0.0), "0 dB");
        assert_eq!(gain_label(-2.5), "\u{2212}2.5 dB");
    }
}

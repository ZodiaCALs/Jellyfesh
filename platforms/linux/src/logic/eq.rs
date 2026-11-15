//! The equalizer group: which preset the bands match, and edits.
//!
//! The presets and their order are the core's (`jf_core::eq_presets()`);
//! the group only recognises which one the current bands match, edits
//! gains and the preamp, and resets to flat.

use jf_core::{EqPreset, EqSettings};

/// Band gains and the preamp the sliders allow, in dB.
pub const GAIN_RANGE_DB: f32 = 12.0;

/// The presets every app offers, in the core's order.
pub fn presets() -> Vec<EqPreset> {
    jf_core::eq_presets()
}

/// The position in `presets` of the one whose bands `eq` matches (same
/// frequencies and shapes, gains within 0.05 dB), or `None` once a band has
/// moved off every preset (Custom). The preamp is a control of its own and
/// does not count.
pub fn detect(presets: &[EqPreset], eq: &EqSettings) -> Option<usize> {
    presets.iter().position(|preset| {
        let p = &preset.settings;
        p.bands.len() == eq.bands.len()
            && p.bands.iter().zip(&eq.bands).all(|(a, b)| {
                (a.freq_hz - b.freq_hz).abs() < 0.5
                    && (a.gain_db - b.gain_db).abs() < 0.05
                    && a.kind == b.kind
            })
    })
}

/// `current` with the bands of `preset`, keeping whether the equalizer is on
/// and the person's preamp.
pub fn apply_preset(current: &EqSettings, preset: &EqSettings) -> EqSettings {
    EqSettings {
        enabled: current.enabled,
        preamp_db: current.preamp_db,
        bands: preset.bands.clone(),
    }
}

/// Reset to Flat: every band and the preamp at 0 dB, keeping the bands'
/// frequencies and whether the equalizer is on.
pub fn reset_to_flat(current: &EqSettings) -> EqSettings {
    let mut eq = current.clone();
    eq.preamp_db = 0.0;
    for band in &mut eq.bands {
        band.gain_db = 0.0;
    }
    eq
}

/// Whether every band and the preamp are at 0 dB (Reset to Flat would change
/// nothing).
pub fn is_flat(eq: &EqSettings) -> bool {
    eq.preamp_db.abs() < 0.05 && eq.bands.iter().all(|b| b.gain_db.abs() < 0.05)
}

/// A gain as the sliders keep it: within the range, to 0.1 dB.
fn slider_gain(gain_db: f32) -> f32 {
    (gain_db.clamp(-GAIN_RANGE_DB, GAIN_RANGE_DB) * 10.0).round() / 10.0
}

/// Sets band `index` to `gain_db` (clamped to the slider range).
pub fn set_gain(current: &EqSettings, index: usize, gain_db: f32) -> EqSettings {
    let mut eq = current.clone();
    if let Some(band) = eq.bands.get_mut(index) {
        band.gain_db = slider_gain(gain_db);
    }
    eq
}

/// Sets the preamp to `gain_db` (clamped to the slider range).
pub fn set_preamp(current: &EqSettings, gain_db: f32) -> EqSettings {
    EqSettings {
        preamp_db: slider_gain(gain_db),
        ..current.clone()
    }
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

    fn position(id: &str) -> Option<usize> {
        presets().iter().position(|p| p.id == id)
    }

    #[test]
    fn the_presets_are_the_cores() {
        let ids: Vec<String> = presets().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["flat", "bass_boost", "vocal", "treble"]);
    }

    #[test]
    fn detects_presets() {
        let all = presets();
        assert_eq!(detect(&all, &EqSettings::flat()), position("flat"));
        assert_eq!(detect(&all, &EqSettings::bass_boost()), position("bass_boost"));
        assert_eq!(detect(&all, &EqSettings::vocal()), position("vocal"));
        assert_eq!(detect(&all, &EqSettings::treble()), position("treble"));
        // Custom once a band moves.
        let custom = set_gain(&EqSettings::flat(), 3, 2.0);
        assert_eq!(detect(&all, &custom), None);
        assert_eq!(detect(&[], &EqSettings::flat()), None);
    }

    #[test]
    fn the_preamp_does_not_make_it_custom() {
        let all = presets();
        let louder = set_preamp(&EqSettings::bass_boost(), 3.0);
        assert_eq!(detect(&all, &louder), position("bass_boost"));
    }

    #[test]
    fn presets_keep_the_switch_and_the_preamp() {
        let all = presets();
        let mut off = set_preamp(&EqSettings::flat(), -4.0);
        off.enabled = false;
        let bass = all.iter().find(|p| p.id == "bass_boost").expect("bass boost");
        let applied = apply_preset(&off, &bass.settings);
        assert!(!applied.enabled);
        assert_eq!(applied.preamp_db, -4.0);
        assert_eq!(applied.bands, bass.settings.bands);
        assert_eq!(detect(&all, &applied), position("bass_boost"));
    }

    #[test]
    fn reset_to_flat_zeroes_bands_and_preamp() {
        let mut eq = set_preamp(&EqSettings::vocal(), 2.5);
        eq.enabled = false;
        assert!(!is_flat(&eq));
        let flat = reset_to_flat(&eq);
        assert!(is_flat(&flat));
        assert!(!flat.enabled);
        assert_eq!(detect(&presets(), &flat), position("flat"));
        assert!(is_flat(&EqSettings::flat()));
        assert!(!is_flat(&set_preamp(&EqSettings::flat(), 1.0)));
    }

    #[test]
    fn gains_are_clamped_and_rounded() {
        let eq = set_gain(&EqSettings::flat(), 0, 30.0);
        assert_eq!(eq.bands[0].gain_db, GAIN_RANGE_DB);
        let eq = set_gain(&eq, 1, -2.345);
        assert_eq!(eq.bands[1].gain_db, -2.3);
        assert_eq!(set_gain(&eq, 99, 1.0), eq);
        assert_eq!(set_preamp(&eq, -40.0).preamp_db, -GAIN_RANGE_DB);
        assert_eq!(set_preamp(&eq, 1.26).preamp_db, 1.3);
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

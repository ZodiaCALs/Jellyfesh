//! Preferences → Appearance (design-controls.md, "Preferences →
//! Appearance"): the accent swatches and the custom hue, the transparency
//! slider, and the segmented controls' choices, as settings values. The
//! core keeps every value (`SetSettings`); these functions only map
//! controls to settings and back.

use jf_core::{AppearancePref, BackdropStyle, HueSource, LyricsAlign, LyricsSize};

/// The accent swatches' hues in the order shown: blue, sky, teal, green,
/// lime, amber, orange, red, pink, violet.
pub const ACCENT_HUES: [f32; 10] = [250.0, 210.0, 180.0, 150.0, 120.0, 60.0, 30.0, 0.0, 330.0, 290.0];

/// A hue in degrees, wrapped into 0 ≤ h < 360.
pub fn wrap_hue(hue: f64) -> f32 {
    if !hue.is_finite() {
        return 0.0;
    }
    let h = hue.rem_euclid(360.0);
    // rem_euclid of a value just under 0 can round up to 360.
    if h >= 360.0 { 0.0 } else { h as f32 }
}

/// The hue the custom slider (0 to 360) stands for, in whole degrees.
pub fn slider_hue(value: f64) -> f32 {
    wrap_hue(value.round())
}

/// The swatch showing `hue` (within half a degree), if any.
pub fn swatch_index(hue: f32) -> Option<usize> {
    let h = wrap_hue(f64::from(hue));
    ACCENT_HUES.iter().position(|&s| {
        let d = (s - h).abs();
        d.min(360.0 - d) < 0.5
    })
}

/// Whether the accent follows the artwork ("Follow the artwork" on).
pub fn follows_artwork(source: &HueSource) -> bool {
    matches!(source, HueSource::Artwork)
}

/// The hue the custom slider and the preview show: the fixed hue, or the
/// theme's current hue while the accent follows the artwork.
pub fn shown_hue(source: &HueSource, theme_hue: Option<f32>) -> f32 {
    match source {
        HueSource::Fixed { hue } => wrap_hue(f64::from(*hue)),
        HueSource::Artwork => wrap_hue(f64::from(theme_hue.unwrap_or(jf_tokens::DEFAULT_HUE))),
    }
}

/// The hue source when "Follow the artwork" is switched: off keeps the
/// hue on screen, so the colours do not jump.
pub fn hue_source_for(follow: bool, shown: f32) -> HueSource {
    if follow {
        HueSource::Artwork
    } else {
        HueSource::Fixed {
            hue: wrap_hue(f64::from(shown)),
        }
    }
}

/// Transparency as a whole percentage, 0 (Solid) to 100 (Clear).
pub fn transparency_percent(transparency: f32) -> u32 {
    let t = if transparency.is_finite() {
        transparency
    } else {
        0.5
    };
    (t.clamp(0.0, 1.0) * 100.0).round() as u32
}

/// The transparency setting for a slider at `percent`.
pub fn transparency_from_percent(percent: f64) -> f32 {
    if !percent.is_finite() {
        return 0.5;
    }
    ((percent.round() / 100.0).clamp(0.0, 1.0)) as f32
}

/// The transparency glass is drawn with: the setting, or solid with
/// "Reduce motion and transparency" or high contrast (Misty MAT-08).
pub fn glass_transparency(setting: f32, reduce: bool, high_contrast: bool) -> f32 {
    if reduce || high_contrast {
        0.0
    } else if setting.is_finite() {
        setting.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// Theme: System · Light · Dark.
pub fn appearance_index(pref: AppearancePref) -> u32 {
    match pref {
        AppearancePref::System => 0,
        AppearancePref::Light => 1,
        AppearancePref::Dark => 2,
    }
}

pub fn appearance_at(index: u32) -> AppearancePref {
    match index {
        1 => AppearancePref::Light,
        2 => AppearancePref::Dark,
        _ => AppearancePref::System,
    }
}

/// Now Playing background: Artwork · Misty.
pub fn style_index(style: BackdropStyle) -> u32 {
    match style {
        BackdropStyle::Artwork => 0,
        BackdropStyle::Misty => 1,
    }
}

pub fn style_at(index: u32) -> BackdropStyle {
    if index == 1 {
        BackdropStyle::Misty
    } else {
        BackdropStyle::Artwork
    }
}

/// Lyrics size: Small · Medium · Large.
pub fn lyrics_size_index(size: LyricsSize) -> u32 {
    match size {
        LyricsSize::Small => 0,
        LyricsSize::Medium => 1,
        LyricsSize::Large => 2,
    }
}

pub fn lyrics_size_at(index: u32) -> LyricsSize {
    match index {
        0 => LyricsSize::Small,
        2 => LyricsSize::Large,
        _ => LyricsSize::Medium,
    }
}

/// Lyrics alignment: Start · Centre.
pub fn lyrics_align_index(align: LyricsAlign) -> u32 {
    match align {
        LyricsAlign::Start => 0,
        LyricsAlign::Center => 1,
    }
}

pub fn lyrics_align_at(index: u32) -> LyricsAlign {
    if index == 1 {
        LyricsAlign::Center
    } else {
        LyricsAlign::Start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swatches_are_the_ten_hues_of_the_spec() {
        assert_eq!(ACCENT_HUES.len(), 10);
        assert_eq!(swatch_index(250.0), Some(0));
        assert_eq!(swatch_index(0.0), Some(7));
        assert_eq!(swatch_index(359.8), Some(7));
        assert_eq!(swatch_index(290.3), Some(9));
        assert_eq!(swatch_index(291.0), None);
        assert_eq!(swatch_index(-110.0), Some(0));
    }

    #[test]
    fn hues_wrap_into_a_circle() {
        assert_eq!(wrap_hue(360.0), 0.0);
        assert_eq!(wrap_hue(-30.0), 330.0);
        assert_eq!(wrap_hue(725.0), 5.0);
        assert_eq!(wrap_hue(f64::NAN), 0.0);
        assert_eq!(slider_hue(359.6), 0.0);
        assert_eq!(slider_hue(42.4), 42.0);
    }

    #[test]
    fn following_the_artwork_keeps_the_hue_on_screen() {
        assert!(follows_artwork(&HueSource::Artwork));
        assert!(!follows_artwork(&HueSource::Fixed { hue: 30.0 }));
        assert_eq!(shown_hue(&HueSource::Fixed { hue: 400.0 }, Some(10.0)), 40.0);
        assert_eq!(shown_hue(&HueSource::Artwork, Some(18.0)), 18.0);
        assert_eq!(shown_hue(&HueSource::Artwork, None), jf_tokens::DEFAULT_HUE);
        assert_eq!(hue_source_for(false, 18.0), HueSource::Fixed { hue: 18.0 });
        assert_eq!(hue_source_for(true, 18.0), HueSource::Artwork);
    }

    #[test]
    fn transparency_is_a_percentage() {
        assert_eq!(transparency_percent(0.5), 50);
        assert_eq!(transparency_percent(0.0), 0);
        assert_eq!(transparency_percent(1.0), 100);
        assert_eq!(transparency_percent(7.0), 100);
        assert_eq!(transparency_percent(f32::NAN), 50);
        assert_eq!(transparency_from_percent(50.0), 0.5);
        assert_eq!(transparency_from_percent(33.4), 0.33);
        assert_eq!(transparency_from_percent(150.0), 1.0);
        assert_eq!(transparency_from_percent(f64::NAN), 0.5);
    }

    #[test]
    fn reduced_transparency_and_high_contrast_make_glass_solid() {
        assert_eq!(glass_transparency(0.8, false, false), 0.8);
        assert_eq!(glass_transparency(0.8, true, false), 0.0);
        assert_eq!(glass_transparency(0.8, false, true), 0.0);
        assert_eq!(glass_transparency(f32::NAN, false, false), 0.5);
        assert_eq!(glass_transparency(-1.0, false, false), 0.0);
    }

    #[test]
    fn segmented_choices_round_trip() {
        for i in 0..3 {
            assert_eq!(appearance_index(appearance_at(i)), i);
            assert_eq!(lyrics_size_index(lyrics_size_at(i)), i);
        }
        for i in 0..2 {
            assert_eq!(style_index(style_at(i)), i);
            assert_eq!(lyrics_align_index(lyrics_align_at(i)), i);
        }
        assert_eq!(appearance_at(9), AppearancePref::System);
        assert_eq!(lyrics_size_at(9), LyricsSize::Medium);
    }
}

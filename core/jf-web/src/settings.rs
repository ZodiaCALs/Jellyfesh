//! Jellyfesh Web's settings: the part of the core's `Settings` a browser
//! player uses, with the same names, defaults and `serde` shapes (a test
//! compares them with `jf_core::Settings`), plus two of its own: the
//! streaming quality and the interface language.

use jf_plugin_model::Quality;
use serde::{Deserialize, Serialize};

use crate::rules::{CrossfadeSettings, EqSettings, ReplayGainMode};

/// Light, dark, or following the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearancePref {
    #[default]
    System,
    Light,
    Dark,
}

/// The Now Playing backdrop: the baked artwork, or Misty's own field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackdropStyle {
    #[default]
    Artwork,
    Misty,
}

/// Where the theme hue comes from.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HueSource {
    /// The current track's artwork (the fallback hue without one).
    #[default]
    Artwork,
    /// One fixed hue in degrees.
    Fixed { hue: f32 },
}

/// The size of lyrics on Now Playing (sizes multiply by the `lyrics.scale-*`
/// tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// How lyrics lines line up on Now Playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsAlign {
    #[default]
    Start,
    Center,
}

/// The settings. Missing fields load as their defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebSettings {
    pub appearance: AppearancePref,
    pub style: BackdropStyle,
    pub hue_source: HueSource,
    /// Fewer translucent layers and shorter motion, beyond the system setting.
    pub reduce_effects: bool,
    /// How see-through glass is, from 0 (solid) to 1 (clearest); 0.5 keeps
    /// the tokens' own alphas (`jf_palette::glass_alpha`).
    pub transparency: f32,
    pub lyrics_size: LyricsSize,
    pub lyrics_align: LyricsAlign,
    pub show_translation: bool,
    pub show_romanization: bool,
    pub crossfade: CrossfadeSettings,
    pub replaygain_mode: ReplayGainMode,
    pub preamp_db: f32,
    pub eq: EqSettings,
    /// Start playing the restored queue when the page opens.
    pub resume_on_launch: bool,
    /// The quality asked of the server first (web only).
    pub quality: Quality,
    /// The interface language (`en`, `zh-Hans`, `zh-Hant`, `ja`); `None`
    /// follows the browser (web only).
    pub language: Option<String>,
}

impl Default for WebSettings {
    fn default() -> Self {
        WebSettings {
            appearance: AppearancePref::System,
            style: BackdropStyle::Artwork,
            hue_source: HueSource::Artwork,
            reduce_effects: false,
            transparency: 0.5,
            lyrics_size: LyricsSize::Medium,
            lyrics_align: LyricsAlign::Start,
            show_translation: true,
            show_romanization: true,
            crossfade: CrossfadeSettings::default(),
            replaygain_mode: ReplayGainMode::Off,
            preamp_db: 0.0,
            eq: EqSettings::flat(),
            resume_on_launch: false,
            quality: crate::rules::FIRST_QUALITY,
            language: None,
        }
    }
}

impl WebSettings {
    /// The settings as the core stores them: the preamp within ±15 dB, the
    /// crossfade within 1–12 s, a known language or none.
    pub fn normalized(mut self) -> Self {
        self.preamp_db = if self.preamp_db.is_finite() {
            self.preamp_db.clamp(-15.0, 15.0)
        } else {
            0.0
        };
        self.crossfade = self.crossfade.clamped();
        self.transparency = if self.transparency.is_finite() {
            self.transparency.clamp(0.0, 1.0)
        } else {
            0.5
        };
        if let HueSource::Fixed { hue } = &mut self.hue_source {
            *hue = if hue.is_finite() {
                hue.rem_euclid(360.0)
            } else {
                0.0
            };
        }
        if !matches!(
            self.language.as_deref(),
            None | Some("en" | "zh-Hans" | "zh-Hant" | "ja")
        ) {
            self.language = None;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_partial_settings_and_normalizes() {
        let s: WebSettings = serde_json::from_str(r#"{"style": "misty", "preamp_db": 40}"#).unwrap();
        assert_eq!(s.style, BackdropStyle::Misty);
        let s = s.normalized();
        assert_eq!(s.preamp_db, 15.0);
        assert_eq!(s.quality, Quality::Lossless);
        let fixed = WebSettings {
            hue_source: HueSource::Fixed { hue: -30.0 },
            language: Some("xx".into()),
            ..WebSettings::default()
        }
        .normalized();
        assert_eq!(fixed.hue_source, HueSource::Fixed { hue: 330.0 });
        assert_eq!(fixed.language, None);
    }
}

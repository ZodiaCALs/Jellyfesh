//! Jellyfesh Web's settings: the part of the core's `Settings` a browser
//! player uses, with the same names, defaults and `serde` shapes (a test
//! compares them with `jf_core::Settings`), plus one of its own: the
//! interface language. Settings saved before v0.0.8 kept the streaming
//! quality as `quality` (`"Lossless"`…); they read as `streaming_quality`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::quality::StreamingQuality;
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

/// Settings › Appearance › Mini Player (ADR-0026 §4), as the core keeps
/// them. The web shows Show Lyrics Line only: a picture-in-picture window
/// always stays on top and closes with the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MiniPlayerSettings {
    pub keep_on_top: bool,
    pub open_when_window_closed: bool,
    /// The lyrics bar's current line in the mini player. On by default.
    pub show_lyrics_line: bool,
    pub android_pip: bool,
}

impl Default for MiniPlayerSettings {
    fn default() -> Self {
        MiniPlayerSettings {
            keep_on_top: true,
            open_when_window_closed: false,
            show_lyrics_line: true,
            android_pip: false,
        }
    }
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
    /// Streaming Quality (ADR-0026 §1): `Auto` by default.
    pub streaming_quality: StreamingQuality,
    /// The interface language (`en`, `zh-Hans`, `zh-Hant`, `ja`); `None`
    /// follows the browser (web only).
    pub language: Option<String>,
    /// Show Lyrics Bar (Settings › Appearance › Lyrics): the one-line lyrics
    /// bar under the player. On by default, as in the core.
    pub show_lyrics_bar: bool,
    /// Desktop Lyrics (Settings › Appearance › Lyrics): the current line in a
    /// window of its own above other apps. On the web that window is a
    /// Document Picture-in-Picture window, where the browser offers one. Off
    /// by default, as in the core.
    pub desktop_lyrics: bool,
    /// Settings › Appearance › Mini Player.
    pub mini_player: MiniPlayerSettings,
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
            streaming_quality: StreamingQuality::Auto,
            language: None,
            show_lyrics_bar: true,
            desktop_lyrics: false,
            mini_player: MiniPlayerSettings::default(),
        }
    }
}

impl WebSettings {
    /// Settings as the page kept them, from any version: the `quality` of
    /// earlier versions becomes `streaming_quality`.
    pub fn from_json(text: &str) -> serde_json::Result<WebSettings> {
        let mut value: Value = serde_json::from_str(text)?;
        if let Value::Object(fields) = &mut value
            && !fields.contains_key("streaming_quality")
            && let Some(Value::String(old)) = fields.remove("quality")
        {
            let level = match old.as_str() {
                "Lossless" => Some("lossless"),
                "High" => Some("high"),
                "Normal" => Some("normal"),
                "Low" => Some("low"),
                _ => None,
            };
            if let Some(level) = level {
                fields.insert("streaming_quality".into(), Value::String(level.into()));
            }
        }
        serde_json::from_value(value)
    }

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
        assert_eq!(s.streaming_quality, StreamingQuality::Auto);
        let fixed = WebSettings {
            hue_source: HueSource::Fixed { hue: -30.0 },
            language: Some("xx".into()),
            ..WebSettings::default()
        }
        .normalized();
        assert_eq!(fixed.hue_source, HueSource::Fixed { hue: 330.0 });
        assert_eq!(fixed.language, None);
    }

    #[test]
    fn settings_saved_before_the_lyrics_bar_take_its_defaults() {
        let s: WebSettings = serde_json::from_str(r#"{"show_translation": false}"#).unwrap();
        assert!(!s.show_translation, "saved fields keep their values");
        assert!(s.show_lyrics_bar);
        assert!(!s.desktop_lyrics);
        let s: WebSettings =
            serde_json::from_str(r#"{"show_lyrics_bar": false, "desktop_lyrics": true}"#).unwrap();
        assert!(!s.show_lyrics_bar);
        assert!(s.desktop_lyrics);
    }

    #[test]
    fn the_old_quality_setting_becomes_streaming_quality() {
        let old = WebSettings::from_json(r#"{"quality": "High", "style": "misty"}"#).unwrap();
        assert_eq!(old.streaming_quality, StreamingQuality::High);
        assert_eq!(old.style, BackdropStyle::Misty);
        let both = WebSettings::from_json(r#"{"quality": "Low", "streaming_quality": "normal"}"#).unwrap();
        assert_eq!(
            both.streaming_quality,
            StreamingQuality::Normal,
            "the new one wins"
        );
        let none = WebSettings::from_json("{}").unwrap();
        assert_eq!(none.streaming_quality, StreamingQuality::Auto);
        assert_eq!(none.mini_player, MiniPlayerSettings::default());
        assert!(none.mini_player.show_lyrics_line);
        let saved = serde_json::to_value(&old).unwrap();
        assert!(saved.get("quality").is_none(), "only the new name is written");
    }
}

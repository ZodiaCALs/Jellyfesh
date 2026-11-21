//! The look (ADR-0020 §2), resolved as the core resolves it (`jf-core/src/look.rs`, the
//! reference): the active appearance plugin's look, checked by `jf_plugin_model::sanitize::look`,
//! with the Luminous defaults for everything it leaves out; the defaults alone without one, or
//! while Pro is off. The page applies the resolved values.

use jf_plugin_model::{
    Density, Highlight, HomeSection, Look, LyricsWeight, MiniPlayerStyle, NowPlayingLayout, Place, Typeface,
};
use serde::Serialize;

/// The look, every value resolved (the core's `LookSnapshot`, without its version).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LookSnapshot {
    /// Radii × this (0–2).
    pub corner_scale: f32,
    /// Glass alpha and blur × this (0–1).
    pub glass: f32,
    /// The aurora field's strength (0–1).
    pub aurora: f32,
    pub typeface: Typeface,
    /// Every type size × this (0.85–1.30).
    pub text_scale: f32,
    pub density: Density,
    pub lyrics_weight: LyricsWeight,
    /// The glow around the sung line (0–1).
    pub lyrics_glow: f32,
    pub lyrics_highlight: Highlight,
    /// The places shown, in order (Settings is never listed, always shown).
    pub sidebar: Vec<Place>,
    /// Home's sections, in order.
    pub home: Vec<HomeSection>,
    pub now_playing: NowPlayingLayout,
    pub mini_player: MiniPlayerStyle,
    /// An appearance plugin sets the look (Pro active).
    pub customized: bool,
    /// That plugin.
    pub plugin: Option<String>,
}

impl LookSnapshot {
    /// The Luminous defaults (the core's `LookSnapshot::defaults`).
    pub fn defaults() -> LookSnapshot {
        LookSnapshot {
            corner_scale: 1.0,
            glass: 1.0,
            aurora: jf_tokens::field::AURORA_STRENGTH,
            typeface: Typeface::System,
            text_scale: 1.0,
            density: Density::Standard,
            lyrics_weight: LyricsWeight::Bold,
            lyrics_glow: 0.6,
            lyrics_highlight: Highlight::Artwork,
            sidebar: Place::ALL.to_vec(),
            home: HomeSection::DEFAULT.to_vec(),
            now_playing: NowPlayingLayout::Stage,
            mini_player: MiniPlayerStyle::Bar,
            customized: false,
            plugin: None,
        }
    }
}

/// The look from `plugin` (checked here as the host checks it), or the defaults without one.
pub fn resolve(look: Option<Look>, plugin: Option<&str>) -> LookSnapshot {
    let defaults = LookSnapshot::defaults();
    let (Some(look), Some(plugin)) = (look, plugin) else {
        return defaults;
    };
    let look = jf_plugin_model::sanitize::look(look);
    LookSnapshot {
        corner_scale: look.corner_scale.unwrap_or(defaults.corner_scale),
        glass: look.glass.unwrap_or(defaults.glass),
        aurora: look.aurora.unwrap_or(defaults.aurora),
        typeface: look.typeface.unwrap_or(defaults.typeface),
        text_scale: look.text_scale.unwrap_or(defaults.text_scale),
        density: look.density.unwrap_or(defaults.density),
        lyrics_weight: look.lyrics_weight.unwrap_or(defaults.lyrics_weight),
        lyrics_glow: look.lyrics_glow.unwrap_or(defaults.lyrics_glow),
        lyrics_highlight: look.lyrics_highlight.unwrap_or(defaults.lyrics_highlight),
        sidebar: look.sidebar.unwrap_or(defaults.sidebar),
        home: look.home.unwrap_or(defaults.home),
        now_playing: look.now_playing.unwrap_or(defaults.now_playing),
        mini_player: look.mini_player.unwrap_or(defaults.mini_player),
        customized: true,
        plugin: Some(plugin.to_owned()),
    }
}

/// Spacing × this for a density (docs/core-api.md "The look").
pub fn density_scale(density: Density) -> f32 {
    match density {
        Density::Compact => 0.8,
        Density::Standard => 1.0,
        Density::Roomy => 1.2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_fill_what_the_plugin_leaves_out() {
        // As jf-core's look test.
        let look = Look {
            corner_scale: Some(1.5),
            typeface: Some(Typeface::Serif),
            sidebar: Some(vec![Place::Songs, Place::Home]),
            lyrics_highlight: Some(Highlight::Custom(0x00ff_8800)),
            ..Look::default()
        };
        let resolved = resolve(Some(look), Some("org.x"));
        assert!(resolved.customized);
        assert_eq!(resolved.corner_scale, 1.5);
        assert_eq!(resolved.glass, 1.0);
        assert_eq!(resolved.aurora, jf_tokens::field::AURORA_STRENGTH);
        assert_eq!(resolved.typeface, Typeface::Serif);
        assert_eq!(resolved.sidebar, vec![Place::Songs, Place::Home]);
        assert_eq!(resolved.home, LookSnapshot::defaults().home);
        assert_eq!(resolved.lyrics_highlight, Highlight::Custom(0x00ff_8800));
        assert_eq!(resolved.plugin.as_deref(), Some("org.x"));
        let none = resolve(None, None);
        assert_eq!(none, LookSnapshot::defaults());
        assert!(!none.customized);
        assert_eq!(
            resolve(Some(Look::default()), None),
            LookSnapshot::defaults(),
            "no plugin, no look"
        );
    }

    #[test]
    fn the_plugins_values_are_checked() {
        let look = Look {
            corner_scale: Some(9.0),
            text_scale: Some(0.1),
            ..Look::default()
        };
        let resolved = resolve(Some(look), Some("org.x"));
        assert!(resolved.corner_scale <= 2.0);
        assert!(resolved.text_scale >= 0.85);
        assert_eq!(density_scale(Density::Roomy), 1.2);
    }
}

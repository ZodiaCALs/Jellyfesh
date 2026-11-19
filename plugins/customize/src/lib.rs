//! Customize: the app's look and layout from the plugin's settings
//! (ADR-0020 §4), a Jellyfesh Pro capability (`appearance`).
//!
//! Three groups of settings: **Look** (Corners, Glass, Aurora, Typeface,
//! Text Size, Density), **Lyrics** (Weight, Glow, Highlight, Highlight
//! Colour) and **Layout** (Sidebar, Home, Now Playing, Mini-Player). The
//! host calls `look` when the plugin loads and after its settings change;
//! [`look`] leaves out every value at its Luminous default, so the app keeps
//! following its own defaults for them. The host clamps the numbers, drops
//! duplicates and keeps a place of the library in the sidebar. No network.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;
use serde_json::Value;

struct Customize;

/// The places of the sidebar, as the settings name them, in their default
/// order (docs/ui.md, Navigation).
const PLACES: [(&str, Place); 8] = [
    ("home", Place::Home),
    ("albums", Place::Albums),
    ("artists", Place::Artists),
    ("songs", Place::Songs),
    ("playlists", Place::Playlists),
    ("favourites", Place::Favourites),
    ("search", Place::Search),
    ("sources", Place::Sources),
];

/// Home's sections; the first four are shown by default.
const SECTIONS: [(&str, HomeSection); 5] = [
    ("summary", HomeSection::Summary),
    ("recently-added", HomeSection::RecentlyAdded),
    ("recently-played", HomeSection::RecentlyPlayed),
    ("most-played", HomeSection::MostPlayed),
    ("favourites", HomeSection::Favourites),
];
const DEFAULT_SECTIONS: usize = 4;

/// The Luminous defaults (ADR-0020 §2): the token `field.aurora-strength`
/// is 0.75.
const CORNERS: f32 = 1.0;
const GLASS: f32 = 1.0;
const AURORA: f32 = 0.75;
const TEXT_SIZE: f32 = 1.0;
const GLOW: f32 = 0.6;

/// The settings, as the schema has them.
#[derive(Debug, Clone, PartialEq)]
struct Settings {
    corners: f32,
    glass: f32,
    aurora: f32,
    typeface: Typeface,
    text_size: f32,
    density: Density,
    lyrics_weight: LyricsWeight,
    lyrics_glow: f32,
    /// `artwork`, `accent` or `custom`.
    highlight: String,
    highlight_colour: Option<u32>,
    sidebar: Vec<Place>,
    home: Vec<HomeSection>,
    now_playing: NowPlayingLayout,
    mini_player: MiniPlayerStyle,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            corners: CORNERS,
            glass: GLASS,
            aurora: AURORA,
            typeface: Typeface::System,
            text_size: TEXT_SIZE,
            density: Density::Standard,
            lyrics_weight: LyricsWeight::Bold,
            lyrics_glow: GLOW,
            highlight: "artwork".into(),
            highlight_colour: Some(0xFF8A3D),
            sidebar: PLACES.iter().map(|(_, p)| *p).collect(),
            home: SECTIONS[..DEFAULT_SECTIONS].iter().map(|(_, s)| *s).collect(),
            now_playing: NowPlayingLayout::Stage,
            mini_player: MiniPlayerStyle::Bar,
        }
    }
}

/// `#RRGGBB` as `0xRRGGBB`.
fn colour(text: &str) -> Option<u32> {
    let hex = text.trim().strip_prefix('#')?;
    (hex.len() == 6)
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
}

impl Settings {
    /// Reads the settings object; anything missing or unknown keeps its default.
    fn parse(value: &Value) -> Settings {
        let defaults = Settings::default();
        let number = |key: &str, default: f32| {
            value
                .get(key)
                .and_then(Value::as_f64)
                .filter(|n| n.is_finite())
                .map_or(default, |n| n as f32)
        };
        let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default();
        let list = |key: &str| -> Option<Vec<&str>> {
            value
                .get(key)?
                .as_array()
                .map(|items| items.iter().filter_map(Value::as_str).collect())
        };
        Settings {
            corners: number("corners", CORNERS),
            glass: number("glass", GLASS),
            aurora: number("aurora", AURORA),
            typeface: match text("typeface") {
                "rounded" => Typeface::Rounded,
                "serif" => Typeface::Serif,
                "mono" => Typeface::Mono,
                _ => Typeface::System,
            },
            text_size: number("text_size", TEXT_SIZE),
            density: match text("density") {
                "compact" => Density::Compact,
                "roomy" => Density::Roomy,
                _ => Density::Standard,
            },
            lyrics_weight: match text("lyrics_weight") {
                "regular" => LyricsWeight::Regular,
                "medium" => LyricsWeight::Medium,
                "semibold" => LyricsWeight::Semibold,
                "heavy" => LyricsWeight::Heavy,
                _ => LyricsWeight::Bold,
            },
            lyrics_glow: number("lyrics_glow", GLOW),
            highlight: match text("highlight") {
                value @ ("accent" | "custom") => value.to_owned(),
                _ => defaults.highlight,
            },
            highlight_colour: value
                .get("highlight_colour")
                .and_then(Value::as_str)
                .map_or(defaults.highlight_colour, colour),
            sidebar: list("sidebar").map_or(defaults.sidebar, |names| {
                names
                    .iter()
                    .filter_map(|name| PLACES.iter().find(|(n, _)| n == name).map(|(_, p)| *p))
                    .collect()
            }),
            home: list("home").map_or(defaults.home, |names| {
                names
                    .iter()
                    .filter_map(|name| SECTIONS.iter().find(|(n, _)| n == name).map(|(_, s)| *s))
                    .collect()
            }),
            now_playing: match text("now_playing") {
                "artwork" => NowPlayingLayout::Artwork,
                "lyrics" => NowPlayingLayout::Lyrics,
                _ => NowPlayingLayout::Stage,
            },
            mini_player: match text("mini_player") {
                "compact" => MiniPlayerStyle::Compact,
                "floating" => MiniPlayerStyle::Floating,
                _ => MiniPlayerStyle::Bar,
            },
        }
    }
}

/// `value` unless it is (about) the default.
fn changed(value: f32, default: f32) -> Option<f32> {
    ((value - default).abs() > 1e-4).then_some(value)
}

/// `value` unless it is the default.
fn unless<T: PartialEq>(value: T, default: T) -> Option<T> {
    (value != default).then_some(value)
}

/// The look for the settings, every default left out.
fn look(settings: &Settings) -> Look {
    let defaults = Settings::default();
    Look {
        corner_scale: changed(settings.corners, CORNERS),
        glass: changed(settings.glass, GLASS),
        aurora: changed(settings.aurora, AURORA),
        typeface: unless(settings.typeface, Typeface::System),
        text_scale: changed(settings.text_size, TEXT_SIZE),
        density: unless(settings.density, Density::Standard),
        lyrics_weight: unless(settings.lyrics_weight, LyricsWeight::Bold),
        lyrics_glow: changed(settings.lyrics_glow, GLOW),
        lyrics_highlight: match settings.highlight.as_str() {
            "accent" => Some(Highlight::Accent),
            "custom" => settings.highlight_colour.map(Highlight::Custom),
            _ => None,
        },
        sidebar: unless(settings.sidebar.clone(), defaults.sidebar),
        home: unless(settings.home.clone(), defaults.home),
        now_playing: unless(settings.now_playing, NowPlayingLayout::Stage),
        mini_player: unless(settings.mini_player, MiniPlayerStyle::Bar),
    }
}

thread_local! {
    static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default());
}

impl Plugin for Customize {
    fn capabilities() -> Vec<Capability> {
        // appearance is a manifest capability that `meta` cannot report.
        Vec::new()
    }

    fn settings_changed(settings: &Value) -> Result<()> {
        let parsed = Settings::parse(settings);
        SETTINGS.with(|s| *s.borrow_mut() = parsed);
        Ok(())
    }

    fn look() -> Result<Look> {
        Ok(SETTINGS.with(|s| look(&s.borrow())))
    }
}

jf_plugin_sdk::export_plugin!(Customize);

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn untouched() -> Look {
        look(&Settings::default())
    }

    #[test]
    fn the_defaults_leave_everything_to_the_app() {
        let empty = untouched();
        assert_eq!(
            (
                empty.corner_scale,
                empty.glass,
                empty.aurora,
                empty.typeface,
                empty.text_scale,
                empty.density
            ),
            (None, None, None, None, None, None)
        );
        assert_eq!(
            (empty.lyrics_weight, empty.lyrics_glow, empty.lyrics_highlight),
            (None, None, None)
        );
        assert_eq!(
            (empty.sidebar, empty.home, empty.now_playing, empty.mini_player),
            (None, None, None, None)
        );
        assert_eq!(look(&Settings::parse(&json!({}))), untouched());
    }

    /// The schema's defaults are the plugin's.
    #[test]
    fn the_schema_says_the_same_defaults() {
        let schema: Value = serde_json::from_str(include_str!("../settings.schema.json")).unwrap();
        let defaults: serde_json::Map<String, Value> = schema["properties"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, p)| (name.clone(), p["default"].clone()))
            .collect();
        assert_eq!(Settings::parse(&Value::Object(defaults)), Settings::default());
        assert_eq!(schema["properties"].as_object().unwrap().len(), 14);
    }

    #[test]
    fn each_setting_maps_to_its_part_of_the_look() {
        let settings = Settings::parse(&json!({
            "corners": 1.5, "glass": 0.25, "aurora": 0.0, "typeface": "serif", "text_size": 1.15,
            "density": "roomy", "lyrics_weight": "heavy", "lyrics_glow": 0.0,
            "highlight": "custom", "highlight_colour": "#12AbEF",
            "sidebar": ["search", "albums", "sources"], "home": ["favourites", "summary"],
            "now_playing": "lyrics", "mini_player": "floating"
        }));
        let look = look(&settings);
        assert_eq!(look.corner_scale, Some(1.5));
        assert_eq!(look.glass, Some(0.25));
        assert_eq!(look.aurora, Some(0.0));
        assert_eq!(look.typeface, Some(Typeface::Serif));
        assert_eq!(look.text_scale, Some(1.15));
        assert_eq!(look.density, Some(Density::Roomy));
        assert_eq!(look.lyrics_weight, Some(LyricsWeight::Heavy));
        assert_eq!(look.lyrics_glow, Some(0.0));
        assert_eq!(look.lyrics_highlight, Some(Highlight::Custom(0x12ABEF)));
        assert_eq!(
            look.sidebar,
            Some(vec![Place::Search, Place::Albums, Place::Sources])
        );
        assert_eq!(
            look.home,
            Some(vec![HomeSection::Favourites, HomeSection::Summary])
        );
        assert_eq!(look.now_playing, Some(NowPlayingLayout::Lyrics));
        assert_eq!(look.mini_player, Some(MiniPlayerStyle::Floating));
    }

    #[test]
    fn highlights_and_lists() {
        let accent = look(&Settings::parse(
            &json!({ "highlight": "accent", "highlight_colour": "#000000" }),
        ));
        assert_eq!(accent.lyrics_highlight, Some(Highlight::Accent));
        let custom = look(&Settings::parse(&json!({ "highlight": "custom" })));
        assert_eq!(
            custom.lyrics_highlight,
            Some(Highlight::Custom(0xFF8A3D)),
            "the default colour"
        );
        let broken = look(&Settings::parse(
            &json!({ "highlight": "custom", "highlight_colour": "red" }),
        ));
        assert_eq!(broken.lyrics_highlight, None);
        // The same places in another order are a change; the same order is not.
        let reordered = look(&Settings::parse(
            &json!({ "sidebar": ["albums", "home", "artists", "songs", "playlists", "favourites", "search", "sources"] }),
        ));
        assert_eq!(reordered.sidebar.map(|s| s[0]), Some(Place::Albums));
        let same = look(&Settings::parse(
            &json!({ "home": ["summary", "recently-added", "recently-played", "most-played"] }),
        ));
        assert_eq!(same.home, None);
        let none = look(&Settings::parse(&json!({ "sidebar": [] })));
        assert_eq!(
            none.sidebar,
            Some(Vec::new()),
            "the host keeps a place of the library"
        );
        let nearly = look(&Settings::parse(
            &json!({ "corners": 1.000_01, "lyrics_glow": 0.6 }),
        ));
        assert_eq!((nearly.corner_scale, nearly.lyrics_glow), (None, None));
    }

    #[test]
    fn manifest_version_matches_the_crate() {
        let manifest = include_str!("../manifest.toml");
        let line = format!("version = \"{}\"", env!("CARGO_PKG_VERSION"));
        assert!(
            manifest.lines().any(|l| l == line),
            "manifest.toml must say {line}"
        );
    }
}

//! The look (ADR-0020, docs/ui.md "The Look slice"): the core's resolved
//! values as the numbers and names the views draw with. With `customized`
//! false every value is the Luminous default, whatever the slice holds, so
//! the app draws exactly as in v0.0.6.
//!
//! Most of the look is CSS: the stylesheet templates are expanded again
//! from the design tokens with the look applied ([`tokens`]): radii × the
//! corner scale, spacing × the density, every type size × the text scale,
//! the aurora's strength, glass blur × the glass, and the lyrics' weight;
//! [`extra_css`] adds the typeface. Glass alpha is the Transparency
//! setting × the glass ([`transparency`]); the glow and the highlight are
//! drawn in code; the sidebar, Home and the layouts are widgets.

use jf_core::{
    Density, HomeSection, LookSnapshot, LyricsHighlight, LyricsWeight, MiniPlayerStyle, NowPlayingLayout,
    Place, Typeface,
};
use serde_json::Value;

/// The colour of sung words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sung {
    /// The lyrics' sung ink (`lyrics.ink-sung`): v0.0.6, and Artwork.
    Ink,
    /// The accent text colour.
    Accent,
    /// The person's colour, `0xRRGGBB`.
    Rgb(u32),
}

/// What sung words are drawn in.
pub fn sung(look: &LookSnapshot) -> Sung {
    if !look.customized {
        return Sung::Ink;
    }
    match look.lyrics_highlight {
        LyricsHighlight::Artwork => Sung::Ink,
        LyricsHighlight::Accent => Sung::Accent,
        LyricsHighlight::Custom { rgb } => Sung::Rgb(rgb & 0xFF_FF_FF),
    }
}

fn clamp(v: f32, lo: f32, hi: f32, otherwise: f32) -> f64 {
    f64::from(if v.is_finite() { v.clamp(lo, hi) } else { otherwise })
}

/// Radii ×, 0–2 (1 is Luminous).
pub fn corner_scale(look: &LookSnapshot) -> f64 {
    if look.customized {
        clamp(look.corner_scale, 0.0, 2.0, 1.0)
    } else {
        1.0
    }
}

/// How much glass shows what is behind it, 0 (solid) to 1 (Luminous).
pub fn glass(look: &LookSnapshot) -> f64 {
    if look.customized {
        clamp(look.glass, 0.0, 1.0, 1.0)
    } else {
        1.0
    }
}

/// The Transparency the glass is drawn with: the setting's, times the
/// look's glass, so the setting and Reduce Transparency still win.
pub fn transparency(setting: f32, look: &LookSnapshot) -> f32 {
    (f64::from(setting.clamp(0.0, 1.0)) * glass(look)) as f32
}

/// The field's aurora strength (`field.aurora-strength` while not
/// customized).
pub fn aurora(look: &LookSnapshot) -> f64 {
    let token = f64::from(jf_tokens::field::AURORA_STRENGTH);
    if look.customized {
        clamp(look.aurora, 0.0, 1.0, token as f32)
    } else {
        token
    }
}

/// Every type size ×, 0.85–1.30.
pub fn text_scale(look: &LookSnapshot) -> f64 {
    if look.customized {
        clamp(look.text_scale, 0.85, 1.3, 1.0)
    } else {
        1.0
    }
}

/// Spacing × 0.8, 1 or 1.2 for the density.
pub fn density_scale(look: &LookSnapshot) -> f64 {
    if !look.customized {
        return 1.0;
    }
    match look.density {
        Density::Compact => 0.8,
        Density::Standard => 1.0,
        Density::Roomy => 1.2,
    }
}

/// The lyrics' weight as CSS: `None` while not customized (the token's).
pub fn lyrics_weight(look: &LookSnapshot) -> Option<u32> {
    look.customized.then_some(match look.lyrics_weight {
        LyricsWeight::Regular => 400,
        LyricsWeight::Medium => 500,
        LyricsWeight::Semibold => 600,
        LyricsWeight::Bold => 700,
        LyricsWeight::Heavy => 800,
    })
}

/// The soft light around a sung line, as a blur radius for a font size:
/// none while not customized (the macOS app's rule: 0.35 × the size × the
/// glow).
pub fn glow_radius(look: &LookSnapshot, font_size: f64) -> f64 {
    if look.customized {
        font_size * 0.35 * clamp(look.lyrics_glow, 0.0, 1.0, 0.0)
    } else {
        0.0
    }
}

/// The typeface as a CSS font family: the system's own faces. GNOME has no
/// rounded face, so Rounded is its sans (docs/ui.md, "Allowed
/// differences").
pub fn font_family(look: &LookSnapshot) -> Option<&'static str> {
    if !look.customized {
        return None;
    }
    match look.typeface {
        Typeface::System | Typeface::Rounded => None,
        Typeface::Serif => Some("serif"),
        Typeface::Mono => Some("monospace"),
    }
}

/// Now Playing's layout: Stage (v0.0.6), Artwork or Lyrics.
pub fn now_playing(look: &LookSnapshot) -> NowPlayingLayout {
    if look.customized {
        look.now_playing
    } else {
        NowPlayingLayout::Stage
    }
}

/// The player at the bottom: Bar (v0.0.6), Compact or Floating.
pub fn mini_player(look: &LookSnapshot) -> MiniPlayerStyle {
    if look.customized {
        look.mini_player
    } else {
        MiniPlayerStyle::Bar
    }
}

const LIBRARY: [Place; 6] = [
    Place::Home,
    Place::Albums,
    Place::Artists,
    Place::Songs,
    Place::Playlists,
    Place::Favourites,
];

const ALL_PLACES: [Place; 8] = [
    Place::Home,
    Place::Albums,
    Place::Artists,
    Place::Songs,
    Place::Playlists,
    Place::Favourites,
    Place::Search,
    Place::Sources,
];

/// The sidebar's places, in order: the look's, each once; all eight (the
/// Luminous order) while not customized, or when none of the library's
/// would be left (the core keeps one, so this only guards a bad answer).
pub fn sidebar(look: &LookSnapshot) -> Vec<Place> {
    if !look.customized {
        return ALL_PLACES.to_vec();
    }
    let mut places: Vec<Place> = Vec::new();
    for p in &look.sidebar {
        if !places.contains(p) {
            places.push(*p);
        }
    }
    if places.iter().any(|p| LIBRARY.contains(p)) {
        places
    } else {
        ALL_PLACES.to_vec()
    }
}

/// Home's sections, in order, each once (Luminous's while not
/// customized).
pub fn home(look: &LookSnapshot) -> Vec<HomeSection> {
    let source = if look.customized {
        look.home.clone()
    } else {
        LookSnapshot::defaults(0).home
    };
    let mut out: Vec<HomeSection> = Vec::new();
    for s in source {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// Scales a dimension token's `$value` (px or rem) by `k`.
fn scale_dimension(token: &mut Value, k: f64) {
    if let Some(v) = token.pointer_mut("/$value/value")
        && let Some(n) = v.as_f64()
    {
        *v = Value::from(n * k);
    }
}

/// Scales the type size of a typography token by `k`.
fn scale_type(token: &mut Value, k: f64) {
    if let Some(v) = token.pointer_mut("/$value/fontSize/value")
        && let Some(n) = v.as_f64()
    {
        *v = Value::from(n * k);
    }
}

/// Applies `f` to every token of `$type` `kind` in `group`.
fn each(tokens: &mut Value, group: &str, kind: &str, f: impl Fn(&str, &mut Value)) {
    let Some(map) = tokens.get_mut(group).and_then(Value::as_object_mut) else {
        return;
    };
    for (name, token) in map.iter_mut() {
        if token.get("$type").and_then(Value::as_str) == Some(kind) {
            f(name, token);
        }
    }
}

/// The design tokens with the look applied: radii × the corner scale (not
/// the pill radius), spacing × the density, every type size × the text
/// scale, the aurora's strength, glass blur × the glass, and the lyrics'
/// weight. Not customized, the tokens are as they are.
pub fn tokens(base: &Value, look: &LookSnapshot) -> Value {
    let mut t = base.clone();
    if !look.customized {
        return t;
    }
    let corners = corner_scale(look);
    each(&mut t, "radius", "dimension", |name, token| {
        if name != "r-full" {
            scale_dimension(token, corners);
        }
    });
    // The artwork's own radii (list, grid, hero), as its widgets draw them.
    each(&mut t, "artwork", "dimension", |name, token| {
        if name.starts_with("radius") {
            scale_dimension(token, corners);
        }
    });
    let density = density_scale(look);
    each(&mut t, "space", "dimension", |_, token| {
        scale_dimension(token, density)
    });
    let text = text_scale(look);
    each(&mut t, "type", "dimension", |name, token| {
        if name != "measure" {
            scale_dimension(token, text);
        }
    });
    each(&mut t, "type-roles", "typography", |_, token| {
        scale_type(token, text)
    });
    each(&mut t, "lyrics", "typography", |_, token| scale_type(token, text));
    let glass = glass(look);
    each(&mut t, "glass", "dimension", |name, token| {
        if name.starts_with("glass-blur") {
            scale_dimension(token, glass);
        }
    });
    if let Some(v) = t.pointer_mut("/field/aurora-strength/$value") {
        *v = Value::from(aurora(look));
    }
    if let (Some(weight), Some(v)) = (
        lyrics_weight(look),
        t.pointer_mut("/lyrics/line/$value/fontWeight"),
    ) {
        *v = Value::from(weight);
    }
    t
}

/// The fluid lyric line sizes the stylesheet has classes for
/// (`.lyrics-<px>`, the Lyrics pane).
const LYRIC_SIZES: std::ops::RangeInclusive<u32> = 19..=48;

/// What the tokens cannot say: the typeface for every window, the Lyrics
/// pane's fluid sizes at the text scale, and the lyrics' weight where the
/// stylesheet gives one of its own (the lyrics bar and Desktop Lyrics).
/// Empty while not customized.
pub fn extra_css(look: &LookSnapshot) -> String {
    use crate::logic::tokens_css::num;
    let mut css = String::new();
    let k = text_scale(look);
    if (k - 1.0).abs() > 1e-6 {
        for px in LYRIC_SIZES {
            css.push_str(&format!(
                ".lyrics-{px} .lyric-text {{ font-size: {}px; }}\n",
                num(f64::from(px) * k)
            ));
        }
        for size in [
            jf_core::LyricsSize::Small,
            jf_core::LyricsSize::Medium,
            jf_core::LyricsSize::Large,
        ] {
            let (translation, romanization) = crate::logic::lyrics::aux_sizes(size);
            css.push_str(&format!(
                ".lyrics-aux-{t} .lyric-translation {{ font-size: {}px; }}\n\
                 .lyrics-aux-{t} .lyric-romanization {{ font-size: {}px; }}\n",
                num(translation * k),
                num(romanization * k),
                t = translation as i32
            ));
        }
    }
    if let Some(family) = font_family(look) {
        css.push_str(&format!(
            "window, popover, tooltip, .jf-settings {{ font-family: {family}; }}\n"
        ));
    }
    if let Some(weight) = lyrics_weight(look) {
        css.push_str(&format!(
            ".lyrics-bar-text, .desktop-lyrics .lyrics-bar-text, .desktop-lyrics-title {{ font-weight: {weight}; }}\n"
        ));
    }
    css
}

/// A stylesheet template expanded with the look: the build's own CSS while
/// not customized.
pub fn stylesheet(template: &str, base: &Value, look: &LookSnapshot) -> Result<String, String> {
    let mut css = crate::logic::tokens_css::expand(template, &tokens(base, look))?;
    css.push_str(&extra_css(look));
    Ok(css)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn custom() -> LookSnapshot {
        LookSnapshot {
            customized: true,
            ..LookSnapshot::defaults(1)
        }
    }

    fn base() -> Value {
        json!({
            "radius": {
                "r-lg": { "$type": "dimension", "$value": { "value": 0.75, "unit": "rem" } },
                "r-full": { "$type": "dimension", "$value": { "value": 999, "unit": "px" } }
            },
            "space": { "s3": { "$type": "dimension", "$value": { "value": 0.75, "unit": "rem" } } },
            "artwork": {
                "radius-hero": { "$type": "dimension", "$value": { "value": 24, "unit": "px" } },
                "size-mini": { "$type": "dimension", "$value": { "value": 44, "unit": "px" } }
            },
            "type": {
                "text-h1": { "$type": "dimension", "$value": { "value": 2, "unit": "rem" } },
                "measure": { "$type": "dimension", "$value": { "value": 640, "unit": "px" } }
            },
            "type-roles": {
                "list-title": { "$type": "typography", "$value": { "fontSize": { "value": 15, "unit": "px" }, "lineHeight": 1.3, "fontWeight": 600 } }
            },
            "lyrics": {
                "line": { "$type": "typography", "$value": { "fontSize": { "value": 40, "unit": "px" }, "lineHeight": 1.18, "fontWeight": 700 } }
            },
            "glass": { "glass-blur": { "$type": "dimension", "$value": { "value": 24, "unit": "px" } } },
            "field": { "aurora-strength": { "$type": "number", "$value": 0.75 } }
        })
    }

    const TEMPLATE: &str = ".card { border-radius: {{radius.r-lg}}; padding: {{space.s3}}; } \
         .pill { border-radius: {{radius.r-full}}; } h1 { font-size: {{type.text-h1}}; max-width: {{type.measure}}; } \
         .row { {{type-roles.list-title}}; } .lyric-text { {{lyrics.line}}; } \
         .glass { filter: blur({{glass.glass-blur}}); } .field { opacity: {{field.aurora-strength * 1.07}}; } \
         .hero { border-radius: {{artwork.radius-hero}}; min-width: {{artwork.size-mini}}; }";

    #[test]
    fn not_customized_is_luminous() {
        let look = LookSnapshot {
            lyrics_highlight: LyricsHighlight::Accent,
            corner_scale: 2.0,
            typeface: Typeface::Serif,
            now_playing: NowPlayingLayout::Lyrics,
            sidebar: vec![Place::Songs],
            ..LookSnapshot::defaults(1)
        };
        assert_eq!(sung(&look), Sung::Ink);
        assert_eq!(tokens(&base(), &look), base());
        assert_eq!(extra_css(&look), "");
        assert_eq!(
            stylesheet(TEMPLATE, &base(), &look).unwrap(),
            crate::logic::tokens_css::expand(TEMPLATE, &base()).unwrap(),
            "the build's own stylesheet"
        );
        assert_eq!(now_playing(&look), NowPlayingLayout::Stage);
        assert_eq!(mini_player(&look), MiniPlayerStyle::Bar);
        assert_eq!(sidebar(&look).len(), 8);
        assert_eq!(glow_radius(&look, 40.0), 0.0);
        assert_eq!(transparency(0.5, &look), 0.5);
    }

    #[test]
    fn the_highlight() {
        let mut look = custom();
        assert_eq!(sung(&look), Sung::Ink);
        look.lyrics_highlight = LyricsHighlight::Accent;
        assert_eq!(sung(&look), Sung::Accent);
        look.lyrics_highlight = LyricsHighlight::Custom { rgb: 0xFF12_3456 };
        assert_eq!(sung(&look), Sung::Rgb(0x12_3456));
    }

    #[test]
    fn the_stylesheet_takes_the_look() {
        let look = LookSnapshot {
            corner_scale: 1.5,
            density: Density::Compact,
            text_scale: 1.2,
            glass: 0.5,
            aurora: 0.3,
            lyrics_weight: LyricsWeight::Medium,
            typeface: Typeface::Serif,
            ..custom()
        };
        let css = stylesheet(TEMPLATE, &base(), &look).unwrap();
        // 12 px × 1.5; the pill stays a pill.
        assert!(
            css.contains(".card { border-radius: 18px; padding: 9.6px; }"),
            "{css}"
        );
        assert!(css.contains(".pill { border-radius: 999px; }"), "{css}");
        // 32 px × 1.2; the measure is a width, not a type size.
        assert!(
            css.contains("h1 { font-size: 38.4px; max-width: 640px; }"),
            "{css}"
        );
        assert!(
            css.contains(".row { font-size: 18px; line-height: 1.3; font-weight: 600; }"),
            "{css}"
        );
        assert!(
            css.contains(".lyric-text { font-size: 48px; line-height: 1.18; font-weight: 500; }"),
            "{css}"
        );
        assert!(css.contains("filter: blur(12px)"), "{css}");
        // The artwork's radius with the rest; its size stays.
        assert!(
            css.contains(".hero { border-radius: 36px; min-width: 44px; }"),
            "{css}"
        );
        assert!(css.contains("opacity: 0.321"), "{css}");
        assert!(css.contains("font-family: serif"), "{css}");
        assert!(css.contains(
            ".lyrics-bar-text, .desktop-lyrics .lyrics-bar-text, .desktop-lyrics-title { font-weight: 500; }"
        ));
        // The Lyrics pane's fluid sizes, at the text scale.
        assert!(
            css.contains(".lyrics-40 .lyric-text { font-size: 48px; }"),
            "{css}"
        );
        assert!(
            css.contains(".lyrics-aux-18 .lyric-translation { font-size: 21.6px; }"),
            "{css}"
        );
    }

    #[test]
    fn values_stay_in_range() {
        let look = LookSnapshot {
            corner_scale: 9.0,
            text_scale: 0.1,
            glass: f32::NAN,
            aurora: -1.0,
            ..custom()
        };
        assert_eq!(corner_scale(&look), 2.0);
        assert!((text_scale(&look) - 0.85).abs() < 1e-6);
        assert_eq!(glass(&look), 1.0);
        assert_eq!(aurora(&look), 0.0);
        assert!(
            (transparency(
                0.8,
                &LookSnapshot {
                    glass: 0.5,
                    ..custom()
                }
            ) - 0.4)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn typefaces_and_glow() {
        let face = |typeface| font_family(&LookSnapshot { typeface, ..custom() });
        assert_eq!(face(Typeface::System), None);
        assert_eq!(face(Typeface::Rounded), None, "GNOME's sans");
        assert_eq!(face(Typeface::Serif), Some("serif"));
        assert_eq!(face(Typeface::Mono), Some("monospace"));
        let look = LookSnapshot {
            lyrics_glow: 0.5,
            ..custom()
        };
        assert!((glow_radius(&look, 40.0) - 7.0).abs() < 1e-6);
    }

    #[test]
    fn places_and_sections_in_the_looks_order() {
        let look = LookSnapshot {
            sidebar: vec![Place::Songs, Place::Search, Place::Songs, Place::Home],
            home: vec![
                HomeSection::MostPlayed,
                HomeSection::Favourites,
                HomeSection::MostPlayed,
            ],
            ..custom()
        };
        assert_eq!(sidebar(&look), [Place::Songs, Place::Search, Place::Home]);
        assert_eq!(home(&look), [HomeSection::MostPlayed, HomeSection::Favourites]);
        // Without a library place: every place.
        let none = LookSnapshot {
            sidebar: vec![Place::Search],
            ..custom()
        };
        assert_eq!(sidebar(&none).len(), 8);
        assert_eq!(
            home(&LookSnapshot::defaults(1)),
            [
                HomeSection::Summary,
                HomeSection::RecentlyAdded,
                HomeSection::RecentlyPlayed,
                HomeSection::MostPlayed
            ]
        );
    }
}

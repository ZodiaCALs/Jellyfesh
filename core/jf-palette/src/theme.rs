//! Misty theme resolution: every themed colour from one hue, in light or dark appearance.

use std::collections::BTreeMap;

use jf_tokens::{ColorDef, DEFAULT_HUE, THEME_COLORS};
use serde::{Deserialize, Serialize};

use crate::color::{normalize_hue, oklch_to_srgb64, quantize};

/// Light or dark appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    /// Dark ink on a light field.
    Light,
    /// Light ink on a dark field.
    Dark,
}

impl Appearance {
    /// Both appearances, light first.
    pub const ALL: [Appearance; 2] = [Appearance::Light, Appearance::Dark];

    /// Whether this is the dark appearance.
    pub const fn is_dark(self) -> bool {
        matches!(self, Appearance::Dark)
    }

    /// `"light"` or `"dark"`, as serialised (for cache keys such as `backdrop/<hash>-dark.png`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }
}

/// Every theme colour resolved to 8-bit sRGB with alpha, keyed by token name (`ink.ink-90`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Theme {
    /// The theme hue in degrees, normalised to 0..360.
    pub hue: f32,
    /// The appearance the colours are for.
    pub appearance: Appearance,
    /// RGBA per colour name.
    pub colors: BTreeMap<String, [u8; 4]>,
}

impl Theme {
    /// The colour named `name` (such as `"accent.accent"`), if the theme has it.
    pub fn color(&self, name: &str) -> Option<[u8; 4]> {
        self.colors.get(name).copied()
    }

    /// This theme with the alpha of its glass tints ([`GLASS_TINTS`]) set for
    /// the transparency setting (see [`glass_alpha`]); other colours are
    /// unchanged.
    pub fn with_transparency(&self, transparency: f32) -> Theme {
        let mut theme = self.clone();
        for name in GLASS_TINTS {
            if let Some(rgba) = theme.colors.get_mut(name) {
                let alpha = glass_alpha(f32::from(rgba[3]) / 255.0, transparency);
                rgba[3] = (alpha * 255.0).round() as u8;
            }
        }
        theme
    }
}

/// The theme colours that are glass grades (Clear, Mist, Veil), whose alpha
/// the transparency setting scales.
pub const GLASS_TINTS: [&str; 3] = ["glass.tint-clear", "glass.tint-mist", "glass.tint-veil"];

/// The alpha of a glass grade whose token alpha is `alpha`, at the
/// transparency setting `transparency` (0 solid, 1 clearest).
///
/// At 0.5 the token's alpha applies unchanged. Toward 0 it rises linearly to
/// 1, so glass becomes opaque; toward 1 it falls linearly to half the
/// token's alpha. Inputs are clamped to 0..=1; a transparency that is not
/// finite counts as 0.5 and an alpha that is not finite as 1. Every app's
/// glass follows this (contract vectors: `tests/contract/glass-alpha.json`).
pub fn glass_alpha(alpha: f32, transparency: f32) -> f32 {
    let a = if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let t = if transparency.is_finite() {
        transparency.clamp(0.0, 1.0)
    } else {
        0.5
    };
    // In f64, so the ends and the default come out exact.
    let (a, t) = (f64::from(a), f64::from(t));
    let out = if t <= 0.5 {
        a + (1.0 - a) * (1.0 - t / 0.5)
    } else {
        a * (1.0 - (t - 0.5))
    };
    (out as f32).clamp(0.0, 1.0)
}

/// Resolve every colour in [`jf_tokens::THEME_COLORS`] at `hue` for `appearance`.
///
/// Relative hues are offsets added to `hue`; fixed hues (status colours, white edges) ignore it.
/// A hue that is not finite falls back to [`jf_tokens::DEFAULT_HUE`].
pub fn resolve_theme(hue: f32, appearance: Appearance) -> Theme {
    let hue = if hue.is_finite() {
        normalize_hue(hue)
    } else {
        DEFAULT_HUE
    };
    let colors = THEME_COLORS
        .iter()
        .map(|def| (def.name.to_owned(), resolve_color(def, hue, appearance)))
        .collect();
    Theme {
        hue,
        appearance,
        colors,
    }
}

/// One theme colour at `hue` for `appearance`, as 8-bit RGBA.
pub fn resolve_color(def: &ColorDef, hue: f32, appearance: Appearance) -> [u8; 4] {
    let lcha = def.appearance(appearance.is_dark());
    let [r, g, b] = oklch_to_srgb64(lcha.l.into(), lcha.c.into(), lcha.hue_at(hue).into()).map(quantize);
    [r, g, b, quantize(lcha.alpha.into())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{composite, contrast_ratio, hex};

    #[test]
    fn glass_alpha_keeps_the_token_alpha_at_one_half() {
        for a in [0.0, 0.3, 0.42, 0.62, 1.0] {
            assert_eq!(glass_alpha(a, 0.5), a);
        }
    }

    #[test]
    fn glass_alpha_runs_from_solid_to_half_the_token_alpha() {
        assert_eq!(glass_alpha(0.42, 0.0), 1.0);
        assert!((glass_alpha(0.42, 0.25) - 0.71).abs() < 1e-6);
        assert!((glass_alpha(0.42, 0.75) - 0.315).abs() < 1e-6);
        assert!((glass_alpha(0.42, 1.0) - 0.21).abs() < 1e-6);
        // Monotonic: more transparency, never more opaque.
        let mut last = 1.0;
        for step in 0..=100 {
            let a = glass_alpha(0.62, step as f32 / 100.0);
            assert!(a <= last + 1e-6, "step {step}: {a} > {last}");
            last = a;
        }
    }

    #[test]
    fn glass_alpha_clamps_its_inputs() {
        assert_eq!(glass_alpha(0.42, -1.0), 1.0);
        assert!((glass_alpha(0.42, 2.0) - 0.21).abs() < 1e-6);
        assert_eq!(glass_alpha(0.42, f32::NAN), 0.42);
        assert_eq!(glass_alpha(f32::NAN, 0.75), 0.75);
        assert_eq!(glass_alpha(1.5, 0.5), 1.0);
    }

    #[test]
    fn with_transparency_changes_only_the_glass_tints() {
        let theme = resolve_theme(250.0, Appearance::Light);
        let solid = theme.with_transparency(0.0);
        let same = theme.with_transparency(0.5);
        for name in GLASS_TINTS {
            assert!(theme.colors.contains_key(name), "{name} is a theme colour");
            assert_eq!(solid.color(name).unwrap()[3], 255, "{name} solid");
            assert_eq!(same.color(name), theme.color(name), "{name} unchanged at 0.5");
            let base = theme.color(name).unwrap();
            assert_eq!(
                solid.color(name).unwrap()[..3],
                base[..3],
                "{name} keeps its colour"
            );
        }
        for (name, rgba) in &theme.colors {
            if !GLASS_TINTS.contains(&name.as_str()) {
                assert_eq!(solid.colors[name], *rgba, "{name} untouched");
            }
        }
    }

    /// `$value.hex` of every colour token in `design/tokens.json`: Misty's own sRGB at hue 250,
    /// light appearance (alpha is not part of the hex).
    const MISTY_LIGHT_250: &[(&str, &str)] = &[
        ("field.sky", "#eef3f9"),
        ("field.sky-high", "#f7fafe"),
        ("field.aurora-1", "#97d2ff"),
        ("field.aurora-2", "#e5ccff"),
        ("field.aurora-3", "#beefe4"),
        ("ink.ink-90", "#101820"),
        ("ink.ink-70", "#101820"),
        ("ink.ink-50", "#101820"),
        ("ink.ink-30", "#101820"),
        ("ink.ink-10", "#101820"),
        ("ink.ink-06", "#101820"),
        ("accent.accent", "#0059a7"),
        ("accent.accent-hover", "#00509d"),
        ("accent.accent-press", "#004794"),
        ("accent.accent-text", "#0059a2"),
        ("accent.accent-soft", "#d2ebff"),
        ("accent.on-accent", "#f9fcff"),
        ("semantic.success", "#00723b"),
        ("semantic.warning", "#845100"),
        ("semantic.danger", "#be222a"),
        ("semantic.info", "#1a609e"),
        ("semantic.creative", "#764297"),
        ("line.line", "#101820"),
        ("line.line-soft", "#101820"),
        ("line.line-strong", "#101820"),
        ("line.fill", "#101820"),
        ("line.fill-hover", "#101820"),
        ("line.fill-press", "#101820"),
        ("glass.glass-edge", "#ffffff"),
        ("glass.glass-edge-soft", "#ffffff"),
        ("glass.cloud", "#f9fbfe"),
        ("glass.cloud-soft", "#f2f6fb"),
        ("elevation.scrim", "#0b1723"),
    ];

    /// Dark neutrals and accent at hue 250 from `docs/design-tokens.md`.
    const MISTY_DARK_250: &[(&str, &str)] = &[
        ("field.sky", "#0a1016"),
        ("field.sky-high", "#12181e"),
        ("glass.cloud", "#191f25"),
        ("glass.cloud-soft", "#1f262e"),
        ("ink.ink-90", "#f6f9fc"),
        ("accent.accent", "#7abdff"),
    ];

    fn opaque_hex(theme: &Theme, name: &str) -> String {
        let [r, g, b, _] = theme.color(name).unwrap_or_else(|| panic!("{name} missing"));
        hex([r, g, b, 255])
    }

    #[test]
    fn matches_every_misty_colour_at_hue_250() {
        let light = resolve_theme(250.0, Appearance::Light);
        for (name, expected) in MISTY_LIGHT_250 {
            assert_eq!(opaque_hex(&light, name), *expected, "light {name}");
        }
        let dark = resolve_theme(250.0, Appearance::Dark);
        for (name, expected) in MISTY_DARK_250 {
            assert_eq!(opaque_hex(&dark, name), *expected, "dark {name}");
        }
        assert_eq!(light.color("ink.ink-90"), Some([0x10, 0x18, 0x20, 235]));
    }

    #[test]
    fn covers_every_theme_colour() {
        let theme = resolve_theme(DEFAULT_HUE, Appearance::Dark);
        assert_eq!(theme.colors.len(), THEME_COLORS.len());
        assert!(THEME_COLORS.iter().all(|def| theme.colors.contains_key(def.name)));
        assert_eq!(theme.appearance, Appearance::Dark);
    }

    #[test]
    fn relative_hues_follow_and_fixed_hues_stay() {
        let a = resolve_theme(30.0, Appearance::Light);
        let b = resolve_theme(200.0, Appearance::Light);
        assert_ne!(a.color("accent.accent"), b.color("accent.accent"));
        assert_ne!(a.color("field.sky"), b.color("field.sky"));
        assert_eq!(a.color("semantic.success"), b.color("semantic.success"));
        assert_eq!(a.color("glass.glass-edge"), b.color("glass.glass-edge"));
    }

    #[test]
    fn hue_is_normalised() {
        assert_eq!(
            resolve_theme(610.0, Appearance::Light),
            resolve_theme(250.0, Appearance::Light)
        );
        assert_eq!(
            resolve_theme(-110.0, Appearance::Dark),
            resolve_theme(250.0, Appearance::Dark)
        );
        assert_eq!(resolve_theme(f32::NAN, Appearance::Dark).hue, DEFAULT_HUE);
        assert_eq!(resolve_theme(360.0, Appearance::Dark).hue, 0.0);
    }

    #[test]
    fn appearance_names_and_colour_lookup() {
        assert_eq!(Appearance::Dark.as_str(), "dark");
        let theme = resolve_theme(250.0, Appearance::Light);
        assert_eq!(theme.colors["accent.accent"], [0x00, 0x59, 0xa7, 255]);
    }

    /// Misty's contrast promise (COL-10): text pairs hold WCAG AA at every hue, both appearances.
    #[test]
    fn text_pairs_hold_aa_at_every_hue() {
        let text = [
            "ink.ink-90",
            "ink.ink-70",
            "ink.ink-50",
            "accent.accent-text",
            "accent.accent",
        ];
        let surfaces = ["field.sky", "field.sky-high", "glass.cloud", "glass.cloud-soft"];
        let mut pairs: Vec<(&str, &str)> = text
            .iter()
            .flat_map(|fg| surfaces.iter().map(move |bg| (*fg, *bg)))
            .collect();
        pairs.push(("accent.on-accent", "accent.accent"));
        let status = [
            "semantic.success",
            "semantic.warning",
            "semantic.danger",
            "semantic.info",
            "semantic.creative",
        ];
        pairs.extend(status.map(|fg| (fg, "field.sky")));
        for appearance in Appearance::ALL {
            for hue in 0..360 {
                let theme = resolve_theme(hue as f32, appearance);
                for (fg, bg) in &pairs {
                    let [r, g, b, _] = theme.colors[*bg];
                    let bg_rgb = [r, g, b].map(|v| f32::from(v) / 255.0);
                    let ratio = contrast_ratio(composite(theme.colors[*fg], bg_rgb), bg_rgb);
                    assert!(
                        ratio >= 4.5,
                        "{fg} on {bg} at hue {hue} ({appearance:?}): {ratio:.2}"
                    );
                }
            }
        }
    }
}

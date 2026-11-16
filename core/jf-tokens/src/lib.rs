//! Design tokens generated from `design/tokens.json` (do not edit `src/generated.rs` by hand).
//!
//! Run `cargo xtask tokens` after changing the token file; CI runs `cargo xtask tokens --check`
//! and fails when the generated file is stale.
//!
//! * One module per token group (`space`, `radius`, `type_`, `motion`, `glass`, `backdrop`, …)
//!   holds its static metrics as constants. Dimensions are px (1 rem = 16 px), durations are
//!   milliseconds (`_MS`), curves are cubic-bezier control points `[x1, y1, x2, y2]`.
//! * Theme colours are OKLCH definitions, not sRGB: [`THEME_COLORS`] lists every one, and
//!   `jf-palette` resolves them at run time from one hue (Misty PLT-03). [`DEFAULT_HUE`] is the
//!   hue to use before an artwork hue is known.
//!
//! The types below are written by hand; `generated.rs` only holds data.

mod generated;

pub use generated::*;

/// One appearance of a theme colour in OKLCH, with alpha.
///
/// When `hue_relative` is set, `h` is an offset in degrees from the theme hue; otherwise it is
/// the hue itself (status colours and neutrals that never follow the artwork).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lcha {
    /// OKLab lightness, 0 to 1.
    pub l: f32,
    /// OKLCH chroma.
    pub c: f32,
    /// Hue in degrees, or an offset from the theme hue.
    pub h: f32,
    /// Opacity, 0 to 1.
    pub alpha: f32,
    /// Whether `h` is added to the theme hue.
    pub hue_relative: bool,
}

impl Lcha {
    /// A colour whose hue follows the theme hue, offset by `h` degrees.
    pub const fn relative(l: f32, c: f32, h: f32, alpha: f32) -> Self {
        Lcha {
            l,
            c,
            h,
            alpha,
            hue_relative: true,
        }
    }

    /// A colour with a fixed hue of `h` degrees.
    pub const fn fixed(l: f32, c: f32, h: f32, alpha: f32) -> Self {
        Lcha {
            l,
            c,
            h,
            alpha,
            hue_relative: false,
        }
    }

    /// The hue in degrees at theme hue `theme_hue` (not normalised to 0..360).
    pub fn hue_at(&self, theme_hue: f32) -> f32 {
        if self.hue_relative {
            theme_hue + self.h
        } else {
            self.h
        }
    }
}

/// A themed colour: its name (`group.token`, such as `ink.ink-90`) and both appearances.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorDef {
    /// `group.token`, the key the theme snapshot uses.
    pub name: &'static str,
    /// The light appearance.
    pub light: Lcha,
    /// The dark appearance.
    pub dark: Lcha,
}

impl ColorDef {
    /// The definition for the dark (`true`) or light (`false`) appearance.
    pub const fn appearance(&self, dark: bool) -> Lcha {
        if dark { self.dark } else { self.light }
    }
}

/// Look up a theme colour by name, such as `"accent.accent"`.
pub fn theme_color(name: &str) -> Option<&'static ColorDef> {
    THEME_COLORS.iter().find(|def| def.name == name)
}

/// A number with a different value in dark appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Themed {
    /// The value in light appearance.
    pub light: f32,
    /// The value in dark appearance.
    pub dark: f32,
}

impl Themed {
    /// A light and a dark value.
    pub const fn new(light: f32, dark: f32) -> Self {
        Themed { light, dark }
    }

    /// The value for the dark (`true`) or light (`false`) appearance.
    pub const fn appearance(self, dark: bool) -> f32 {
        if dark { self.dark } else { self.light }
    }
}

/// A text style: size in px, line height as a multiple of the size, weight 1 to 1000.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Typography {
    /// Font size in px (pt on macOS, dp on Android, epx on Windows).
    pub size: f32,
    /// Line height as a multiple of the size.
    pub line_height: f32,
    /// Font weight, 1 to 1000 (400 regular, 700 bold).
    pub weight: u16,
}

impl Typography {
    /// A text style.
    pub const fn new(size: f32, line_height: f32, weight: u16) -> Self {
        Typography {
            size,
            line_height,
            weight,
        }
    }
}

/// The range a fluid size moves in as the window grows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fluid {
    /// Smallest size in px.
    pub min: f32,
    /// Largest size in px.
    pub max: f32,
    /// Preferred size in percent of the viewport width, when the token gives one.
    pub vw: Option<f32>,
}

impl Fluid {
    /// A fluid range.
    pub const fn new(min: f32, max: f32, vw: Option<f32>) -> Self {
        Fluid { min, max, vw }
    }
}

/// A glass grade: tint alpha per appearance and the blur radius behind it (CSS sigma, px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassGrade {
    /// `clear`, `mist` or `veil`.
    pub name: &'static str,
    /// Tint alpha in light and dark appearance.
    pub alpha: Themed,
    /// Backdrop blur radius in px.
    pub blur: f32,
}

impl GlassGrade {
    /// A glass grade.
    pub const fn new(name: &'static str, alpha: Themed, blur: f32) -> Self {
        GlassGrade { name, alpha, blur }
    }
}

/// One layer of a shadow; offsets, blur and spread in px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowLayer {
    /// Horizontal offset.
    pub x: f32,
    /// Vertical offset.
    pub y: f32,
    /// Blur radius.
    pub blur: f32,
    /// Spread (negative shrinks the shadow).
    pub spread: f32,
    /// Name of the theme colour the layer is drawn in.
    pub color: &'static str,
}

impl ShadowLayer {
    /// A shadow layer.
    pub const fn new(x: f32, y: f32, blur: f32, spread: f32, color: &'static str) -> Self {
        ShadowLayer {
            x,
            y,
            blur,
            spread,
            color,
        }
    }
}

/// A shadow, which has different geometry in light and dark appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Layers in light appearance, bottom first.
    pub light: &'static [ShadowLayer],
    /// Layers in dark appearance, bottom first.
    pub dark: &'static [ShadowLayer],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_matches_the_token_file() {
        assert_eq!(accent::ACCENT.name, "accent.accent");
        assert_eq!(accent::ACCENT.light, Lcha::relative(0.46, 0.15, 0.0, 1.0));
        assert_eq!(accent::ACCENT.dark, Lcha::relative(0.78, 0.12, 0.0, 1.0));
        assert_eq!(theme_color("accent.accent"), Some(&accent::ACCENT));
    }

    #[test]
    fn colours_keep_their_hue_rule_and_alpha() {
        assert_eq!(ink::INK_90.light, Lcha::relative(0.205, 0.02, 0.0, 0.92));
        assert_eq!(ink::INK_50.dark.alpha, 0.55);
        assert_eq!(field::AURORA_3.light.h, -70.0);
        assert_eq!(semantic::WARNING.dark, Lcha::fixed(0.84, 0.12, 80.0, 1.0));
        assert_eq!(line::LINE.dark, Lcha::fixed(1.0, 0.0, 0.0, 0.1));
        assert_eq!(glass::GLASS_EDGE.light.alpha, 0.55);
        assert_eq!(DEFAULT_HUE, 250.0);
    }

    #[test]
    fn static_metrics_match_the_token_file() {
        assert_eq!(space::S1, 4.0);
        assert_eq!(space::S12, 96.0);
        assert_eq!(radius::R_SM, 6.0);
        assert_eq!(radius::R_FULL, 999.0);
        assert_eq!(type_::TEXT_H1, 48.0);
        assert_eq!(type_::TEXT_DISPLAY_FLUID, Fluid::new(52.0, 92.0, Some(6.6)));
        assert_eq!(motion::DUR_0_MS, 80);
        assert_eq!(motion::DUR_4_MS, 600);
        assert_eq!(motion::EASE_FLUID, [0.22, 1.0, 0.36, 1.0]);
        assert_eq!(motion::EASE_SPRING, [0.34, 1.36, 0.64, 1.0]);
        assert_eq!(layout::NAV_H, 72.0);
        assert_eq!(field::GRAIN, Themed::new(0.045, 0.065));
        assert_eq!(type_roles::LIST_TITLE, Typography::new(14.0, 1.3, 500));
        assert_eq!(lyrics::LINE_FLUID, Fluid::new(28.0, 40.0, None));
        assert_eq!(lyrics::ANCHOR_POSITION, 0.38);
        assert_eq!(backdrop::L_BAND_DARK, [0.16, 0.34]);
        assert_eq!(backdrop::L_BAND_LIGHT, [0.86, 0.95]);
        assert_eq!(backdrop::BLUR_SIGMA_RATIO, 0.085);
        assert_eq!(edge_blur::TOP_HEIGHT, 56.0);
        assert_eq!(glass_budget::LIVE_LAYERS_DESKTOP, 3.0);
        assert_eq!(player::ROW_HEIGHT, 52.0);
        assert_eq!(brand::HUE_FALLBACK, 250.0);
        assert_eq!(brand::HUE_MIN_CHROMA, 0.04);
        assert_eq!(brand::ICON_DEFAULT_LIGHT, "ice");
        assert_eq!(brand::ICON_DEFAULT_DARK, "steel");
    }

    #[test]
    fn glass_grades_pair_alpha_and_blur() {
        assert_eq!(glass::GRADES, &[glass::CLEAR, glass::MIST, glass::VEIL]);
        assert_eq!(
            glass::CLEAR,
            GlassGrade::new("clear", Themed::new(0.3, 0.36), 24.0)
        );
        assert_eq!(glass::MIST, GlassGrade::new("mist", Themed::new(0.42, 0.5), 26.0));
        assert_eq!(
            glass::VEIL,
            GlassGrade::new("veil", Themed::new(0.62, 0.68), 32.0)
        );
        assert_eq!(glass::TINT_VEIL.light, Lcha::relative(0.995, 0.004, 0.0, 0.62));
        assert_eq!(glass::TINT_VEIL.dark, Lcha::relative(0.215, 0.02, 0.0, 0.68));
    }

    #[test]
    fn theme_colours_are_unique_and_shadows_resolve() {
        let mut names: Vec<&str> = THEME_COLORS.iter().map(|def| def.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate theme colour names");
        for name in [
            "field.sky",
            "ink.ink-90",
            "line.fill",
            "glass.cloud",
            "elevation.scrim",
            "glass.tint-mist",
        ] {
            assert!(theme_color(name).is_some(), "{name} missing");
        }
        for shadow in [
            elevation::SHADOW_1,
            elevation::SHADOW_2,
            elevation::SHADOW_3,
            elevation::FOCUS,
        ] {
            for layer in shadow.light.iter().chain(shadow.dark) {
                assert!(theme_color(layer.color).is_some(), "{} missing", layer.color);
            }
        }
        assert_eq!(elevation::FOCUS.light[1].color, "accent.accent-text");
        assert_eq!(
            elevation::SHADOW_1.dark[1],
            ShadowLayer::new(0.0, 12.0, 30.0, -16.0, "elevation.shadow-1.1")
        );
    }
}

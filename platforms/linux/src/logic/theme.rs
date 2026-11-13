//! Run-time theme CSS: the core's Misty colours (resolved from the artwork
//! hue, sent as sRGB) become GTK named colours, and libadwaita's own named
//! colours are pointed at them so native widgets match.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::logic::tokens_css::color_name;

/// Colour name (`ink.ink-90`) to RGBA, as in the core's `Theme`.
pub type Colors = BTreeMap<String, [u8; 4]>;

/// A colour for GTK CSS.
pub fn css_color([r, g, b, a]: [u8; 4]) -> String {
    if a == 255 {
        format!("rgb({r},{g},{b})")
    } else {
        format!(
            "rgba({r},{g},{b},{})",
            crate::logic::tokens_css::num(f64::from(a) / 255.0)
        )
    }
}

/// `a` blended towards `b` by `t` (0 to 1), per channel.
pub fn mix(a: [u8; 4], b: [u8; 4], t: f64) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    let mut out = [0u8; 4];
    for i in 0..4 {
        out[i] = (f64::from(a[i]) + (f64::from(b[i]) - f64::from(a[i])) * t).round() as u8;
    }
    out
}

/// Every colour of `to`, blended from the same colour in `from` (a hue
/// change crossfades, architecture §6).
pub fn mix_maps(from: &Colors, to: &Colors, t: f64) -> Colors {
    to.iter()
        .map(|(name, &target)| {
            let start = from.get(name).copied().unwrap_or(target);
            (name.clone(), mix(start, target, t))
        })
        .collect()
}

/// libadwaita 1.5 named colours and the Misty colour each one takes.
pub const ADWAITA: &[(&str, &str)] = &[
    ("accent_color", "accent.accent-text"),
    ("accent_bg_color", "accent.accent"),
    ("accent_fg_color", "accent.on-accent"),
    ("destructive_color", "semantic.danger"),
    ("destructive_bg_color", "semantic.danger"),
    ("destructive_fg_color", "accent.on-accent"),
    ("success_color", "semantic.success"),
    ("success_bg_color", "semantic.success"),
    ("success_fg_color", "accent.on-accent"),
    ("warning_color", "semantic.warning"),
    ("warning_bg_color", "semantic.warning"),
    ("warning_fg_color", "accent.on-accent"),
    ("error_color", "semantic.danger"),
    ("error_bg_color", "semantic.danger"),
    ("error_fg_color", "accent.on-accent"),
    ("window_bg_color", "field.sky"),
    ("window_fg_color", "ink.ink-90"),
    ("view_bg_color", "field.sky-high"),
    ("view_fg_color", "ink.ink-90"),
    ("headerbar_bg_color", "field.sky"),
    ("headerbar_fg_color", "ink.ink-90"),
    ("headerbar_border_color", "ink.ink-90"),
    ("headerbar_backdrop_color", "field.sky"),
    ("headerbar_shade_color", "line.line-soft"),
    ("headerbar_darker_shade_color", "line.line"),
    ("sidebar_bg_color", "glass.cloud-soft"),
    ("sidebar_fg_color", "ink.ink-90"),
    ("sidebar_backdrop_color", "glass.cloud-soft"),
    ("sidebar_shade_color", "line.line-soft"),
    ("secondary_sidebar_bg_color", "glass.cloud"),
    ("secondary_sidebar_fg_color", "ink.ink-90"),
    ("secondary_sidebar_backdrop_color", "glass.cloud"),
    ("secondary_sidebar_shade_color", "line.line-soft"),
    ("card_bg_color", "glass.cloud"),
    ("card_fg_color", "ink.ink-90"),
    ("card_shade_color", "line.line-soft"),
    ("dialog_bg_color", "glass.cloud"),
    ("dialog_fg_color", "ink.ink-90"),
    ("popover_bg_color", "glass.cloud"),
    ("popover_fg_color", "ink.ink-90"),
    ("popover_shade_color", "line.line-soft"),
    ("thumbnail_bg_color", "glass.cloud"),
    ("thumbnail_fg_color", "ink.ink-90"),
    ("shade_color", "line.line-soft"),
    ("scrollbar_outline_color", "glass.glass-edge-soft"),
];

/// The theme stylesheet: one `@define-color jf_*` per Misty colour, then
/// libadwaita's colours pointed at them.
pub fn theme_css(colors: &Colors) -> String {
    let mut css = String::with_capacity(colors.len() * 48 + ADWAITA.len() * 48);
    for (name, &rgba) in colors {
        let _ = writeln!(css, "@define-color {} {};", color_name(name), css_color(rgba));
    }
    for (adw, misty) in ADWAITA {
        if colors.contains_key(*misty) {
            let _ = writeln!(css, "@define-color {adw} @{};", color_name(misty));
        }
    }
    css
}

/// How glass surfaces are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glass {
    /// Tinted backdrop blur (CSS `backdrop-filter`, GTK 4.22 and later).
    Live,
    /// Solid Cloud (Misty MAT-08): older GTK, high contrast, reduced effects.
    Solid,
}

/// Live glass needs GTK 4.22 and is off under high contrast, reduced
/// effects, or with animations off (ADR-0006).
pub fn glass(gtk_minor: u32, high_contrast: bool, reduce_effects: bool, animations: bool) -> Glass {
    if gtk_minor >= 22 && !high_contrast && !reduce_effects && animations {
        Glass::Live
    } else {
        Glass::Solid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors() -> Colors {
        let mut c = Colors::new();
        c.insert("ink.ink-90".into(), [16, 24, 32, 235]);
        c.insert("field.sky".into(), [238, 243, 249, 255]);
        c.insert("accent.accent".into(), [0, 89, 167, 255]);
        c
    }

    #[test]
    fn css_colours() {
        assert_eq!(css_color([1, 2, 3, 255]), "rgb(1,2,3)");
        assert_eq!(css_color([16, 24, 32, 235]), "rgba(16,24,32,0.9216)");
        assert_eq!(css_color([0, 0, 0, 0]), "rgba(0,0,0,0)");
    }

    #[test]
    fn mixes() {
        assert_eq!(mix([0, 0, 0, 0], [200, 100, 50, 255], 0.5), [100, 50, 25, 128]);
        assert_eq!(mix([9, 9, 9, 9], [1, 1, 1, 1], 7.0), [1, 1, 1, 1]);
        let mut to = colors();
        to.insert("new.colour".into(), [5, 5, 5, 5]);
        let m = mix_maps(&colors(), &to, 0.3);
        assert_eq!(m["ink.ink-90"], [16, 24, 32, 235]);
        assert_eq!(m["new.colour"], [5, 5, 5, 5], "no start colour: shown at once");
    }

    #[test]
    fn stylesheet_defines_misty_then_adwaita_colours() {
        let css = theme_css(&colors());
        assert!(css.contains("@define-color jf_ink_90 rgba(16,24,32,0.9216);"));
        assert!(css.contains("@define-color jf_sky rgb(238,243,249);"));
        assert!(css.contains("@define-color window_bg_color @jf_sky;"));
        assert!(css.contains("@define-color accent_bg_color @jf_accent;"));
        // Colours the theme lacks are not referenced.
        assert!(!css.contains("card_bg_color"));
    }

    #[test]
    fn every_adwaita_colour_points_at_a_core_colour() {
        let theme = jf_core::theme::theme_for_artwork(None);
        for (adw, misty) in ADWAITA {
            assert!(theme.light.colors.contains_key(*misty), "{adw} -> {misty}");
        }
    }

    #[test]
    fn glass_needs_new_gtk_and_no_reduction() {
        assert_eq!(glass(22, false, false, true), Glass::Live);
        assert_eq!(glass(24, false, false, true), Glass::Live);
        assert_eq!(glass(14, false, false, true), Glass::Solid);
        assert_eq!(glass(24, true, false, true), Glass::Solid);
        assert_eq!(glass(24, false, true, true), Glass::Solid);
        assert_eq!(glass(24, false, false, false), Glass::Solid);
    }
}

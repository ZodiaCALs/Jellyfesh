//! Misty colours at any hue, for what Appearance shows before the core
//! resolves a choice: the accent swatches, the custom hue slider's track and
//! the previews. A port of jf-palette's OKLCH to sRGB step (the core does
//! not expose it); the contract test below checks it against the core's own
//! theme, colour by colour. The theme the app uses always comes from the
//! core's theme slice.

use jf_tokens::{Lcha, theme_color};

const OKLAB_TO_LMS: [[f64; 3]; 3] = [
    [1.0, 0.396_337_777_376_174_9, 0.215_803_757_309_913_6],
    [1.0, -0.105_561_345_815_658_6, -0.063_854_172_825_813_3],
    [1.0, -0.089_484_177_529_811_9, -1.291_485_548_019_409_2],
];
const LMS_TO_XYZ: [[f64; 3]; 3] = [
    [
        1.226_879_875_845_924_3,
        -0.557_814_994_460_217_1,
        0.281_391_045_665_964_7,
    ],
    [
        -0.040_575_745_214_800_8,
        1.112_286_803_280_317,
        -0.071_711_058_065_516_4,
    ],
    [
        -0.076_372_936_674_660_1,
        -0.421_493_332_402_243_2,
        1.586_924_019_836_781_6,
    ],
];
const XYZ_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
    [12831.0 / 3959.0, -329.0 / 214.0, -1974.0 / 3959.0],
    [
        -851_781.0 / 878_810.0,
        1_648_619.0 / 878_810.0,
        36519.0 / 878_810.0,
    ],
    [705.0 / 12673.0, -2585.0 / 12673.0, 705.0 / 667.0],
];

fn mul(m: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn encode_srgb(v: f64) -> f64 {
    let a = v.abs();
    let encoded = if a > 0.003_130_8 {
        1.055 * a.powf(1.0 / 2.4) - 0.055
    } else {
        12.92 * a
    };
    encoded.copysign(v)
}

fn quantize(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8
}

/// OKLCH to 8-bit sRGB, clipped to the gamut.
pub fn oklch_to_rgb8(l: f64, c: f64, h: f64) -> [u8; 3] {
    let h = h.to_radians();
    let lms = mul(&OKLAB_TO_LMS, [l, c * h.cos(), c * h.sin()]).map(|v| v * v * v);
    mul(&XYZ_TO_LINEAR_SRGB, mul(&LMS_TO_XYZ, lms)).map(|v| quantize(encode_srgb(v).clamp(0.0, 1.0)))
}

/// One appearance of a theme colour at theme hue `hue`, as RGBA.
pub fn resolve(lcha: &Lcha, hue: f32) -> [u8; 4] {
    let [r, g, b] = oklch_to_rgb8(f64::from(lcha.l), f64::from(lcha.c), f64::from(lcha.hue_at(hue)));
    [r, g, b, quantize(f64::from(lcha.alpha))]
}

/// The theme colour `name` (`accent.accent`) at `hue`, light or dark.
pub fn color_at(name: &str, hue: f32, dark: bool) -> Option<[u8; 4]> {
    theme_color(name).map(|def| resolve(&def.appearance(dark), hue))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_cores_theme_at_the_fallback_hue() {
        let theme = jf_core::theme::theme_for_artwork(None);
        let hue = theme.light.hue;
        for def in jf_tokens::THEME_COLORS {
            for (dark, resolved) in [(false, &theme.light), (true, &theme.dark)] {
                let ours = resolve(&def.appearance(dark), hue);
                let core = resolved.colors[def.name];
                for i in 0..4 {
                    assert!(
                        ours[i].abs_diff(core[i]) <= 1,
                        "{} (dark {dark}): {ours:?} vs the core's {core:?}",
                        def.name
                    );
                }
            }
        }
    }

    #[test]
    fn accents_differ_by_hue_and_appearance() {
        let blue = color_at("accent.accent", 250.0, false).unwrap();
        let red = color_at("accent.accent", 0.0, false).unwrap();
        assert_ne!(blue, red);
        assert!(blue[2] > blue[0], "hue 250 is blue: {blue:?}");
        assert!(red[0] > red[2], "hue 0 is red: {red:?}");
        assert_ne!(color_at("accent.accent", 250.0, true), Some(blue));
        assert_eq!(color_at("nope", 250.0, false), None);
    }
}

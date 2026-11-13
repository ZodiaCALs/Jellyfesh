//! OKLCH and sRGB conversion (CSS Color 4), WCAG 2.x contrast and alpha compositing.
//!
//! The conversion matches the Misty packages: the CSS Color 4 OKLab matrices in `f64`, sRGB
//! clipped per channel to 0..1, and 8-bit rounding as `floor(v * 255 + 0.5)`.

use serde::Serialize;

/// A colour in OKLCH: OKLab lightness (0 to 1), chroma and hue in degrees (0 to 360).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Oklch {
    /// Lightness, 0 to 1.
    pub l: f32,
    /// Chroma, 0 for greys; about 0.37 at most inside sRGB.
    pub c: f32,
    /// Hue in degrees, 0 to 360 (0 for greys).
    pub h: f32,
}

// CSS Color 4, section 17 sample code.
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
const LINEAR_SRGB_TO_XYZ: [[f64; 3]; 3] = [
    [506_752.0 / 1_228_815.0, 87881.0 / 245_763.0, 12673.0 / 70218.0],
    [87098.0 / 409_605.0, 175_762.0 / 245_763.0, 12673.0 / 175_545.0],
    [7918.0 / 409_605.0, 87881.0 / 737_289.0, 1_001_167.0 / 1_053_270.0],
];
const XYZ_TO_LMS: [[f64; 3]; 3] = [
    [
        0.819_022_437_996_703,
        0.361_906_260_052_890_4,
        -0.128_873_781_520_987_9,
    ],
    [
        0.032_983_653_932_388_5,
        0.929_286_861_586_343_4,
        0.036_144_666_350_642_4,
    ],
    [
        0.048_177_189_359_624_2,
        0.264_239_531_752_730_8,
        0.633_547_828_469_430_9,
    ],
];
const LMS_TO_OKLAB: [[f64; 3]; 3] = [
    [
        0.210_454_268_309_314,
        0.793_617_774_702_305_4,
        -0.004_072_043_011_619_3,
    ],
    [
        1.977_998_532_431_168_4,
        -2.428_592_242_048_58,
        0.450_593_709_617_411,
    ],
    [
        0.025_904_042_465_547_8,
        0.782_771_712_457_529_6,
        -0.808_675_754_923_077_4,
    ],
];

fn mul(m: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// OKLab to linear-light sRGB (not clipped).
pub(crate) fn oklab_to_linear_srgb(lab: [f64; 3]) -> [f64; 3] {
    let lms = mul(&OKLAB_TO_LMS, lab).map(|v| v * v * v);
    mul(&XYZ_TO_LINEAR_SRGB, mul(&LMS_TO_XYZ, lms))
}

/// Linear-light sRGB to OKLab.
pub(crate) fn linear_srgb_to_oklab(rgb: [f64; 3]) -> [f64; 3] {
    let lms = mul(&XYZ_TO_LMS, mul(&LINEAR_SRGB_TO_XYZ, rgb)).map(f64::cbrt);
    mul(&LMS_TO_OKLAB, lms)
}

/// The sRGB transfer function (linear to gamma-encoded), extended to negative values.
pub(crate) fn encode_srgb(v: f64) -> f64 {
    let a = v.abs();
    let encoded = if a > 0.003_130_8 {
        1.055 * a.powf(1.0 / 2.4) - 0.055
    } else {
        12.92 * a
    };
    encoded.copysign(v)
}

/// The inverse sRGB transfer function (gamma-encoded to linear), extended to negative values.
pub(crate) fn decode_srgb(v: f64) -> f64 {
    let a = v.abs();
    let decoded = if a <= 0.040_45 {
        a / 12.92
    } else {
        ((a + 0.055) / 1.055).powf(2.4)
    };
    decoded.copysign(v)
}

/// Gamma-encoded sRGB, each channel clipped to 0..1.
pub(crate) fn oklch_to_srgb64(l: f64, c: f64, h: f64) -> [f64; 3] {
    let h = h.to_radians();
    oklab_to_linear_srgb([l, c * h.cos(), c * h.sin()]).map(|v| encode_srgb(v).clamp(0.0, 1.0))
}

/// One channel to 8 bits the way Misty rounds: `floor(v * 255 + 0.5)` after clipping to 0..1.
pub(crate) fn quantize(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8
}

/// A hue in degrees, normalised to `0 <= h < 360`.
pub(crate) fn normalize_hue(h: f32) -> f32 {
    let h = h.rem_euclid(360.0);
    if h >= 360.0 { 0.0 } else { h }
}

/// OKLCH to gamma-encoded sRGB (0..1 per channel), with the CSS Color 4 OKLab matrices and
/// each channel clipped to 0..1, as the Misty packages do.
pub fn oklch_to_srgb(l: f32, c: f32, h: f32) -> [f32; 3] {
    oklch_to_srgb64(l.into(), c.into(), h.into()).map(|v| v as f32)
}

/// Gamma-encoded sRGB (0..1 per channel) to OKLCH. Greys get hue 0.
pub fn srgb_to_oklch(rgb: [f32; 3]) -> Oklch {
    let [l, a, b] = linear_srgb_to_oklab(rgb.map(|v| decode_srgb(v.into())));
    let c = a.hypot(b);
    let h = if c < 1e-6 {
        0.0
    } else {
        normalize_hue(b.atan2(a).to_degrees() as f32)
    };
    Oklch {
        l: l as f32,
        c: c as f32,
        h,
    }
}

/// sRGB (0..1) and alpha to 8-bit RGBA, rounding half up like Misty: `floor(v * 255 + 0.5)`.
pub fn to_rgba8(rgb: [f32; 3], alpha: f32) -> [u8; 4] {
    let [r, g, b] = rgb.map(|v| quantize(v.into()));
    [r, g, b, quantize(alpha.into())]
}

/// WCAG 2.x relative luminance of a gamma-encoded sRGB colour (0..1 per channel).
pub fn relative_luminance(rgb: [f32; 3]) -> f32 {
    let linear = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = rgb.map(linear);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG 2.x contrast ratio between two sRGB colours, from 1 (same) to 21 (black on white).
pub fn contrast_ratio(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `fg` drawn over an opaque `bg` (alpha-over in gamma-encoded sRGB, as browsers and UI
/// toolkits composite).
pub fn composite(fg: [u8; 4], bg: [f32; 3]) -> [f32; 3] {
    let alpha = f32::from(fg[3]) / 255.0;
    [0, 1, 2].map(|i| f32::from(fg[i]) / 255.0 * alpha + bg[i] * (1.0 - alpha))
}

/// `#rrggbb`, or `#rrggbbaa` when the colour is not opaque.
pub fn hex(rgba: [u8; 4]) -> String {
    let [r, g, b, a] = rgba;
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn hex_of(l: f32, c: f32, h: f32) -> String {
        hex(to_rgba8(oklch_to_srgb(l, c, h), 1.0))
    }

    #[test]
    fn misty_reference_values_at_hue_250() {
        assert_eq!(hex_of(0.962, 0.01, 250.0), "#eef3f9"); // light sky
        assert_eq!(hex_of(0.46, 0.15, 250.0), "#0059a7"); // light accent
        assert_eq!(hex_of(0.78, 0.12, 250.0), "#7abdff"); // dark accent
        assert_eq!(hex_of(0.17, 0.015, 250.0), "#0a1016"); // dark sky
        assert_eq!(hex_of(0.98, 0.005, 250.0), "#f6f9fc"); // dark ink
        assert_eq!(hex_of(1.0, 0.0, 0.0), "#ffffff");
        assert_eq!(hex_of(0.0, 0.0, 0.0), "#000000");
    }

    #[test]
    fn rounding_is_half_up() {
        assert_eq!(to_rgba8([0.5 / 255.0, 1.0, 0.0], 0.92), [1, 255, 0, 235]);
        assert_eq!(to_rgba8([0.49 / 255.0, -0.2, 1.7], 0.0), [0, 0, 255, 0]);
    }

    #[test]
    fn srgb_to_oklch_matches_known_values() {
        let red = srgb_to_oklch([1.0, 0.0, 0.0]);
        assert!((red.l - 0.627_955).abs() < 1e-4, "{red:?}");
        assert!((red.c - 0.257_683).abs() < 1e-4, "{red:?}");
        assert!((red.h - 29.234).abs() < 1e-2, "{red:?}");
        let white = srgb_to_oklch([1.0, 1.0, 1.0]);
        assert!(
            (white.l - 1.0).abs() < 1e-6 && white.c < 1e-6 && white.h == 0.0,
            "{white:?}"
        );
    }

    #[test]
    fn contrast_of_black_and_white_is_21() {
        assert!((contrast_ratio([0.0; 3], [1.0; 3]) - 21.0).abs() < 1e-4);
        assert!((contrast_ratio([1.0; 3], [0.0; 3]) - 21.0).abs() < 1e-4);
        assert_eq!(contrast_ratio([0.3, 0.6, 0.9], [0.3, 0.6, 0.9]), 1.0);
        assert!((relative_luminance([1.0; 3]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn composite_is_alpha_over() {
        assert_eq!(composite([255, 0, 0, 0], [0.25, 0.5, 0.75]), [0.25, 0.5, 0.75]);
        assert_eq!(composite([255, 0, 51, 255], [0.25, 0.5, 0.75]), [1.0, 0.0, 0.2]);
        let half = composite([255, 255, 255, 51], [0.0; 3]);
        assert!(half.iter().all(|v| (v - 0.2).abs() < 1e-6), "{half:?}");
    }

    #[test]
    fn hex_includes_alpha_only_when_translucent() {
        assert_eq!(hex([0, 89, 167, 255]), "#0059a7");
        assert_eq!(hex([16, 24, 32, 235]), "#101820eb");
    }

    proptest! {
        #[test]
        fn srgb_round_trips_through_oklch(r in 0.0f32..=1.0, g in 0.0f32..=1.0, b in 0.0f32..=1.0) {
            let lch = srgb_to_oklch([r, g, b]);
            let back = oklch_to_srgb(lch.l, lch.c, lch.h);
            for (x, y) in [r, g, b].iter().zip(back) {
                prop_assert!((x - y).abs() < 1e-4, "{:?} -> {:?} -> {:?}", [r, g, b], lch, back);
            }
            prop_assert!((0.0..360.0).contains(&lch.h));
        }

        #[test]
        fn conversion_stays_in_gamut(l in 0.0f32..=1.0, c in 0.0f32..0.5, h in -720.0f32..720.0) {
            let rgb = oklch_to_srgb(l, c, h);
            prop_assert!(rgb.iter().all(|v| (0.0..=1.0).contains(v)), "{:?}", rgb);
        }

        #[test]
        fn contrast_is_symmetric_and_bounded(a in any::<[u8; 3]>(), b in any::<[u8; 3]>()) {
            let (a, b) = (a.map(|v| f32::from(v) / 255.0), b.map(|v| f32::from(v) / 255.0));
            let ratio = contrast_ratio(a, b);
            prop_assert_eq!(ratio, contrast_ratio(b, a));
            prop_assert!((1.0..=21.0001).contains(&ratio));
        }
    }
}

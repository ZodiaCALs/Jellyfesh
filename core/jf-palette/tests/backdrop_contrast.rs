//! Property: on every baked backdrop pixel, ink-90 of the theme for that appearance,
//! alpha-composited over the pixel, has at least 4.5:1 contrast against it.

use jf_palette::image::{DynamicImage, Rgb, RgbImage, Rgba, RgbaImage};
use jf_palette::{
    Appearance, BackdropParams, bake_backdrop, composite, contrast_ratio, extract_hue, resolve_theme,
    srgb_to_oklch,
};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Pattern {
    Noise(u32),
    TranslucentNoise(u32),
    Solid([u8; 3]),
    Gradient([u8; 3], [u8; 3], u8),
}

fn pattern() -> impl Strategy<Value = Pattern> {
    prop_oneof![
        any::<u32>().prop_map(Pattern::Noise),
        any::<u32>().prop_map(Pattern::TranslucentNoise),
        any::<[u8; 3]>().prop_map(Pattern::Solid),
        (any::<[u8; 3]>(), any::<[u8; 3]>(), 0u8..3).prop_map(|(a, b, d)| Pattern::Gradient(a, b, d)),
    ]
}

fn xorshift(state: &mut u32) -> [u8; 4] {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    state.to_le_bytes()
}

fn render(pattern: &Pattern, width: u32, height: u32) -> DynamicImage {
    match *pattern {
        Pattern::Noise(seed) => {
            let mut state = seed | 1;
            DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |_, _| {
                let [r, g, b, _] = xorshift(&mut state);
                Rgb([r, g, b])
            }))
        }
        Pattern::TranslucentNoise(seed) => {
            let mut state = seed | 1;
            DynamicImage::ImageRgba8(RgbaImage::from_fn(width, height, |_, _| {
                Rgba(xorshift(&mut state))
            }))
        }
        Pattern::Solid(rgb) => DynamicImage::ImageRgb8(RgbImage::from_pixel(width, height, Rgb(rgb))),
        Pattern::Gradient(from, to, direction) => {
            DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
                let t = match direction {
                    0 => x as f32 / (width.max(2) - 1) as f32,
                    1 => y as f32 / (height.max(2) - 1) as f32,
                    _ => (x + y) as f32 / (width + height).saturating_sub(2).max(1) as f32,
                };
                Rgb([0, 1, 2].map(|c| {
                    (f32::from(from[c]) + (f32::from(to[c]) - f32::from(from[c])) * t).round() as u8
                }))
            }))
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn ink_90_reads_on_every_backdrop_pixel(
        pattern in pattern(),
        width in 1u32..=120,
        height in 1u32..=120,
        hue in 0.0f32..360.0,
    ) {
        let img = render(&pattern, width, height);
        let artwork_hue = extract_hue(&img, 0.04, 250.0).hue;
        for appearance in Appearance::ALL {
            let params = BackdropParams::for_appearance(appearance);
            let out = bake_backdrop(&img, appearance, &params);
            prop_assert!(out.width() >= 1 && out.height() >= 1);
            let (lo, hi) = params.l_band;
            for theme_hue in [hue, artwork_hue] {
                let ink = resolve_theme(theme_hue, appearance).colors["ink.ink-90"];
                for px in out.pixels() {
                    prop_assert_eq!(px[3], 255);
                    let bg = [px[0], px[1], px[2]].map(|v| f32::from(v) / 255.0);
                    let ratio = contrast_ratio(composite(ink, bg), bg);
                    prop_assert!(ratio >= 4.5, "{:?} at hue {}: {:?} gives {}", appearance, theme_hue, px, ratio);
                    let l = srgb_to_oklch(bg).l;
                    prop_assert!(l >= lo - 0.02 && l <= hi + 0.02, "{:?}: L {} outside the band", appearance, l);
                }
            }
        }
    }

    /// The guarantee does not depend on the token band: with any band, saturation or blur the
    /// bake moves pixels the ink cannot read on until it can.
    #[test]
    fn contrast_holds_for_any_parameters(
        pattern in pattern(),
        width in 1u32..=64,
        height in 1u32..=64,
        band in (0.0f32..=1.0, 0.0f32..=1.0),
        saturation in 0.0f32..3.0,
        sigma_ratio in 0.0f32..0.3,
        hue in 0.0f32..360.0,
    ) {
        let img = render(&pattern, width, height);
        for appearance in Appearance::ALL {
            let params = BackdropParams { l_band: band, saturation, sigma_ratio, ..BackdropParams::for_appearance(appearance) };
            let ink = resolve_theme(hue, appearance).colors["ink.ink-90"];
            for px in bake_backdrop(&img, appearance, &params).pixels() {
                let bg = [px[0], px[1], px[2]].map(|v| f32::from(v) / 255.0);
                let ratio = contrast_ratio(composite(ink, bg), bg);
                prop_assert!(ratio >= 4.5, "{:?} {:?}: {:?} gives {}", appearance, params, px, ratio);
            }
        }
    }
}

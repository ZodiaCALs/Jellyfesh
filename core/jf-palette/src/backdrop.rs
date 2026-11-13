//! The Now Playing backdrop: artwork downscaled, blurred, tone-mapped into a contrast-safe
//! lightness band and dithered, baked once per artwork and appearance.

use image::{DynamicImage, Rgba, RgbaImage};
use jf_tokens::backdrop as tokens;

use crate::PaletteError;
use crate::color::{
    composite, contrast_ratio, encode_srgb, linear_srgb_to_oklab, oklab_to_linear_srgb, quantize,
};
use crate::scale::{downscale_rgba, scaled_size, srgb8_to_linear};
use crate::theme::{Appearance, resolve_color};

/// Long-edge cap after downsampling, in px. The token file states it only in the description
/// of `backdrop.downsample` ("long edge capped at 256 px"), so it lives here for now.
pub const BACKDROP_MAX_EDGE: u32 = 256;

/// Contrast that ink-90, composited over any backdrop pixel, keeps against that pixel (WCAG AA).
pub const BACKDROP_MIN_CONTRAST: f32 = 4.5;

/// Head-room over [`BACKDROP_MIN_CONTRAST`] for `f32` rounding in the check.
const CONTRAST_MARGIN: f32 = 1e-3;

/// How a backdrop is baked. [`BackdropParams::for_appearance`] reads the `backdrop.*` tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackdropParams {
    /// Scale applied to the artwork before blurring (at most 1).
    pub downsample: f32,
    /// Cap on the long edge after downsampling, in px.
    pub max_edge: u32,
    /// Gaussian sigma as a fraction of the short edge after downsampling.
    pub sigma_ratio: f32,
    /// OKLCH chroma multiplier applied after the blur.
    pub saturation: f32,
    /// OKLab lightness band the image's own lightness range is mapped into.
    pub l_band: (f32, f32),
    /// Ordered-dither amplitude in 8-bit steps (each pixel moves by at most ±`dither`/255).
    pub dither: f32,
}

impl BackdropParams {
    /// The token values for `appearance` (`backdrop.l-band-light` or `backdrop.l-band-dark`).
    pub fn for_appearance(appearance: Appearance) -> Self {
        let band = match appearance {
            Appearance::Light => tokens::L_BAND_LIGHT,
            Appearance::Dark => tokens::L_BAND_DARK,
        };
        BackdropParams {
            downsample: tokens::DOWNSAMPLE,
            max_edge: BACKDROP_MAX_EDGE,
            sigma_ratio: tokens::BLUR_SIGMA_RATIO,
            saturation: tokens::SATURATION,
            l_band: (band[0], band[1]),
            dither: tokens::DITHER,
        }
    }
}

/// Bake the backdrop for `appearance`: an opaque RGBA image.
///
/// 1. Downscale to `downsample` of the artwork, long edge at most `max_edge` (area average;
///    transparent artwork sits on black in dark appearance and white in light).
/// 2. Separable Gaussian blur in OKLab, sigma = `sigma_ratio` × the short edge.
/// 3. Multiply OKLCH chroma by `saturation`.
/// 4. Map the image's own OKLab lightness range linearly into `l_band`: compressed when wider
///    than the band, otherwise moved there unchanged (never stretched), so light and dark
///    areas keep their order.
/// 5. Reduce chroma where a colour falls outside sRGB (hue and lightness kept), add a ±`dither`
///    step ordered dither and round to 8 bits.
///
/// Every pixel is then checked: ink-90 for `appearance`, at any theme hue and alpha-composited
/// over the pixel, keeps at least [`BACKDROP_MIN_CONTRAST`] against it. With the token bands that
/// holds by a wide margin; for parameters where it does not, the pixel's lightness moves away
/// from the ink until it does, so the guarantee holds for any input.
pub fn bake_backdrop(img: &DynamicImage, appearance: Appearance, p: &BackdropParams) -> RgbaImage {
    let (lo, hi) = band(p.l_band);
    let ink = least_readable_ink(appearance);
    let Some((width, height)) = scaled_size(img.width(), img.height(), p.downsample, p.max_edge) else {
        // No pixels at all: one neutral pixel in the middle of the band.
        let [r, g, b] = safe_pixel([(lo + hi) / 2.0, 0.0, 0.0], 0.0, appearance, &ink);
        return RgbaImage::from_pixel(1, 1, Rgba([r, g, b, 255]));
    };
    let (w, h) = (width as usize, height as usize);

    let behind = if appearance.is_dark() { 0.0 } else { 1.0 };
    let linear = srgb8_to_linear();
    let mut planes = [vec![0.0f32; w * h], vec![0.0f32; w * h], vec![0.0f32; w * h]];
    for (i, px) in downscale_rgba(img, width, height).chunks_exact(4).enumerate() {
        let alpha = f64::from(px[3]) / 255.0;
        let rgb = [0, 1, 2].map(|c| linear[usize::from(px[c])] * alpha + behind * (1.0 - alpha));
        for (plane, v) in planes.iter_mut().zip(linear_srgb_to_oklab(rgb)) {
            plane[i] = v as f32;
        }
    }

    gaussian_blur(&mut planes, w, h, p.sigma_ratio * width.min(height) as f32);

    let saturation = if p.saturation.is_finite() {
        p.saturation.max(0.0)
    } else {
        1.0
    };
    let tone = ToneMap::new(&planes[0], lo, hi);
    let mut out = RgbaImage::new(width, height);
    for (i, px) in out.pixels_mut().enumerate() {
        let lab = [
            tone.apply(planes[0][i]),
            planes[1][i] * saturation,
            planes[2][i] * saturation,
        ];
        let [r, g, b] = safe_pixel(lab, dither_offset(i % w, i / w, p.dither), appearance, &ink);
        *px = Rgba([r, g, b, 255]);
    }
    out
}

/// Decode artwork (JPEG or PNG), bake it with the token parameters for `appearance` and encode
/// the result as an 8-bit RGB PNG (the backdrop is opaque).
pub fn bake_backdrop_png(bytes: &[u8], appearance: Appearance) -> Result<Vec<u8>, PaletteError> {
    let img = image::load_from_memory(bytes)?;
    let baked = bake_backdrop(&img, appearance, &BackdropParams::for_appearance(appearance));
    let rgb: Vec<u8> = baked.pixels().flat_map(|px| [px[0], px[1], px[2]]).collect();
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, baked.width(), baked.height());
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgb)?;
    writer.finish()?;
    Ok(png)
}

/// The band, ordered and clamped to 0..1.
fn band((a, b): (f32, f32)) -> (f32, f32) {
    let clean = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.5 };
    let (a, b) = (clean(a), clean(b));
    (a.min(b), a.max(b))
}

/// ink-90 at its least readable over every theme hue: per channel the value closest to the
/// backdrop (darkest in dark appearance, lightest in light), one 8-bit step further to cover hues
/// between the whole degrees sampled. Contrast against this ink bounds contrast against ink-90
/// at any hue, because the composite's luminance moves monotonically with each ink channel.
fn least_readable_ink(appearance: Appearance) -> [u8; 4] {
    let dark = appearance.is_dark();
    let mut ink = resolve_color(&jf_tokens::ink::INK_90, 0.0, appearance);
    for hue in 1..360 {
        let other = resolve_color(&jf_tokens::ink::INK_90, hue as f32, appearance);
        for c in 0..3 {
            ink[c] = if dark {
                ink[c].min(other[c])
            } else {
                ink[c].max(other[c])
            };
        }
    }
    for channel in &mut ink[..3] {
        *channel = if dark {
            channel.saturating_sub(1)
        } else {
            channel.saturating_add(1)
        };
    }
    ink
}

fn ink_reads(ink: &[u8; 4], px: [u8; 3]) -> bool {
    let bg = px.map(|v| f32::from(v) / 255.0);
    contrast_ratio(composite(*ink, bg), bg) >= BACKDROP_MIN_CONTRAST + CONTRAST_MARGIN
}

/// The 8-bit pixel for OKLab `lab` with `dither` added, moved away from the ink in lightness if
/// the ink would not read on it.
fn safe_pixel(lab: [f32; 3], dither: f64, appearance: Appearance, ink: &[u8; 4]) -> [u8; 3] {
    let [mut l, a, b] = lab.map(f64::from);
    let px = to_srgb8(l, a, b, dither);
    if ink_reads(ink, px) {
        return px;
    }
    let step = if appearance.is_dark() { -0.01 } else { 0.01 };
    for _ in 0..100 {
        l = (l + step).clamp(0.0, 1.0);
        let px = to_srgb8(l, a, b, 0.0);
        if ink_reads(ink, px) {
            return px;
        }
    }
    if appearance.is_dark() {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}

fn to_srgb8(l: f64, a: f64, b: f64, dither: f64) -> [u8; 3] {
    fit_gamut(l, a, b).map(|v| quantize(v + dither))
}

/// Gamma-encoded sRGB for OKLab (l, a, b), with chroma reduced until the colour fits the sRGB
/// gamut, so hue and lightness stay as asked.
fn fit_gamut(l: f64, a: f64, b: f64) -> [f64; 3] {
    let inside = |rgb: &[f64; 3]| rgb.iter().all(|v| (-1e-9..=1.0 + 1e-9).contains(v));
    let mut rgb = oklab_to_linear_srgb([l, a, b]);
    if !inside(&rgb) {
        let (mut fits, mut fails) = (0.0, 1.0);
        for _ in 0..20 {
            let mid = (fits + fails) / 2.0;
            if inside(&oklab_to_linear_srgb([l, a * mid, b * mid])) {
                fits = mid;
            } else {
                fails = mid;
            }
        }
        rgb = oklab_to_linear_srgb([l, a * fits, b * fits]);
    }
    rgb.map(|v| encode_srgb(v.clamp(0.0, 1.0)))
}

/// Linear map of the image's lightness range into the band.
#[derive(Debug, Clone, Copy)]
struct ToneMap {
    scale: f32,
    offset: f32,
    lo: f32,
    hi: f32,
}

impl ToneMap {
    fn new(lightness: &[f32], lo: f32, hi: f32) -> Self {
        let (min, max) = lightness
            .iter()
            .fold((1.0f32, 0.0f32), |(min, max), &l| (min.min(l), max.max(l)));
        let min = min.clamp(0.0, 1.0);
        let max = max.clamp(min, 1.0);
        let (range, width) = (max - min, hi - lo);
        let (scale, offset) = if range > width {
            let scale = width / range;
            (scale, lo - min * scale)
        } else {
            // Narrower than the band: keep the image's own contrast and put the range where it
            // sits within 0..1 (black artwork at the bottom of the band, white at the top).
            let position = if range < 1.0 {
                (min / (1.0 - range)).clamp(0.0, 1.0)
            } else {
                0.5
            };
            (1.0, lo + (width - range) * position - min)
        };
        ToneMap {
            scale,
            offset,
            lo,
            hi,
        }
    }

    fn apply(&self, l: f32) -> f32 {
        (l * self.scale + self.offset).clamp(self.lo, self.hi)
    }
}

/// Separable Gaussian blur of each plane, edges clamped.
fn gaussian_blur(planes: &mut [Vec<f32>; 3], w: usize, h: usize, sigma: f32) {
    if sigma.is_nan() || sigma < 0.3 {
        return;
    }
    let radius = (3.0 * sigma).ceil() as usize;
    let mut kernel: Vec<f32> = (0..=2 * radius)
        .map(|i| {
            let x = i as f32 - radius as f32;
            (-x * x / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let total: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= total);

    let mut rows = vec![0.0f32; w * h];
    for plane in planes.iter_mut() {
        for (src, dst) in plane.chunks_exact(w).zip(rows.chunks_exact_mut(w)) {
            for (x, out) in dst.iter_mut().enumerate() {
                *out = kernel
                    .iter()
                    .enumerate()
                    .map(|(j, k)| k * src[(x + j).saturating_sub(radius).min(w - 1)])
                    .sum();
            }
        }
        for y in 0..h {
            for x in 0..w {
                plane[y * w + x] = kernel
                    .iter()
                    .enumerate()
                    .map(|(j, k)| k * rows[(y + j).saturating_sub(radius).min(h - 1) * w + x])
                    .sum();
            }
        }
    }
}

/// 8 × 8 Bayer matrix.
const BAYER: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Ordered-dither offset for pixel (x, y), strictly within ±`amplitude`/255.
fn dither_offset(x: usize, y: usize, amplitude: f32) -> f64 {
    if !amplitude.is_finite() || amplitude <= 0.0 {
        return 0.0;
    }
    let threshold = (f64::from(BAYER[y % 8][x % 8]) + 0.5) / 64.0;
    (2.0 * threshold - 1.0) * f64::from(amplitude) / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::srgb_to_oklch;
    use image::{GenericImageView, ImageFormat, Rgb, RgbImage};

    fn lightness(px: &Rgba<u8>) -> f32 {
        srgb_to_oklch([px[0], px[1], px[2]].map(|v| f32::from(v) / 255.0)).l
    }

    fn noise(width: u32, height: u32, seed: u32) -> DynamicImage {
        let mut state = seed | 1;
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let [r, g, b, _] = state.to_le_bytes();
            Rgb([r, g, b])
        }))
    }

    #[test]
    fn params_come_from_tokens() {
        let dark = BackdropParams::for_appearance(Appearance::Dark);
        assert_eq!(dark.l_band, (0.16, 0.34));
        assert_eq!(
            (dark.downsample, dark.max_edge, dark.sigma_ratio),
            (0.25, 256, 0.085)
        );
        assert_eq!((dark.saturation, dark.dither), (1.2, 1.0));
        assert_eq!(
            BackdropParams::for_appearance(Appearance::Light).l_band,
            (0.86, 0.95)
        );
    }

    #[test]
    fn downsamples_and_caps_the_long_edge() {
        let img = noise(2000, 1000, 7);
        let out = bake_backdrop(
            &img,
            Appearance::Dark,
            &BackdropParams::for_appearance(Appearance::Dark),
        );
        assert_eq!(out.dimensions(), (256, 128));
        let small = noise(400, 300, 7);
        let out = bake_backdrop(
            &small,
            Appearance::Light,
            &BackdropParams::for_appearance(Appearance::Light),
        );
        assert_eq!(out.dimensions(), (100, 75));
    }

    #[test]
    fn output_is_opaque_and_in_band() {
        for appearance in Appearance::ALL {
            let p = BackdropParams::for_appearance(appearance);
            let out = bake_backdrop(&noise(160, 120, 3), appearance, &p);
            for px in out.pixels() {
                assert_eq!(px[3], 255);
                let l = lightness(px);
                assert!(
                    l >= p.l_band.0 - 0.02 && l <= p.l_band.1 + 0.02,
                    "{appearance:?}: L {l}"
                );
            }
        }
    }

    #[test]
    fn keeps_light_and_dark_in_order() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_fn(200, 100, |x, _| {
            if x < 100 {
                Rgb([20, 20, 30])
            } else {
                Rgb([235, 230, 220])
            }
        }));
        for appearance in Appearance::ALL {
            let out = bake_backdrop(&img, appearance, &BackdropParams::for_appearance(appearance));
            let (left, right) = (lightness(out.get_pixel(0, 10)), lightness(out.get_pixel(49, 10)));
            assert!(left + 0.02 < right, "{appearance:?}: {left} vs {right}");
        }
    }

    #[test]
    fn solid_colour_is_flat_apart_from_dither() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 64, Rgb([200, 40, 60])));
        let out = bake_backdrop(
            &img,
            Appearance::Dark,
            &BackdropParams::for_appearance(Appearance::Dark),
        );
        for c in 0..3 {
            let (min, max) = out
                .pixels()
                .fold((255, 0), |(lo, hi), px| (px[c].min(lo), px[c].max(hi)));
            assert!(max - min <= 2, "channel {c}: {min}..{max}");
        }
        let px = out.get_pixel(3, 3);
        assert!(px[0] > px[1] && px[0] > px[2], "a red artwork stays red: {px:?}");
    }

    #[test]
    fn bake_is_deterministic() {
        let img = noise(123, 77, 11);
        let p = BackdropParams::for_appearance(Appearance::Light);
        assert_eq!(
            bake_backdrop(&img, Appearance::Light, &p),
            bake_backdrop(&img, Appearance::Light, &p)
        );
    }

    #[test]
    fn contrast_holds_even_with_an_unsafe_band() {
        // A band where ink-90 cannot read: the guard must still move every pixel to 4.5:1.
        let p = BackdropParams {
            l_band: (0.55, 0.75),
            ..BackdropParams::for_appearance(Appearance::Dark)
        };
        let out = bake_backdrop(&noise(64, 64, 5), Appearance::Dark, &p);
        let ink = resolve_color(&jf_tokens::ink::INK_90, 250.0, Appearance::Dark);
        for px in out.pixels() {
            let bg = [px[0], px[1], px[2]].map(|v| f32::from(v) / 255.0);
            assert!(contrast_ratio(composite(ink, bg), bg) >= BACKDROP_MIN_CONTRAST);
        }
    }

    #[test]
    fn empty_artwork_gives_one_pixel() {
        let out = bake_backdrop(
            &DynamicImage::new_rgb8(0, 0),
            Appearance::Dark,
            &BackdropParams::for_appearance(Appearance::Dark),
        );
        assert_eq!(out.dimensions(), (1, 1));
    }

    #[test]
    fn png_round_trip() {
        let mut jpeg = std::io::Cursor::new(Vec::new());
        noise(96, 64, 9).write_to(&mut jpeg, ImageFormat::Jpeg).unwrap();
        let png = bake_backdrop_png(jpeg.get_ref(), Appearance::Light).unwrap();
        let decoded = image::load_from_memory_with_format(&png, ImageFormat::Png).unwrap();
        assert_eq!(decoded.dimensions(), (24, 16));
        assert!(matches!(
            bake_backdrop_png(b"nope", Appearance::Dark),
            Err(PaletteError::Decode(_))
        ));
    }

    #[test]
    fn dither_stays_within_amplitude() {
        for y in 0..8 {
            for x in 0..8 {
                assert!(dither_offset(x, y, 1.0).abs() < 1.0 / 255.0);
            }
        }
        assert_eq!(dither_offset(3, 3, 0.0), 0.0);
    }
}

//! Artwork hue: the hue of the most colourful, largest cluster of artwork colours in OKLab.

use image::DynamicImage;
use jf_tokens::brand;
use serde::Serialize;

use crate::PaletteError;
use crate::color::{linear_srgb_to_oklab, normalize_hue};
use crate::scale::{downscale_rgba, scaled_size, srgb8_to_linear};

/// Long edge of the image the clusters are computed on.
const ANALYSIS_EDGE: u32 = 64;
/// Number of clusters.
const CLUSTERS: usize = 5;
const MAX_ITERATIONS: usize = 32;
/// Added to each pixel's chroma when weighting, so grey artwork still clusters by area.
const CHROMA_FLOOR: f32 = 0.02;

/// The hue picked for an artwork.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct HuePick {
    /// Theme hue in degrees (0..360): the artwork's, or the fallback.
    pub hue: f32,
    /// OKLCH chroma of the winning cluster, reported also when it was too grey to use.
    pub chroma: f32,
    /// Whether `hue` comes from the artwork; `false` means the fallback hue.
    pub from_artwork: bool,
}

/// Find the theme hue of an artwork.
///
/// The image is area-averaged to at most 64 × 64 and its pixels clustered in OKLab with a
/// weighted k-means (k = 5, deterministic seeding), each pixel weighted by its area (alpha) and
/// chroma. The cluster with the highest chroma × area wins. When its chroma is below
/// `min_chroma` (nearly grey artwork), or the image has no opaque pixels, the result is
/// `fallback_hue` with `from_artwork: false`. The same image always gives the same pick.
pub fn extract_hue(img: &DynamicImage, min_chroma: f32, fallback_hue: f32) -> HuePick {
    let fallback = |chroma: f32| HuePick {
        hue: normalize_hue(fallback_hue),
        chroma,
        from_artwork: false,
    };
    let Some((width, height)) = scaled_size(img.width(), img.height(), 1.0, ANALYSIS_EDGE) else {
        return fallback(0.0);
    };
    let linear = srgb8_to_linear();
    let samples: Vec<Sample> = downscale_rgba(img, width, height)
        .chunks_exact(4)
        .filter(|px| px[3] > 0)
        .map(|px| {
            let lab = linear_srgb_to_oklab([0, 1, 2].map(|c| linear[usize::from(px[c])])).map(|v| v as f32);
            let area = f32::from(px[3]) / 255.0;
            Sample {
                lab,
                area,
                weight: area * (chroma(lab) + CHROMA_FLOOR),
            }
        })
        .collect();
    if samples.is_empty() {
        return fallback(0.0);
    }

    let total_area: f32 = samples.iter().map(|s| s.area).sum();
    let mut best: Option<(f32, Cluster)> = None;
    for cluster in weighted_kmeans(&samples, CLUSTERS) {
        let score = chroma(cluster.centroid) * cluster.area / total_area;
        if best.is_none_or(|(top, _)| score > top) {
            best = Some((score, cluster));
        }
    }
    let Some((_, cluster)) = best else {
        return fallback(0.0);
    };
    let picked = chroma(cluster.centroid);
    if picked < min_chroma {
        return fallback(picked);
    }
    let [_, a, b] = cluster.centroid;
    HuePick {
        hue: normalize_hue(b.atan2(a).to_degrees()),
        chroma: picked,
        from_artwork: true,
    }
}

/// Decode artwork (JPEG or PNG) and find its hue with the `brand.hue-min-chroma` and
/// `brand.hue-fallback` tokens.
pub fn extract_hue_from_bytes(bytes: &[u8]) -> Result<HuePick, PaletteError> {
    let img = image::load_from_memory(bytes)?;
    Ok(extract_hue(&img, brand::HUE_MIN_CHROMA, brand::HUE_FALLBACK))
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    lab: [f32; 3],
    area: f32,
    weight: f32,
}

#[derive(Debug, Clone, Copy)]
struct Cluster {
    centroid: [f32; 3],
    area: f32,
}

fn chroma(lab: [f32; 3]) -> f32 {
    lab[1].hypot(lab[2])
}

fn distance2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Index of the first largest value.
fn argmax(values: impl Iterator<Item = f32>) -> usize {
    let mut best = (0, f32::NEG_INFINITY);
    for (i, value) in values.enumerate() {
        if value > best.1 {
            best = (i, value);
        }
    }
    best.0
}

/// Index of the nearest centroid (the first on ties).
fn nearest(centroids: &[[f32; 3]], point: [f32; 3]) -> usize {
    argmax(centroids.iter().map(|c| -distance2(*c, point)))
}

/// Weighted k-means. Seeding is deterministic: the heaviest sample first, then repeatedly the
/// sample with the largest weight × squared distance to its nearest seed (a greedy k-means++).
/// Fewer than `k` clusters come back when the samples have fewer distinct colours.
fn weighted_kmeans(samples: &[Sample], k: usize) -> Vec<Cluster> {
    let mut centroids = vec![samples[argmax(samples.iter().map(|s| s.weight))].lab];
    let mut nearest_d2: Vec<f32> = samples.iter().map(|s| distance2(s.lab, centroids[0])).collect();
    while centroids.len() < k {
        let next = argmax(samples.iter().zip(&nearest_d2).map(|(s, d2)| s.weight * d2));
        if samples[next].weight * nearest_d2[next] <= 0.0 {
            break;
        }
        let seed = samples[next].lab;
        centroids.push(seed);
        for (d2, s) in nearest_d2.iter_mut().zip(samples) {
            *d2 = d2.min(distance2(s.lab, seed));
        }
    }

    let mut assignment = vec![usize::MAX; samples.len()];
    for _ in 0..MAX_ITERATIONS {
        let mut changed = false;
        for (slot, s) in assignment.iter_mut().zip(samples) {
            let cluster = nearest(&centroids, s.lab);
            changed |= *slot != cluster;
            *slot = cluster;
        }
        if !changed {
            break;
        }
        let mut sums = vec![[0.0f64; 4]; centroids.len()];
        for (s, &cluster) in samples.iter().zip(&assignment) {
            let weight = f64::from(s.weight);
            for (sum, v) in sums[cluster].iter_mut().zip(s.lab) {
                *sum += weight * f64::from(v);
            }
            sums[cluster][3] += weight;
        }
        for (centroid, sum) in centroids.iter_mut().zip(&sums) {
            if sum[3] > 0.0 {
                *centroid = [0, 1, 2].map(|i| (sum[i] / sum[3]) as f32);
            }
        }
    }

    let mut area = vec![0.0f32; centroids.len()];
    for (s, &cluster) in samples.iter().zip(&assignment) {
        area[cluster] += s.area;
    }
    centroids
        .into_iter()
        .zip(area)
        .map(|(centroid, area)| Cluster { centroid, area })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

    fn solid(rgb: [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 30, Rgb(rgb)))
    }

    fn pick(img: &DynamicImage) -> HuePick {
        extract_hue(img, brand::HUE_MIN_CHROMA, brand::HUE_FALLBACK)
    }

    #[test]
    fn solid_red_gives_a_red_hue() {
        let red = pick(&solid([255, 0, 0]));
        assert!(red.from_artwork);
        assert!((20.0..40.0).contains(&red.hue), "{red:?}");
        assert!((red.chroma - 0.2577).abs() < 0.005, "{red:?}");
    }

    #[test]
    fn grey_black_and_white_fall_back() {
        for rgb in [[128, 128, 128], [0, 0, 0], [255, 255, 255]] {
            let grey = pick(&solid(rgb));
            assert_eq!((grey.hue, grey.from_artwork), (250.0, false), "{rgb:?}: {grey:?}");
        }
        let custom = extract_hue(&solid([90, 90, 90]), 0.04, 400.0);
        assert_eq!(custom.hue, 40.0);
    }

    #[test]
    fn min_chroma_decides_between_artwork_and_fallback() {
        // A muted sand colour, chroma about 0.03.
        let sand = solid([176, 166, 150]);
        assert!(!extract_hue(&sand, 0.04, 250.0).from_artwork);
        let used = extract_hue(&sand, 0.02, 250.0);
        assert!(used.from_artwork && (40.0..100.0).contains(&used.hue), "{used:?}");
    }

    #[test]
    fn colour_beats_a_larger_grey_area() {
        // 70% grey, 30% blue: the grey cluster has no chroma, so blue wins.
        let img = RgbImage::from_fn(100, 60, |x, _| {
            if x < 70 {
                Rgb([120, 120, 120])
            } else {
                Rgb([30, 60, 220])
            }
        });
        let blue = pick(&DynamicImage::ImageRgb8(img));
        assert!(
            blue.from_artwork && (250.0..280.0).contains(&blue.hue),
            "{blue:?}"
        );
    }

    #[test]
    fn larger_area_wins_between_similar_chromas() {
        let img = RgbImage::from_fn(100, 60, |x, _| {
            if x < 70 {
                Rgb([20, 90, 230])
            } else {
                Rgb([230, 30, 30])
            }
        });
        let blue = pick(&DynamicImage::ImageRgb8(img));
        assert!((250.0..280.0).contains(&blue.hue), "{blue:?}");
    }

    #[test]
    fn transparent_pixels_do_not_count() {
        let img = RgbaImage::from_fn(50, 50, |x, _| {
            if x < 40 {
                Rgba([255, 0, 0, 0])
            } else {
                Rgba([0, 160, 60, 255])
            }
        });
        let green = pick(&DynamicImage::ImageRgba8(img));
        assert!(
            green.from_artwork && (130.0..160.0).contains(&green.hue),
            "{green:?}"
        );
        let clear = pick(&DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            9,
            9,
            Rgba([255, 0, 0, 0]),
        )));
        assert!(!clear.from_artwork);
    }

    #[test]
    fn same_image_same_pick() {
        let mut state = 0x9e37_79b9_u32;
        let img = RgbImage::from_fn(300, 200, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let [r, g, b, _] = state.to_le_bytes();
            Rgb([r, g, b])
        });
        let img = DynamicImage::ImageRgb8(img);
        assert_eq!(pick(&img), pick(&img));
    }

    #[test]
    fn empty_image_falls_back() {
        assert!(!pick(&DynamicImage::new_rgb8(0, 0)).from_artwork);
    }

    #[test]
    fn decodes_png_bytes() {
        let mut png = std::io::Cursor::new(Vec::new());
        solid([255, 0, 0]).write_to(&mut png, ImageFormat::Png).unwrap();
        let red = extract_hue_from_bytes(png.get_ref()).unwrap();
        assert!(red.from_artwork && (20.0..40.0).contains(&red.hue), "{red:?}");
        assert!(matches!(
            extract_hue_from_bytes(b"not an image"),
            Err(PaletteError::Decode(_))
        ));
    }
}

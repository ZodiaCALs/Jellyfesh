//! Artwork thumbnails: `<cache_dir>/artwork/<hash>/<size>.jpg`.
//!
//! The cached cover is decoded (JPEG or PNG, upright per its EXIF orientation),
//! flattened onto white if it has transparency, scaled with a Lanczos3 filter to
//! fit a square of one of [`SIZES`] (keeping its aspect ratio, never enlarged)
//! and encoded as a JPEG at [`QUALITY`]. Files are written atomically, so
//! concurrent requests for one thumbnail are safe, and reused once written.

use std::io::Cursor;
use std::path::Path;

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageReader, RgbImage};

use crate::util::write_atomic;

/// The thumbnail sizes, in pixels per side of the square the image fits in.
pub(crate) const SIZES: [u32; 5] = [64, 128, 256, 512, 1024];

/// JPEG quality of thumbnails.
const QUALITY: u8 = 85;

/// The smallest of [`SIZES`] at least `size_px`; the largest for anything above it.
pub(crate) fn bucket(size_px: u32) -> u32 {
    SIZES
        .into_iter()
        .find(|&size| size >= size_px)
        .unwrap_or(SIZES[SIZES.len() - 1])
}

/// Whether `hash` looks like a blake3 hex digest (and so is safe in a path).
pub(crate) fn is_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `width × height` fitted into a `box_px` square, keeping the aspect ratio and
/// never enlarged; each side at least 1 px.
pub(crate) fn fit(width: u32, height: u32, box_px: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= box_px {
        return (width.max(1), height.max(1));
    }
    let scale = |side: u32| {
        let scaled = (u64::from(side) * u64::from(box_px) + u64::from(long) / 2) / u64::from(long);
        u32::try_from(scaled).unwrap_or(box_px).clamp(1, box_px)
    };
    (scale(width), scale(height))
}

/// Decodes `source` and writes its thumbnail for a `box_px` square to `target`.
pub(crate) fn make(source: &[u8], box_px: u32, target: &Path) -> Result<(), String> {
    let image = decode(source)?;
    let rgb = flatten(&image);
    let (width, height) = fit(rgb.width(), rgb.height(), box_px);
    let rgb = if (width, height) == rgb.dimensions() {
        rgb
    } else {
        resize(rgb, width, height)?
    };
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, QUALITY)
        .encode(rgb.as_raw(), width, height, ExtendedColorType::Rgb8)
        .map_err(|err| err.to_string())?;
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    }
    write_atomic(target, &jpeg, false).map_err(|err| err.to_string())
}

/// Decodes with the image crate's default limits (512 MiB), upright.
fn decode(source: &[u8]) -> Result<DynamicImage, String> {
    let mut decoder = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|err| err.to_string())?
        .into_decoder()
        .map_err(|err| err.to_string())?;
    let orientation = decoder.orientation().map_err(|err| err.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|err| err.to_string())?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// RGB, with any transparency composited onto white.
fn flatten(image: &DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba
        .pixels()
        .flat_map(|pixel| {
            let [r, g, b, a] = pixel.0;
            let over_white =
                |c: u8| ((u32::from(c) * u32::from(a) + 255 * (255 - u32::from(a)) + 127) / 255) as u8;
            [over_white(r), over_white(g), over_white(b)]
        })
        .collect();
    RgbImage::from_raw(width, height, pixels).expect("three bytes per pixel")
}

fn resize(rgb: RgbImage, width: u32, height: u32) -> Result<RgbImage, String> {
    let (src_width, src_height) = rgb.dimensions();
    let src = Image::from_vec_u8(src_width, src_height, rgb.into_raw(), PixelType::U8x3)
        .map_err(|err| err.to_string())?;
    let mut dst = Image::new(width, height, PixelType::U8x3);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
    Resizer::new()
        .resize(&src, &mut dst, &options)
        .map_err(|err| err.to_string())?;
    RgbImage::from_raw(width, height, dst.into_vec())
        .ok_or_else(|| "resized buffer has the wrong size".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_snap_up_to_a_bucket() {
        assert_eq!(bucket(0), 64);
        assert_eq!(bucket(64), 64);
        assert_eq!(bucket(65), 128);
        assert_eq!(bucket(300), 512);
        assert_eq!(bucket(1024), 1024);
        assert_eq!(bucket(4096), 1024);
    }

    #[test]
    fn images_fit_the_box_and_are_never_enlarged() {
        assert_eq!(fit(1000, 500, 256), (256, 128));
        assert_eq!(fit(500, 1000, 256), (128, 256));
        assert_eq!(fit(100, 50, 256), (100, 50));
        assert_eq!(fit(3000, 1, 64), (64, 1));
        assert_eq!(fit(1001, 1000, 64), (64, 64));
    }

    #[test]
    fn hashes_are_checked() {
        assert!(is_hash(&"a1".repeat(32)));
        assert!(!is_hash(&"A1".repeat(32)));
        assert!(!is_hash("../../etc/passwd"));
        assert!(!is_hash(&"a".repeat(63)));
    }

    #[test]
    fn transparency_is_flattened_onto_white() {
        let rgba = image::RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                image::Rgba([0, 0, 0, 0])
            } else {
                image::Rgba([10, 20, 30, 255])
            }
        });
        let rgb = flatten(&DynamicImage::ImageRgba8(rgba));
        assert_eq!(rgb.as_raw(), &[255, 255, 255, 10, 20, 30]);
    }

    #[test]
    fn webp_and_gif_covers_make_thumbnails() {
        let picture = image::RgbImage::from_fn(40, 20, |x, _| image::Rgb([200, (x * 6) as u8, 30]));
        let dir = tempfile::tempdir().unwrap();
        for format in [image::ImageFormat::WebP, image::ImageFormat::Gif] {
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(picture.clone())
                .write_to(&mut bytes, format)
                .unwrap();
            let target = dir.path().join(format!("{format:?}.jpg"));
            make(bytes.get_ref(), 64, &target).unwrap_or_else(|e| panic!("{format:?}: {e}"));
            let made = image::open(&target).unwrap();
            assert_eq!((made.width(), made.height()), (40, 20), "{format:?}: never enlarged");
        }
    }
}

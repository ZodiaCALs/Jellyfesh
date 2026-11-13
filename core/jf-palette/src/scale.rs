//! Area-averaged downscaling (fast_image_resize) shared by hue analysis and the backdrop bake.

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::DynamicImage;

/// `width × height` scaled by `scale` (at most 1, so never enlarged), then capped at `max_edge`
/// on the long edge, keeping the aspect ratio. Each side is at least 1 px. `None` for an empty
/// image.
pub(crate) fn scaled_size(width: u32, height: u32, scale: f32, max_edge: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let mut factor = if scale.is_finite() && scale > 0.0 {
        f64::from(scale).min(1.0)
    } else {
        1.0
    };
    let long = f64::from(width.max(height));
    let cap = f64::from(max_edge.max(1));
    if long * factor > cap {
        factor = cap / long;
    }
    let fit = |side: u32| ((f64::from(side) * factor).round() as u32).clamp(1, side);
    Some((fit(width), fit(height)))
}

/// The image as straight-alpha RGBA8, area-averaged down to `width × height`.
///
/// Averaging is alpha-weighted, so transparent pixels contribute no colour. The caller passes a
/// size from [`scaled_size`], which is never larger than the image.
pub(crate) fn downscale_rgba(img: &DynamicImage, width: u32, height: u32) -> Vec<u8> {
    let rgba = img.to_rgba8();
    let (src_width, src_height) = rgba.dimensions();
    if (src_width, src_height) == (width, height) {
        return rgba.into_raw();
    }
    let src = Image::from_vec_u8(src_width, src_height, rgba.into_raw(), PixelType::U8x4)
        .expect("an RGBA8 buffer always matches its own size");
    let mut dst = Image::new(width, height, PixelType::U8x4);
    let options = ResizeOptions::new()
        .resize_alg(ResizeAlg::Convolution(FilterType::Box))
        .use_alpha(true);
    Resizer::new()
        .resize(&src, &mut dst, &options)
        .expect("source and destination are both RGBA8");
    dst.into_vec()
}

/// Linear-light value of each 8-bit sRGB code.
pub(crate) fn srgb8_to_linear() -> [f64; 256] {
    std::array::from_fn(|code| crate::color::decode_srgb(code as f64 / 255.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn sizes_keep_aspect_and_never_enlarge() {
        assert_eq!(scaled_size(2000, 1000, 0.25, 256), Some((256, 128)));
        assert_eq!(scaled_size(400, 300, 0.25, 256), Some((100, 75)));
        assert_eq!(scaled_size(3, 1, 0.25, 256), Some((1, 1)));
        assert_eq!(scaled_size(30, 20, 2.0, 64), Some((30, 20)));
        assert_eq!(scaled_size(500, 100, 1.0, 64), Some((64, 13)));
        assert_eq!(scaled_size(0, 10, 0.25, 256), None);
    }

    #[test]
    fn downscale_ignores_transparent_colour() {
        let img = RgbaImage::from_fn(8, 8, |x, _| {
            if x < 4 {
                Rgba([255, 0, 0, 0])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let small = downscale_rgba(&DynamicImage::ImageRgba8(img), 1, 1);
        assert_eq!(&small[..3], &[0, 0, 255]);
        assert!((120..=135).contains(&small[3]), "{small:?}");
    }
}

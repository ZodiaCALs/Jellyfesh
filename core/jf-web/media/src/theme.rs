//! Theme colours and the baked Now Playing backdrops, from artwork pixels.
//!
//! The browser decodes the artwork (its own JPEG and PNG decoders, off the
//! main thread with `createImageBitmap`) and hands over RGBA pixels; the
//! hue, the Misty themes and the backdrops are `jf-palette`'s, exactly as
//! the apps compute them from the same pixels (ADR-0013).

use jf_palette::image::{DynamicImage, RgbaImage};
use jf_palette::{Appearance, BackdropParams, HuePick, Theme};
use serde::Serialize;

/// The themes of one hue.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Themes {
    /// Theme hue in degrees.
    pub hue: f32,
    /// False when the hue is the fallback (no artwork, or nearly grey).
    pub from_artwork: bool,
    pub light: Theme,
    pub dark: Theme,
}

/// The themes for a fixed hue (no artwork, or a fixed-hue setting).
pub fn for_hue(hue: f32) -> Themes {
    Themes {
        hue,
        from_artwork: false,
        light: jf_palette::resolve_theme(hue, Appearance::Light),
        dark: jf_palette::resolve_theme(hue, Appearance::Dark),
    }
}

/// The artwork as an image, if the pixels fit the size.
pub fn image(width: u32, height: u32, rgba: Vec<u8>) -> Option<DynamicImage> {
    RgbaImage::from_raw(width, height, rgba).map(DynamicImage::ImageRgba8)
}

/// The hue of the artwork (the fallback hue when it is nearly grey) and its
/// themes.
pub fn for_artwork(img: &DynamicImage) -> Themes {
    let HuePick {
        hue, from_artwork, ..
    } = jf_palette::extract_hue(
        img,
        jf_tokens::brand::HUE_MIN_CHROMA,
        jf_tokens::brand::HUE_FALLBACK,
    );
    Themes {
        from_artwork,
        ..for_hue(hue)
    }
}

/// The parsed appearance name (`light` or `dark`).
pub fn appearance(name: &str) -> Option<Appearance> {
    match name {
        "light" => Some(Appearance::Light),
        "dark" => Some(Appearance::Dark),
        _ => None,
    }
}

/// The baked backdrop for `appearance`: opaque RGBA, at most
/// [`jf_palette::BACKDROP_MAX_EDGE`] on its long edge.
pub fn backdrop(img: &DynamicImage, appearance: Appearance) -> RgbaImage {
    jf_palette::bake_backdrop(img, appearance, &BackdropParams::for_appearance(appearance))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(width: u32, height: u32, a: [u8; 4], b: [u8; 4]) -> Vec<u8> {
        (0..width * height)
            .flat_map(|i| {
                if (i % width + i / width).is_multiple_of(2) {
                    a
                } else {
                    b
                }
            })
            .collect()
    }

    #[test]
    fn colourful_artwork_gives_its_hue_and_grey_the_fallback() {
        let red = image(32, 32, checker(32, 32, [200, 30, 30, 255], [180, 20, 40, 255])).unwrap();
        let themes = for_artwork(&red);
        assert!(themes.from_artwork);
        assert!(themes.hue < 40.0 || themes.hue > 340.0, "{}", themes.hue);
        assert!(themes.light.color("accent.accent").is_some());
        let grey = image(8, 8, checker(8, 8, [120, 120, 120, 255], [130, 130, 130, 255])).unwrap();
        let themes = for_artwork(&grey);
        assert!(!themes.from_artwork);
        assert_eq!(themes.hue, jf_tokens::brand::HUE_FALLBACK);
        assert!(image(2, 2, vec![0; 3]).is_none(), "wrong pixel count");
    }

    #[test]
    fn backdrops_are_small_and_opaque() {
        let img = image(
            1024,
            512,
            checker(1024, 512, [10, 80, 200, 255], [240, 200, 20, 255]),
        )
        .unwrap();
        let baked = backdrop(&img, appearance("dark").unwrap());
        assert_eq!((baked.width(), baked.height()), (256, 128));
        assert!(baked.pixels().all(|p| p[3] == 255));
        assert!(appearance("dim").is_none());
    }
}

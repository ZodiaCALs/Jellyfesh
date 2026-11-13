//! Colour: OKLCH conversion, Misty theme resolution from one hue, artwork hue extraction and baked backdrops.
//!
//! * Every themed colour is defined in OKLCH from one hue ([`jf_tokens::THEME_COLORS`]).
//!   [`resolve_theme`] converts them with the CSS Color 4 OKLab matrices (per-channel clipping,
//!   Misty's rounding), so every platform shows the same sRGB.
//! * [`extract_hue`] finds that hue in the artwork, or falls back to `brand.hue-fallback` when
//!   the artwork is nearly grey.
//! * [`bake_backdrop`] turns the artwork into the blurred Now Playing backdrop, tone-mapped so
//!   ink-90 keeps 4.5:1 on every pixel.

mod backdrop;
mod color;
mod hue;
mod scale;
mod theme;

pub use backdrop::{
    BACKDROP_MAX_EDGE, BACKDROP_MIN_CONTRAST, BackdropParams, bake_backdrop, bake_backdrop_png,
};
pub use color::{
    Oklch, composite, contrast_ratio, hex, oklch_to_srgb, relative_luminance, srgb_to_oklch, to_rgba8,
};
pub use hue::{HuePick, extract_hue, extract_hue_from_bytes};
pub use theme::{Appearance, GLASS_TINTS, Theme, glass_alpha, resolve_color, resolve_theme};

/// The `image` crate this API takes and returns images from.
pub use image;

/// Errors from decoding artwork or encoding a backdrop.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PaletteError {
    /// The bytes are not an image this build decodes (JPEG or PNG).
    #[error("cannot decode the artwork: {0}")]
    Decode(#[from] image::ImageError),
    /// The backdrop could not be encoded as PNG.
    #[error("cannot encode the backdrop PNG: {0}")]
    Encode(#[from] png::EncodingError),
}

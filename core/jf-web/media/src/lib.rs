//! Jellyfesh Web's media worker: theme colours and baked backdrops from
//! artwork (`jf-palette`), and the tags of files the person drags in
//! (lofty). The page loads it on demand in a Web Worker, as the apps bake
//! on their media thread (`docs/architecture.md`, section 4).
//!
//! JavaScript API: [`Artwork`] (themes and backdrops from RGBA pixels the
//! browser decoded), [`Tags`] (a file's tags and cover), structured values
//! as JSON text.

pub mod tags;
pub mod theme;

use serde::Serialize;
use wasm_bindgen::prelude::*;

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}

fn error(message: impl std::fmt::Display) -> JsError {
    JsError::new(&message.to_string())
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// Reports Rust panics on the console instead of an opaque trap.
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| console_error(&format!("jf-web-media: {info}"))));
}

/// An RGBA image.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[wasm_bindgen]
impl Image {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The pixels, RGBA, row by row.
    pub fn pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }
}

/// Decoded artwork: its themes and backdrops.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Artwork {
    image: jf_palette::image::DynamicImage,
}

#[wasm_bindgen]
impl Artwork {
    /// Artwork from RGBA pixels the browser decoded.
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Artwork, JsError> {
        let image =
            theme::image(width, height, rgba).ok_or_else(|| error("the pixels do not fit the size"))?;
        Ok(Artwork { image })
    }

    /// The artwork's hue and themes: `{hue, from_artwork, light, dark}`.
    pub fn themes(&self) -> String {
        json(&theme::for_artwork(&self.image))
    }

    /// The baked backdrop for `light` or `dark`.
    pub fn backdrop(&self, appearance: &str) -> Result<Image, JsError> {
        let appearance = theme::appearance(appearance).ok_or_else(|| error("appearance is light or dark"))?;
        let baked = theme::backdrop(&self.image, appearance);
        Ok(Image {
            width: baked.width(),
            height: baked.height(),
            pixels: baked.into_raw(),
        })
    }
}

/// A dragged-in file's tags.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Tags {
    tags: tags::FileTags,
}

#[wasm_bindgen]
impl Tags {
    /// Reads the tags of a file's bytes.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8], name: &str) -> Result<Tags, JsError> {
        Ok(Tags {
            tags: tags::read(bytes, name).map_err(error)?,
        })
    }

    /// `FileTags` as JSON (without the cover).
    pub fn json(&self) -> String {
        json(&self.tags)
    }

    /// The embedded cover's bytes, if any.
    pub fn cover(&self) -> Option<Vec<u8>> {
        self.tags.cover.clone()
    }
}

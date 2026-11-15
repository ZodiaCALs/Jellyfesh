//! What the browser computes from decoded artwork pixels must be what the
//! apps compute from the artwork file. Runs on the host only.

#![cfg(not(target_arch = "wasm32"))]

use jf_palette::Appearance;

fn artwork_png() -> Vec<u8> {
    let mut img = image::RgbaImage::new(96, 64);
    for (x, y, px) in img.enumerate_pixels_mut() {
        *px = image::Rgba([(x * 2) as u8, (y * 3) as u8, 160, 255]);
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    png
}

#[test]
fn backdrops_and_hues_match_the_native_bake() {
    let png = artwork_png();
    let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
    let artwork = jf_web_media::theme::image(decoded.width(), decoded.height(), decoded.into_raw()).unwrap();
    for appearance in [Appearance::Light, Appearance::Dark] {
        let ours = jf_web_media::theme::backdrop(&artwork, appearance);
        let native_png = jf_palette::bake_backdrop_png(&png, appearance).unwrap();
        let native = image::load_from_memory(&native_png).unwrap().to_rgba8();
        assert_eq!(ours.dimensions(), native.dimensions());
        assert_eq!(ours.into_raw(), native.into_raw(), "{appearance:?}");
    }
    let hue = jf_web_media::theme::for_artwork(&artwork).hue;
    assert_eq!(hue, jf_palette::extract_hue_from_bytes(&png).unwrap().hue);
}

#[test]
fn years_are_read_as_the_library_reads_them() {
    for text in [
        "2021",
        "2021-04-01",
        "2021/04",
        "(P) 2021",
        "12021",
        "0000",
        "",
        "Spring 1999 tour",
        "20210401",
    ] {
        assert_eq!(
            jf_web_media::tags::parse_year(text),
            jf_library::rules::parse_year(text),
            "{text:?}"
        );
    }
}

//! Theme colours and Now Playing backdrops from artwork.

use std::path::{Path, PathBuf};

use jf_palette::{Appearance, HuePick, Theme};

use crate::CoreError;

/// Bumped whenever the bake changes, so stale cached backdrops are not reused.
const BACKDROP_VERSION: u32 = 1;

/// Misty theme for one hue in both appearances.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtworkTheme {
    pub pick: HuePick,
    pub light: Theme,
    pub dark: Theme,
}

/// The theme for artwork bytes, or for the fallback hue when there is no
/// artwork or it cannot be decoded.
pub fn theme_for_artwork(bytes: Option<&[u8]>) -> ArtworkTheme {
    let pick = bytes
        .and_then(|b| jf_palette::extract_hue_from_bytes(b).ok())
        .unwrap_or(HuePick {
            hue: jf_tokens::DEFAULT_HUE,
            chroma: 0.0,
            from_artwork: false,
        });
    ArtworkTheme {
        light: jf_palette::resolve_theme(pick.hue, Appearance::Light),
        dark: jf_palette::resolve_theme(pick.hue, Appearance::Dark),
        pick,
    }
}

/// Bakes the light and dark backdrops for artwork whose content hash is
/// `key`, reusing cached files. Returns `(light, dark)` paths.
pub fn backdrops(bytes: &[u8], key: &str, cache_dir: &Path) -> Result<(PathBuf, PathBuf), CoreError> {
    let dir = cache_dir.join("backdrop");
    std::fs::create_dir_all(&dir)?;
    let mut paths = Vec::with_capacity(2);
    for (appearance, name) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let path = dir.join(format!("{key}-v{BACKDROP_VERSION}-{name}.png"));
        if !path.is_file() {
            let png = jf_palette::bake_backdrop_png(bytes, appearance)?;
            write_atomically(&path, &png)?;
        }
        paths.push(path);
    }
    let dark = paths.pop().unwrap_or_default();
    let light = paths.pop().unwrap_or_default();
    Ok((light, dark))
}

/// Bakes one backdrop from artwork bytes, as PNG.
pub fn backdrop_png(bytes: &[u8], appearance: Appearance) -> Result<Vec<u8>, CoreError> {
    Ok(jf_palette::bake_backdrop_png(bytes, appearance)?)
}

/// Content key for artwork bytes.
pub fn artwork_key(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// A cover image in the audio file's folder (`cover.jpg`, `folder.png`, …),
/// by the library's rules (the scanner gives the same picture to tracks
/// without one of their own).
pub fn folder_artwork(audio: &Path) -> Option<PathBuf> {
    jf_library::folder_artwork(audio)
}

fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_artwork_gives_the_fallback_theme() {
        let t = theme_for_artwork(None);
        assert!(!t.pick.from_artwork);
        assert_eq!(t.pick.hue, jf_tokens::DEFAULT_HUE);
        assert!(!t.light.colors.is_empty());
        assert_eq!(
            theme_for_artwork(Some(b"not an image")).pick.hue,
            jf_tokens::DEFAULT_HUE
        );
    }

    #[test]
    fn finds_folder_art_by_preference() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("01.flac");
        std::fs::write(&audio, b"").unwrap();
        assert_eq!(folder_artwork(&audio), None);
        std::fs::write(dir.path().join("Front.PNG"), b"").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"").unwrap();
        assert_eq!(folder_artwork(&audio), Some(dir.path().join("Front.PNG")));
        std::fs::write(dir.path().join("cover.jpg"), b"").unwrap();
        assert_eq!(folder_artwork(&audio), Some(dir.path().join("cover.jpg")));
    }
}

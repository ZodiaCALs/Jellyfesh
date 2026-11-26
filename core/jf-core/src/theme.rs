//! Theme colours and Now Playing backdrops from artwork.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use jf_palette::{Appearance, HuePick, Theme};

use crate::CoreError;

/// Bumped whenever the bake changes, so stale cached backdrops are not reused.
const BACKDROP_VERSION: u32 = 1;
/// The baked backdrops go, least recently used first, down to three quarters
/// of this when the folder grows past it. A light and dark pair takes about
/// 33 KB, so over a thousand covers stay baked.
const BACKDROP_CACHE_BYTES: u64 = 48 * 1024 * 1024;
/// New bakes between checks of the folder's size (the first bake in a run
/// checks too).
const TRIM_EVERY: u32 = 32;

/// Backdrops baked in this process, for [`TRIM_EVERY`].
static BAKED: AtomicU32 = AtomicU32::new(0);

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
/// `key`, reusing cached files. Returns `(light, dark)` paths. The folder is
/// kept to [`BACKDROP_CACHE_BYTES`]: a backdrop gone is baked again when its
/// cover plays.
pub fn backdrops(bytes: &[u8], key: &str, cache_dir: &Path) -> Result<(PathBuf, PathBuf), CoreError> {
    let dir = cache_dir.join("backdrop");
    std::fs::create_dir_all(&dir)?;
    let mut paths = Vec::with_capacity(2);
    let mut baked = false;
    for (appearance, name) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let path = dir.join(format!("{key}-v{BACKDROP_VERSION}-{name}.png"));
        if path.is_file() {
            crate::artwork::touch(&path);
        } else {
            let png = jf_palette::bake_backdrop_png(bytes, appearance)?;
            write_atomically(&path, &png)?;
            baked = true;
        }
        paths.push(path);
    }
    if baked && BAKED.fetch_add(1, Ordering::Relaxed).is_multiple_of(TRIM_EVERY) {
        trim(&dir, BACKDROP_CACHE_BYTES, &paths);
    }
    let dark = paths.pop().unwrap_or_default();
    let light = paths.pop().unwrap_or_default();
    Ok((light, dark))
}

/// Deletes the least recently used files in `dir` while it holds more than
/// `max_bytes`, down to three quarters of it; `keep` (the backdrops just
/// handed out) stays.
fn trim(dir: &Path, max_bytes: u64, keep: &[PathBuf]) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, PathBuf, u64)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let meta = entry.metadata().ok().filter(|m| m.is_file())?;
            Some((meta.modified().unwrap_or(UNIX_EPOCH), entry.path(), meta.len()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.2).sum();
    if total <= max_bytes {
        return;
    }
    files.sort();
    for (_, path, bytes) in files {
        if total <= max_bytes / 4 * 3 {
            break;
        }
        if !keep.contains(&path) && std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(bytes);
        }
    }
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
    use std::time::Duration;

    use super::*;

    fn set_modified(path: &Path, at: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(at)
            .unwrap();
    }

    #[test]
    fn trimming_keeps_the_newest_backdrops_and_the_ones_handed_out() {
        let dir = tempfile::tempdir().unwrap();
        let names = ["a", "b", "c", "d"];
        for (n, name) in names.iter().enumerate() {
            let path = dir.path().join(format!("{name}.png"));
            std::fs::write(&path, vec![0u8; 1000]).unwrap();
            set_modified(
                &path,
                UNIX_EPOCH + Duration::from_secs(1_000_000 + n as u64 * 100),
            );
        }
        // 4000 bytes against a limit of 3000: the oldest go until 2250 is left,
        // except "a", the oldest, which was just handed out.
        trim(dir.path(), 3000, &[dir.path().join("a.png")]);
        let left: Vec<bool> = names
            .iter()
            .map(|n| dir.path().join(format!("{n}.png")).exists())
            .collect();
        assert_eq!(left, [true, false, false, true]);
        // Within the limit nothing goes.
        trim(dir.path(), 3000, &[]);
        assert!(dir.path().join("a.png").exists() && dir.path().join("d.png").exists());
    }

    #[test]
    fn a_backdrop_shown_again_is_reused_and_marked_as_used() {
        let cache = tempfile::tempdir().unwrap();
        let cover = cache.path().join("cover.png");
        image::RgbImage::from_pixel(64, 64, image::Rgb([30, 120, 200]))
            .save(&cover)
            .unwrap();
        let bytes = std::fs::read(&cover).unwrap();
        let (light, dark) = backdrops(&bytes, "k1", cache.path()).unwrap();
        assert!(light.is_file() && dark.is_file());
        let long_ago = SystemTime::now() - Duration::from_secs(2 * 60 * 60);
        for path in [&light, &dark] {
            set_modified(path, long_ago);
        }
        assert_eq!(
            backdrops(&bytes, "k1", cache.path()).unwrap(),
            (light.clone(), dark.clone())
        );
        for path in [&light, &dark] {
            let at = std::fs::metadata(path).unwrap().modified().unwrap();
            assert!(
                at > long_ago + Duration::from_secs(60 * 60),
                "{} was not marked as used",
                path.display()
            );
        }
    }

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

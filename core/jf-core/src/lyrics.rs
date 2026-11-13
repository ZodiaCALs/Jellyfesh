//! Finding and loading lyrics for a track.
//!
//! Order of preference: a sidecar file next to the audio file (`.ttml`, then
//! `.lrc`, then `.vtt`, so word-timed formats win), then lyrics embedded in
//! the file's tags. Online lyric sources arrive with plugins in a later
//! release.

use std::path::{Path, PathBuf};

use jf_lyrics::{Format, LyricDoc};

use crate::CoreError;
use crate::types::LyricsSource;

/// Lyric files above this size are refused (the parsers' own limit).
const MAX_LYRICS_FILE: u64 = 4 * 1024 * 1024;

/// Sidecar extensions, most precise timing first.
const SIDECARS: [(&str, Format); 3] = [
    ("ttml", Format::Ttml),
    ("lrc", Format::Lrc),
    ("vtt", Format::WebVtt),
];

/// Sidecar paths to try for `audio`, in order of preference.
pub fn sidecar_candidates(audio: &Path) -> Vec<(PathBuf, Format)> {
    let mut out = Vec::with_capacity(SIDECARS.len() * 2);
    for (ext, format) in SIDECARS {
        out.push((audio.with_extension(ext), format));
        out.push((audio.with_extension(ext.to_ascii_uppercase()), format));
    }
    out
}

/// The parser hint for a lyrics file's extension.
pub fn format_for_path(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "ttml" | "xml" => Some(Format::Ttml),
        "lrc" => Some(Format::Lrc),
        "vtt" => Some(Format::WebVtt),
        "txt" => Some(Format::Plain),
        _ => None,
    }
}

/// Reads and parses a lyrics file. The format comes from the extension and
/// is sniffed from the content when the extension says nothing.
pub fn load_lyrics_file(path: &Path) -> Result<LyricDoc, CoreError> {
    let len = std::fs::metadata(path)?.len();
    if len > MAX_LYRICS_FILE {
        return Err(CoreError::Other(format!(
            "{} is larger than 4 MiB",
            path.display()
        )));
    }
    let text = decode_text(&std::fs::read(path)?);
    let mut doc = jf_lyrics::parse(&text, format_for_path(path))?;
    doc.source.origin = Some(path.display().to_string());
    Ok(doc)
}

/// Finds lyrics for an audio file: a sidecar first, then `embedded()`.
pub fn find_lyrics(
    audio: &Path,
    embedded: impl FnOnce() -> Option<String>,
) -> Option<(LyricDoc, LyricsSource)> {
    for (path, _) in sidecar_candidates(audio) {
        if !path.is_file() {
            continue;
        }
        match load_lyrics_file(&path) {
            Ok(doc) => return Some((doc, LyricsSource::Sidecar(path))),
            Err(e) => tracing::warn!("ignoring lyrics file {}: {e}", path.display()),
        }
    }
    let text = embedded()?;
    match jf_lyrics::parse(&text, None) {
        Ok(mut doc) => {
            doc.source.origin = Some("embedded".into());
            Some((doc, LyricsSource::Embedded))
        }
        Err(e) => {
            tracing::warn!("ignoring embedded lyrics in {}: {e}", audio.display());
            None
        }
    }
}

/// Finds lyrics for an audio file that may not be in the library: a sidecar
/// first, then lyrics embedded in its tags.
pub fn find_lyrics_for_file(audio: &Path) -> Option<(LyricDoc, LyricsSource)> {
    find_lyrics(audio, || jf_library::read_file_tags(audio, false).ok()?.lyrics)
}

/// Text from a lyrics file: UTF-8 (with or without a byte-order mark) or
/// UTF-16 with a byte-order mark. Other encodings decode lossily as UTF-8.
pub fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_byte_order_marks() {
        assert_eq!(decode_text(b"\xEF\xBB\xBFhi"), "hi");
        assert_eq!(decode_text(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
        assert_eq!(decode_text(&[0xFE, 0xFF, 0, b'h', 0, b'i']), "hi");
        assert_eq!(decode_text("夜".as_bytes()), "夜");
    }

    #[test]
    fn prefers_word_timed_sidecars() {
        let c = sidecar_candidates(Path::new("/m/a/01 Song.flac"));
        assert_eq!(c[0].0, PathBuf::from("/m/a/01 Song.ttml"));
        assert_eq!(c[2].0, PathBuf::from("/m/a/01 Song.lrc"));
        assert_eq!(c[4].0, PathBuf::from("/m/a/01 Song.vtt"));
    }

    #[test]
    fn finds_a_sidecar_before_embedded_lyrics() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("tone.wav");
        std::fs::write(&audio, b"not audio").unwrap();
        // Original test text, not from any song.
        std::fs::write(
            dir.path().join("tone.lrc"),
            "[00:01.00]A test line\n[00:03.50]Another line\n",
        )
        .unwrap();
        let (doc, source) = find_lyrics(&audio, || Some("[00:00.00]embedded".into())).unwrap();
        assert_eq!(source, LyricsSource::Sidecar(dir.path().join("tone.lrc")));
        assert_eq!(doc.lines.len(), 2);

        std::fs::remove_file(dir.path().join("tone.lrc")).unwrap();
        let (_, source) = find_lyrics(&audio, || Some("[00:00.00]embedded".into())).unwrap();
        assert_eq!(source, LyricsSource::Embedded);
        assert!(find_lyrics(&audio, || None).is_none());
    }
}

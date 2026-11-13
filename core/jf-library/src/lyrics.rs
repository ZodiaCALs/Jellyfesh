//! Lyrics files next to audio files, and embedded synced lyrics as LRC.
//!
//! Parsing lyrics is `jf-lyrics`' job; this module only finds and decodes text.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::fs::{FileSystem, ReadSeek};

/// Sidecar extensions in order of preference: TTML carries word timing, agents and
/// translations; LRC is the most common; WebVTT last.
const SIDECAR_EXTENSIONS: &[&str] = &["ttml", "lrc", "vtt"];

/// Larger "lyrics" files are not lyrics.
const MAX_SIDECAR_BYTES: u64 = 4 << 20;

/// Finds `<stem>.ttml`, `<stem>.lrc` or `<stem>.vtt` (lower or upper case extension)
/// next to `audio` and returns its path and decoded text.
pub(crate) fn find_sidecar(fs: &dyn FileSystem, audio: &Path) -> Option<(PathBuf, String)> {
    for ext in SIDECAR_EXTENSIONS {
        for candidate in [
            audio.with_extension(ext),
            audio.with_extension(ext.to_ascii_uppercase()),
        ] {
            if candidate == audio {
                continue;
            }
            match fs.open(&candidate).and_then(read_text) {
                Ok(text) => return Some((candidate, text)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => tracing::warn!(path = %candidate.display(), %err, "cannot read lyrics file"),
            }
        }
    }
    None
}

fn read_text(reader: Box<dyn ReadSeek>) -> io::Result<String> {
    let mut bytes = Vec::new();
    reader.take(MAX_SIDECAR_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SIDECAR_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "larger than 4 MiB"));
    }
    Ok(decode_text(&bytes))
}

/// UTF-8 (BOM optional) or UTF-16 with a BOM; invalid sequences become U+FFFD.
pub(crate) fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        String::from_utf8_lossy(rest).into_owned()
    } else if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        decode_utf16(rest, u16::from_le_bytes)
    } else if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        decode_utf16(rest, u16::from_be_bytes)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn decode_utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let units = bytes.chunks_exact(2).map(|pair| unit([pair[0], pair[1]]));
    char::decode_utf16(units)
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// Whether any line starts with an LRC time tag such as `[01:23.45]`.
pub(crate) fn looks_like_lrc(text: &str) -> bool {
    text.lines().any(|line| {
        let Some(rest) = line.trim_start().strip_prefix('[') else {
            return false;
        };
        let Some((minutes, rest)) = rest.split_once(':') else {
            return false;
        };
        let seconds: String = rest.chars().take_while(char::is_ascii_digit).collect();
        !minutes.is_empty()
            && minutes.bytes().all(|b| b.is_ascii_digit())
            && !seconds.is_empty()
            && rest[seconds.len()..].starts_with(['.', ':', ']'])
    })
}

/// Converts SYLT entries (milliseconds, text) to LRC.
///
/// Writers use one entry per line, or one per word with a leading newline marking
/// each new line. Word entries become enhanced-LRC word stamps (`<mm:ss.xx>`).
pub(crate) fn sylt_to_lrc(entries: &[(u32, String)]) -> String {
    let is_break = |text: &str| text.starts_with(['\n', '\r']);
    let per_word = entries.iter().skip(1).any(|(_, text)| is_break(text));
    let mut out = String::new();
    for (index, (ms, text)) in entries.iter().enumerate() {
        if !per_word {
            out.push_str(&format!("[{}]{}\n", stamp(*ms), text.trim()));
            continue;
        }
        if index == 0 || is_break(text) {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&format!("[{}]", stamp(*ms)));
        }
        out.push_str(&format!(
            "<{}>{}",
            stamp(*ms),
            text.trim_start_matches(['\n', '\r'])
        ));
    }
    if per_word {
        out.push('\n');
    }
    out
}

/// `mm:ss.xx`; minutes keep counting past 59.
fn stamp(ms: u32) -> String {
    let centis = ms / 10;
    format!(
        "{:02}:{:02}.{:02}",
        centis / 6000,
        centis / 100 % 60,
        centis % 100
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_lrc() {
        assert!(looks_like_lrc("[ti:Song]\n[00:12.34]Line"));
        assert!(looks_like_lrc("  [1:02]Line"));
        assert!(!looks_like_lrc("[ti:Song]\nplain words"));
        assert!(!looks_like_lrc("Verse [x:1]"));
    }

    #[test]
    fn converts_sylt_lines_and_words() {
        let lines = [(1000, "Hello".to_owned()), (62_500, "World".to_owned())];
        assert_eq!(sylt_to_lrc(&lines), "[00:01.00]Hello\n[01:02.50]World\n");

        let words = [
            (1000, "Hel".to_owned()),
            (1500, "lo ".to_owned()),
            (2000, "\nWorld".to_owned()),
        ];
        assert_eq!(
            sylt_to_lrc(&words),
            "[00:01.00]<00:01.00>Hel<00:01.50>lo \n[00:02.00]<00:02.00>World\n"
        );
    }

    #[test]
    fn decodes_boms() {
        assert_eq!(decode_text(b"\xEF\xBB\xBFhi"), "hi");
        assert_eq!(decode_text(b"\xFF\xFEh\0i\0"), "hi");
        assert_eq!(decode_text(b"\xFE\xFF\0h\0i"), "hi");
        assert_eq!(decode_text("晴天".as_bytes()), "晴天");
    }
}

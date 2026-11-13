//! Lyrics for Now Playing: from a plugin's lyric payload, from a dragged-in
//! file's tags, or from a lyrics file dragged in beside it, always parsed by
//! `jf-lyrics` into one `LyricDoc` plus the flat timeline the views'
//! cursor reads (the TypeScript port of `LyricTimeline::cursor`).

use jf_lyrics::{Format, LyricDoc, LyricTimeline, Timing};
use jf_plugin_model::{LyricFormat, LyricPayload};
use serde::Serialize;

/// Lyric files above this size are refused (the parsers' own limit).
pub const MAX_LYRICS_FILE: usize = 4 * 1024 * 1024;

/// A document and its timeline.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Lyrics {
    pub doc: LyricDoc,
    pub timeline: LyricTimeline,
    /// False when the source's terms ask for unsynced lyrics.
    pub synced: bool,
}

impl Lyrics {
    fn new(doc: LyricDoc, synced: bool) -> Lyrics {
        let timeline = doc.timeline();
        Lyrics {
            doc,
            timeline,
            synced,
        }
    }
}

/// The parser for a plugin's lyric format; `None` for a format the core does
/// not read (JSON syllables).
pub fn format_of(format: LyricFormat) -> Option<Format> {
    match format {
        LyricFormat::Plain => Some(Format::Plain),
        LyricFormat::Lrc => Some(Format::Lrc),
        LyricFormat::Elrc => Some(Format::EnhancedLrc),
        LyricFormat::Ttml => Some(Format::Ttml),
        LyricFormat::Vtt => Some(Format::WebVtt),
        LyricFormat::JsonSyllables => None,
    }
}

/// Lyrics from a plugin, as the core makes them: attributed to the payload's
/// attribution (else the source's name), its language when the document has
/// none, and unsynced when the source's content policy forbids synced lyrics.
pub fn from_payload(payload: LyricPayload, source_name: &str, lyrics_sync: bool) -> Result<Lyrics, String> {
    let format = format_of(payload.format).ok_or("lyrics in a format Jellyfesh does not read")?;
    let mut doc = jf_lyrics::parse(&payload.text, Some(format)).map_err(|e| e.to_string())?;
    doc.source.origin = Some(payload.attribution.unwrap_or_else(|| source_name.to_owned()));
    if doc.language.is_none() {
        doc.language = payload.language;
    }
    if !lyrics_sync {
        doc.timing = Timing::Unsynced;
    }
    Ok(Lyrics::new(doc, lyrics_sync))
}

/// The parser hint for a lyrics file's name.
pub fn format_for_name(name: &str) -> Option<Format> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "ttml" | "xml" => Some(Format::Ttml),
        "lrc" => Some(Format::Lrc),
        "vtt" => Some(Format::WebVtt),
        "txt" => Some(Format::Plain),
        _ => None,
    }
}

/// Lyrics from a lyrics file's bytes (UTF-8, or UTF-16 with a byte-order
/// mark); the format comes from the name, else from the content.
pub fn from_file(bytes: &[u8], name: &str) -> Result<Lyrics, String> {
    if bytes.len() > MAX_LYRICS_FILE {
        return Err(format!("{name} is larger than 4 MiB"));
    }
    let text = decode_text(bytes);
    let mut doc = jf_lyrics::parse(&text, format_for_name(name)).map_err(|e| e.to_string())?;
    doc.source.origin = Some(name.to_owned());
    Ok(Lyrics::new(doc, true))
}

/// Lyrics embedded in a file's tags (text, LRC from SYLT), format sniffed.
pub fn from_tags(text: &str) -> Result<Lyrics, String> {
    let mut doc = jf_lyrics::parse(text, None).map_err(|e| e.to_string())?;
    doc.source.origin = Some("embedded".into());
    Ok(Lyrics::new(doc, true))
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

/// What a dragged-in file is, from its name and MIME type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DroppedKind {
    Audio,
    Lyrics,
    Other,
}

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "m4a", "m4b", "mp4", "aac", "ogg", "oga", "opus", "wav", "wave", "aif", "aiff", "webm",
    "weba", "mka", "caf",
];

/// Classifies a dragged-in file.
pub fn classify(name: &str, mime: &str) -> DroppedKind {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if format_for_name(name).is_some() && ext != "xml" {
        DroppedKind::Lyrics
    } else if mime.starts_with("audio/") || AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        DroppedKind::Audio
    } else {
        DroppedKind::Other
    }
}

/// Pairs dragged-in files: for each audio file (by index into `names`), the
/// lyrics file with the same stem, preferring word-timed formats (TTML, then
/// LRC, then WebVTT), as the apps pick sidecars.
pub fn pair_sidecars(names: &[(String, String)]) -> Vec<(usize, Option<usize>)> {
    let stem = |name: &str| {
        name.rsplit_once('.')
            .map_or(name, |(stem, _)| stem)
            .to_lowercase()
    };
    let rank = |name: &str| match name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()) {
        Some(e) if e == "ttml" => Some(0),
        Some(e) if e == "lrc" => Some(1),
        Some(e) if e == "vtt" => Some(2),
        _ => None,
    };
    names
        .iter()
        .enumerate()
        .filter(|(_, (name, mime))| classify(name, mime) == DroppedKind::Audio)
        .map(|(index, (name, _))| {
            let wanted = stem(name);
            let sidecar = names
                .iter()
                .enumerate()
                .filter(|(_, (other, _))| stem(other) == wanted)
                .filter_map(|(i, (other, _))| rank(other).map(|r| (r, i)))
                .min()
                .map(|(_, i)| i);
            (index, sidecar)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(format: LyricFormat, text: &str) -> LyricPayload {
        LyricPayload {
            format,
            text: text.into(),
            language: Some("en".into()),
            attribution: None,
        }
    }

    #[test]
    fn plugin_lyrics_follow_the_core() {
        let lyrics = from_payload(
            payload(LyricFormat::Lrc, "[00:01.00]One\n[00:02.00]Two\n"),
            "Subsonic",
            true,
        )
        .unwrap();
        assert_eq!(lyrics.doc.source.origin.as_deref(), Some("Subsonic"));
        assert_eq!(lyrics.doc.language.as_deref(), Some("en"));
        assert_eq!(lyrics.timeline.line_starts, [1000, 2000]);
        let unsynced = from_payload(payload(LyricFormat::Lrc, "[00:01.00]One\n"), "S", false).unwrap();
        assert!(unsynced.timeline.line_starts.is_empty());
        assert!(!unsynced.synced);
        assert!(from_payload(payload(LyricFormat::JsonSyllables, "{}"), "S", true).is_err());
    }

    #[test]
    fn files_decode_and_sniff() {
        let lrc = from_file(b"\xEF\xBB\xBF[00:01.00]Hi", "song.lrc").unwrap();
        assert_eq!(lrc.doc.lines[0].text, "Hi");
        assert_eq!(lrc.doc.source.origin.as_deref(), Some("song.lrc"));
        assert_eq!(decode_text(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
        assert_eq!(from_tags("plain words").unwrap().doc.lines[0].text, "plain words");
    }

    #[test]
    fn pairs_sidecars_by_stem_and_precision() {
        let names: Vec<(String, String)> = [
            ("01 Song.flac", "audio/flac"),
            ("01 song.lrc", ""),
            ("01 Song.ttml", ""),
            ("02 Other.mp3", "audio/mpeg"),
            ("cover.jpg", "image/jpeg"),
        ]
        .iter()
        .map(|(n, m)| (n.to_string(), m.to_string()))
        .collect();
        assert_eq!(pair_sidecars(&names), [(0, Some(2)), (3, None)]);
        assert_eq!(classify("x.LRC", ""), DroppedKind::Lyrics);
        assert_eq!(classify("x.bin", "audio/ogg"), DroppedKind::Audio);
        assert_eq!(classify("cover.jpg", "image/jpeg"), DroppedKind::Other);
    }
}

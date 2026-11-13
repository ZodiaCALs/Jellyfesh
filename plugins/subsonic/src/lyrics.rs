//! OpenSubsonic structured lyrics (`getLyricsBySongId`) as LRC or plain text.
//!
//! * Synced lyrics become LRC with one `[mm:ss.xxx]` stamp per line. The
//!   entry's `offset` (milliseconds, positive means sooner, as in LRC's
//!   `[offset:]`) is applied to the stamps, which stop at zero.
//! * Unsynced lyrics become plain text, one line per line.
//! * Of several entries, the main (not a translation or pronunciation) synced
//!   one wins, then the main unsynced one.

use jf_plugin_sdk::prelude::*;

use crate::model::StructuredLyrics;

/// Picks the entry to show.
pub fn choose(list: &[StructuredLyrics]) -> Option<&StructuredLyrics> {
    let main: Vec<&StructuredLyrics> = list
        .iter()
        .filter(|l| l.kind.as_deref().is_none_or(|k| k.eq_ignore_ascii_case("main")))
        .filter(|l| l.line.iter().any(|line| !line.value.trim().is_empty()))
        .collect();
    main.iter()
        .find(|l| is_synced(l))
        .or_else(|| main.first())
        .copied()
}

/// Whether an entry has usable timing.
fn is_synced(lyrics: &StructuredLyrics) -> bool {
    lyrics.synced.unwrap_or(false) && lyrics.line.iter().any(|l| l.start.is_some())
}

/// The payload for an entry: LRC when synced, plain text otherwise.
pub fn payload(lyrics: &StructuredLyrics) -> LyricPayload {
    let mut payload = if is_synced(lyrics) {
        LyricPayload::new(LyricFormat::Lrc, lrc(lyrics))
    } else {
        LyricPayload::new(LyricFormat::Plain, plain(lyrics))
    };
    payload.language = lyrics.lang.as_deref().and_then(language);
    payload
}

fn lrc(lyrics: &StructuredLyrics) -> String {
    let offset = lyrics.offset.unwrap_or(0.0);
    let mut out = String::new();
    let mut last = 0.0f64;
    for line in &lyrics.line {
        // A line without a time in synced lyrics keeps its place.
        let start = line.start.unwrap_or(last);
        last = start;
        let at = (start - offset).round().max(0.0) as u64;
        out.push_str(&stamp(at));
        out.push_str(&one_line(&line.value));
        out.push('\n');
    }
    out
}

fn plain(lyrics: &StructuredLyrics) -> String {
    let lines: Vec<String> = lyrics.line.iter().map(|l| one_line(&l.value)).collect();
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

/// `[mm:ss.xxx]`; minutes go past 59 as LRC allows.
pub fn stamp(ms: u64) -> String {
    format!("[{:02}:{:02}.{:03}]", ms / 60_000, (ms / 1000) % 60, ms % 1000)
}

/// A language tag as BCP 47 asks for it: the two-letter code when ISO 639-1
/// has one (`eng` → `en`), otherwise the tag as given; none for "unknown"
/// (`und`, the taggers' `xxx`, `zxx`).
pub fn language(tag: &str) -> Option<String> {
    let tag = tag.trim();
    if tag.is_empty()
        || tag.len() > 35
        || !tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    let tag = tag.replace('_', "-");
    let (primary, rest) = match tag.split_once('-') {
        Some((p, r)) => (p.to_ascii_lowercase(), Some(r)),
        None => (tag.to_ascii_lowercase(), None),
    };
    if matches!(primary.as_str(), "und" | "xxx" | "zxx") || !(2..=3).contains(&primary.len()) {
        return None;
    }
    let primary = ISO_639_2
        .iter()
        .find(|(three, _)| *three == primary)
        .map(|(_, two)| two.to_string())
        .unwrap_or(primary);
    Some(match rest {
        Some(rest) => format!("{primary}-{rest}"),
        None => primary,
    })
}

/// ISO 639-2 (bibliographic and terminology codes) to ISO 639-1, for the
/// languages lyrics are most often tagged with.
const ISO_639_2: &[(&str, &str)] = &[
    ("afr", "af"),
    ("ara", "ar"),
    ("baq", "eu"),
    ("ben", "bn"),
    ("bul", "bg"),
    ("cat", "ca"),
    ("ces", "cs"),
    ("chi", "zh"),
    ("cym", "cy"),
    ("cze", "cs"),
    ("dan", "da"),
    ("deu", "de"),
    ("dut", "nl"),
    ("ell", "el"),
    ("eng", "en"),
    ("epo", "eo"),
    ("est", "et"),
    ("eus", "eu"),
    ("fas", "fa"),
    ("fin", "fi"),
    ("fra", "fr"),
    ("fre", "fr"),
    ("ger", "de"),
    ("gle", "ga"),
    ("glg", "gl"),
    ("gre", "el"),
    ("heb", "he"),
    ("hin", "hi"),
    ("hrv", "hr"),
    ("hun", "hu"),
    ("ice", "is"),
    ("ind", "id"),
    ("isl", "is"),
    ("ita", "it"),
    ("jpn", "ja"),
    ("kor", "ko"),
    ("lat", "la"),
    ("lav", "lv"),
    ("lit", "lt"),
    ("may", "ms"),
    ("msa", "ms"),
    ("nld", "nl"),
    ("nob", "nb"),
    ("nno", "nn"),
    ("nor", "no"),
    ("per", "fa"),
    ("pol", "pl"),
    ("por", "pt"),
    ("ron", "ro"),
    ("rum", "ro"),
    ("rus", "ru"),
    ("slk", "sk"),
    ("slo", "sk"),
    ("slv", "sl"),
    ("spa", "es"),
    ("srp", "sr"),
    ("swa", "sw"),
    ("swe", "sv"),
    ("tam", "ta"),
    ("tel", "te"),
    ("tgl", "tl"),
    ("tha", "th"),
    ("tur", "tr"),
    ("ukr", "uk"),
    ("urd", "ur"),
    ("vie", "vi"),
    ("wel", "cy"),
    ("zho", "zh"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LyricLine;

    fn line(start: Option<f64>, value: &str) -> LyricLine {
        LyricLine {
            start,
            value: value.into(),
        }
    }

    #[test]
    fn stamps() {
        assert_eq!(stamp(0), "[00:00.000]");
        assert_eq!(stamp(83_456), "[01:23.456]");
        assert_eq!(stamp(3_600_000 + 5), "[60:00.005]");
    }

    #[test]
    fn synced_lyrics_become_lrc_with_the_offset_applied() {
        let lyrics = StructuredLyrics {
            lang: Some("eng".into()),
            synced: Some(true),
            offset: Some(250.0),
            kind: None,
            line: vec![
                line(Some(100.0), "First"),
                line(Some(2000.4), "Second\nhalf"),
                line(None, "No time"),
                line(Some(61_000.0), ""),
            ],
        };
        let payload = payload(&lyrics);
        assert_eq!(payload.format, LyricFormat::Lrc);
        assert_eq!(payload.language.as_deref(), Some("en"));
        assert_eq!(
            payload.text,
            "[00:00.000]First\n[00:01.750]Second half\n[00:01.750]No time\n[01:00.750]\n"
        );
    }

    #[test]
    fn unsynced_lyrics_become_plain_text() {
        let lyrics = StructuredLyrics {
            lang: Some("xxx".into()),
            synced: Some(false),
            line: vec![line(None, "One"), line(None, "Two")],
            ..StructuredLyrics::default()
        };
        let payload = payload(&lyrics);
        assert_eq!(payload.format, LyricFormat::Plain);
        assert_eq!(payload.text, "One\nTwo\n");
        assert_eq!(payload.language, None);
    }

    #[test]
    fn prefers_main_synced_lyrics() {
        let entry = |kind: Option<&str>, synced: bool, text: &str| StructuredLyrics {
            kind: kind.map(str::to_string),
            synced: Some(synced),
            line: vec![line(synced.then_some(0.0), text)],
            ..StructuredLyrics::default()
        };
        let list = vec![
            entry(Some("translation"), true, "translated"),
            entry(None, false, "plain"),
            entry(Some("main"), true, "synced"),
        ];
        assert_eq!(choose(&list).unwrap().line[0].value, "synced");
        assert_eq!(choose(&list[..2]).unwrap().line[0].value, "plain");
        assert!(choose(&list[..1]).is_none(), "a translation alone is not shown");
        assert!(
            choose(&[entry(None, false, " ")]).is_none(),
            "empty lyrics are none"
        );
    }

    #[test]
    fn language_tags() {
        assert_eq!(language("eng").as_deref(), Some("en"));
        assert_eq!(language("JPN").as_deref(), Some("ja"));
        assert_eq!(language("ko-Latn").as_deref(), Some("ko-Latn"));
        assert_eq!(language("pt_BR").as_deref(), Some("pt-BR"));
        assert_eq!(language("haw").as_deref(), Some("haw"));
        assert_eq!(language("und"), None);
        assert_eq!(language("xxx"), None);
        assert_eq!(language("english"), None);
        assert_eq!(language(""), None);
    }
}

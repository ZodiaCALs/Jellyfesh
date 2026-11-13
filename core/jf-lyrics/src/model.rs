//! The format-independent lyrics model.
//!
//! Every parser produces a [`LyricDoc`]. Times are milliseconds from the start
//! of the audio with any source offset already applied.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One lyric document, whatever format it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricDoc {
    /// BCP 47 language of the lyrics when the source declares it (for example `ja`).
    pub language: Option<String>,
    /// The best timing precision present in the document.
    pub timing: Timing,
    /// The offset the source declared (LRC `[offset:]`), already applied to every time.
    pub offset_ms: i32,
    /// Singers and voices referenced by [`Line::agent`].
    pub agents: Vec<Agent>,
    /// Lines in display order: sorted by start time when the document is synced.
    pub lines: Vec<Line>,
    /// Translation and romanization tracks, aligned to [`Line::id`].
    pub tracks: Vec<AuxTrack>,
    /// Where the document came from.
    pub source: SourceInfo,
    /// Source metadata such as LRC `ti`/`ar`/`al` tags, keys lowercased.
    pub metadata: BTreeMap<String, String>,
}

impl LyricDoc {
    /// An empty document (no lines) of the given format.
    pub fn empty(format: Format) -> Self {
        Self {
            language: None,
            timing: Timing::Unsynced,
            offset_ms: 0,
            agents: Vec::new(),
            lines: Vec::new(),
            tracks: Vec::new(),
            source: SourceInfo { format, origin: None },
            metadata: BTreeMap::new(),
        }
    }

    /// True when the document carries line or word times.
    pub fn is_synced(&self) -> bool {
        self.timing != Timing::Unsynced
    }

    /// The first auxiliary track of the given kind, if any.
    pub fn track(&self, kind: AuxKind) -> Option<&AuxTrack> {
        self.tracks.iter().find(|t| t.kind == kind)
    }
}

/// Timing precision of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Timing {
    /// No times: static text.
    #[default]
    Unsynced,
    /// Each line has a start (and usually an end).
    Line,
    /// At least one line has word (or syllable) times.
    Word,
}

/// A singer or voice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    /// Identifier referenced by [`Line::agent`] (for example `v1`).
    pub id: String,
    /// Display name when the source gives one.
    pub name: Option<String>,
    /// What kind of agent this is.
    pub kind: AgentKind,
}

/// Kind of [`Agent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    /// One singer.
    Person,
    /// Several singers together (a choir, "all").
    Group,
    /// Anything else.
    Other,
}

/// One lyric line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// Position of the line in [`LyricDoc::lines`]; auxiliary tracks are keyed by it.
    pub id: u32,
    /// Start time; `None` only in unsynced documents.
    pub start_ms: Option<u32>,
    /// End time; inferred from the next later line when the source has none.
    /// The last line of a document may have no end.
    pub end_ms: Option<u32>,
    /// [`Agent::id`] of the singer, for duets.
    pub agent: Option<String>,
    /// Display text, NFC-normalised and trimmed. Empty for instrumental gaps.
    pub text: String,
    /// Word (or syllable) times; empty when the line only has line timing.
    pub words: Vec<Word>,
    /// Background vocals, shown smaller.
    pub background: Vec<Word>,
    /// Writing direction, from the first strong character.
    pub direction: Direction,
}

/// A timed word or syllable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Word {
    /// Start time.
    pub start_ms: u32,
    /// End time, never before `start_ms`. Equal to `start_ms` for zero-length words.
    pub end_ms: u32,
    /// Text of the word without surrounding whitespace.
    pub text: String,
    /// True when no space separates this word from the next one (CJK, syllables).
    pub joins_next: bool,
}

/// Writing direction of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Left to right.
    Ltr,
    /// Right to left (Hebrew, Arabic and related scripts).
    Rtl,
    /// No strong character: the shell decides.
    #[default]
    Auto,
}

/// A translation or romanization track, aligned line by line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuxTrack {
    /// Translation or romanization.
    pub kind: AuxKind,
    /// BCP 47 language of the track when known (for example `en` or `ja-Latn`).
    pub language: Option<String>,
    /// Lines of the track keyed by [`Line::id`]. Lines without an entry have no counterpart.
    pub lines: BTreeMap<u32, AuxLine>,
}

/// Kind of [`AuxTrack`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuxKind {
    /// The meaning in another language.
    Translation,
    /// The same words in another script (for example romaji).
    Romanization,
}

/// One line of an [`AuxTrack`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuxLine {
    /// Display text.
    pub text: String,
    /// Word times when the source provides them.
    pub words: Vec<Word>,
}

/// Where a document came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    /// The format that was parsed.
    pub format: Format,
    /// Free-form origin set by the caller (file path, tag name, plugin id). Parsers leave it `None`.
    pub origin: Option<String>,
}

/// Lyric source formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// LRC with line timestamps.
    Lrc,
    /// LRC with word timestamps (A2 extension) or voice prefixes.
    EnhancedLrc,
    /// Timed Text Markup Language (Apple-style lyrics TTML).
    Ttml,
    /// WebVTT.
    #[serde(rename = "webvtt")]
    WebVtt,
    /// Plain, unsynced text.
    Plain,
}

impl Format {
    /// The serialised name of the format (`lrc`, `enhanced_lrc`, `ttml`, `webvtt`, `plain`).
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Lrc => "lrc",
            Format::EnhancedLrc => "enhanced_lrc",
            Format::Ttml => "ttml",
            Format::WebVtt => "webvtt",
            Format::Plain => "plain",
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_serialise_as_lowercase_strings() {
        let names: Vec<String> = [
            Format::Lrc,
            Format::EnhancedLrc,
            Format::Ttml,
            Format::WebVtt,
            Format::Plain,
        ]
        .iter()
        .map(|f| serde_json::to_string(f).unwrap())
        .collect();
        assert_eq!(
            names,
            [
                r#""lrc""#,
                r#""enhanced_lrc""#,
                r#""ttml""#,
                r#""webvtt""#,
                r#""plain""#
            ]
        );
        for f in [Format::Lrc, Format::EnhancedLrc, Format::WebVtt] {
            assert_eq!(serde_json::to_string(&f).unwrap(), format!("\"{f}\""));
        }
        assert_eq!(serde_json::to_string(&Timing::Word).unwrap(), r#""word""#);
        assert_eq!(serde_json::to_string(&Direction::Rtl).unwrap(), r#""rtl""#);
        assert_eq!(serde_json::to_string(&AgentKind::Group).unwrap(), r#""group""#);
        assert_eq!(
            serde_json::to_string(&AuxKind::Romanization).unwrap(),
            r#""romanization""#
        );
    }

    #[test]
    fn aux_track_keys_round_trip_through_json() {
        let mut lines = BTreeMap::new();
        lines.insert(
            10,
            AuxLine {
                text: "ten".into(),
                words: vec![],
            },
        );
        lines.insert(
            2,
            AuxLine {
                text: "two".into(),
                words: vec![],
            },
        );
        let track = AuxTrack {
            kind: AuxKind::Translation,
            language: Some("en".into()),
            lines,
        };
        let json = serde_json::to_string(&track).unwrap();
        assert!(json.find("\"2\"").unwrap() < json.find("\"10\"").unwrap());
        let back: AuxTrack = serde_json::from_str(&json).unwrap();
        assert_eq!(back, track);
    }
}

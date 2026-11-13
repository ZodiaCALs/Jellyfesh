//! Lyrics model and parsers: LRC, enhanced LRC, TTML and WebVTT into one `LyricDoc`.
//!
//! Every source converts into one [`LyricDoc`], so Now Playing renders all
//! lyrics the same way (see `docs/lyrics-model.md`).
//!
//! # Shared rules
//!
//! * Times are milliseconds from the start of the audio, after applying the
//!   source's offset. Times that would be negative clamp to 0.
//! * Lines are sorted by start (source order for equal starts). A line without
//!   an end ends where the next later line starts; the last line may keep no end.
//! * Blank timed lines are kept as empty lines: they mark instrumental gaps.
//! * Text is trimmed and NFC-normalised. Characters are never transliterated.
//! * [`Line::direction`] is [`Direction::Rtl`] when the first strong character
//!   is Hebrew, Arabic or another right-to-left script, [`Direction::Ltr`] for
//!   any other letter (Latin, CJK, Cyrillic, …) and [`Direction::Auto`] for
//!   text without letters.
//! * Word starts never decrease within a line; a word without its own end ends
//!   where the next word starts, else at the line end, else at its own start.
//! * [`Line::id`] is the line's position in [`LyricDoc::lines`].
//!
//! # Timeline and cursor
//!
//! [`LyricDoc::timeline`] flattens a document into sorted arrays and
//! [`LyricTimeline::cursor`] finds the active line and word with two binary
//! searches. The active line is the line that started last among those whose
//! `start ≤ t < end`; it is `None` before the first line, in gaps and after the
//! last line ends. The rules are documented on [`LyricTimeline`] and tested
//! against a linear scan.
//!
//! # Limits
//!
//! Lyrics can come from untrusted plugins, so every fallible parser rejects
//! input larger than [`MAX_INPUT_BYTES`] or with more than [`MAX_LINES`] lines,
//! and documents that would grow past [`MAX_LINES`] lines (or, through
//! repeated LRC stamps, past [`MAX_EXPANDED_BYTES`] of text). [`parse_plain`]
//! cannot fail and does not check them (its output never outgrows its input);
//! [`parse`] checks them for plain text too.
//!
//! # LRC
//!
//! * Timestamps `[mm:ss]`, `[mm:ss.x]`, `[mm:ss.xx]`, `[mm:ss.xxx]` and
//!   `[mm:ss:xx]` (the fraction after a colon reads like after a dot).
//!   Several stamps before one text repeat the line at each time.
//! * Tags `[key:value]` (`ti`, `ar`, `al`, `au`, `length`, `by`, `offset`,
//!   `re`, `ve`, and any other ASCII key) go to [`LyricDoc::metadata`] with
//!   lowercased keys.
//! * `[offset:+500]` makes the lyrics appear 500 ms *earlier*: every time is
//!   `stamp − offset`, clamped at 0 (the de-facto LRC convention). The value is
//!   kept in [`LyricDoc::offset_ms`]; the last `offset` tag wins.
//! * Lines that are neither a tag nor start with a valid stamp are skipped.
//! * Enhanced LRC (A2): word stamps `<mm:ss.xx>` inside the text start the
//!   following word; a stamp after the last word ends it and the line. Text
//!   before the first word stamp starts at the line stamp. When a line has
//!   several line stamps, its word stamps belong to the earliest one and are
//!   shifted by the same amount for the others. A voice prefix `v1:` at the
//!   start of the text sets [`Line::agent`] and adds an [`Agent`].
//! * Bilingual LRC: when a timestamp is shared by exactly two non-blank lines
//!   and one is mostly Latin while the other is mostly CJK, the second line
//!   (in file order) moves to a [`AuxKind::Translation`] track. "Mostly" means
//!   at least 60% of the line's units, counting each CJK character and each
//!   run of other letters as one unit. To stay conservative, nothing moves
//!   unless at least half of all such shared-stamp pairs qualify. Turn it off
//!   with [`LrcOptions::split_bilingual`].
//!
//! # TTML
//!
//! Apple-style lyrics TTML: `tt/body/div/p/span`. Attributes and elements are
//! matched by local name, so any namespace prefix works.
//!
//! * Times (`begin`, `end`, `dur`) accept clock time (`hh:mm:ss.fff`,
//!   `mm:ss.fff`, `hh:mm:ss:ff` with frames) and offset time (`12.3s`, `450ms`,
//!   `1.5m`, `1h`, `30f`, `100t`); a bare number is seconds (`12.345`). Times
//!   are absolute: like Apple's player, the parser does not add a `<div>`'s
//!   `begin` to the `<p>`s inside it.
//! * `itunes:timing="None"` drops all times, `"Line"` keeps only line times,
//!   `"Word"` or no attribute keeps whatever the spans provide. A `<p>` without
//!   times takes them from its timed spans.
//! * Whitespace between spans separates words; spans with no whitespace between
//!   them are parts of one word and set [`Word::joins_next`] (CJK, syllables).
//! * `ttm:agent` on `<p>` (or inherited from `<div>`/`<body>`) sets the agent;
//!   `<ttm:agent type="person|group|other" xml:id="…">` in the head defines it,
//!   with the first `<ttm:name>` as its name.
//! * `<span ttm:role="x-bg">` content becomes [`Line::background`] (one word
//!   spanning the line when it has no times of its own).
//! * `<span ttm:role="x-translation">` and `x-roman` inside a `<p>` become
//!   translation and romanization tracks, keyed by the span's `xml:lang`.
//!   Apple's `iTunesMetadata` `<translations>`/`<transliterations>` blocks are
//!   matched to lines through `<text for="…">` and the line's `itunes:key` (or
//!   `xml:id`). Background vocals inside these blocks are ignored.
//! * `xml:lang` on `<tt>` sets [`LyricDoc::language`]; without it the first
//!   `xml:lang` on `<body>`, `<div>` or `<p>` is used. `ttm:title` and Apple's
//!   songwriters go to the metadata as `title` and `songwriters`.
//! * No DTD is read and no external entity is ever resolved. Only the five
//!   predefined entities and character references are decoded; other entity
//!   references stay in the text as written.
//!
//! # WebVTT
//!
//! See [`parse_vtt`]: cues with optional identifiers, `<v Speaker>` voices as
//! agents (`v1`, `v2`, … in order of appearance, named after the speaker),
//! inline timestamps for word timing, `<c>` and other tags stripped, `NOTE`
//! blocks, cue settings, positioning and styling ignored.

mod build;
mod error;
mod lrc;
mod model;
mod plain;
mod text;
mod timeline;
mod ttml;
mod vtt;
mod words;

pub use error::LyricsError;
pub use lrc::{LrcOptions, parse_lrc, parse_lrc_with};
pub use model::{
    Agent, AgentKind, AuxKind, AuxLine, AuxTrack, Direction, Format, Line, LyricDoc, SourceInfo, Timing, Word,
};
pub use plain::parse_plain;
pub use timeline::{Cursor, LyricTimeline, NO_LINE};
pub use ttml::parse_ttml;
pub use vtt::parse_vtt;

/// Parses lyrics in the given format, or in the format [`sniff`] detects when `hint` is `None`.
///
/// [`Format::Lrc`] and [`Format::EnhancedLrc`] both go to [`parse_lrc`], which
/// reports the one it found. Plain text gets the same size limits as the other
/// formats here.
pub fn parse(input: &str, hint: Option<Format>) -> Result<LyricDoc, LyricsError> {
    match hint.unwrap_or_else(|| sniff(input)) {
        Format::Lrc | Format::EnhancedLrc => parse_lrc(input),
        Format::Ttml => parse_ttml(input),
        Format::WebVtt => parse_vtt(input),
        Format::Plain => {
            check_limits(input)?;
            Ok(parse_plain(input))
        }
    }
}

use text::{count_lines, split_lines, strip_bom};

/// Largest accepted input, in bytes (4 MiB).
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

/// Largest accepted number of input lines, and of lines in a parsed document.
pub const MAX_LINES: usize = 20_000;

/// Largest amount of line text that repeated LRC timestamps may expand to (16 MiB).
pub const MAX_EXPANDED_BYTES: usize = 4 * MAX_INPUT_BYTES;

/// Rejects input over [`MAX_INPUT_BYTES`] or [`MAX_LINES`] lines.
pub(crate) fn check_limits(input: &str) -> Result<(), LyricsError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(LyricsError::InputTooLarge {
            bytes: input.len(),
            limit: MAX_INPUT_BYTES,
        });
    }
    let lines = count_lines(input);
    if lines > MAX_LINES {
        return Err(LyricsError::TooManyLines {
            lines,
            limit: MAX_LINES,
        });
    }
    Ok(())
}

/// Guesses the format of `input`.
///
/// After an optional byte order mark and leading whitespace: `<?xml` or a
/// `<tt` root is TTML, `WEBVTT` is WebVTT, any line starting with an LRC
/// timestamp (`[` digits `:` digit) is LRC, and everything else is plain text.
/// Enhanced LRC is reported as [`Format::Lrc`]; [`parse_lrc`] tells them apart.
pub fn sniff(input: &str) -> Format {
    let s = strip_bom(input).trim_start();
    let tt_root = s
        .strip_prefix("<tt")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c.is_whitespace() || matches!(c, '>' | ':' | '/'));
    if s.starts_with("<?xml") || tt_root {
        return Format::Ttml;
    }
    if let Some(rest) = s.strip_prefix("WEBVTT")
        && rest
            .chars()
            .next()
            .is_none_or(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
    {
        return Format::WebVtt;
    }
    if split_lines(s).any(|line| starts_with_lrc_stamp(line.trim_start())) {
        return Format::Lrc;
    }
    Format::Plain
}

fn starts_with_lrc_stamp(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('[') else {
        return false;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0
        && rest[digits..]
            .strip_prefix(':')
            .is_some_and(|r| r.bytes().next().is_some_and(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_formats() {
        assert_eq!(sniff("\u{feff}<?xml version=\"1.0\"?><tt/>"), Format::Ttml);
        assert_eq!(sniff("  <tt xmlns=\"http://www.w3.org/ns/ttml\">"), Format::Ttml);
        assert_eq!(sniff("<tt:tt xmlns:tt=\"x\">"), Format::Ttml);
        assert_eq!(sniff("<ttx>"), Format::Plain);
        assert_eq!(sniff("WEBVTT\n\n00:01.000 --> 00:02.000\nhi"), Format::WebVtt);
        assert_eq!(sniff("WEBVTT - title"), Format::WebVtt);
        assert_eq!(sniff("WEBVTTX"), Format::Plain);
        assert_eq!(sniff("[ti:Title]\n[00:01.00]hi"), Format::Lrc);
        assert_eq!(sniff("[1:02]hi"), Format::Lrc);
        assert_eq!(sniff("[Chorus]\nla la"), Format::Plain);
        assert_eq!(sniff(""), Format::Plain);
    }

    #[test]
    fn parse_dispatches_on_the_sniffed_format() {
        let cases = [
            ("WEBVTT\n\n00:01.000 --> 00:02.000\nhi\n", Format::WebVtt),
            ("<tt><body><p begin=\"1s\">hi</p></body></tt>", Format::Ttml),
            ("[00:01.00]<00:01.00>hi", Format::EnhancedLrc),
            ("[00:01.00]hi", Format::Lrc),
            ("hi", Format::Plain),
        ];
        for (input, format) in cases {
            let doc = parse(input, None).unwrap();
            assert_eq!(doc.source.format, format, "{input:?}");
            assert_eq!(doc.lines[0].text, "hi");
        }
        assert!(parse("hi", Some(Format::Ttml)).is_err());
        let many = "x\n".repeat(MAX_LINES + 1);
        assert!(matches!(
            parse(&many, Some(Format::Plain)),
            Err(LyricsError::TooManyLines { .. })
        ));
    }

    #[test]
    fn rejects_large_input() {
        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        assert_eq!(
            check_limits(&big),
            Err(LyricsError::InputTooLarge {
                bytes: MAX_INPUT_BYTES + 1,
                limit: MAX_INPUT_BYTES
            })
        );
        assert!(parse_lrc(&big).is_err());
        let many = "\n".repeat(MAX_LINES + 1);
        assert_eq!(
            parse_lrc(&many),
            Err(LyricsError::TooManyLines {
                lines: MAX_LINES + 1,
                limit: MAX_LINES
            })
        );
        assert!(check_limits(&"\n".repeat(MAX_LINES)).is_ok());
    }

    #[test]
    fn error_messages_are_readable() {
        let e = LyricsError::TooManyLines {
            lines: 20_001,
            limit: 20_000,
        };
        assert_eq!(
            e.to_string(),
            "lyrics have 20001 lines, more than the limit of 20000 lines"
        );
    }
}

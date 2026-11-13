//! LRC, enhanced LRC (A2 word stamps, voice prefixes) and bilingual LRC.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::build::{DocBuilder, RawAux, RawLine};
use crate::error::LyricsError;
use crate::model::{AuxKind, Format, LyricDoc};
use crate::text::{Script, clean, dominant_script, split_lines, strip_bom};
use crate::words::{assemble, stamped_pieces};
use crate::{MAX_EXPANDED_BYTES, MAX_LINES, check_limits};

/// Options for [`parse_lrc_with`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LrcOptions {
    /// Detect bilingual pairs (two lines with the same stamp in different
    /// scripts) and move the second line of each pair to a translation track.
    /// On by default.
    pub split_bilingual: bool,
}

impl Default for LrcOptions {
    fn default() -> Self {
        Self {
            split_bilingual: true,
        }
    }
}

/// Parses plain or enhanced LRC with the default [`LrcOptions`].
///
/// See the crate documentation for the accepted syntax. Lines that are neither
/// a tag nor start with a valid timestamp are skipped, so one malformed line
/// never loses the rest of the file.
pub fn parse_lrc(input: &str) -> Result<LyricDoc, LyricsError> {
    parse_lrc_with(input, LrcOptions::default())
}

/// Parses plain or enhanced LRC.
pub fn parse_lrc_with(input: &str, options: LrcOptions) -> Result<LyricDoc, LyricsError> {
    check_limits(input)?;
    let input = strip_bom(input);
    let mut builder = DocBuilder::new(Format::Lrc);
    let mut sources: Vec<SourceLine<'_>> = Vec::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut expanded = 0usize;
    let mut offset: i32 = 0;
    let mut enhanced = false;

    for raw in split_lines(input) {
        match classify(raw.trim()) {
            Parsed::Tag(key, value) => {
                if key == "offset"
                    && let Some(v) = parse_offset(value)
                {
                    offset = v;
                }
                builder.metadata.insert(key, clean(value));
            }
            Parsed::Timed(stamps, text) => {
                let (voice, segments) = split_text(text);
                enhanced |= segments.len() > 1 || voice.is_some();
                let total = entries.len() + stamps.len();
                if total > MAX_LINES {
                    return Err(LyricsError::TooManyLines {
                        lines: total,
                        limit: MAX_LINES,
                    });
                }
                expanded = expanded.saturating_add(text.len().saturating_mul(stamps.len()));
                if expanded > MAX_EXPANDED_BYTES {
                    return Err(LyricsError::ExpandedTooLarge {
                        limit: MAX_EXPANDED_BYTES,
                    });
                }
                let source = sources.len();
                sources.push(SourceLine {
                    voice,
                    segments,
                    reference: stamps.iter().copied().min().unwrap_or(0),
                });
                entries.extend(stamps.into_iter().map(|stamp| Entry { stamp, source }));
            }
            Parsed::Skip => {}
        }
    }

    // Positive offsets make lyrics appear earlier (the de-facto LRC convention).
    let apply = |t: u32| (i64::from(t) - i64::from(offset)).clamp(0, i64::from(u32::MAX)) as u32;
    for entry in &entries {
        let source = &sources[entry.source];
        let start = apply(entry.stamp);
        let mut line = RawLine {
            start: Some(start),
            agent: source.voice.as_deref().map(|v| builder.agent(v)),
            ..RawLine::default()
        };
        if source.segments.len() > 1 {
            // Word stamps are written for the first occurrence of a repeated
            // line; later occurrences keep the same timing relative to the line.
            let delta = entry.stamp - source.reference;
            let segments: Vec<(Option<u32>, &str)> = source
                .segments
                .iter()
                .map(|&(s, t)| (s.map(|s| apply(s.saturating_add(delta))), t))
                .collect();
            // A stamp after the last word ends it, and the line.
            line.end = match segments.last() {
                Some(&(Some(t), text)) if text.trim().is_empty() => Some(t),
                _ => None,
            };
            let assembled = assemble(&stamped_pieces(start, &segments, None));
            line.text = assembled.text;
            line.words = assembled.words;
        } else {
            line.text = source.segments[0].1.to_owned();
        }
        builder.lines.push(line);
    }

    builder.offset_ms = offset;
    if enhanced {
        builder.format = Format::EnhancedLrc;
    }
    if options.split_bilingual {
        // Group by the stamps as written: after the offset, distinct stamps
        // clamped to 0 would look like a shared one.
        let stamps: Vec<u32> = entries.iter().map(|e| e.stamp).collect();
        split_bilingual(&mut builder, &stamps);
    }
    Ok(builder.finish())
}

/// The text part of a timed source line, shared by all its timestamps.
#[derive(Debug)]
struct SourceLine<'a> {
    voice: Option<String>,
    /// Text split at word stamps; the first segment has no stamp of its own.
    segments: Vec<(Option<u32>, &'a str)>,
    /// Earliest line stamp: word stamps are relative to it.
    reference: u32,
}

/// One timestamp of a source line.
#[derive(Debug)]
struct Entry {
    stamp: u32,
    source: usize,
}

#[derive(Debug, PartialEq)]
enum Parsed<'a> {
    Tag(String, &'a str),
    Timed(Vec<u32>, &'a str),
    Skip,
}

/// Classifies one trimmed source line.
fn classify(line: &str) -> Parsed<'_> {
    let mut stamps = Vec::new();
    let mut rest = line;
    while let Some(after) = rest.strip_prefix('[') {
        let Some(close) = after.find(']') else { break };
        let Some(ms) = parse_stamp(&after[..close]) else {
            break;
        };
        stamps.push(ms);
        rest = &after[close + 1..];
    }
    if !stamps.is_empty() {
        return Parsed::Timed(stamps, rest);
    }
    if let Some(after) = line.strip_prefix('[')
        && let Some(close) = after.rfind(']')
        && let Some((key, value)) = after[..close].split_once(':')
        && is_tag_key(key.trim())
    {
        return Parsed::Tag(key.trim().to_ascii_lowercase(), value.trim());
    }
    Parsed::Skip
}

/// Tag keys: `#` or an ASCII word starting with a letter (`ti`, `ar`, `offset`, `ve`, …).
fn is_tag_key(key: &str) -> bool {
    key == "#"
        || (key.len() <= 32
            && key.starts_with(|c: char| c.is_ascii_alphabetic())
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
}

fn is_digits(s: &str, min_len: usize, max_len: usize) -> bool {
    (min_len..=max_len).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}

/// Parses the inside of an LRC timestamp: `mm:ss`, `mm:ss.x`, `mm:ss.xx`,
/// `mm:ss.xxx`, or the same with `:` before the fraction. Minutes may have
/// up to five digits, seconds one or two (below 60). Returns milliseconds.
pub(crate) fn parse_stamp(s: &str) -> Option<u32> {
    let s = s.trim();
    let (min, rest) = s.split_once(':')?;
    let (sec, frac) = match rest.find(['.', ':']) {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    if !is_digits(min, 1, 5) || !is_digits(sec, 1, 2) {
        return None;
    }
    let min: u32 = min.parse().ok()?;
    let sec: u32 = sec.parse().ok()?;
    if sec >= 60 {
        return None;
    }
    let frac_ms = match frac {
        None => 0,
        Some(f) if is_digits(f, 1, 3) => f.parse::<u32>().ok()? * 10u32.pow(3 - f.len() as u32),
        Some(_) => return None,
    };
    min.checked_mul(60_000)?.checked_add(sec * 1000 + frac_ms)
}

/// Parses an `[offset:]` value: an optionally signed integer of milliseconds.
fn parse_offset(value: &str) -> Option<i32> {
    let v = value.trim();
    let digits = v.strip_prefix(['+', '-']).unwrap_or(v);
    if !is_digits(digits, 1, 9) {
        return None;
    }
    v.parse().ok()
}

/// Splits the text after the line stamps into an optional voice prefix and
/// segments separated by enhanced-LRC word stamps `<mm:ss.xx>`.
fn split_text(text: &str) -> (Option<String>, Vec<(Option<u32>, &str)>) {
    let mut body = text;
    let mut voice = None;
    if let Some((id, rest)) = voice_prefix(text.trim_start()) {
        voice = Some(id);
        body = rest;
    }
    let mut segments = Vec::new();
    let mut stamp = None;
    let mut seg_start = 0;
    let mut search = 0;
    while let Some(open) = body[search..].find('<').map(|i| i + search) {
        let Some(close) = body[open..].find('>').map(|i| i + open) else {
            break;
        };
        match parse_stamp(&body[open + 1..close]) {
            Some(ms) => {
                segments.push((stamp, &body[seg_start..open]));
                stamp = Some(ms);
                seg_start = close + 1;
                search = close + 1;
            }
            None => search = open + 1,
        }
    }
    segments.push((stamp, &body[seg_start..]));
    (voice, segments)
}

/// A voice prefix `v1:` … `v9999:` (case-insensitive) at the start of the text.
fn voice_prefix(s: &str) -> Option<(String, &str)> {
    let rest = s.strip_prefix(['v', 'V'])?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if !(1..=4).contains(&digits) {
        return None;
    }
    let after = rest[digits..].strip_prefix(':')?;
    Some((format!("v{}", &rest[..digits]), after))
}

/// Moves the second line of bilingual pairs to a translation track.
///
/// A *pair* is a timestamp shared by exactly two non-blank lines. A pair
/// *qualifies* when one line is mostly Latin and the other mostly CJK (see
/// [`dominant_script`]); the line that comes second in the file becomes the
/// translation of the first. Nothing is moved unless at least half of all
/// pairs qualify, so a monolingual file that happens to repeat a stamp, or a
/// duet with simultaneous lines, is left alone. The translation track has no
/// language: the file does not say which one it is.
///
/// `stamps[i]` is the timestamp of `builder.lines[i]` as written in the file.
fn split_bilingual(builder: &mut DocBuilder, stamps: &[u32]) {
    let mut groups: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, &stamp) in stamps.iter().enumerate() {
        groups.entry(stamp).or_default().push(i);
    }
    let lines = &builder.lines;
    let blank = |i: usize| lines[i].text.trim().is_empty();
    let pairs: Vec<(usize, usize)> = groups
        .values()
        .filter(|g| g.len() == 2 && !blank(g[0]) && !blank(g[1]))
        .map(|g| (g[0], g[1]))
        .collect();
    let qualifying: Vec<(usize, usize)> = pairs
        .iter()
        .copied()
        .filter(|&(a, b)| {
            matches!(
                (dominant_script(&lines[a].text), dominant_script(&lines[b].text)),
                (Some(Script::Latin), Some(Script::Cjk)) | (Some(Script::Cjk), Some(Script::Latin))
            )
        })
        .collect();
    if qualifying.is_empty() || qualifying.len() * 2 < pairs.len() {
        return;
    }
    let track = builder.track(AuxKind::Translation, None);
    let mut moved = vec![false; builder.lines.len()];
    for (a, b) in qualifying {
        let translation = std::mem::take(&mut builder.lines[b]);
        builder.lines[a].aux.push(RawAux {
            track,
            text: translation.text,
            words: translation.words,
        });
        moved[b] = true;
    }
    let mut index = 0;
    builder.lines.retain(|_| {
        index += 1;
        !moved[index - 1]
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Timing;

    #[test]
    fn stamp_variants() {
        assert_eq!(parse_stamp("01:02"), Some(62_000));
        assert_eq!(parse_stamp("01:02.5"), Some(62_500));
        assert_eq!(parse_stamp("01:02.50"), Some(62_500));
        assert_eq!(parse_stamp("01:02.507"), Some(62_507));
        assert_eq!(parse_stamp("01:02:50"), Some(62_500));
        assert_eq!(parse_stamp("1:2.05"), Some(62_050));
        assert_eq!(parse_stamp(" 00:00.00 "), Some(0));
        assert_eq!(parse_stamp("123:00.00"), Some(7_380_000));
    }

    #[test]
    fn invalid_stamps() {
        for s in [
            "",
            "1",
            "aa:bb",
            "00:60.00",
            "00:1x.00",
            "00:01.",
            "00:01.1234",
            "-1:00.00",
            "00:01.0.0",
            "+1:00",
            "00:+1",
            "999999:00",
            "ti:Title",
        ] {
            assert_eq!(parse_stamp(s), None, "{s:?}");
        }
    }

    #[test]
    fn offset_values() {
        assert_eq!(parse_offset("+500"), Some(500));
        assert_eq!(parse_offset("-250"), Some(-250));
        assert_eq!(parse_offset(" 40 "), Some(40));
        assert_eq!(parse_offset("abc"), None);
        assert_eq!(parse_offset("+"), None);
        assert_eq!(parse_offset("1.5"), None);
    }

    #[test]
    fn classify_lines() {
        assert_eq!(classify("[ti: A Title ]"), Parsed::Tag("ti".into(), "A Title"));
        assert_eq!(classify("[TI:x]"), Parsed::Tag("ti".into(), "x"));
        assert_eq!(
            classify("[ti:Song [Live]]"),
            Parsed::Tag("ti".into(), "Song [Live]")
        );
        assert_eq!(
            classify("[00:01.00][00:02.00]hi"),
            Parsed::Timed(vec![1000, 2000], "hi")
        );
        assert_eq!(
            classify("[00:01.00][Chorus] la"),
            Parsed::Timed(vec![1000], "[Chorus] la")
        );
        assert_eq!(classify("[00:01.00]"), Parsed::Timed(vec![1000], ""));
        assert_eq!(classify("[00:xx.00]bad"), Parsed::Skip);
        assert_eq!(classify("just text"), Parsed::Skip);
        assert_eq!(classify("[Chorus]"), Parsed::Skip);
    }

    #[test]
    fn voice_prefixes() {
        assert_eq!(voice_prefix("v1: hi"), Some(("v1".into(), " hi")));
        assert_eq!(voice_prefix("V12:hi"), Some(("v12".into(), "hi")));
        assert_eq!(voice_prefix("very: hi"), None);
        assert_eq!(voice_prefix("v: hi"), None);
    }

    #[test]
    fn enhanced_words_and_trailing_end() {
        let doc =
            parse_lrc("[00:10.00]<00:10.00>Hel<00:10.40>lo <00:11.00>you<00:12.00>\n[00:20.00]next").unwrap();
        assert_eq!(doc.source.format, Format::EnhancedLrc);
        assert_eq!(doc.timing, Timing::Word);
        let line = &doc.lines[0];
        assert_eq!(line.text, "Hello you");
        assert_eq!(line.end_ms, Some(12_000));
        let words: Vec<_> = line
            .words
            .iter()
            .map(|w| (w.text.as_str(), w.start_ms, w.end_ms, w.joins_next))
            .collect();
        assert_eq!(
            words,
            [
                ("Hel", 10_000, 10_400, true),
                ("lo", 10_400, 11_000, false),
                ("you", 11_000, 12_000, false)
            ]
        );
    }

    #[test]
    fn repeated_stamps_shift_word_times() {
        let doc = parse_lrc("[00:30.00][00:10.00]<00:10.00>la <00:10.50>la").unwrap();
        let starts: Vec<Vec<u32>> = doc
            .lines
            .iter()
            .map(|l| l.words.iter().map(|w| w.start_ms).collect())
            .collect();
        assert_eq!(starts, [vec![10_000, 10_500], vec![30_000, 30_500]]);
        // The last word of the first occurrence ends where the next line starts.
        assert_eq!(doc.lines[0].words[1].end_ms, 30_000);
    }

    #[test]
    fn bilingual_needs_most_pairs_to_qualify() {
        // Two pairs, only one qualifies: half of the pairs is enough.
        let doc = parse_lrc("[00:01.00]Lanterns\n[00:01.00]灯笼\n[00:02.00]Hello\n[00:02.00]Hi").unwrap();
        assert_eq!(doc.lines.len(), 3);
        assert_eq!(doc.tracks[0].lines[&0].text, "灯笼");
        // Three pairs, one qualifies: nothing moves.
        let doc = parse_lrc("[00:01.00]A\n[00:01.00]灯\n[00:02.00]B\n[00:02.00]C\n[00:03.00]D\n[00:03.00]E")
            .unwrap();
        assert_eq!(doc.lines.len(), 6);
        assert!(doc.tracks.is_empty());
        // Stamps that only coincide after clamping at 0 are not a pair.
        let doc = parse_lrc("[offset:+5000]\n[00:01.00]Lanterns\n[00:02.00]灯笼").unwrap();
        assert_eq!(doc.lines.len(), 2);
        assert!(doc.tracks.is_empty());
        // The option turns it off.
        let doc = parse_lrc_with(
            "[00:01.00]Lanterns\n[00:01.00]灯笼",
            LrcOptions {
                split_bilingual: false,
            },
        )
        .unwrap();
        assert_eq!(doc.lines.len(), 2);
    }

    #[test]
    fn repeated_stamps_cannot_explode() {
        let stamps = "[00:00.00]".repeat(30_000);
        let err = parse_lrc(&format!("{stamps}la")).unwrap_err();
        assert!(matches!(err, LyricsError::TooManyLines { .. }), "{err}");
        let text = "x".repeat(3_000_000);
        let err = parse_lrc(&format!("{}{text}", "[00:00.00]".repeat(10))).unwrap_err();
        assert_eq!(
            err,
            LyricsError::ExpandedTooLarge {
                limit: MAX_EXPANDED_BYTES
            }
        );
    }
}

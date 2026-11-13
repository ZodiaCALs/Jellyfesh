//! WebVTT: cues, `<v Speaker>` voices and inline timestamps for word timing.

use std::borrow::Cow;

use crate::build::{DocBuilder, RawLine};
use crate::check_limits;
use crate::error::LyricsError;
use crate::model::{Agent, AgentKind, Format, LyricDoc};
use crate::text::{clean, split_lines, strip_bom};
use crate::words::{assemble, stamped_pieces};

/// Parses a WebVTT file.
///
/// The first line must be `WEBVTT`, optionally followed by a space or tab and
/// free text. Header lines of the form `Key: value` before the first blank
/// line go to the metadata, and `Language:` also sets the document language.
/// `NOTE`, `STYLE` and `REGION` blocks are ignored, as are cue identifiers and
/// cue settings. A block whose timing line does not parse is skipped, as the
/// WebVTT parsing rules require.
///
/// In cue text, `<v Speaker>` sets the line's agent (the first voice of a cue
/// wins), inline timestamps `<00:00:01.000>` start words, ruby text (`<rt>`)
/// is dropped, other tags such as `<c>`, `<i>` and `<b>` are removed with their
/// text kept, and character references (`&amp;`, `&lt;`, `&nbsp;`, `&#…;`) are
/// decoded. The lines of a multi-line cue are joined with a space.
pub fn parse_vtt(input: &str) -> Result<LyricDoc, LyricsError> {
    check_limits(input)?;
    let lines: Vec<&str> = split_lines(strip_bom(input)).collect();
    let header = lines.first().copied().unwrap_or("");
    let valid_header = header
        .strip_prefix("WEBVTT")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t']));
    if !valid_header {
        return Err(LyricsError::MissingVttHeader);
    }

    let mut builder = DocBuilder::new(Format::WebVtt);
    let mut i = 1;
    while i < lines.len() && !lines[i].trim().is_empty() && !lines[i].contains("-->") {
        if let Some((key, value)) = lines[i].split_once(':') {
            let key = key.trim().to_ascii_lowercase();
            let value = clean(value);
            if key == "language" && !value.is_empty() {
                builder.language = Some(value.clone());
            }
            if !key.is_empty() {
                builder.metadata.insert(key, value);
            }
        }
        i += 1;
    }

    let mut voices: Vec<String> = Vec::new();
    while i < lines.len() {
        if lines[i].trim().is_empty() {
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && !lines[i].trim().is_empty() {
            i += 1;
        }
        if let Some(line) = parse_block(&lines[start..i], &mut voices, &mut builder) {
            builder.lines.push(line);
        }
    }
    Ok(builder.finish())
}

/// Parses one block; `None` for comments, style and region blocks and malformed cues.
fn parse_block(block: &[&str], voices: &mut Vec<String>, builder: &mut DocBuilder) -> Option<RawLine> {
    let first = block[0];
    let keyword = |word: &str| {
        first
            .strip_prefix(word)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t']))
    };
    if keyword("NOTE") || keyword("STYLE") || keyword("REGION") {
        return None;
    }
    let timing = if first.contains("-->") {
        0
    } else if block.len() > 1 && block[1].contains("-->") {
        1
    } else {
        return None;
    };
    let (start, end) = parse_timing(block[timing])?;
    let payload = block[timing + 1..].join(" ");
    let cue = parse_cue_text(&payload);

    let agent = cue.voice.map(|name| {
        let index = match voices.iter().position(|v| *v == name) {
            Some(index) => index,
            None => {
                voices.push(name.clone());
                builder.agents.push(Agent {
                    id: format!("v{}", voices.len()),
                    name: Some(name),
                    kind: AgentKind::Person,
                });
                voices.len() - 1
            }
        };
        format!("v{}", index + 1)
    });
    let mut line = RawLine {
        start: Some(start),
        end: Some(end),
        agent,
        ..RawLine::default()
    };
    if cue.segments.len() > 1 {
        let assembled = assemble(&stamped_pieces(start, &cue.segments, Some(end)));
        line.text = assembled.text;
        line.words = assembled.words;
    } else {
        line.text = cue
            .segments
            .into_iter()
            .next()
            .map(|(_, text)| text)
            .unwrap_or_default();
    }
    Some(line)
}

/// Parses `start --> end [settings]`; settings are ignored.
fn parse_timing(line: &str) -> Option<(u32, u32)> {
    let (left, right) = line.split_once("-->")?;
    let start = parse_vtt_time(left.trim())?;
    let end = parse_vtt_time(right.split_whitespace().next()?)?;
    Some((start, end))
}

fn digits(s: &str, len: std::ops::RangeInclusive<usize>) -> Option<u64> {
    if len.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

/// Parses a WebVTT timestamp `hh:mm:ss.ttt` or `mm:ss.ttt` into milliseconds.
/// Minutes and seconds have two digits (below 60), the fraction three.
pub(crate) fn parse_vtt_time(s: &str) -> Option<u32> {
    let (clock, fraction) = s.split_once('.')?;
    let fraction = digits(fraction, 3..=3)?;
    let (hours, minutes, seconds) = match clock.split(':').collect::<Vec<_>>().as_slice() {
        [m, s] => (0, digits(m, 2..=2)?, digits(s, 2..=2)?),
        [h, m, s] => (digits(h, 1..=6)?, digits(m, 2..=2)?, digits(s, 2..=2)?),
        _ => return None,
    };
    if minutes >= 60 || seconds >= 60 {
        return None;
    }
    let ms = ((hours * 60 + minutes) * 60 + seconds) * 1000 + fraction;
    u32::try_from(ms).ok()
}

#[derive(Debug, PartialEq)]
struct CueText {
    voice: Option<String>,
    /// Text split at inline timestamps; the first segment has no stamp.
    segments: Vec<(Option<u32>, String)>,
}

fn parse_cue_text(text: &str) -> CueText {
    let mut voice = None;
    let mut segments = vec![(None, String::new())];
    let mut ruby_text = 0u32;
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('<')
            && let Some(close) = after.find('>')
        {
            let tag = &after[..close];
            rest = &after[close + 1..];
            if let Some(ms) = parse_vtt_time(tag.trim()) {
                segments.push((Some(ms), String::new()));
            } else if let Some(closing) = tag.strip_prefix('/') {
                if tag_name(closing) == "rt" {
                    ruby_text = ruby_text.saturating_sub(1);
                }
            } else {
                match tag_name(tag) {
                    "v" if voice.is_none() => {
                        voice = tag
                            .split_once([' ', '\t'])
                            .map(|(_, annotation)| clean(annotation))
                            .filter(|name| !name.is_empty());
                    }
                    "rt" => ruby_text += 1,
                    _ => {}
                }
            }
            continue;
        }
        // Text up to the next tag; a `<` without a closing `>` is kept as text.
        let skip = usize::from(rest.starts_with('<'));
        let next = rest[skip..].find('<').map_or(rest.len(), |i| i + skip);
        if ruby_text == 0
            && let Some((_, segment)) = segments.last_mut()
        {
            segment.push_str(&decode_references(&rest[..next]));
        }
        rest = &rest[next..];
    }
    CueText { voice, segments }
}

/// The tag name: up to the first class dot or whitespace.
fn tag_name(tag: &str) -> &str {
    tag.split(|c: char| c == '.' || c.is_whitespace())
        .next()
        .unwrap_or("")
}

/// Decodes the character references WebVTT text uses; unknown ones stay as written.
fn decode_references(s: &str) -> Cow<'_, str> {
    if !s.contains('&') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let decoded = after
            .find(';')
            .filter(|&semi| semi <= 32)
            .and_then(|semi| reference(&after[..semi]).map(|c| (c, semi)));
        match decoded {
            Some((c, semi)) => {
                out.push(c);
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

fn reference(name: &str) -> Option<char> {
    let code = match name {
        "amp" => return Some('&'),
        "lt" => return Some('<'),
        "gt" => return Some('>'),
        "quot" => return Some('"'),
        "apos" => return Some('\''),
        "nbsp" => return Some('\u{a0}'),
        "lrm" => return Some('\u{200e}'),
        "rlm" => return Some('\u{200f}'),
        _ => match name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => name.strip_prefix('#')?.parse().ok()?,
        },
    };
    char::from_u32(code).filter(|&c| c != '\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(parse_vtt_time("00:01.000"), Some(1_000));
        assert_eq!(parse_vtt_time("01:02:03.004"), Some(3_723_004));
        assert_eq!(parse_vtt_time("100:00:00.000"), Some(360_000_000));
        for s in [
            "1:02.000",
            "00:01.00",
            "00:60.000",
            "00:01,000",
            "aa:01.000",
            "00:01.0000",
            "1:2:3.000",
        ] {
            assert_eq!(parse_vtt_time(s), None, "{s:?}");
        }
        assert_eq!(
            parse_timing("00:01.000 --> 00:02.500 align:start line:0"),
            Some((1_000, 2_500))
        );
        assert_eq!(parse_timing("00:01.000 -> 00:02.500"), None);
    }

    #[test]
    fn cue_text_tags_and_references() {
        let cue = parse_cue_text("<v.loud Ana Lee><c.red>Rock</c> &amp; <i>roll</i> &#x263A; &bogus; 1 < 2");
        assert_eq!(cue.voice.as_deref(), Some("Ana Lee"));
        assert_eq!(
            cue.segments,
            [(None, "Rock & roll \u{263A} &bogus; 1 < 2".to_owned())]
        );
        let cue = parse_cue_text("<ruby>空<rt>そら</rt></ruby><00:00:01.500>だ");
        assert_eq!(
            cue.segments,
            [(None, "空".to_owned()), (Some(1_500), "だ".to_owned())]
        );
    }

    #[test]
    fn header_is_required() {
        assert_eq!(
            parse_vtt("00:01.000 --> 00:02.000\nhi"),
            Err(LyricsError::MissingVttHeader)
        );
        assert_eq!(parse_vtt("WEBVTTX\n"), Err(LyricsError::MissingVttHeader));
        assert!(parse_vtt("\u{feff}WEBVTT\tKaraoke\n").unwrap().lines.is_empty());
    }
}

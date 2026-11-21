//! NetEase's word-timed lyrics ("YRC") as enhanced LRC.
//!
//! NetEase sends word timing in a format of its own, which the core does not
//! read, so the plugin converts it (ADR-0022 §6). A YRC line is a line stamp
//! `[start,duration]` followed by words, each `(start,duration,0)text`, all
//! in milliseconds from the start of the song:
//!
//! ```text
//! [16210,3460](16210,670,0)还(16880,410,0)没(17290,430,0)好
//! [20500,2200](20500,400,0)Hold (20900,600,0)on
//! ```
//!
//! becomes, in the enhanced LRC the core reads ("Enhanced LRC (A2)"):
//!
//! ```text
//! [00:16.21]<00:16.21>还<00:16.88>没<00:17.29>好<00:17.72>
//! [00:20.50]<00:20.50>Hold <00:20.90>on<00:21.50>
//! ```
//!
//! A stamp before each word starts it; the stamp after the last word ends it
//! and the line. Spaces stay where the words had them (CJK characters come
//! without). Credit lines come as JSON, as in NetEase's LRC, and become
//! plain timed lines. A translation (LRC) goes under the line it translates,
//! with the same stamp, which the core reads as a translation. Word stamps
//! that start more than a second before their line are read as relative to
//! it (the older karaoke form). Pieces that do not parse are left out, never
//! guessed at.

use jf_community_common::lrc;
use serde::Deserialize;

/// A translation line belongs to a lyric line that starts this close to it.
const TRANSLATION_SLACK_MS: u64 = 1000;
/// Word stamps this much before their line are relative to it.
const RELATIVE_SLACK_MS: u64 = 1000;

/// One timed word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub start_ms: u64,
    pub end_ms: u64,
    /// The word, trimmed, with no stamp-like characters.
    pub text: String,
    /// Whether a space separates it from the next word.
    pub space_after: bool,
}

/// One line: timed words, or (for credits) plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub start_ms: u64,
    pub words: Vec<Word>,
    /// The text of a line without word times.
    pub text: String,
}

impl Line {
    fn is_sung(&self) -> bool {
        !self.words.is_empty()
    }

    fn sung_text(&self) -> String {
        let mut text = String::new();
        for word in &self.words {
            text.push_str(&word.text);
            if word.space_after {
                text.push(' ');
            }
        }
        text.trim_end().to_owned()
    }
}

/// One JSON credit line: `{"t":0,"c":[{"tx":"作词: "},{"tx":"Someone"}]}`.
#[derive(Debug, Deserialize)]
struct CreditLine {
    t: u64,
    #[serde(default)]
    c: Vec<CreditPiece>,
}

#[derive(Debug, Deserialize)]
struct CreditPiece {
    #[serde(default)]
    tx: String,
}

/// Text that cannot be read as a stamp, on one line, spaces collapsed.
fn clean(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('<', "‹")
        .replace('>', "›")
}

/// Two or three integers in `open … close` at the start of `s`:
/// `[16210,3460]`, `(16210,670,0)`. The first two are kept.
fn stamp(s: &str, open: char, close: char) -> Option<(u64, u64, usize)> {
    let inner = s.strip_prefix(open)?;
    let end = inner.find(close).filter(|e| *e <= 40)?;
    let parts: Vec<&str> = inner[..end].split(',').map(str::trim).collect();
    let number = |p: &str| -> Option<u64> {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        p.parse().ok()
    };
    let (start, duration) = match parts.as_slice() {
        [a, b] => (number(a)?, number(b)?),
        [a, b, c]
            if c.strip_prefix('-')
                .unwrap_or(c)
                .bytes()
                .all(|b| b.is_ascii_digit()) =>
        {
            (number(a)?, number(b)?)
        }
        _ => return None,
    };
    Some((start, duration, open.len_utf8() + end + close.len_utf8()))
}

/// The next word stamp in `s`: where it is, its times and its length.
fn next_word_stamp(s: &str) -> Option<(usize, u64, u64, usize)> {
    s.match_indices('(').find_map(|(at, _)| {
        stamp(&s[at..], '(', ')').map(|(start, duration, len)| (at, start, duration, len))
    })
}

/// The words of one YRC line body (what follows the line stamp).
fn words(body: &str, line_start: u64) -> (Vec<Word>, String) {
    // (start, duration, raw text) of each stamped piece.
    let mut pieces: Vec<(u64, u64, String)> = Vec::new();
    let mut leading = String::new();
    let mut rest = body;
    loop {
        let found = next_word_stamp(rest);
        // Text up to the next stamp belongs to the word before it.
        let before = found.map_or(rest, |(at, ..)| &rest[..at]);
        match pieces.last_mut() {
            Some(last) => last.2.push_str(before),
            None => leading.push_str(before),
        }
        let Some((at, start, duration, len)) = found else {
            break;
        };
        pieces.push((start, duration, String::new()));
        rest = &rest[at + len..];
    }
    if let Some(first) = pieces.first_mut()
        && !leading.trim().is_empty()
    {
        first.2.insert_str(0, &leading);
        leading.clear();
    }
    let relative = pieces
        .first()
        .is_some_and(|(start, ..)| start + RELATIVE_SLACK_MS < line_start);
    let mut out: Vec<Word> = Vec::new();
    for (start, duration, raw) in pieces {
        let start = if relative { start + line_start } else { start };
        let text = clean(&raw);
        if raw.starts_with(char::is_whitespace)
            && let Some(previous) = out.last_mut()
        {
            previous.space_after = true;
        }
        if text.is_empty() {
            continue;
        }
        // Words never start before the word before them.
        let start = out.last().map_or(start, |w| start.max(w.start_ms));
        out.push(Word {
            start_ms: start,
            end_ms: start + duration,
            text,
            space_after: raw.ends_with(char::is_whitespace),
        });
    }
    if let Some(last) = out.last_mut() {
        last.space_after = false;
    }
    (out, clean(&leading))
}

/// Reads YRC text. Lines that are neither stamped lines nor JSON credits
/// are skipped.
pub fn parse(text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r').trim();
        if line.starts_with('{') {
            if let Ok(credit) = serde_json::from_str::<CreditLine>(line) {
                let text: String = credit.c.iter().map(|p| p.tx.as_str()).collect();
                let text = clean(&text);
                if !text.is_empty() {
                    lines.push(Line {
                        start_ms: credit.t,
                        words: Vec::new(),
                        text,
                    });
                }
            }
            continue;
        }
        let Some((start, _, len)) = stamp(line, '[', ']') else {
            continue;
        };
        let (words, text) = words(&line[len..], start);
        let start = words.first().map_or(start, |w| w.start_ms.min(start));
        if words.is_empty() && text.is_empty() {
            continue;
        }
        lines.push(Line {
            start_ms: start,
            words,
            text,
        });
    }
    // In time order; lines with the same start keep theirs.
    lines.sort_by_key(|l| l.start_ms);
    lines
}

/// For each line, the translation line that starts closest to it (within a
/// second), each used once. Empty lines, `//` (NetEase's "no translation
/// here") and lines that repeat the original are left out.
fn translations(lines: &[Line], translation: &lrc::Lrc) -> Vec<Option<String>> {
    let mut used = vec![false; translation.lines.len()];
    lines
        .iter()
        .map(|line| {
            if !line.is_sung() {
                return None;
            }
            let (i, found) = translation
                .lines
                .iter()
                .enumerate()
                .filter(|(i, t)| !used[*i] && t.time_ms.abs_diff(line.start_ms) <= TRANSLATION_SLACK_MS)
                .min_by_key(|(_, t)| t.time_ms.abs_diff(line.start_ms))?;
            used[i] = true;
            let text = clean(&found.text);
            (!text.is_empty() && text != "//" && text != line.sung_text()).then_some(text)
        })
        .collect()
}

/// The lines as enhanced LRC, with the translation's lines under theirs.
pub fn render(lines: &[Line], translation: Option<&lrc::Lrc>) -> String {
    let translated = translation.map_or_else(|| vec![None; lines.len()], |t| translations(lines, t));
    let mut out = String::new();
    for (line, translated) in lines.iter().zip(translated) {
        let stamp = lrc::format_time(line.start_ms);
        out.push_str(&format!("[{stamp}]"));
        if line.is_sung() {
            for word in &line.words {
                out.push_str(&format!("<{}>{}", lrc::format_time(word.start_ms), word.text));
                if word.space_after {
                    out.push(' ');
                }
            }
            let end = line.words.iter().map(|w| w.end_ms).max().unwrap_or(line.start_ms);
            out.push_str(&format!("<{}>", lrc::format_time(end)));
        } else {
            out.push_str(&line.text);
        }
        out.push('\n');
        if let Some(text) = translated {
            out.push_str(&format!("[{stamp}]{text}\n"));
        }
    }
    out
}

/// Enhanced LRC from YRC and an optional translation in LRC, or `None` when
/// the YRC times no word.
pub fn to_elrc(yrc: &str, translation: Option<&str>) -> Option<String> {
    let lines = parse(yrc);
    if !lines.iter().any(Line::is_sung) {
        return None;
    }
    let translation = translation.map(lrc::parse).filter(lrc::Lrc::is_synced);
    Some(render(&lines, translation.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_take_two_or_three_numbers() {
        assert_eq!(stamp("[16210,3460]x", '[', ']'), Some((16210, 3460, 12)));
        assert_eq!(stamp("(1,2,0)x", '(', ')'), Some((1, 2, 7)));
        assert_eq!(stamp("( 1 , 2 , -1 )", '(', ')'), Some((1, 2, 14)));
        assert_eq!(stamp("(feat. Someone)", '(', ')'), None);
        assert_eq!(stamp("(1,2,3,4)", '(', ')'), None);
        assert_eq!(stamp("[ti:Title]", '[', ']'), None);
        assert_eq!(stamp("[00:12.00]", '[', ']'), None);
        assert_eq!(stamp("(1,2", '(', ')'), None);
    }

    #[test]
    fn words_keep_their_spaces_and_order() {
        let lines = parse("[1000,2000](1000,300,0)Hold (1300,200,0)on (1500,100,0) (1600,900,0)(tight)ly\n");
        let words: Vec<(&str, u64, u64, bool)> = lines[0]
            .words
            .iter()
            .map(|w| (w.text.as_str(), w.start_ms, w.end_ms, w.space_after))
            .collect();
        assert_eq!(
            words,
            [
                ("Hold", 1000, 1300, true),
                ("on", 1300, 1500, true),
                ("(tight)ly", 1600, 2500, false)
            ]
        );
        assert_eq!(lines[0].sung_text(), "Hold on (tight)ly");
    }

    #[test]
    fn relative_word_stamps_are_moved_to_the_line() {
        let lines = parse("[60000,1000](0,400,0)Mo(400,600,0)ment\n");
        assert_eq!(lines[0].words[0].start_ms, 60_000);
        assert_eq!(lines[0].words[1].end_ms, 61_000);
    }

    #[test]
    fn nothing_timed_is_none() {
        assert_eq!(to_elrc("", None), None);
        assert_eq!(
            to_elrc("{\"t\":0,\"c\":[{\"tx\":\"作词: Someone\"}]}\n", None),
            None
        );
        assert_eq!(to_elrc("[00:01.00]An LRC line\n", None), None);
    }
}

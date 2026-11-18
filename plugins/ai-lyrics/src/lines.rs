//! Lyrics from recognised words alone, for songs without plain lyrics
//! (ADR-0019 §6): a line per recognised segment, split at pauses over
//! 0.8 s and after about ten words (sixteen CJK characters), sooner at a
//! comma or a full stop once a line has six.
//!
//! Also the words of a transcript as the plugin uses them everywhere:
//! annotations such as `[Music]` and `(applause)` and marks without letters
//! (`♪`) left out, and segments without word times given words spread over
//! their time by length.

use crate::model::{Line, Segment, TimedWord, Transcript, Word, finish};
use crate::text::{self, is_cjk};

/// A pause longer than this ends a line.
const PAUSE_MS: u32 = 800;
/// A line ends after this many words (a CJK character is 10/16 of one).
const FULL_LINE: f32 = 10.0;
/// From this many words on, a line ends at a comma or a full stop.
const CLAUSE_LINE: f32 = 6.0;

/// Words inside brackets that are notes about the sound, not singing.
const NOTES: [&str; 12] = [
    "music",
    "instrumental",
    "applause",
    "laugh",
    "silence",
    "inaudible",
    "noise",
    "blankaudio",
    "humming",
    "whistling",
    "cheering",
    "音楽",
];

/// How much of a line `word` fills.
fn weight(word: &str) -> f32 {
    let cjk = word.chars().filter(|c| is_cjk(*c)).count();
    let other = word.chars().any(|c| c.is_alphanumeric() && !is_cjk(c));
    cjk as f32 * (FULL_LINE / 16.0) + if other { 1.0 } else { 0.0 }
}

/// Whether a word ends a clause: `,` `.` `!` `?` and their CJK forms.
fn ends_clause(word: &str) -> bool {
    word.trim_end().chars().last().is_some_and(|c| {
        matches!(
            c,
            ',' | '.' | '!' | '?' | ';' | ':' | '，' | '。' | '！' | '？' | '；' | '：' | '、' | '…'
        )
    })
}

/// Drops annotations: everything inside square brackets, and parentheses,
/// asterisks or music marks around a note such as "(music)".
fn without_notes(words: Vec<Word>) -> Vec<Word> {
    let mut out = Vec::with_capacity(words.len());
    let mut group: Vec<Word> = Vec::new();
    let mut closing: Option<char> = None;
    for word in words {
        let text = word.text.trim();
        if let Some(close) = closing {
            group.push(word.clone());
            if text.ends_with(close) {
                closing = None;
                let joined: String = group
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if close != ']' && !is_note(&joined) {
                    out.append(&mut group);
                }
                group.clear();
            }
            continue;
        }
        let open = text.chars().next();
        let close = match open {
            Some('[') => Some(']'),
            Some('(') => Some(')'),
            Some('*') => Some('*'),
            Some('（') => Some('）'),
            _ => None,
        };
        match close {
            Some(close) if text.chars().count() > 1 && text.ends_with(close) => {
                if close != ']' && !is_note(text) {
                    out.push(word);
                }
            }
            Some(close) => {
                closing = Some(close);
                group.push(word);
            }
            None => out.push(word),
        }
    }
    // An annotation never closed: keep the words.
    if closing.is_some_and(|c| c != ']') {
        out.extend(group);
    }
    out.retain(|w| !text::key(&w.text).is_empty());
    out
}

fn is_note(text: &str) -> bool {
    let key = text::key(text);
    NOTES.iter().any(|note| key.contains(note))
}

/// The words of a segment: its own (trimmed), or, when the recogniser gave
/// none, its text's words spread over its time by length.
pub fn segment_words(segment: &Segment) -> Vec<Word> {
    let words: Vec<Word> = if segment.words.is_empty() {
        let units = text::units(&segment.text);
        let total: u64 = units.iter().map(|u| u64::from(u.weight())).sum::<u64>().max(1);
        let length = u64::from(segment.end_ms.saturating_sub(segment.start_ms));
        let mut done = 0;
        units
            .iter()
            .map(|unit| {
                let start = segment.start_ms + (length * done / total) as u32;
                done += u64::from(unit.weight());
                Word {
                    text: unit.text.clone(),
                    start_ms: start,
                    end_ms: segment.start_ms + (length * done / total) as u32,
                }
            })
            .collect()
    } else {
        segment
            .words
            .iter()
            .map(|w| Word {
                text: w.text.split_whitespace().collect::<Vec<_>>().join(" "),
                start_ms: w.start_ms,
                end_ms: w.end_ms.max(w.start_ms),
            })
            .filter(|w| !w.text.is_empty())
            .collect()
    };
    without_notes(words)
}

/// Every word of a transcript, in time order.
pub fn words(transcript: &Transcript) -> Vec<Word> {
    let mut words: Vec<Word> = transcript.segments.iter().flat_map(segment_words).collect();
    words.sort_by_key(|w| w.start_ms);
    words
}

/// Lines made from the recognised words alone.
pub fn lines(transcript: &Transcript) -> Vec<Line> {
    let mut segments: Vec<&Segment> = transcript.segments.iter().collect();
    segments.sort_by_key(|s| s.start_ms);
    let mut lines: Vec<Line> = Vec::new();
    for segment in segments {
        let mut current: Vec<TimedWord> = Vec::new();
        let mut filled = 0.0;
        for word in segment_words(segment) {
            if let Some(last) = current.last() {
                let pause = word.start_ms.saturating_sub(last.end_ms) > PAUSE_MS;
                let full = filled >= FULL_LINE;
                let clause = filled >= CLAUSE_LINE && ends_clause(&last.text);
                if pause || full || clause {
                    lines.push(line(std::mem::take(&mut current)));
                    filled = 0.0;
                }
            }
            filled += weight(&word.text);
            current.push(TimedWord {
                text: word.text,
                start_ms: word.start_ms,
                end_ms: word.end_ms,
                joins_next: false,
            });
        }
        if !current.is_empty() {
            lines.push(line(current));
        }
    }
    finish(lines)
}

/// A line of words, CJK words joined where both sides are CJK.
fn line(mut words: Vec<TimedWord>) -> Line {
    for i in 1..words.len() {
        let joined = words[i - 1]
            .text
            .chars()
            .last()
            .is_some_and(is_cjk_or_punctuation)
            && words[i].text.chars().next().is_some_and(is_cjk);
        words[i - 1].joins_next = joined;
    }
    Line {
        start_ms: words.first().map_or(0, |w| w.start_ms),
        words,
    }
}

/// CJK, or the punctuation written with it.
fn is_cjk_or_punctuation(c: char) -> bool {
    is_cjk(c) || matches!(c, '，' | '。' | '！' | '？' | '、' | '」' | '』' | '）' | '…')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start_ms: u32, end_ms: u32) -> Word {
        Word {
            text: text.into(),
            start_ms,
            end_ms,
        }
    }

    fn segment(words: Vec<Word>) -> Segment {
        Segment {
            text: String::new(),
            start_ms: words.first().map_or(0, |w| w.start_ms),
            end_ms: words.last().map_or(0, |w| w.end_ms),
            words,
        }
    }

    fn texts(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| {
                l.words
                    .iter()
                    .map(|w| format!("{}{}", w.text, if w.joins_next { "" } else { " " }))
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn notes_about_the_sound_are_left_out() {
        let words = vec![
            word("[Music]", 0, 900),
            word("(upbeat", 1000, 1100),
            word("music)", 1100, 1200),
            word("hello", 2000, 2300),
            word("(oh", 2400, 2500),
            word("yeah)", 2500, 2700),
            word("♪", 2800, 2900),
            word("*applause*", 3000, 3100),
        ];
        let kept: Vec<String> = without_notes(words).into_iter().map(|w| w.text).collect();
        assert_eq!(kept, ["hello", "(oh", "yeah)"], "backing vocals in brackets stay");
    }

    #[test]
    fn segments_without_words_spread_their_text() {
        let words = segment_words(&Segment {
            text: " Hello there, 你好".into(),
            start_ms: 1000,
            end_ms: 3000,
            words: Vec::new(),
        });
        let times: Vec<(String, u32, u32)> = words
            .into_iter()
            .map(|w| (w.text, w.start_ms, w.end_ms))
            .collect();
        // Weights: hello 5, there 5, 你 2, 好 2 (of 14).
        assert_eq!(
            times,
            [
                ("Hello".into(), 1000, 1714),
                ("there,".into(), 1714, 2428),
                ("你".into(), 2428, 2714),
                ("好".into(), 2714, 3000)
            ]
        );
    }

    #[test]
    fn lines_split_at_pauses_length_and_clauses() {
        let mut words = Vec::new();
        for i in 0..14u32 {
            words.push(word(&format!("w{i}"), 1000 + i * 300, 1250 + i * 300));
        }
        words.push(word("late", 8000, 8300));
        let lines = lines(&Transcript {
            language: None,
            segments: vec![segment(words)],
        });
        assert_eq!(
            texts(&lines),
            ["w0 w1 w2 w3 w4 w5 w6 w7 w8 w9", "w10 w11 w12 w13", "late"]
        );
        let clause = lines_of(&["one", "two", "three", "four", "five", "six,", "seven", "eight"]);
        assert_eq!(texts(&clause), ["one two three four five six,", "seven eight"]);
    }

    fn lines_of(texts: &[&str]) -> Vec<Line> {
        let words = texts
            .iter()
            .enumerate()
            .map(|(i, t)| word(t, i as u32 * 300, i as u32 * 300 + 250))
            .collect();
        lines(&Transcript {
            language: None,
            segments: vec![segment(words)],
        })
    }

    #[test]
    fn cjk_lines_hold_sixteen_characters_and_join() {
        let chars: Vec<String> = "一二三四五六七八九十甲乙丙丁戊己庚辛"
            .chars()
            .map(String::from)
            .collect();
        let refs: Vec<&str> = chars.iter().map(String::as_str).collect();
        let lines = lines_of(&refs);
        assert_eq!(texts(&lines), ["一二三四五六七八九十甲乙丙丁戊己", "庚辛"]);
    }
}

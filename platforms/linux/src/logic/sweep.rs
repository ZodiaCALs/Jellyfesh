//! The karaoke sweep: which part of a word-timed lyric line is lit.
//!
//! A word-timed line is shown as its words joined with spaces (none after a
//! word that `joins_next`, as in CJK), the same text the CLI draws. The UI
//! measures where each word sits (one span per visual line the word covers)
//! and asks [`sung_spans`] for the lit region at the cursor: every word before
//! the current one in full, with the space after it, and the current word up
//! to its progress. The result is one span per visual line, so the widget
//! draws the line at most once per visual line with a clip.

use std::ops::Range;

use jf_core::{Cursor, Line};

/// The display text of a lyric line and the byte range of each word in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineText {
    pub text: String,
    /// One range per `Line::words` entry; empty for line-timed lines.
    pub words: Vec<Range<usize>>,
}

/// The text to lay out for `line`.
pub fn line_text(line: &Line) -> LineText {
    if line.words.is_empty() {
        return LineText {
            text: line.text.clone(),
            words: Vec::new(),
        };
    }
    let mut text = String::new();
    let mut words = Vec::with_capacity(line.words.len());
    for (i, word) in line.words.iter().enumerate() {
        let start = text.len();
        text.push_str(&word.text);
        words.push(start..text.len());
        if !word.joins_next && i + 1 < line.words.len() {
            text.push(' ');
        }
    }
    LineText { text, words }
}

/// Where (part of) a word sits: a visual line of the layout and an x range
/// with `x0 <= x1`, in the layout's coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub line: usize,
    pub x0: f32,
    pub x1: f32,
}

impl Span {
    fn width(&self) -> f32 {
        (self.x1 - self.x0).max(0.0)
    }
}

/// The lit region of a line: one span per visual line.
///
/// `words[i]` holds the spans of word `i` in reading order. With no current
/// word nothing is lit (a word-timed line before its first word). `rtl`
/// sweeps right to left.
pub fn sung_spans(words: &[Vec<Span>], current: Option<usize>, progress: f32, rtl: bool) -> Vec<Span> {
    let Some(current) = current else {
        return Vec::new();
    };
    let mut lit: Vec<Span> = Vec::new();
    for (i, spans) in words.iter().enumerate().take(current.saturating_add(1)) {
        if i < current {
            for (k, span) in spans.iter().enumerate() {
                let mut span = *span;
                // The space after the word lights with it, when the next word
                // continues on the same visual line.
                if k + 1 == spans.len()
                    && let Some(next) = words.get(i + 1).and_then(|w| w.first())
                    && next.line == span.line
                {
                    if rtl {
                        span.x0 = span.x0.min(next.x1);
                    } else {
                        span.x1 = span.x1.max(next.x0);
                    }
                }
                lit.push(span);
            }
        } else {
            let total: f32 = spans.iter().map(Span::width).sum();
            let mut left = total * progress.clamp(0.0, 1.0);
            for span in spans {
                if left <= 0.0 {
                    break;
                }
                let w = span.width().min(left);
                left -= w;
                lit.push(if rtl {
                    Span {
                        line: span.line,
                        x0: span.x1 - w,
                        x1: span.x1,
                    }
                } else {
                    Span {
                        line: span.line,
                        x0: span.x0,
                        x1: span.x0 + w,
                    }
                });
            }
        }
    }
    merge_by_line(lit)
}

fn merge_by_line(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for span in spans {
        if span.width() <= 0.0 {
            continue;
        }
        match out.iter_mut().find(|s| s.line == span.line) {
            Some(s) => {
                s.x0 = s.x0.min(span.x0);
                s.x1 = s.x1.max(span.x1);
            }
            None => out.push(span),
        }
    }
    out.sort_by_key(|s| s.line);
    out
}

/// True while the current word of `line` is being swept, so the view must
/// draw every frame; otherwise it can sleep until the timeline's next change.
pub fn is_sweeping(line: &Line, cursor: &Cursor) -> bool {
    let Some(word) = cursor.word.and_then(|w| line.words.get(w)) else {
        return false;
    };
    word.end_ms > word.start_ms && cursor.word_progress < 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_core::Word;

    fn word(text: &str, start: u32, end: u32, joins_next: bool) -> Word {
        Word {
            start_ms: start,
            end_ms: end,
            text: text.into(),
            joins_next,
        }
    }

    /// jf-core does not re-export the lyrics `Direction` type, so test lines
    /// are built through serde.
    fn line(words: Vec<Word>) -> Line {
        serde_json::from_value(serde_json::json!({
            "id": 0, "start_ms": 0, "end_ms": 10_000, "agent": null,
            "text": "ignored for word-timed lines",
            "words": words, "background": [], "direction": "auto"
        }))
        .unwrap()
    }

    fn span(line: usize, x0: f32, x1: f32) -> Span {
        Span { line, x0, x1 }
    }

    #[test]
    fn builds_text_from_words() {
        let l = line(vec![
            word("Every", 0, 100, false),
            word("rope", 100, 200, false),
            word("港", 200, 300, true),
            word("湾", 300, 400, false),
        ]);
        let t = line_text(&l);
        assert_eq!(t.text, "Every rope 港湾");
        assert_eq!(&t.text[t.words[2].clone()], "港");
        assert_eq!(&t.text[t.words[3].clone()], "湾");
        let plain = Line {
            words: Vec::new(),
            text: "A line".into(),
            ..l
        };
        assert_eq!(line_text(&plain).text, "A line");
        assert!(line_text(&plain).words.is_empty());
    }

    #[test]
    fn lights_words_before_and_part_of_the_current_one() {
        // "ab cd" on one line: ab at 0..20, cd at 25..45.
        let words = vec![vec![span(0, 0.0, 20.0)], vec![span(0, 25.0, 45.0)]];
        assert!(sung_spans(&words, None, 0.0, false).is_empty());
        assert_eq!(sung_spans(&words, Some(0), 0.5, false), vec![span(0, 0.0, 10.0)]);
        // The space after a sung word lights too.
        assert_eq!(sung_spans(&words, Some(1), 0.0, false), vec![span(0, 0.0, 25.0)]);
        assert_eq!(sung_spans(&words, Some(1), 0.5, false), vec![span(0, 0.0, 35.0)]);
        assert_eq!(sung_spans(&words, Some(1), 1.0, false), vec![span(0, 0.0, 45.0)]);
    }

    #[test]
    fn follows_wrapped_lines() {
        // Word 1 ends line 0; word 2 starts line 1; word 2 wraps itself.
        let words = vec![
            vec![span(0, 0.0, 50.0)],
            vec![span(0, 60.0, 100.0)],
            vec![span(1, 0.0, 30.0), span(2, 0.0, 30.0)],
        ];
        let lit = sung_spans(&words, Some(2), 0.75, false);
        assert_eq!(
            lit,
            vec![span(0, 0.0, 100.0), span(1, 0.0, 30.0), span(2, 0.0, 15.0)]
        );
    }

    #[test]
    fn sweeps_right_to_left() {
        let words = vec![vec![span(0, 60.0, 100.0)], vec![span(0, 0.0, 50.0)]];
        assert_eq!(sung_spans(&words, Some(0), 0.5, true), vec![span(0, 80.0, 100.0)]);
        assert_eq!(sung_spans(&words, Some(1), 0.2, true), vec![span(0, 40.0, 100.0)]);
    }

    #[test]
    fn sweeping_needs_a_timed_unfinished_word() {
        let l = line(vec![word("a", 0, 100, false), word("b", 100, 100, false)]);
        let at = |word: Option<usize>, p: f32| Cursor {
            line: Some(0),
            word,
            word_progress: p,
        };
        assert!(is_sweeping(&l, &at(Some(0), 0.5)));
        assert!(!is_sweeping(&l, &at(Some(0), 1.0)));
        assert!(!is_sweeping(&l, &at(Some(1), 0.0)), "zero-length word");
        assert!(!is_sweeping(&l, &at(None, 0.0)));
        assert!(!is_sweeping(&l, &at(Some(9), 0.0)));
    }
}

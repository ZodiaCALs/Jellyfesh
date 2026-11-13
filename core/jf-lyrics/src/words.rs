//! Turns runs of text, each optionally tied to a timed span, into display text and words.
//!
//! All timed formats describe a line as a sequence of text pieces: LRC and
//! WebVTT pieces start at inline timestamps, TTML pieces are the text inside
//! timed `<span>`s (or loose text between them). [`assemble`] applies one set
//! of rules to all of them:
//!
//! * Whitespace separates words; runs of whitespace collapse to one space in the text.
//! * Text of one timed span stays one word, even when it contains spaces.
//! * Two timed spans with no whitespace between them are two words, and the
//!   first gets `joins_next` (CJK characters, syllables of one word).
//! * Untimed text glued to a timed word (punctuation, brackets) becomes part of it.
//! * Untimed text standing alone is kept in the line text but produces no word.

/// Time of the span a piece of text belongs to. `id` tells spans apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpanTime {
    pub id: u32,
    pub start: u32,
    /// `None` when the source leaves it open; resolved later from the next word or the line end.
    pub end: Option<u32>,
}

/// Text plus the timed span it sits in, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Piece {
    pub text: String,
    pub span: Option<SpanTime>,
}

impl Piece {
    pub(crate) fn new(text: impl Into<String>, span: Option<SpanTime>) -> Self {
        Self {
            text: text.into(),
            span,
        }
    }
}

/// A word whose end may still be open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawWord {
    pub start: u32,
    pub end: Option<u32>,
    pub text: String,
    pub joins_next: bool,
}

/// Result of [`assemble`]: the display text (not yet normalised) and the timed words.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Assembled {
    pub text: String,
    pub words: Vec<RawWord>,
}

#[derive(Debug)]
struct Token {
    text: String,
    span: Option<SpanTime>,
    joins_next: bool,
}

/// Applies the rules in the module documentation.
pub(crate) fn assemble(pieces: &[Piece]) -> Assembled {
    let mut tokens: Vec<Token> = Vec::new();
    // Whitespace seen since the last token.
    let mut gap = false;
    for piece in pieces {
        for (is_space, run) in runs(&piece.text) {
            if is_space {
                gap = true;
                continue;
            }
            let span_id = piece.span.map(|s| s.id);
            let push = match tokens.last_mut() {
                Some(last) if span_id.is_some() && last.span.map(|s| s.id) == span_id => {
                    if gap {
                        last.text.push(' ');
                    }
                    last.text.push_str(run);
                    false
                }
                Some(last) if !gap && span_id.is_none() => {
                    last.text.push_str(run);
                    false
                }
                Some(last) if !gap && last.span.is_none() => {
                    last.text.push_str(run);
                    last.span = piece.span;
                    false
                }
                Some(last) if !gap => {
                    last.joins_next = true;
                    true
                }
                _ => true,
            };
            if push {
                tokens.push(Token {
                    text: run.to_owned(),
                    span: piece.span,
                    joins_next: false,
                });
            }
            gap = false;
        }
    }

    let mut text = String::new();
    for (i, token) in tokens.iter().enumerate() {
        text.push_str(&token.text);
        if !token.joins_next && i + 1 < tokens.len() {
            text.push(' ');
        }
    }
    let words = tokens
        .into_iter()
        .filter_map(|t| {
            t.span.map(|s| RawWord {
                start: s.start,
                end: s.end,
                text: t.text,
                joins_next: t.joins_next,
            })
        })
        .collect();
    Assembled { text, words }
}

/// Splits `s` into alternating runs of whitespace and non-whitespace: `(is_space, run)`.
fn runs(s: &str) -> impl Iterator<Item = (bool, &str)> {
    let mut rest = s;
    std::iter::from_fn(move || {
        let first = rest.chars().next()?;
        let is_space = first.is_whitespace();
        let end = rest
            .char_indices()
            .find(|&(_, c)| c.is_whitespace() != is_space)
            .map_or(rest.len(), |(i, _)| i);
        let (run, tail) = rest.split_at(end);
        rest = tail;
        Some((is_space, run))
    })
}

/// Builds pieces for formats where inline timestamps start each piece
/// (enhanced LRC, WebVTT): piece `k` runs from its stamp to the next stamp.
///
/// `segments[0]` is the text before the first inline stamp and starts at
/// `line_start`; the last segment ends at `last_end` when given.
pub(crate) fn stamped_pieces<T: AsRef<str>>(
    line_start: u32,
    segments: &[(Option<u32>, T)],
    last_end: Option<u32>,
) -> Vec<Piece> {
    segments
        .iter()
        .enumerate()
        .map(|(k, (stamp, text))| {
            let start = stamp.unwrap_or(line_start);
            let end = match segments.get(k + 1) {
                Some((next, _)) => Some(next.unwrap_or(start)),
                None => last_end,
            };
            Piece::new(
                text.as_ref(),
                Some(SpanTime {
                    id: k as u32,
                    start,
                    end,
                }),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed(id: u32, start: u32, end: u32, text: &str) -> Piece {
        Piece::new(
            text,
            Some(SpanTime {
                id,
                start,
                end: Some(end),
            }),
        )
    }

    fn loose(text: &str) -> Piece {
        Piece::new(text, None)
    }

    fn summary(a: &Assembled) -> Vec<(String, u32, bool)> {
        a.words
            .iter()
            .map(|w| (w.text.clone(), w.start, w.joins_next))
            .collect()
    }

    #[test]
    fn whitespace_separates_words() {
        let a = assemble(&[timed(0, 0, 5, "Hello"), loose(" \n  "), timed(1, 5, 9, "world")]);
        assert_eq!(a.text, "Hello world");
        assert_eq!(
            summary(&a),
            [("Hello".into(), 0, false), ("world".into(), 5, false)]
        );
    }

    #[test]
    fn adjacent_spans_join() {
        let a = assemble(&[
            timed(0, 0, 5, "夜"),
            timed(1, 5, 9, "空"),
            loose(" "),
            timed(2, 9, 12, "Beau"),
            timed(3, 12, 14, "ty"),
        ]);
        assert_eq!(a.text, "夜空 Beauty");
        assert_eq!(
            summary(&a),
            [
                ("夜".into(), 0, true),
                ("空".into(), 5, false),
                ("Beau".into(), 9, true),
                ("ty".into(), 12, false)
            ]
        );
    }

    #[test]
    fn spaces_inside_one_span_keep_one_word() {
        let a = assemble(&[timed(0, 0, 5, " take  it "), timed(1, 5, 9, "slow")]);
        assert_eq!(a.text, "take it slow");
        assert_eq!(
            summary(&a),
            [("take it".into(), 0, false), ("slow".into(), 5, false)]
        );
    }

    #[test]
    fn glued_punctuation_joins_the_word() {
        let a = assemble(&[
            loose("("),
            timed(0, 0, 5, "ooh"),
            loose("), "),
            timed(1, 5, 9, "yeah"),
            loose(" !"),
        ]);
        assert_eq!(a.text, "(ooh), yeah !");
        assert_eq!(
            summary(&a),
            [("(ooh),".into(), 0, false), ("yeah".into(), 5, false)]
        );
    }

    #[test]
    fn untimed_text_has_no_words() {
        let a = assemble(&[loose("  just   text \t")]);
        assert_eq!(a.text, "just text");
        assert!(a.words.is_empty());
    }

    #[test]
    fn stamped_pieces_end_at_the_next_stamp() {
        let segs = vec![
            (None, String::new()),
            (Some(100), "Hi ".to_owned()),
            (Some(400), "there".to_owned()),
            (Some(900), String::new()),
        ];
        let a = assemble(&stamped_pieces(50, &segs, None));
        assert_eq!(a.text, "Hi there");
        let ends: Vec<_> = a.words.iter().map(|w| (w.start, w.end)).collect();
        assert_eq!(ends, [(100, Some(400)), (400, Some(900))]);
    }
}

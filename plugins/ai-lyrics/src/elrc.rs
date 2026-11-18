//! Enhanced LRC with word times, as the core's parser reads it
//! (`core/jf-lyrics`, "Enhanced LRC (A2)"):
//!
//! ```text
//! [00:12.40]<00:12.40>Every <00:12.90>word <00:13.30>timed<00:13.95>
//! [00:16.00]<00:16.00>我<00:16.30>愛<00:16.62>你<00:17.10>
//! ```
//!
//! A stamp before each word starts it; the stamp after the last word ends
//! it and the line. Words of space-separated scripts are separated by a
//! space, CJK characters are not. A pause is an empty timed line.

use crate::model::Line;

/// `mm:ss.xx` for a time in milliseconds (centiseconds, rounded down).
pub fn stamp(ms: u32) -> String {
    format!(
        "{:02}:{:02}.{:02}",
        ms / 60_000,
        (ms / 1000) % 60,
        (ms % 1000) / 10
    )
}

/// A word's text on one line: whitespace inside it collapsed, nothing that
/// could read as a stamp.
fn clean(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('<', "‹")
        .replace('>', "›")
}

/// The lines as enhanced LRC.
pub fn render(lines: &[Line]) -> String {
    let mut out = String::new();
    for line in lines {
        out.push('[');
        out.push_str(&stamp(line.start_ms));
        out.push(']');
        for (i, word) in line.words.iter().enumerate() {
            out.push('<');
            out.push_str(&stamp(word.start_ms));
            out.push('>');
            out.push_str(&clean(&word.text));
            if i + 1 < line.words.len() && !word.joins_next {
                out.push(' ');
            }
        }
        if let Some(last) = line.words.last() {
            out.push('<');
            out.push_str(&stamp(last.end_ms));
            out.push('>');
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TimedWord;

    fn word(text: &str, start_ms: u32, end_ms: u32, joins_next: bool) -> TimedWord {
        TimedWord {
            text: text.into(),
            start_ms,
            end_ms,
            joins_next,
        }
    }

    #[test]
    fn stamps_are_centiseconds() {
        assert_eq!(stamp(0), "00:00.00");
        assert_eq!(stamp(83_456), "01:23.45");
        assert_eq!(stamp(6_000_000), "100:00.00");
    }

    #[test]
    fn renders_words_and_pauses() {
        let text = render(&[
            Line {
                start_ms: 12_400,
                words: vec![
                    word("Every", 12_400, 12_900, false),
                    word("word <3", 12_900, 13_300, false),
                    word("timed", 13_300, 13_950, false),
                ],
            },
            Line {
                start_ms: 13_950,
                words: Vec::new(),
            },
            Line {
                start_ms: 16_000,
                words: vec![
                    word("我", 16_000, 16_300, true),
                    word("愛", 16_300, 16_620, true),
                    word("你", 16_620, 17_100, false),
                ],
            },
        ]);
        assert_eq!(
            text,
            "[00:12.40]<00:12.40>Every <00:12.90>word ‹3 <00:13.30>timed<00:13.95>\n\
             [00:13.95]\n\
             [00:16.00]<00:16.00>我<00:16.30>愛<00:16.62>你<00:17.10>\n"
        );
    }
}

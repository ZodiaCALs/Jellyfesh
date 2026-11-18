//! What recognition gives and what the plugin makes of it, independent of
//! where the words came from (the app's recogniser or an online provider).

use serde::{Deserialize, Serialize};

/// A recognised word.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

/// A stretch of recognised speech, about a line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    /// Its words; empty when the recogniser gave no word times.
    #[serde(default)]
    pub words: Vec<Word>,
}

/// Everything recognised in a song.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    #[serde(default)]
    pub language: Option<String>,
    pub segments: Vec<Segment>,
}

/// A word of the lyrics made, with its time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedWord {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    /// No space before the next word (CJK).
    pub joins_next: bool,
}

/// A line of the lyrics made. A line without words is a pause (an
/// instrumental gap) starting at `start_ms`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub start_ms: u32,
    pub words: Vec<TimedWord>,
}

impl Line {
    /// Where the line ends: its last word's end (its start for a pause).
    pub fn end_ms(&self) -> u32 {
        self.words.last().map_or(self.start_ms, |w| w.end_ms)
    }
}

/// The shortest time a word gets (ADR-0019 §6).
pub const MIN_WORD_MS: u32 = 60;
/// A pause this long between lines is marked with an empty line.
pub const PAUSE_LINE_MS: u32 = 5000;

/// Makes word starts rise by at least [`MIN_WORD_MS`] across the lines,
/// every word at least that long and ending no later than the next word
/// starts, each line starting at its first word, and pauses of
/// [`PAUSE_LINE_MS`] or more between lines marked with an empty line.
/// Lines without words are dropped first.
pub fn finish(lines: Vec<Line>) -> Vec<Line> {
    let mut lines: Vec<Line> = lines.into_iter().filter(|l| !l.words.is_empty()).collect();
    let mut previous: Option<u32> = None;
    for line in &mut lines {
        for word in &mut line.words {
            if let Some(before) = previous {
                word.start_ms = word.start_ms.max(before.saturating_add(MIN_WORD_MS));
            }
            previous = Some(word.start_ms);
        }
    }
    // Ends: at least the shortest word, at most the next word's start.
    let starts: Vec<u32> = lines
        .iter()
        .flat_map(|l| l.words.iter().map(|w| w.start_ms))
        .collect();
    let mut index = 0;
    for line in &mut lines {
        for word in &mut line.words {
            // Starts rise by at least the shortest word, so this keeps it.
            let mut end = word.end_ms.max(word.start_ms + MIN_WORD_MS);
            if let Some(next) = starts.get(index + 1) {
                end = end.min(*next);
            }
            word.end_ms = end;
            index += 1;
        }
        line.start_ms = line.words[0].start_ms;
        if let Some(last) = line.words.last_mut() {
            last.joins_next = false;
        }
    }
    let mut out = Vec::with_capacity(lines.len() * 2);
    for line in lines {
        if let Some(previous) = out.last().map(Line::end_ms)
            && line.start_ms >= previous.saturating_add(PAUSE_LINE_MS)
        {
            out.push(Line {
                start_ms: previous,
                words: Vec::new(),
            });
        }
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start_ms: u32, end_ms: u32) -> TimedWord {
        TimedWord {
            text: text.into(),
            start_ms,
            end_ms,
            joins_next: false,
        }
    }

    #[test]
    fn starts_rise_and_ends_fit() {
        let lines = finish(vec![
            Line {
                start_ms: 0,
                words: vec![word("a", 1000, 1000), word("b", 1000, 1500), word("c", 900, 2000)],
            },
            Line {
                start_ms: 0,
                words: Vec::new(),
            },
            Line {
                start_ms: 0,
                words: vec![word("d", 1500, 1600)],
            },
        ]);
        let times: Vec<(u32, u32)> = lines
            .iter()
            .flat_map(|l| l.words.iter().map(|w| (w.start_ms, w.end_ms)))
            .collect();
        assert_eq!(times, [(1000, 1060), (1060, 1120), (1120, 1500), (1500, 1600)]);
        assert_eq!(lines.len(), 2, "the line without words is gone");
        assert_eq!(lines[1].start_ms, 1500);
    }

    #[test]
    fn long_pauses_get_an_empty_line() {
        let lines = finish(vec![
            Line {
                start_ms: 0,
                words: vec![word("a", 1000, 2000)],
            },
            Line {
                start_ms: 0,
                words: vec![word("b", 7000, 7500)],
            },
            Line {
                start_ms: 0,
                words: vec![word("c", 9000, 9500)],
            },
        ]);
        assert_eq!(lines.len(), 4);
        assert!(lines[1].words.is_empty());
        assert_eq!(lines[1].start_ms, 2000);
        assert_eq!(lines[2].start_ms, 7000);
    }
}

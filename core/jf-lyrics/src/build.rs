//! Format-independent finishing: every parser fills a [`DocBuilder`] and calls
//! [`DocBuilder::finish`], which applies the rules shared by all formats.

use std::collections::BTreeMap;

use crate::model::{
    Agent, AgentKind, AuxKind, AuxLine, AuxTrack, Format, Line, LyricDoc, SourceInfo, Timing, Word,
};
use crate::text::{clean, direction};
use crate::words::RawWord;

/// A line as a parser produced it, before sorting and normalisation.
#[derive(Debug, Clone, Default)]
pub(crate) struct RawLine {
    pub start: Option<u32>,
    pub end: Option<u32>,
    pub agent: Option<String>,
    pub text: String,
    pub words: Vec<RawWord>,
    pub background: Vec<RawWord>,
    pub aux: Vec<RawAux>,
}

/// A translation or romanization of one line; `track` indexes [`DocBuilder::tracks`].
#[derive(Debug, Clone)]
pub(crate) struct RawAux {
    pub track: usize,
    pub text: String,
    pub words: Vec<RawWord>,
}

#[derive(Debug)]
pub(crate) struct DocBuilder {
    pub format: Format,
    pub language: Option<String>,
    pub offset_ms: i32,
    pub agents: Vec<Agent>,
    pub lines: Vec<RawLine>,
    /// Auxiliary tracks in order of first appearance.
    pub tracks: Vec<(AuxKind, Option<String>)>,
    pub metadata: BTreeMap<String, String>,
}

impl DocBuilder {
    pub(crate) fn new(format: Format) -> Self {
        Self {
            format,
            language: None,
            offset_ms: 0,
            agents: Vec::new(),
            lines: Vec::new(),
            tracks: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// Index of the track of this kind and language, created on first use.
    pub(crate) fn track(&mut self, kind: AuxKind, language: Option<String>) -> usize {
        if let Some(i) = self.tracks.iter().position(|(k, l)| *k == kind && *l == language) {
            return i;
        }
        self.tracks.push((kind, language));
        self.tracks.len() - 1
    }

    /// Returns the id of the agent, adding a person with that id when it is new.
    pub(crate) fn agent(&mut self, id: &str) -> String {
        if !self.agents.iter().any(|a| a.id == id) {
            self.agents.push(Agent {
                id: id.to_owned(),
                name: None,
                kind: AgentKind::Person,
            });
        }
        id.to_owned()
    }

    /// Applies the shared rules and produces the document:
    ///
    /// 1. In a synced document, a line without a start inherits the start of the
    ///    line before it (or 0), so every line can be placed.
    /// 2. Lines are sorted by start, keeping source order for equal starts.
    /// 3. A missing line end becomes the start of the next line that starts
    ///    later; the last line(s) may keep no end. Ends never precede starts.
    /// 4. Word starts are made non-decreasing within a line; an open word end
    ///    becomes the next word's start, else the line end, else the word start.
    /// 5. Text is trimmed and NFC-normalised; direction comes from the text.
    /// 6. Line ids are positions; auxiliary lines move to their tracks.
    pub(crate) fn finish(self) -> LyricDoc {
        let mut lines = self.lines;
        let synced = lines.iter().any(|l| l.start.is_some());
        if synced {
            let mut previous = 0;
            for line in &mut lines {
                match line.start {
                    Some(s) => previous = s,
                    None => line.start = Some(previous),
                }
            }
            lines.sort_by_key(|l| l.start);
            infer_ends(&mut lines);
        } else {
            for line in &mut lines {
                line.end = None;
                line.words.clear();
                line.background.clear();
            }
        }

        let mut tracks: Vec<AuxTrack> = self
            .tracks
            .into_iter()
            .map(|(kind, language)| AuxTrack {
                kind,
                language,
                lines: BTreeMap::new(),
            })
            .collect();
        let mut out = Vec::with_capacity(lines.len());
        for (id, raw) in lines.into_iter().enumerate() {
            let id = id as u32;
            let end = raw.end;
            for aux in raw.aux {
                if let Some(track) = tracks.get_mut(aux.track) {
                    track.lines.insert(
                        id,
                        AuxLine {
                            text: clean(&aux.text),
                            words: if synced {
                                finish_words(aux.words, end)
                            } else {
                                Vec::new()
                            },
                        },
                    );
                }
            }
            let text = clean(&raw.text);
            out.push(Line {
                id,
                start_ms: raw.start,
                end_ms: end,
                agent: raw.agent,
                direction: direction(&text),
                text,
                words: finish_words(raw.words, end),
                background: finish_words(raw.background, end),
            });
        }
        tracks.retain(|t| !t.lines.is_empty());

        let timing = if out.iter().any(|l| !l.words.is_empty()) {
            Timing::Word
        } else if synced {
            Timing::Line
        } else {
            Timing::Unsynced
        };
        let agents = self
            .agents
            .into_iter()
            .map(|a| Agent {
                name: a.name.map(|n| clean(&n)).filter(|n| !n.is_empty()),
                ..a
            })
            .collect();
        LyricDoc {
            language: self.language,
            timing,
            offset_ms: self.offset_ms,
            agents,
            lines: out,
            tracks,
            source: SourceInfo {
                format: self.format,
                origin: None,
            },
            metadata: self.metadata,
        }
    }
}

/// Fills missing ends from the next strictly later start and clamps ends to starts.
/// `lines` must be sorted by start and every start must be set.
fn infer_ends(lines: &mut [RawLine]) {
    let mut next_later: Option<u32> = None;
    let mut i = lines.len();
    while i > 0 {
        let start = lines[i - 1].start.unwrap_or(0);
        let mut j = i - 1;
        while j > 0 && lines[j - 1].start.unwrap_or(0) == start {
            j -= 1;
        }
        for line in &mut lines[j..i] {
            let end = line.end.or(next_later);
            line.end = end.map(|e| e.max(start));
        }
        next_later = Some(start);
        i = j;
    }
}

/// Resolves open word ends, keeps starts non-decreasing and normalises text.
fn finish_words(words: Vec<RawWord>, line_end: Option<u32>) -> Vec<Word> {
    let starts: Vec<u32> = words
        .iter()
        .scan(0u32, |floor, w| {
            *floor = (*floor).max(w.start);
            Some(*floor)
        })
        .collect();
    let count = words.len();
    words
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            let start = starts[i];
            let end = w
                .end
                .or_else(|| starts.get(i + 1).copied())
                .or(line_end)
                .unwrap_or(start)
                .max(start);
            Word {
                start_ms: start,
                end_ms: end,
                text: clean(&w.text),
                joins_next: w.joins_next && i + 1 < count,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start: Option<u32>, end: Option<u32>, text: &str) -> RawLine {
        RawLine {
            start,
            end,
            text: text.to_owned(),
            ..RawLine::default()
        }
    }

    #[test]
    fn sorts_and_infers_ends_from_the_next_later_line() {
        let mut b = DocBuilder::new(Format::Lrc);
        b.lines = vec![
            line(Some(5000), None, "c"),
            line(Some(1000), None, "a"),
            line(Some(1000), None, "a2"),
            line(Some(3000), Some(4000), "b"),
        ];
        let doc = b.finish();
        let got: Vec<_> = doc
            .lines
            .iter()
            .map(|l| (l.id, l.text.as_str(), l.start_ms, l.end_ms))
            .collect();
        assert_eq!(
            got,
            [
                (0, "a", Some(1000), Some(3000)),
                (1, "a2", Some(1000), Some(3000)),
                (2, "b", Some(3000), Some(4000)),
                (3, "c", Some(5000), None),
            ]
        );
        assert_eq!(doc.timing, Timing::Line);
    }

    #[test]
    fn untimed_lines_inherit_the_previous_start() {
        let mut b = DocBuilder::new(Format::Ttml);
        b.lines = vec![
            line(None, None, "first"),
            line(Some(2000), None, "b"),
            line(None, None, "c"),
        ];
        let doc = b.finish();
        let starts: Vec<_> = doc.lines.iter().map(|l| l.start_ms).collect();
        assert_eq!(starts, [Some(0), Some(2000), Some(2000)]);
    }

    #[test]
    fn word_ends_and_starts_are_resolved() {
        let w = |start, end, text: &str| RawWord {
            start,
            end,
            text: text.to_owned(),
            joins_next: true,
        };
        let words = finish_words(
            vec![w(100, None, "a"), w(90, Some(80), "b"), w(300, None, "c")],
            Some(500),
        );
        let got: Vec<_> = words
            .iter()
            .map(|w| (w.start_ms, w.end_ms, w.joins_next))
            .collect();
        assert_eq!(got, [(100, 100, true), (100, 100, true), (300, 500, false)]);
        let open = finish_words(vec![w(700, None, "z")], None);
        assert_eq!((open[0].start_ms, open[0].end_ms), (700, 700));
    }
}

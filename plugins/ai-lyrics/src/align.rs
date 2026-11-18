//! Timing plain lyrics with recognised words (ADR-0019 §6).
//!
//! * Both sides become units (words, or characters in Chinese, Japanese and
//!   Korean; a recognised CJK word's time is shared by its characters) with
//!   normalised keys ([`crate::text`]).
//! * They are aligned globally (Needleman–Wunsch in a band around the
//!   diagonal): a pair scores `2·similarity − 1` (similarity is one minus
//!   the edit distance over the longer key), a gap −1.
//! * A lyric unit paired with a recognised one at least 60 % alike takes its
//!   times (an anchor). The units between two anchors share the time between
//!   them by length: the time of the recognised words between the anchors
//!   when there are any, else the gap between the anchors. When that time is
//!   much longer than singing the units takes, units on an anchor's line
//!   stay next to it and whole lines share the rest.
//! * Starts rise by at least 60 ms, each line starts at its first word, and
//!   long pauses get an empty line ([`crate::model::finish`]).

use crate::model::{Line, TimedWord, Transcript, Word, finish};
use crate::text::{self, Unit};

/// A pair this alike (or more) is an anchor.
const ANCHOR_SIMILARITY: f32 = 0.6;
/// A gap's score.
const GAP: f32 = -1.0;
/// At least this share of the lyrics' units must find their recognised
/// word, or the lyrics are not the ones sung.
const MIN_ANCHORED: f32 = 0.1;

/// Why lyrics could not be timed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The lyrics have no words.
    NoLyrics,
    /// Nothing was recognised.
    NothingHeard,
    /// Too few lyric words were recognised: the lyrics do not match.
    NoMatch,
}

/// A unit of the lyrics and the line it is on.
#[derive(Debug, Clone)]
struct Written {
    unit: Unit,
    key: Vec<char>,
    line: usize,
}

/// A recognised unit and its time.
#[derive(Debug, Clone)]
struct Heard {
    key: Vec<char>,
    start: u32,
    end: u32,
}

/// The lines of plain lyrics, as units; labels (`[Chorus]`) and lines
/// without words are left out.
fn written(lyrics: &str) -> Vec<Written> {
    let mut out = Vec::new();
    let mut line = 0;
    for text in lyrics.lines() {
        if text::is_label(text) {
            continue;
        }
        let units = text::units(text);
        if units.is_empty() {
            continue;
        }
        for unit in units {
            out.push(Written {
                key: unit.key.chars().collect(),
                unit,
                line,
            });
        }
        line += 1;
    }
    out
}

/// The recognised words as units, in time order.
fn heard(words: &[Word]) -> Vec<Heard> {
    let mut out = Vec::new();
    for word in words {
        let units = text::units(&word.text);
        if units.is_empty() {
            continue;
        }
        let count = units.len() as u64;
        let span = u64::from(word.end_ms.saturating_sub(word.start_ms));
        for (i, unit) in units.into_iter().enumerate() {
            let i = i as u64;
            out.push(Heard {
                key: unit.key.chars().collect(),
                start: word.start_ms + (span * i / count) as u32,
                end: word.start_ms + (span * (i + 1) / count) as u32,
            });
        }
    }
    out.sort_by_key(|h| h.start);
    out
}

/// How long singing a unit takes when nothing says otherwise.
fn natural_ms(unit: &Unit) -> u32 {
    250 + 40 * unit.weight().min(16)
}

/// The cells of row `i` that are computed: `[lo, hi]`.
#[derive(Debug, Clone, Copy)]
struct Row {
    lo: usize,
    hi: usize,
    at: usize,
}

/// Moves in the score table.
const DIAGONAL: u8 = 0;
const UP: u8 = 1;
const LEFT: u8 = 2;

/// Needleman–Wunsch in a band: for each lyric unit, the recognised unit it
/// is paired with and how alike they are (`None`: a gap).
fn pairs(written: &[Written], heard: &[Heard]) -> Vec<Option<(usize, f32)>> {
    let (n, m) = (written.len(), heard.len());
    let band = n.abs_diff(m) + (n + m).div_ceil(8).max(64);
    let rows: Vec<Row> = {
        let mut at = 0;
        (0..=n)
            .map(|i| {
                let centre = (i * m).checked_div(n).unwrap_or(0);
                let row = Row {
                    lo: centre.saturating_sub(band),
                    hi: (centre + band).min(m),
                    at,
                };
                at += row.hi - row.lo + 1;
                row
            })
            .collect()
    };
    let cells = rows.last().map_or(0, |r| r.at + r.hi - r.lo + 1);
    let mut score = vec![f32::NEG_INFINITY; cells];
    let mut moves = vec![LEFT; cells];
    let mut alike = vec![0.0f32; cells];
    let index = |i: usize, j: usize| -> Option<usize> {
        let row = rows[i];
        (row.lo..=row.hi).contains(&j).then(|| row.at + j - row.lo)
    };
    for i in 0..=n {
        let row = rows[i];
        for j in row.lo..=row.hi {
            let here = row.at + j - row.lo;
            if i == 0 && j == 0 {
                score[here] = 0.0;
                continue;
            }
            let mut best = f32::NEG_INFINITY;
            let mut step = LEFT;
            let mut similarity = 0.0;
            if i > 0
                && j > 0
                && let Some(diagonal) = index(i - 1, j - 1)
                && score[diagonal].is_finite()
            {
                let alike = text::similarity(&written[i - 1].key, &heard[j - 1].key);
                best = score[diagonal] + 2.0 * alike - 1.0;
                step = DIAGONAL;
                similarity = alike;
            }
            if i > 0
                && let Some(up) = index(i - 1, j)
                && score[up] + GAP > best
            {
                best = score[up] + GAP;
                step = UP;
            }
            // On a tie, the lyric unit pairs with an earlier recognised one:
            // a chorus written once takes the first time it is sung.
            if j > row.lo {
                let left = here - 1;
                if score[left] + GAP >= best && score[left].is_finite() {
                    best = score[left] + GAP;
                    step = LEFT;
                }
            }
            score[here] = best;
            moves[here] = step;
            alike[here] = similarity;
        }
    }
    let mut out = vec![None; n];
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        let Some(here) = index(i, j) else {
            break;
        };
        match moves[here] {
            DIAGONAL if i > 0 && j > 0 => {
                out[i - 1] = Some((j - 1, alike[here]));
                i -= 1;
                j -= 1;
            }
            UP if i > 0 => i -= 1,
            _ if j > 0 => j -= 1,
            _ => i -= 1,
        }
    }
    out
}

/// Times for `units` spread over `[start, end]` by weight.
fn spread(units: &[&Written], start: u32, end: u32) -> Vec<(u32, u32)> {
    let total: u64 = units
        .iter()
        .map(|w| u64::from(w.unit.weight()))
        .sum::<u64>()
        .max(1);
    let length = u64::from(end.saturating_sub(start));
    let mut done = 0u64;
    units
        .iter()
        .map(|w| {
            let from = start + (length * done / total) as u32;
            done += u64::from(w.unit.weight());
            (from, start + (length * done / total) as u32)
        })
        .collect()
}

/// Times for `units` sung one after the other from `start`.
fn paced(units: &[&Written], start: u32) -> Vec<(u32, u32)> {
    let mut at = start;
    units
        .iter()
        .map(|w| {
            let from = at;
            at += natural_ms(&w.unit);
            (from, at)
        })
        .collect()
}

fn natural(units: &[&Written]) -> u32 {
    units.iter().map(|w| natural_ms(&w.unit)).sum()
}

/// Times for a run of units without a recognised word, in `[start, end]`:
/// `first_line` and `last_line` say whether its first and last units share
/// a line with the anchor before and after it.
fn place(
    run: &[&Written],
    start: u32,
    end: u32,
    first_line: Option<usize>,
    last_line: Option<usize>,
) -> Vec<(u32, u32)> {
    let length = end.saturating_sub(start);
    if length <= 2 * natural(run) {
        return spread(run, start, end);
    }
    // Too much room: the start of the run goes on with the line before, the
    // end of the run leads into the line after, and whole lines share the rest.
    let head = first_line.map_or(0, |line| run.iter().take_while(|w| w.line == line).count());
    let tail = last_line.map_or(0, |line| {
        run[head..].iter().rev().take_while(|w| w.line == line).count()
    });
    let middle = &run[head..run.len() - tail];
    let mut out = paced(&run[..head], start);
    let middle_start = out.last().map_or(start, |t| t.1);
    let tail_times = {
        let total = natural(&run[run.len() - tail..]);
        paced(
            &run[run.len() - tail..],
            end.saturating_sub(total).max(middle_start),
        )
    };
    let middle_end = tail_times.first().map_or(end, |t| t.0);
    if !middle.is_empty() {
        let total = natural(middle);
        let room = middle_end.saturating_sub(middle_start);
        if room <= total {
            out.extend(spread(middle, middle_start, middle_end));
        } else {
            // Each line gets a share of the room and is sung at its own pace
            // from the start of it.
            let mut at = 0u64;
            let mut index = 0;
            while index < middle.len() {
                let line = middle[index].line;
                let count = middle[index..].iter().take_while(|w| w.line == line).count();
                let words = &middle[index..index + count];
                let slot = middle_start + (u64::from(room) * at / u64::from(total)) as u32;
                at += u64::from(natural(words));
                out.extend(paced(words, slot));
                index += count;
            }
        }
    }
    out.extend(tail_times);
    out
}

/// Plain lyrics timed with the recognised words of `transcript`, in a song
/// of `duration_ms` (when known).
pub fn align(lyrics: &str, transcript: &Transcript, duration_ms: Option<u32>) -> Result<Vec<Line>, Failure> {
    let written = written(lyrics);
    if written.is_empty() {
        return Err(Failure::NoLyrics);
    }
    let heard = heard(&crate::lines::words(transcript));
    if heard.is_empty() {
        return Err(Failure::NothingHeard);
    }
    let pairs = pairs(&written, &heard);
    let anchors: Vec<(usize, usize)> = pairs
        .iter()
        .enumerate()
        .filter_map(|(i, pair)| match pair {
            Some((j, alike)) if *alike >= ANCHOR_SIMILARITY => Some((i, *j)),
            _ => None,
        })
        .collect();
    if (anchors.len() as f32) < (written.len() as f32 * MIN_ANCHORED).max(1.0) {
        return Err(Failure::NoMatch);
    }
    let limit = duration_ms.unwrap_or(u32::MAX);
    let mut times: Vec<Option<(u32, u32)>> = vec![None; written.len()];
    for (i, j) in &anchors {
        times[*i] = Some((heard[*j].start.min(limit), heard[*j].end.min(limit)));
    }
    let mut i = 0;
    while i < written.len() {
        if times[i].is_some() {
            i += 1;
            continue;
        }
        let from = i;
        while i < written.len() && times[i].is_none() {
            i += 1;
        }
        let before = anchors.iter().rev().find(|(a, _)| *a < from).copied();
        let after = anchors.iter().find(|(a, _)| *a >= i).copied();
        let run: Vec<&Written> = written[from..i].iter().collect();
        // The recognised words between the anchors, which these units
        // probably are.
        let heard_from = before.map_or(0, |(_, j)| j + 1);
        let heard_to = after.map_or(heard.len(), |(_, j)| j);
        let (start, end) = if heard_from < heard_to {
            (heard[heard_from].start, heard[heard_to - 1].end)
        } else {
            match (before, after) {
                (Some((_, b)), Some((_, a))) => (heard[b].end, heard[a].start),
                (Some((_, b)), None) => {
                    let start = heard[b].end;
                    (start, start.saturating_add(natural(&run)))
                }
                (None, Some((_, a))) => {
                    let end = heard[a].start;
                    (end.saturating_sub(natural(&run)), end)
                }
                (None, None) => (0, natural(&run)),
            }
        };
        let (start, end) = (start.min(limit), end.min(limit).max(start.min(limit)));
        let first_line = before.map(|(a, _)| written[a].line);
        let last_line = after.map(|(a, _)| written[a].line);
        let placed = place(&run, start, end, first_line, last_line);
        for (k, time) in placed.into_iter().enumerate() {
            times[from + k] = Some(time);
        }
    }
    let mut lines: Vec<Line> = Vec::new();
    for (w, time) in written.iter().zip(times) {
        let (start_ms, end_ms) = time.unwrap_or_default();
        let word = TimedWord {
            text: w.unit.text.clone(),
            start_ms,
            end_ms,
            joins_next: w.unit.joins_next,
        };
        match lines.get_mut(w.line) {
            Some(line) => line.words.push(word),
            None => lines.push(Line {
                start_ms,
                words: vec![word],
            }),
        }
    }
    Ok(finish(lines))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Segment;

    fn transcript(words: &[(&str, u32)]) -> Transcript {
        let words: Vec<Word> = words
            .iter()
            .map(|(text, start)| Word {
                text: (*text).into(),
                start_ms: *start,
                end_ms: start + 300,
            })
            .collect();
        Transcript {
            language: None,
            segments: vec![Segment {
                text: String::new(),
                start_ms: words.first().map_or(0, |w| w.start_ms),
                end_ms: words.last().map_or(0, |w| w.end_ms),
                words,
            }],
        }
    }

    fn starts(lines: &[Line]) -> Vec<(String, u32)> {
        lines
            .iter()
            .flat_map(|l| l.words.iter().map(|w| (w.text.clone(), w.start_ms)))
            .collect()
    }

    #[test]
    fn matching_words_take_the_recognised_times() {
        let lines = align(
            "Hello world\nGood night",
            &transcript(&[("hello", 1000), ("world", 1500), ("good", 3000), ("night", 3400)]),
            None,
        )
        .unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            starts(&lines),
            [
                ("Hello".to_string(), 1000),
                ("world".into(), 1500),
                ("Good".into(), 3000),
                ("night".into(), 3400)
            ]
        );
        assert_eq!(lines[1].start_ms, 3000);
    }

    #[test]
    fn misheard_words_still_anchor_when_alike() {
        let lines = align(
            "the quick brown fox",
            &transcript(&[("the", 100), ("quick", 500), ("brown", 900), ("box", 1300)]),
            None,
        )
        .unwrap();
        assert_eq!(lines[0].words[3].start_ms, 1300);
    }

    #[test]
    fn words_not_heard_share_the_time_between_anchors() {
        let lines = align(
            "one two three four",
            &transcript(&[("one", 1000), ("four", 4000)]),
            None,
        )
        .unwrap();
        let times = starts(&lines);
        assert_eq!(times[0].1, 1000);
        assert_eq!(times[3].1, 4000);
        assert!(
            times[1].1 > 1000 && times[2].1 > times[1].1 && times[2].1 < 4000,
            "{times:?}"
        );
    }

    #[test]
    fn failures_are_told_apart() {
        let heard = transcript(&[("hello", 0)]);
        assert_eq!(align("[Chorus]\n\n", &heard, None), Err(Failure::NoLyrics));
        assert_eq!(
            align("hello", &Transcript::default(), None),
            Err(Failure::NothingHeard)
        );
        assert_eq!(
            align("completely different words entirely here", &heard, None),
            Err(Failure::NoMatch)
        );
    }

    #[test]
    fn the_band_keeps_long_songs_cheap() {
        // 1 200 lyric units against 1 300 recognised ones (an extra chorus).
        let lyrics: String = (0..120)
            .map(|i| format!("line {i} has some words here now ok\n"))
            .collect();
        let mut words = Vec::new();
        for (i, line) in lyrics.lines().enumerate() {
            for (k, word) in line.split(' ').enumerate() {
                words.push((word.to_string(), (i * 4000 + k * 350) as u32));
            }
        }
        for k in 0..100 {
            words.push((format!("extra{k}"), 480_000 + k as u32 * 300));
        }
        let pairs: Vec<(&str, u32)> = words.iter().map(|(w, t)| (w.as_str(), *t)).collect();
        let lines = align(&lyrics, &transcript(&pairs), None).unwrap();
        assert_eq!(lines.len(), 120);
        assert_eq!(lines[119].start_ms, 119 * 4000);
    }
}

//! Words and segments from whisper.cpp's tokens.
//!
//! whisper.cpp gives each segment's tokens with their text as bytes (a token
//! may end inside a UTF-8 character), a probability and times in
//! centiseconds: `t_dtw`, when the token was spoken by dynamic time warping
//! over the model's alignment heads, and `t0`/`t1`, an estimate from the
//! timestamp tokens. Special tokens are dropped before this module.
//!
//! * A token starts at its DTW time, else at `t0`, else where the token
//!   before it starts (the segment's start for the first); never before the
//!   token before it.
//! * A token ends where the next token starts, earlier when its `t1` says it
//!   ended before; the last token ends at its `t1`, else at the segment's end.
//! * Whitespace (Whisper's leading spaces) separates words. Each Chinese,
//!   Japanese or Korean character is a word of its own, and the characters
//!   of one token share its time equally.
//! * Punctuation joins the word it touches; a "word" without letters or
//!   digits ("♪", "…") is dropped, and so is a segment without words.
//! * A word's confidence is the mean probability of its tokens.
//! * Across the transcript, word starts never decrease and a word ends after
//!   it starts and no later than the next word starts; no time is past the
//!   end of the audio. A segment runs from its first word's start to its
//!   last word's end.

use crate::{Segment, Word};

/// A text token of a segment, as whisper.cpp gives it.
#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub(crate) bytes: Vec<u8>,
    /// Probability, 0 to 1.
    pub(crate) p: f32,
    /// When the token was spoken (DTW), in centiseconds; negative when unknown.
    pub(crate) t_dtw: i64,
    /// The timestamp tokens' estimate of its start and end, in centiseconds;
    /// negative when unknown.
    pub(crate) t0: i64,
    pub(crate) t1: i64,
}

/// A segment as whisper.cpp gives it: its text, times in centiseconds and text tokens.
#[derive(Debug, Clone)]
pub(crate) struct RawSegment {
    pub(crate) text: String,
    pub(crate) t0: i64,
    pub(crate) t1: i64,
    pub(crate) tokens: Vec<Token>,
}

/// The transcript's segments, with their words, all within `[0, limit_ms]`
/// (the audio's duration).
pub(crate) fn segments(raw: &[RawSegment], limit_ms: u32) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for segment in raw {
        let mut words = words(&segment.tokens, segment.t0, segment.t1);
        // Word starts never decrease across segments.
        let floor = out.last().and_then(|s| s.words.last()).map_or(0, |w| w.start_ms);
        for word in &mut words {
            word.start_ms = word.start_ms.min(limit_ms).max(floor);
            word.end_ms = word.end_ms.min(limit_ms).max(word.start_ms);
        }
        let (Some(first), Some(last)) = (words.first(), words.last()) else {
            continue;
        };
        let (start_ms, end_ms) = (first.start_ms, last.end_ms);
        if let Some(previous) = out.last_mut().and_then(|s| s.words.last_mut()) {
            previous.end_ms = previous.end_ms.min(start_ms).max(previous.start_ms);
        }
        if let Some(previous) = out.last_mut() {
            previous.end_ms = previous.end_ms.min(start_ms).max(previous.start_ms);
        }
        out.push(Segment {
            text: segment.text.trim().to_owned(),
            start_ms,
            end_ms,
            words,
        });
    }
    out
}

/// The words of one segment spanning `[seg_t0, seg_t1]` (centiseconds).
pub(crate) fn words(tokens: &[Token], seg_t0: i64, seg_t1: i64) -> Vec<Word> {
    let spans = spans(tokens, seg_t0, seg_t1);
    let mut drafts: Vec<Draft> = Vec::new();
    // Whether the last draft may take more characters (no whitespace since).
    let mut open = false;
    for (c, token) in chars(tokens) {
        if c.is_whitespace() {
            open = false;
        } else if is_cjk(c) {
            drafts.push(Draft {
                text: c.to_string(),
                first: token,
                last: token,
                cjk: true,
            });
            open = true;
        } else {
            match drafts.last_mut() {
                // Punctuation after a CJK character joins it; letters start a word.
                Some(draft) if open && (!draft.cjk || !c.is_alphanumeric()) => {
                    draft.text.push(c);
                    draft.last = token;
                }
                _ => drafts.push(Draft {
                    text: c.to_string(),
                    first: token,
                    last: token,
                    cjk: false,
                }),
            }
            open = true;
        }
    }

    let mut words = Vec::with_capacity(drafts.len());
    for (i, draft) in drafts.iter().enumerate() {
        if !draft.text.chars().any(char::is_alphanumeric) {
            continue;
        }
        let (start, end, confidence) = if draft.cjk {
            // This token's CJK characters share its time.
            let (start, end) = spans[draft.first];
            let same: Vec<usize> = (0..drafts.len())
                .filter(|&j| drafts[j].cjk && drafts[j].first == draft.first)
                .collect();
            let n = i64::try_from(same.len()).unwrap_or(1).max(1);
            let k = i64::try_from(same.iter().position(|&j| j == i).unwrap_or(0)).unwrap_or(0);
            let step = end - start;
            (
                start + step * k / n,
                start + step * (k + 1) / n,
                tokens[draft.first].p,
            )
        } else {
            let range = draft.first..=draft.last;
            let count = (draft.last - draft.first + 1) as f32;
            let p = tokens[range].iter().map(|t| t.p).sum::<f32>() / count;
            (spans[draft.first].0, spans[draft.last].1, p)
        };
        words.push(Word {
            text: draft.text.clone(),
            start_ms: ms(start),
            end_ms: ms(end.max(start)),
            confidence: confidence.clamp(0.0, 1.0),
        });
    }
    // A word ends no later than the next one starts.
    for i in 1..words.len() {
        let next_start = words[i].start_ms;
        let word = &mut words[i - 1];
        word.end_ms = word.end_ms.min(next_start).max(word.start_ms);
    }
    words
}

/// A word being assembled: its text and the tokens of its first and last characters.
#[derive(Debug)]
struct Draft {
    text: String,
    first: usize,
    last: usize,
    cjk: bool,
}

/// Each token's start and end in centiseconds (see the module documentation).
fn spans(tokens: &[Token], seg_t0: i64, seg_t1: i64) -> Vec<(i64, i64)> {
    let mut starts = Vec::with_capacity(tokens.len());
    let mut previous = seg_t0.max(0);
    for (i, token) in tokens.iter().enumerate() {
        let known = [token.t_dtw, token.t0].into_iter().find(|&t| t >= 0);
        let start = match known {
            Some(t) if i == 0 => t,
            Some(t) => t.max(previous),
            None => previous,
        };
        starts.push(start);
        previous = start;
    }
    (0..tokens.len())
        .map(|i| {
            let start = starts[i];
            let own_end = Some(tokens[i].t1).filter(|&t1| t1 > start);
            let end = match starts.get(i + 1) {
                Some(&next) => own_end.map_or(next, |t1| t1.min(next)),
                None => own_end.unwrap_or(seg_t1),
            };
            (start, end.max(start))
        })
        .collect()
}

/// The characters of the tokens' text, each with the token its first byte
/// is in. Bytes that are not UTF-8 are skipped.
fn chars(tokens: &[Token]) -> Vec<(char, usize)> {
    let mut bytes = Vec::new();
    let mut owner = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        bytes.extend_from_slice(&token.bytes);
        owner.resize(bytes.len(), i);
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut offset = 0;
    for chunk in bytes.utf8_chunks() {
        for (at, c) in chunk.valid().char_indices() {
            out.push((c, owner[offset + at]));
        }
        offset += chunk.valid().len() + chunk.invalid().len();
    }
    out
}

/// Chinese, Japanese and Korean letters: ideographs, kana, hangul and
/// bopomofo, not the punctuation in their blocks ("・").
fn is_cjk(c: char) -> bool {
    c.is_alphanumeric()
        && matches!(c,
            '\u{1100}'..='\u{11FF}'     // Hangul Jamo
            | '\u{3005}'..='\u{3007}'   // 々 〆 〇
            | '\u{3040}'..='\u{30FF}'   // Hiragana, Katakana
            | '\u{3100}'..='\u{312F}'   // Bopomofo
            | '\u{3130}'..='\u{318F}'   // Hangul Compatibility Jamo
            | '\u{31A0}'..='\u{31BF}'   // Bopomofo Extended
            | '\u{31F0}'..='\u{31FF}'   // Katakana Phonetic Extensions
            | '\u{3400}'..='\u{4DBF}'   // CJK Extension A
            | '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
            | '\u{A960}'..='\u{A97F}'   // Hangul Jamo Extended-A
            | '\u{AC00}'..='\u{D7AF}'   // Hangul Syllables
            | '\u{D7B0}'..='\u{D7FF}'   // Hangul Jamo Extended-B
            | '\u{F900}'..='\u{FAFF}'   // CJK Compatibility Ideographs
            | '\u{FF66}'..='\u{FF9F}'   // Halfwidth Katakana
            | '\u{FFA0}'..='\u{FFDC}'   // Halfwidth Hangul
            | '\u{20000}'..='\u{323AF}' // CJK Extensions B to H, Compatibility Supplement
        )
}

/// Centiseconds to milliseconds.
fn ms(cs: i64) -> u32 {
    u32::try_from(cs.max(0).saturating_mul(10)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str, t_dtw: i64) -> Token {
        Token {
            bytes: text.as_bytes().to_vec(),
            p: 0.5,
            t_dtw,
            t0: -1,
            t1: -1,
        }
    }

    fn timed(words: &[Word]) -> Vec<(&str, u32, u32)> {
        words
            .iter()
            .map(|w| (w.text.as_str(), w.start_ms, w.end_ms))
            .collect()
    }

    #[test]
    fn english_words_from_leading_spaces() {
        let mut tokens = vec![
            token(" Hello", 100),
            token(",", 150),
            token(" wor", 160),
            token("ld", 180),
        ];
        tokens.push(token("!", 220));
        tokens[2].p = 0.9;
        tokens[3].p = 0.7;
        let words = words(&tokens, 90, 300);
        assert_eq!(timed(&words), [("Hello,", 1000, 1600), ("world!", 1600, 3000)]);
        assert!((words[1].confidence - (0.9 + 0.7 + 0.5) / 3.0).abs() < 1e-6);
    }

    #[test]
    fn each_cjk_character_is_a_word_sharing_its_token_time() {
        let tokens = [token("你好", 100), token("世界", 200)];
        assert_eq!(
            timed(&words(&tokens, 100, 300)),
            [
                ("你", 1000, 1500),
                ("好", 1500, 2000),
                ("世", 2000, 2500),
                ("界", 2500, 3000)
            ]
        );
        // Japanese: kanji and kana alike, the long vowel mark too; punctuation joins.
        let tokens = [token("東京", 0), token("タワー", 30), token("。", 60)];
        let words = words(&tokens, 0, 90);
        let texts: Vec<_> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["東", "京", "タ", "ワ", "ー。"]);
        assert_eq!((words[2].start_ms, words[4].end_ms), (300, 600));
        // Korean syllables, separated by spaces or not.
        let texts: Vec<_> = super::words(&[token(" 사랑 해", 0)], 0, 30)
            .into_iter()
            .map(|w| w.text)
            .collect();
        assert_eq!(texts, ["사", "랑", "해"]);
    }

    #[test]
    fn characters_split_across_tokens() {
        // "你" is split over two tokens; "好" is in the second.
        let bytes = "你好".as_bytes();
        let tokens = [
            Token {
                bytes: bytes[..2].to_vec(),
                p: 0.8,
                t_dtw: 100,
                t0: -1,
                t1: -1,
            },
            Token {
                bytes: bytes[2..].to_vec(),
                p: 0.6,
                t_dtw: 140,
                t0: -1,
                t1: -1,
            },
        ];
        let words = words(&tokens, 100, 200);
        assert_eq!(timed(&words), [("你", 1000, 1400), ("好", 1400, 2000)]);
        assert_eq!(words[0].confidence, 0.8);
        // Bytes that never complete a character are skipped.
        let broken = [
            Token {
                bytes: vec![0xe4, 0xbd],
                ..tokens[0]
            },
            token(" ok", 150),
        ];
        assert_eq!(timed(&super::words(&broken, 100, 200)), [("ok", 1500, 2000)]);
    }

    #[test]
    fn mixed_scripts_and_punctuation() {
        let tokens = [
            token(" I", 0),
            token(" love", 10),
            token(" 你", 20),
            token("!", 30),
            token(" ¿Qué", 40),
        ];
        let texts: Vec<_> = words(&tokens, 0, 50).into_iter().map(|w| w.text).collect();
        assert_eq!(texts, ["I", "love", "你!", "¿Qué"]);
        let tokens = [token("好", 0), token("abc", 10)];
        let texts: Vec<_> = words(&tokens, 0, 20).into_iter().map(|w| w.text).collect();
        assert_eq!(texts, ["好", "abc"]);
    }

    #[test]
    fn words_without_letters_are_dropped() {
        let tokens = [
            token(" ♪", 0),
            token(" la", 10),
            token(" ...", 20),
            token(" 2", 30),
        ];
        let texts: Vec<_> = words(&tokens, 0, 40).into_iter().map(|w| w.text).collect();
        assert_eq!(texts, ["la", "2"]);
    }

    #[test]
    fn missing_and_backward_times() {
        // No DTW time: the timestamp estimate, else the previous start.
        let tokens = [
            token(" a", -1),
            Token {
                t0: 120,
                ..token(" b", -1)
            },
            token(" c", -1),
            token(" d", 110), // DTW going backwards
        ];
        assert_eq!(
            timed(&words(&tokens, 100, 200)),
            [
                ("a", 1000, 1200),
                ("b", 1200, 1200),
                ("c", 1200, 1200),
                ("d", 1200, 2000)
            ]
        );
    }

    #[test]
    fn token_end_estimates_leave_gaps() {
        let tokens = [
            Token {
                t1: 130,
                ..token(" a", 100)
            },
            Token {
                t1: 260,
                ..token(" b", 200)
            },
        ];
        assert_eq!(
            timed(&words(&tokens, 90, 400)),
            [("a", 1000, 1300), ("b", 2000, 2600)]
        );
        // An estimate before the start is ignored.
        let tokens = [
            Token {
                t1: 50,
                ..token(" a", 100)
            },
            token(" b", 200),
        ];
        assert_eq!(timed(&words(&tokens, 90, 400))[0], ("a", 1000, 2000));
    }

    #[test]
    fn segments_are_monotonic_and_span_their_words() {
        let raw = [
            RawSegment {
                text: " One two ".into(),
                t0: 0,
                t1: 300,
                tokens: vec![token(" One", 50), token(" two", 100)],
            },
            RawSegment {
                text: " ♪ ".into(),
                t0: 300,
                t1: 400,
                tokens: vec![token(" ♪", 310)],
            },
            RawSegment {
                text: "three".into(),
                t0: 250,
                t1: 500,
                tokens: vec![token(" three", 90)],
            },
        ];
        let segments = segments(&raw, 4000);
        assert_eq!(segments.len(), 2);
        assert_eq!(
            (
                segments[0].text.as_str(),
                segments[0].start_ms,
                segments[0].end_ms
            ),
            ("One two", 500, 1000)
        );
        assert_eq!(
            timed(&segments[0].words),
            [("One", 500, 1000), ("two", 1000, 1000)]
        );
        // DTW put "three" before "two": it is moved to where "two" starts; it
        // ends with the audio, not at the segment's 5 s.
        assert_eq!(timed(&segments[1].words), [("three", 1000, 4000)]);
        assert_eq!((segments[1].start_ms, segments[1].end_ms), (1000, 4000));
        // Times past the end of the audio are brought back to it.
        let late = [RawSegment {
            text: "late".into(),
            t0: 500,
            t1: 700,
            tokens: vec![token(" late", 600)],
        }];
        assert_eq!(
            timed(&super::segments(&late, 5500)[0].words),
            [("late", 5500, 5500)]
        );
    }

    #[test]
    fn cjk_ranges() {
        for c in ['你', '々', 'ぁ', 'ア', 'ー', 'ㄅ', '한', 'ㅎ', 'ｱ', '𠀀', '丽'] {
            assert!(is_cjk(c), "{c}");
        }
        for c in ['a', 'é', 'я', '。', '，', '「', '・', '゛', ' ', '1', 'ก'] {
            assert!(!is_cjk(c), "{c}");
        }
    }

    #[test]
    fn centiseconds_to_milliseconds() {
        assert_eq!(ms(-5), 0);
        assert_eq!(ms(123), 1230);
        assert_eq!(ms(i64::MAX), u32::MAX);
    }
}

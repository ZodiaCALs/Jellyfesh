//! Text helpers shared by the parsers: normalisation, direction, scripts and lines.

use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};

use crate::model::Direction;

/// Removes a leading byte order mark.
pub(crate) fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

/// NFC-normalises `s`, borrowing nothing and allocating once.
pub(crate) fn nfc(s: &str) -> String {
    match is_nfc_quick(s.chars()) {
        IsNormalized::Yes => s.to_owned(),
        _ => s.nfc().collect(),
    }
}

/// Trims and NFC-normalises display text.
pub(crate) fn clean(s: &str) -> String {
    let n = nfc(s.trim());
    // Normalisation never adds whitespace, but trim again in case a leading
    // combining mark composed into something the trim above kept.
    if n.trim().len() == n.len() {
        n
    } else {
        n.trim().to_owned()
    }
}

/// Splits text into lines on `\n`, `\r\n` and lone `\r`.
///
/// A trailing line break does not produce a trailing empty line.
pub(crate) fn split_lines(s: &str) -> Lines<'_> {
    Lines { rest: Some(s) }
}

#[derive(Debug)]
pub(crate) struct Lines<'a> {
    rest: Option<&'a str>,
}

impl<'a> Iterator for Lines<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let rest = self.rest?;
        match rest.find(['\n', '\r']) {
            Some(i) => {
                let line = &rest[..i];
                let skip = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
                let tail = &rest[i + skip..];
                self.rest = if tail.is_empty() { None } else { Some(tail) };
                Some(line)
            }
            None => {
                self.rest = None;
                if rest.is_empty() { None } else { Some(rest) }
            }
        }
    }
}

/// Number of lines as [`split_lines`] counts them.
pub(crate) fn count_lines(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => count += 1,
            b'\r' => {
                count += 1;
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    if !matches!(bytes.last(), None | Some(b'\n' | b'\r')) {
        count += 1;
    }
    count
}

/// True for characters of right-to-left scripts (Hebrew, Arabic, Syriac, Thaana, N'Ko and
/// historic RTL scripts), letters or not.
fn in_rtl_block(c: char) -> bool {
    matches!(c as u32,
        0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFE | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// Direction from the first strong character: right-to-left letters give
/// [`Direction::Rtl`], any other letter (Latin, CJK, Cyrillic, …) gives
/// [`Direction::Ltr`], and text without letters gives [`Direction::Auto`].
/// The marks U+200E and U+200F count as strong characters.
pub(crate) fn direction(s: &str) -> Direction {
    for c in s.chars() {
        match c {
            '\u{200F}' => return Direction::Rtl,
            '\u{200E}' => return Direction::Ltr,
            _ if c.is_alphabetic() => {
                return if in_rtl_block(c) {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                };
            }
            _ => {}
        }
    }
    Direction::Auto
}

/// Han, kana, Hangul and Bopomofo characters, plus the ideographic iteration marks.
pub(crate) fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF       // Hangul Jamo
        | 0x3005..=0x3007     // 々 〆 〇
        | 0x3040..=0x30FF     // Hiragana, Katakana
        | 0x3100..=0x312F     // Bopomofo
        | 0x3130..=0x318F     // Hangul compatibility Jamo
        | 0x31A0..=0x31BF     // Bopomofo extended
        | 0x31F0..=0x31FF     // Katakana phonetic extensions
        | 0x3400..=0x4DBF     // CJK extension A
        | 0x4E00..=0x9FFF     // CJK unified ideographs
        | 0xA960..=0xA97F     // Hangul Jamo extended A
        | 0xAC00..=0xD7FF     // Hangul syllables, Jamo extended B
        | 0xF900..=0xFAFF     // CJK compatibility ideographs
        | 0xFF66..=0xFF9F     // half-width Katakana
        | 0x20000..=0x3134F) // CJK extensions B–G
}

/// Latin letters (ASCII, Latin-1, Latin Extended A/B and Additional) and combining diacritics.
fn is_latin(c: char) -> bool {
    c.is_ascii_alphabetic()
        || matches!(c as u32, 0x00C0..=0x024F | 0x0300..=0x036F | 0x1E00..=0x1EFF) && c != '×' && c != '÷'
}

/// The dominant script of a line, for the bilingual LRC heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Script {
    Latin,
    Cjk,
}

/// Classifies a line as mostly Latin or mostly CJK.
///
/// Every CJK character counts as one unit and every run of Latin letters (a
/// word) counts as one unit, as do runs of other letters. A script is dominant
/// when it has at least 60% of the units; otherwise the line has no dominant
/// script and the function returns `None`.
pub(crate) fn dominant_script(s: &str) -> Option<Script> {
    let (mut latin, mut cjk, mut other) = (0u32, 0u32, 0u32);
    #[derive(PartialEq)]
    enum Run {
        None,
        Latin,
        Other,
    }
    let mut run = Run::None;
    for c in s.chars() {
        if is_cjk(c) {
            cjk += 1;
            run = Run::None;
        } else if is_latin(c) {
            if run != Run::Latin {
                latin += 1;
                run = Run::Latin;
            }
        } else if c.is_alphabetic() {
            if run != Run::Other {
                other += 1;
                run = Run::Other;
            }
        } else {
            run = Run::None;
        }
    }
    let total = latin + cjk + other;
    if total == 0 {
        None
    } else if latin * 10 >= total * 6 {
        Some(Script::Latin)
    } else if cjk * 10 >= total * 6 {
        Some(Script::Cjk)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_split_on_every_line_break() {
        let v: Vec<&str> = split_lines("a\r\nb\nc\rd\n").collect();
        assert_eq!(v, ["a", "b", "c", "d"]);
        let v: Vec<&str> = split_lines("a\n\nb").collect();
        assert_eq!(v, ["a", "", "b"]);
        assert_eq!(split_lines("").count(), 0);
        assert_eq!(split_lines("\n").collect::<Vec<_>>(), [""]);
    }

    #[test]
    fn count_matches_split() {
        for s in ["", "a", "a\n", "a\r\nb", "\n\n", "a\rb\r", "x\r\n\r\ny"] {
            assert_eq!(count_lines(s), split_lines(s).count(), "{s:?}");
        }
    }

    #[test]
    fn direction_uses_first_strong_character() {
        assert_eq!(direction("Hello"), Direction::Ltr);
        assert_eq!(direction("夜の歌"), Direction::Ltr);
        assert_eq!(direction("שלום"), Direction::Rtl);
        assert_eq!(direction("١٢ القمر"), Direction::Rtl); // Arabic-Indic digits are weak
        assert_eq!(direction("12, «القمر» and"), Direction::Rtl);
        assert_eq!(direction("(ok) سلام"), Direction::Ltr);
        assert_eq!(direction("... 123 !"), Direction::Auto);
        assert_eq!(direction(""), Direction::Auto);
        assert_eq!(direction("\u{200F}123"), Direction::Rtl);
    }

    #[test]
    fn dominant_script_weighs_words_against_characters() {
        assert_eq!(
            dominant_script("Walking under paper lanterns"),
            Some(Script::Latin)
        );
        assert_eq!(dominant_script("走在纸灯笼下"), Some(Script::Cjk));
        assert_eq!(dominant_script("我爱 Tokyo"), Some(Script::Cjk));
        assert_eq!(dominant_script("I love 東京"), None);
        assert_eq!(dominant_script("Café déjà vu"), Some(Script::Latin));
        assert_eq!(dominant_script("Привет мир"), None);
        assert_eq!(dominant_script("123 ..."), None);
    }

    #[test]
    fn clean_trims_and_composes() {
        assert_eq!(clean("  e\u{301}cole \t"), "\u{e9}cole");
        assert_eq!(clean("\u{3000}歌\u{3000}"), "歌");
    }
}

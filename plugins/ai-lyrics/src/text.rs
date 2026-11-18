//! Text for alignment (ADR-0019 §6): lyrics and recognised words split into
//! units (words in space-separated scripts, characters in Chinese, Japanese
//! and Korean) and normalised for matching (NFKC, case folding, katakana as
//! hiragana, punctuation removed).

use unicode_normalization::UnicodeNormalization;

/// Whether `c` belongs to a script written without spaces between words:
/// Han, kana, Hangul, and the marks that live with them (`々`, `〆`).
pub fn is_cjk(c: char) -> bool {
    matches!(
        u32::from(c),
        0x1100..=0x11FF       // Hangul Jamo
            | 0x3005..=0x3007 // 々 〆 〇
            | 0x3040..=0x30FF // Hiragana, Katakana (with ー)
            | 0x3130..=0x318F // Hangul compatibility Jamo
            | 0x31F0..=0x31FF // Katakana phonetic extensions
            | 0x3400..=0x4DBF // CJK extension A
            | 0x4E00..=0x9FFF // CJK unified ideographs
            | 0xA960..=0xA97F // Hangul Jamo extended A
            | 0xAC00..=0xD7AF // Hangul syllables
            | 0xF900..=0xFAFF // CJK compatibility ideographs
            | 0xFF66..=0xFF9F // half-width katakana
            | 0x20000..=0x3134F // CJK extensions B–G, compatibility supplement
    )
}

/// Whether `c` is part of a word (a letter, a digit, a combining mark, or
/// an apostrophe inside a word: "don't").
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || is_mark(c)
}

/// Combining marks (accents written separately, Thai and Indic vowel signs).
fn is_mark(c: char) -> bool {
    matches!(
        u32::from(c),
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05C7 | 0x0610..=0x061A | 0x064B..=0x065F
            | 0x0900..=0x0903 | 0x093A..=0x094F | 0x0E31 | 0x0E34..=0x0E3A | 0x0E47..=0x0E4E
            | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
    )
}

/// The key two units are compared by: NFKC, case folded, katakana written
/// as hiragana, and nothing but letters and digits.
pub fn key(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.nfkc() {
        if !(is_word_char(c) || is_cjk(c)) {
            continue;
        }
        let c = match u32::from(c) {
            // Katakana ァ..ヶ to hiragana ぁ..ゖ: the same sounds.
            n @ 0x30A1..=0x30F6 => char::from_u32(n - 0x60).unwrap_or(c),
            _ => c,
        };
        match c {
            'ß' => out.push_str("ss"),
            'ς' => out.push('σ'),
            _ => out.extend(c.to_lowercase()),
        }
    }
    out
}

/// One unit of text: a word, or one CJK character, with the punctuation
/// written around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// As written, with its punctuation ("Hello," "我").
    pub text: String,
    /// What it is compared by ([`key`]).
    pub key: String,
    /// Written without a space before the next unit (CJK characters, and a
    /// word glued to one).
    pub joins_next: bool,
}

impl Unit {
    /// Its weight when time is shared by length: its key's characters, at
    /// least one, a CJK character counting as two letters.
    pub fn weight(&self) -> u32 {
        let letters: u32 = self.key.chars().map(|c| if is_cjk(c) { 2 } else { 1 }).sum();
        letters.max(1)
    }

    /// Whether it is a CJK character.
    pub fn is_cjk(&self) -> bool {
        self.key.chars().next().is_some_and(is_cjk)
    }
}

/// Splits a line of text into units. Punctuation stays with the unit it
/// touches (an opening bracket or quote with the next one); punctuation
/// standing alone joins the unit before it. Text without letters or digits
/// gives no units.
pub fn units(line: &str) -> Vec<Unit> {
    let mut out: Vec<Unit> = Vec::new();
    // Punctuation waiting for the next unit ("(", "“").
    let mut prefix = String::new();
    // The word being read, and whether it has letters yet.
    let mut word = String::new();
    let mut word_has_letters = false;
    // Whether whitespace came since the last unit ended.
    let mut spaced = true;

    fn finish(out: &mut Vec<Unit>, word: &mut String, has_letters: &mut bool, prefix: &mut String) {
        if word.is_empty() {
            return;
        }
        if *has_letters {
            let text = std::mem::take(prefix) + word;
            out.push(Unit {
                key: key(&text),
                text,
                joins_next: false,
            });
        } else {
            // Punctuation only: it belongs to what it touches.
            prefix.push_str(word);
        }
        word.clear();
        *has_letters = false;
    }

    for c in line.chars() {
        if c.is_whitespace() {
            finish(&mut out, &mut word, &mut word_has_letters, &mut prefix);
            // Punctuation that stood alone ("—", "…") joins the unit before.
            if !prefix.is_empty()
                && let Some(last) = out.last_mut()
                && spaced_punctuation(&prefix)
            {
                last.text.push(' ');
                last.text.push_str(&std::mem::take(&mut prefix));
            }
            spaced = true;
            continue;
        }
        if is_cjk(c) {
            if word_has_letters {
                finish(&mut out, &mut word, &mut word_has_letters, &mut prefix);
                if let Some(last) = out.last_mut() {
                    last.joins_next = true;
                }
            } else if !word.is_empty() {
                // Punctuation right before the character: "「愛".
                prefix.push_str(&std::mem::take(&mut word));
            }
            if !spaced
                && prefix.is_empty()
                && let Some(last) = out.last_mut()
            {
                last.joins_next = true;
            }
            let text = std::mem::take(&mut prefix) + &c.to_string();
            out.push(Unit {
                key: key(&text),
                text,
                joins_next: false,
            });
            spaced = false;
            continue;
        }
        if is_word_char(c) {
            if word.is_empty()
                && !spaced
                && prefix.is_empty()
                && let Some(last) = out.last_mut()
            {
                // A word glued to the character before it: "我love".
                if last.is_cjk() {
                    last.joins_next = true;
                }
            }
            word.push(c);
            word_has_letters = true;
            spaced = false;
            continue;
        }
        // Punctuation or a symbol.
        if word_has_letters {
            word.push(c);
        } else if word.is_empty()
            && !spaced
            && let Some(last) = out.last_mut()
        {
            // Right after a unit: "好，" "end."
            last.text.push(c);
        } else {
            word.push(c);
        }
    }
    finish(&mut out, &mut word, &mut word_has_letters, &mut prefix);
    if !prefix.is_empty()
        && let Some(last) = out.last_mut()
    {
        last.text.push(' ');
        last.text.push_str(&prefix);
    }
    if let Some(last) = out.last_mut() {
        last.joins_next = false;
    }
    out
}

/// Whether punctuation standing alone between spaces is a pause mark that
/// joins the word before it ("-", "—", "…", "!") rather than an opening
/// bracket or quote that waits for the next word.
fn spaced_punctuation(text: &str) -> bool {
    !text.chars().last().is_some_and(|c| {
        matches!(
            c,
            '(' | '[' | '{' | '“' | '‘' | '「' | '『' | '《' | '〈' | '（' | '"' | '\''
        )
    })
}

/// Whether a line of plain lyrics is a section label rather than words to
/// sing: `[Chorus]`, `(Verse 2)`, `【副歌】`.
pub fn is_label(line: &str) -> bool {
    let line = line.trim();
    let pairs = [('[', ']'), ('【', '】'), ('〔', '〕')];
    pairs
        .iter()
        .any(|(open, close)| line.starts_with(*open) && line.ends_with(*close))
        || (line.starts_with('(') && line.ends_with(')') && line.chars().count() <= 24 && {
            let inner = key(line).to_lowercase();
            [
                "verse",
                "chorus",
                "bridge",
                "intro",
                "outro",
                "hook",
                "refrain",
                "prechorus",
            ]
            .iter()
            .any(|w| inner.starts_with(w))
        })
}

/// About how many Whisper tokens `text` takes: a CJK character about one,
/// a word of another script about one and a half.
fn tokens(text: &str) -> usize {
    let cjk = text.chars().filter(|c| is_cjk(*c)).count();
    let words = text
        .split(|c: char| c.is_whitespace() || is_cjk(c))
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count();
    cjk + (words * 3).div_ceil(2)
}

/// The most of the lyrics given to the recogniser as its prompt. Whisper
/// reads at most 224 tokens of a prompt, the last ones when it is longer;
/// the lyrics' start, cut at a line, steers it from the first words.
const PROMPT_TOKENS: usize = 200;

/// A prompt for the recogniser from plain lyrics: their first lines (labels
/// such as `[Chorus]` left out), as many as fit in about 200 tokens, cut
/// between words in a line that does not fit. `None` without words.
pub fn prompt(lyrics: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut used = 0;
    for line in lyrics.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() || is_label(&line) || units(&line).is_empty() {
            continue;
        }
        let cost = tokens(&line);
        if used + cost <= PROMPT_TOKENS {
            used += cost;
            lines.push(line);
            continue;
        }
        // Part of the line, unit by unit.
        let mut part = String::new();
        for unit in units(&line) {
            let cost = tokens(&unit.text);
            if used + cost > PROMPT_TOKENS {
                break;
            }
            used += cost;
            part.push_str(&unit.text);
            if !unit.joins_next {
                part.push(' ');
            }
        }
        let part = part.trim_end().to_string();
        if !part.is_empty() {
            lines.push(part);
        }
        break;
    }
    // Lines joined by a space: the app keeps no line breaks in a prompt.
    (!lines.is_empty()).then(|| lines.join(" "))
}

/// The primary language subtag of a BCP 47 tag (`ja` of `ja-JP`), for a
/// recogniser's language hint.
pub fn primary_language(tag: &str) -> Option<String> {
    let primary = tag.trim().split(['-', '_']).next()?.to_ascii_lowercase();
    ((2..=3).contains(&primary.len()) && primary.bytes().all(|b| b.is_ascii_lowercase())).then_some(primary)
}

/// Levenshtein distance over characters.
pub fn distance(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// How alike two keys are: 1 for the same, 0 for nothing in common
/// (1 − edit distance / the longer length).
pub fn similarity(a: &[char], b: &[char]) -> f32 {
    if a == b {
        return 1.0;
    }
    let longer = a.len().max(b.len());
    if longer == 0 {
        return 1.0;
    }
    1.0 - distance(a, b) as f32 / longer as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(line: &str) -> Vec<(String, String, bool)> {
        units(line)
            .into_iter()
            .map(|u| (u.text, u.key, u.joins_next))
            .collect()
    }

    #[test]
    fn keys_fold_case_width_kana_and_punctuation() {
        assert_eq!(key("Don't!"), "dont");
        assert_eq!(key("ＨＥＬＬＯ"), "hello");
        assert_eq!(key("Straße"), "strasse");
        assert_eq!(key("ラブ"), key("らぶ"));
        assert_eq!(key("ｶﾞ"), key("が"), "half-width katakana and its voicing mark");
        assert_eq!(key("“Café,”"), "café");
        assert_eq!(key("…"), "");
    }

    #[test]
    fn latin_lines_split_at_spaces_with_punctuation_kept() {
        assert_eq!(
            texts("Hello, world - (oh) yeah!"),
            vec![
                ("Hello,".into(), "hello".into(), false),
                ("world -".into(), "world".into(), false),
                ("(oh)".into(), "oh".into(), false),
                ("yeah!".into(), "yeah".into(), false),
            ]
        );
        assert!(units("— … ♪").is_empty());
    }

    #[test]
    fn cjk_lines_split_into_characters() {
        assert_eq!(
            texts("你好，世界"),
            vec![
                ("你".into(), "你".into(), true),
                ("好，".into(), "好".into(), true),
                ("世".into(), "世".into(), true),
                ("界".into(), "界".into(), false),
            ]
        );
        assert_eq!(
            texts("「愛してる」"),
            vec![
                ("「愛".into(), "愛".into(), true),
                ("し".into(), "し".into(), true),
                ("て".into(), "て".into(), true),
                ("る」".into(), "る".into(), false),
            ]
        );
    }

    #[test]
    fn mixed_lines_join_where_nothing_separates_them() {
        assert_eq!(
            texts("我love你 so much"),
            vec![
                ("我".into(), "我".into(), true),
                ("love".into(), "love".into(), true),
                ("你".into(), "你".into(), false),
                ("so".into(), "so".into(), false),
                ("much".into(), "much".into(), false),
            ]
        );
        let korean = units("사랑해 너를");
        assert_eq!(korean.len(), 5);
        assert!(
            korean[2].text == "해" && !korean[2].joins_next,
            "a space after 해"
        );
    }

    #[test]
    fn labels_are_not_sung() {
        for label in [
            "[Chorus]",
            "[Verse 2: Ana]",
            "(Chorus)",
            "(Pre-Chorus x2)",
            "【副歌】",
        ] {
            assert!(is_label(label), "{label}");
        }
        for line in [
            "(I want you)",
            "Chorus girls",
            "Oh (chorus of birds singing loud)",
        ] {
            assert!(!is_label(line), "{line}");
        }
    }

    #[test]
    fn prompts_take_the_first_lines_that_fit() {
        let lyrics = "[Verse]\nThe first line here\n\nThe second line\n";
        assert_eq!(
            prompt(lyrics).as_deref(),
            Some("The first line here The second line")
        );
        let long: String = (0..200).map(|i| format!("word{i} ")).collect();
        let cut = prompt(&long).unwrap();
        assert!(tokens(&cut) <= PROMPT_TOKENS, "{}", tokens(&cut));
        assert!(cut.starts_with("word0 word1 "));
        let chinese = "我".repeat(500);
        assert_eq!(prompt(&chinese).unwrap().chars().count(), PROMPT_TOKENS);
        assert_eq!(prompt("\n[Intro]\n…\n"), None);
    }

    #[test]
    fn languages_and_similarity() {
        assert_eq!(primary_language("ja-JP").as_deref(), Some("ja"));
        assert_eq!(primary_language("zh_Hant").as_deref(), Some("zh"));
        assert_eq!(primary_language("x"), None);
        let chars = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(distance(&chars("kitten"), &chars("sitting")), 3);
        assert!((similarity(&chars("fox"), &chars("box")) - 2.0 / 3.0).abs() < 1e-6);
        assert_eq!(similarity(&chars("a"), &chars("a")), 1.0);
        assert_eq!(similarity(&chars("我"), &chars("你")), 0.0);
    }
}

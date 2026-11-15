//! Text normalisation shared by indexing and search.

use unicode_normalization::UnicodeNormalization;

/// Normalises text for matching: NFKC (folds full-width and half-width forms,
/// ligatures and other compatibility characters), lowercase, Latin/Greek/Cyrillic
/// diacritics removed, whitespace collapsed to single spaces and trimmed.
///
/// Only combining marks in U+0300–U+036F are removed, so Japanese voicing marks
/// (U+3099/U+309A) and Arabic vowel marks survive: `が` never becomes `か`.
pub fn normalize(text: &str) -> String {
    let lower = text.nfkc().collect::<String>().to_lowercase();
    let folded: String = lower
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036F}').contains(c))
        .nfc()
        .collect();
    let mut out = String::with_capacity(folded.len());
    for word in folded.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn folds_case_width_and_diacritics() {
        assert_eq!(normalize("Beyoncé"), "beyonce");
        assert_eq!(normalize("ＭＯＯＮ　Ｌｉｇｈｔ"), "moon light");
        assert_eq!(normalize("Sigur Rós"), "sigur ros");
        assert_eq!(normalize("ﬁre"), "fire");
        assert_eq!(normalize("  a \t b  "), "a b");
    }

    #[test]
    fn keeps_cjk_and_arabic_marks() {
        assert_eq!(normalize("がぎ"), "がぎ");
        assert_eq!(normalize("ｶﾞｷﾞ"), "ガギ");
        assert_eq!(normalize("晴天"), "晴天");
        assert_eq!(normalize("سَلام"), "سَلام");
    }
}

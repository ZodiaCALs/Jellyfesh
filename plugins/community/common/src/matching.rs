//! Finding a service's own track for an item from another source.
//!
//! Jellyfesh asks a lyrics or artwork plugin about items from any source
//! (local files, other plugins). The plugin searches its service by title and
//! artist and keeps a result only when it is clearly the same recording:
//! the same title, the same primary artist when both are known, and a
//! duration within three seconds when both are known. Wrong lyrics are worse
//! than none.

use jf_plugin_sdk::Item;

/// Durations further apart than this are different recordings.
pub const DURATION_TOLERANCE_MS: u32 = 3000;

/// What a search result offers for matching.
#[derive(Debug, Clone, Copy, Default)]
pub struct Candidate<'a> {
    pub title: &'a str,
    pub artist: Option<&'a str>,
    pub album: Option<&'a str>,
    pub duration_ms: Option<u32>,
}

/// Full-width ASCII folded to ASCII, lowercase, bracketed parts and a
/// trailing "feat. …" removed, punctuation turned into spaces, white space
/// collapsed.
pub fn normalize(text: &str) -> String {
    let folded: String = text.chars().map(fold_width).collect::<String>().to_lowercase();
    let mut kept = String::with_capacity(folded.len());
    let mut depth = 0usize;
    for c in folded.chars() {
        match c {
            '(' | '[' | '{' | '【' | '〔' | '「' => depth += 1,
            ')' | ']' | '}' | '】' | '〕' | '」' => depth = depth.saturating_sub(1),
            _ if depth == 0 => kept.push(c),
            _ => {}
        }
    }
    for marker in [" feat. ", " feat ", " ft. ", " featuring "] {
        if let Some(at) = kept.find(marker) {
            kept.truncate(at);
        }
    }
    words(&kept)
}

/// The first artist of a display artist ("A, B feat. C" → "a").
pub fn primary_artist(artist: &str) -> String {
    let folded: String = artist.chars().map(fold_width).collect::<String>().to_lowercase();
    let mut cut = folded.len();
    for separator in [
        ",",
        "&",
        ";",
        "、",
        "，",
        "/",
        " x ",
        " × ",
        " feat.",
        " feat ",
        " ft.",
        " ft ",
        " featuring ",
        " with ",
        " vs.",
        " vs ",
    ] {
        if let Some(at) = folded.find(separator) {
            cut = cut.min(at);
        }
    }
    words(&folded[..cut])
}

fn fold_width(c: char) -> char {
    match c {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        '\u{3000}' => ' ',
        _ => c,
    }
}

fn words(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn same_title(a: &str, b: &str) -> bool {
    let (na, nb) = (normalize(a), normalize(b));
    if na.is_empty() || nb.is_empty() {
        // A title of punctuation only: compare it as written.
        return a.trim().to_lowercase() == b.trim().to_lowercase();
    }
    na == nb
}

fn same_artist(wanted: &str, found: &str) -> bool {
    let (pw, pf) = (primary_artist(wanted), primary_artist(found));
    if pw.is_empty() || pf.is_empty() {
        return true;
    }
    if pw == pf {
        return true;
    }
    // "A, B" against "B, A" or "A feat. B": the primary artist of one appears
    // among the artists of the other.
    let (all_w, all_f) = (normalize(wanted), normalize(found));
    contains_words(&all_f, &pw) || contains_words(&all_w, &pf)
}

/// Whether the words of `needle` appear together, as whole words, in `haystack`.
fn contains_words(haystack: &str, needle: &str) -> bool {
    let padded = format!(" {haystack} ");
    padded.contains(&format!(" {needle} "))
}

/// How well `found` matches `wanted`: `None` when it is not the same
/// recording, a higher number for a better match otherwise.
pub fn score(wanted: &Item, found: &Candidate<'_>) -> Option<u32> {
    if wanted.title.trim().is_empty() || !same_title(&wanted.title, found.title) {
        return None;
    }
    if let (Some(w), Some(f)) = (wanted.artist.as_deref(), found.artist)
        && !same_artist(w, f)
    {
        return None;
    }
    let mut score = 100;
    if let (Some(w), Some(f)) = (wanted.duration_ms, found.duration_ms) {
        let diff = w.abs_diff(f);
        if diff > DURATION_TOLERANCE_MS {
            return None;
        }
        if diff <= 1000 {
            score += 10;
        }
    }
    if let (Some(w), Some(f)) = (wanted.album.as_deref(), found.album)
        && normalize(w) == normalize(f)
        && !normalize(w).is_empty()
    {
        score += 20;
    }
    Some(score)
}

/// The best match for `wanted` in `list` (the first of equally good ones,
/// so the service's relevance order breaks ties).
pub fn best<'l, T>(wanted: &Item, list: &'l [T], candidate: impl Fn(&T) -> Candidate<'_>) -> Option<&'l T> {
    let mut best: Option<(u32, &T)> = None;
    for entry in list {
        if let Some(score) = score(wanted, &candidate(entry))
            && best.is_none_or(|(b, _)| score > b)
        {
            best = Some((score, entry));
        }
    }
    best.map(|(_, entry)| entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_sdk::ItemKind;

    fn wanted(title: &str, artist: Option<&str>, ms: Option<u32>) -> Item {
        let mut item = Item::new(ItemKind::Track, "foreign:1", title);
        item.artist = artist.map(str::to_string);
        item.duration_ms = ms;
        item
    }

    fn cand<'a>(title: &'a str, artist: &'a str, ms: u32) -> Candidate<'a> {
        Candidate {
            title,
            artist: Some(artist),
            album: None,
            duration_ms: Some(ms),
        }
    }

    #[test]
    fn normalizes() {
        assert_eq!(normalize("Ｈｅｌｌｏ（Live）"), "hello");
        assert_eq!(normalize("Song [Remastered] feat. X"), "song");
        assert_eq!(normalize("  晴天 "), "晴天");
        assert_eq!(primary_artist("A / B"), "a");
        assert_eq!(primary_artist("Mock Band、Other"), "mock band");
    }

    #[test]
    fn matches_the_same_recording_only() {
        let item = wanted("Mock Song", Some("Mock Artist, Guest"), Some(200_000));
        assert!(score(&item, &cand("mock song", "Mock Artist", 201_500)).is_some());
        assert!(score(&item, &cand("Mock Song (Live)", "Guest / Mock Artist", 199_000)).is_some());
        assert!(score(&item, &cand("Mock Song", "Someone Else", 200_000)).is_none());
        assert!(score(&item, &cand("Other Song", "Mock Artist", 200_000)).is_none());
        assert!(score(&item, &cand("Mock Song", "Mock Artist", 210_000)).is_none());
        let no_artist = wanted("Mock Song", None, None);
        assert!(score(&no_artist, &cand("Mock Song", "Anyone", 1)).is_some());
        assert!(score(&wanted("", None, None), &cand("", "A", 1)).is_none());
    }

    #[test]
    fn picks_the_closest() {
        let item = wanted("Mock Song", Some("Mock Artist"), Some(200_000));
        let list = [
            ("Mock Song", "Mock Artist", 202_500),
            ("Mock Song", "Mock Artist", 200_400),
            ("Mock Song", "Mock Artist", 200_100),
        ];
        let found = best(&item, &list, |(t, a, ms)| cand(t, a, *ms)).unwrap();
        assert_eq!(found.2, 200_400, "the first of the closest");
        assert!(best(&item, &list[..0], |(t, a, ms)| cand(t, a, *ms)).is_none());
    }
}

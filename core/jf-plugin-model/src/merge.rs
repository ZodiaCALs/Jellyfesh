//! Unified search: merging the results of several sources into one list.
//!
//! [`merge`] is a pure function. The core fans a query out to the enabled
//! plugins (each with its own deadline), then merges what came back. Sources
//! whose content policy forbids unified search
//! ([`crate::types::ContentPolicy::unified_search`]) must be left out by the caller.
//!
//! Two items are the same when, in this order:
//!
//! 1. they have the same ISRC; or
//! 2. they are the same kind and have the same normalised title and primary
//!    artist, and, for tracks, durations within ±2 s (both known) and no
//!    conflicting ISRCs. Albums need title and artist, artists the name;
//!    playlists and folders are never merged.
//!
//! Normalisation: NFKC, lowercase, Latin diacritics removed, bracketed parts
//! (`(Remastered 2011)`, `[Live]`, `【…】`) and a trailing `feat. …` removed,
//! punctuation turned into spaces, whitespace collapsed. The primary artist
//! is the part before the first `,` `&` `;` `、` ` x ` `feat.` … separator.
//! Traditional/Simplified Chinese folding is not done yet.
//!
//! Order: results are interleaved by rank (first item of each source in the
//! order given, then the second items, …), and an item that matches an
//! earlier entry joins it instead of appearing again. So each source's own
//! order is kept among the entries it contributes, and a source never has two
//! of its items merged into one entry.

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::sanitize::isrc as canonical_isrc;
use crate::types::{Item, ItemKind};

/// Durations closer than this are the same recording.
pub const DURATION_TOLERANCE_MS: u32 = 2000;

/// One entry of a merged list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergedItem {
    /// The item as the source that returned it first describes it, with
    /// empty fields (ISRC, duration, album, artwork, year) filled in from
    /// the duplicates.
    pub item: Item,
    /// Every source that has it, in merge order.
    pub sources: Vec<SourceRef>,
}

/// One source's copy of a merged item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    /// The plugin id (or any source name the caller uses).
    pub source: String,
    /// The item's id in that source.
    pub id: String,
    /// Whether that source can play it.
    pub playable: bool,
}

struct Entry {
    merged: MergedItem,
    isrcs: Vec<String>,
    title: String,
    artist: String,
    durations: Vec<u32>,
}

/// Merges `(source, items)` lists; see the module docs.
pub fn merge(sources: &[(&str, &[Item])]) -> Vec<MergedItem> {
    let mut entries: Vec<Entry> = Vec::new();
    let longest = sources.iter().map(|(_, items)| items.len()).max().unwrap_or(0);
    for rank in 0..longest {
        for (source, items) in sources {
            let Some(item) = items.get(rank) else {
                continue;
            };
            let isrc = item.isrc.as_deref().and_then(canonical_isrc);
            let title = normalize_title(&item.title);
            let artist = item.artist.as_deref().map(primary_artist).unwrap_or_default();
            let found = find(&entries, source, item, isrc.as_deref(), &title, &artist);
            let reference = SourceRef {
                source: source.to_string(),
                id: item.id.clone(),
                playable: item.playable,
            };
            match found {
                Some(index) => {
                    let entry = &mut entries[index];
                    entry.merged.sources.push(reference);
                    if let Some(isrc) = isrc
                        && !entry.isrcs.contains(&isrc)
                    {
                        entry.isrcs.push(isrc);
                    }
                    entry.durations.extend(item.duration_ms);
                    fill_gaps(&mut entry.merged.item, item);
                }
                None => entries.push(Entry {
                    merged: MergedItem {
                        item: item.clone(),
                        sources: vec![reference],
                    },
                    isrcs: isrc.into_iter().collect(),
                    title,
                    artist,
                    durations: item.duration_ms.into_iter().collect(),
                }),
            }
        }
    }
    entries.into_iter().map(|e| e.merged).collect()
}

fn find(
    entries: &[Entry],
    source: &str,
    item: &Item,
    isrc: Option<&str>,
    title: &str,
    artist: &str,
) -> Option<usize> {
    let candidates = || {
        entries.iter().enumerate().filter(|(_, e)| {
            e.merged.item.kind == item.kind && !e.merged.sources.iter().any(|s| s.source == source)
        })
    };
    if let Some(isrc) = isrc
        && let Some((index, _)) = candidates().find(|(_, e)| e.isrcs.iter().any(|i| i == isrc))
    {
        return Some(index);
    }
    if title.is_empty() {
        return None;
    }
    candidates()
        .find(|(_, e)| {
            if e.title != title {
                return false;
            }
            match item.kind {
                ItemKind::Track => {
                    let isrc_conflict = isrc.is_some() && !e.isrcs.is_empty();
                    // Within the tolerance of every copy already merged, so
                    // matches cannot drift along a chain of near misses.
                    let close = !e.durations.is_empty()
                        && item.duration_ms.is_some_and(|d| {
                            e.durations
                                .iter()
                                .all(|other| d.abs_diff(*other) <= DURATION_TOLERANCE_MS)
                        });
                    !isrc_conflict && close && !artist.is_empty() && e.artist == artist
                }
                ItemKind::Album => !artist.is_empty() && e.artist == artist,
                ItemKind::Artist => true,
                ItemKind::Playlist | ItemKind::Folder => false,
            }
        })
        .map(|(index, _)| index)
}

fn fill_gaps(target: &mut Item, other: &Item) {
    if target.isrc.is_none() {
        target.isrc.clone_from(&other.isrc);
    }
    if target.duration_ms.is_none() {
        target.duration_ms = other.duration_ms;
    }
    if target.album.is_none() {
        target.album.clone_from(&other.album);
    }
    if target.artwork.is_none() {
        target.artwork.clone_from(&other.artwork);
    }
    if target.year.is_none() {
        target.year = other.year;
    }
}

/// NFKC, lowercase, Latin diacritics removed.
fn fold(text: &str) -> String {
    let lower = text.nfkc().collect::<String>().to_lowercase();
    lower
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036F}').contains(c))
        .nfc()
        .collect()
}

/// Punctuation to spaces, whitespace collapsed.
fn words(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A title for matching: see the module docs.
pub fn normalize_title(title: &str) -> String {
    let folded = fold(title);
    let mut out = String::with_capacity(folded.len());
    let mut depth = 0usize;
    for c in folded.chars() {
        match c {
            '(' | '[' | '{' | '【' | '〔' => depth += 1,
            ')' | ']' | '}' | '】' | '〕' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    for marker in [" feat. ", " feat ", " ft. ", " featuring "] {
        if let Some(at) = out.find(marker) {
            out.truncate(at);
        }
    }
    // " - Remastered 2011" and similar suffixes.
    if let Some(at) = out.rfind(" - ") {
        let suffix = &out[at + 3..];
        if ["remaster", "live", "version", "edit", "mono", "stereo", "mix"]
            .iter()
            .any(|w| suffix.contains(w))
        {
            out.truncate(at);
        }
    }
    words(&out)
}

/// The primary artist, for matching: see the module docs.
pub fn primary_artist(artist: &str) -> String {
    let folded = fold(artist);
    let mut cut = folded.len();
    // Not "/": it belongs to names like AC/DC more often than it separates.
    for separator in [
        ",",
        "&",
        ";",
        "、",
        "，",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, title: &str, artist: &str, duration_ms: u32, isrc: Option<&str>) -> Item {
        Item {
            id: id.into(),
            kind: ItemKind::Track,
            title: title.into(),
            artist: Some(artist.into()),
            album: None,
            album_id: None,
            artist_id: None,
            duration_ms: Some(duration_ms),
            track_number: None,
            disc_number: None,
            year: None,
            child_count: None,
            isrc: isrc.map(str::to_string),
            artwork: None,
            explicit: false,
            playable: true,
            unplayable: None,
        }
    }

    fn ids(merged: &[MergedItem]) -> Vec<Vec<String>> {
        merged
            .iter()
            .map(|m| {
                m.sources
                    .iter()
                    .map(|s| format!("{}:{}", s.source, s.id))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn merges_by_isrc_first() {
        let a = [track("1", "Song", "Artist", 200_000, Some("USRC17607839"))];
        let b = [track(
            "x",
            "Song - 2011 Remaster",
            "Someone Else",
            260_000,
            Some("us-rc1-76-07839"),
        )];
        let merged = merge(&[("a", &a), ("b", &b)]);
        assert_eq!(ids(&merged), vec![vec!["a:1", "b:x"]]);
    }

    #[test]
    fn merges_by_title_artist_and_duration() {
        let a = [track(
            "1",
            "Bohemian Rhapsody (Remastered 2011)",
            "Queen",
            354_000,
            None,
        )];
        let b = [track(
            "x",
            "bohemian rhapsody",
            "QUEEN & David Bowie",
            355_900,
            None,
        )];
        let c = [track("y", "Bohemian Rhapsody", "Queen", 357_000, None)];
        let merged = merge(&[("a", &a), ("b", &b), ("c", &c)]);
        assert_eq!(
            ids(&merged),
            vec![vec!["a:1", "b:x"], vec!["c:y"]],
            "3 s apart is another take"
        );
    }

    #[test]
    fn conflicting_isrcs_or_missing_durations_do_not_merge() {
        let a = [track("1", "Song", "Artist", 200_000, Some("USRC17607839"))];
        let b = [track("x", "Song", "Artist", 200_000, Some("GBAYE0601498"))];
        assert_eq!(merge(&[("a", &a), ("b", &b)]).len(), 2);
        let mut no_duration = track("y", "Song", "Artist", 0, None);
        no_duration.duration_ms = None;
        assert_eq!(merge(&[("a", &a), ("b", &[no_duration])]).len(), 2);
    }

    #[test]
    fn interleaves_by_rank_and_keeps_each_source_order() {
        let a = [
            track("a1", "One", "X", 100_000, None),
            track("a2", "Two", "X", 100_000, None),
            track("a3", "Three", "X", 100_000, None),
        ];
        let b = [
            track("b1", "Two", "X", 101_000, None),
            track("b2", "Four", "X", 100_000, None),
        ];
        let merged = merge(&[("a", &a), ("b", &b)]);
        assert_eq!(
            ids(&merged),
            vec![vec!["a:a1"], vec!["b:b1", "a:a2"], vec!["b:b2"], vec!["a:a3"]]
        );
    }

    #[test]
    fn a_source_never_merges_with_itself() {
        let a = [
            track("1", "Song", "Artist", 200_000, None),
            track("2", "Song", "Artist", 200_000, None),
        ];
        assert_eq!(merge(&[("a", &a)]).len(), 2);
    }

    #[test]
    fn kinds_have_their_own_rules() {
        let mut album_a = track("al1", "Abbey Road (Remastered)", "The Beatles", 0, None);
        album_a.kind = ItemKind::Album;
        album_a.duration_ms = None;
        let mut album_b = album_a.clone();
        album_b.id = "al2".into();
        album_b.title = "Abbey Road".into();
        let mut playlist_a = album_a.clone();
        playlist_a.kind = ItemKind::Playlist;
        let mut playlist_b = album_b.clone();
        playlist_b.kind = ItemKind::Playlist;
        let merged = merge(&[("a", &[album_a, playlist_a]), ("b", &[album_b, playlist_b])]);
        assert_eq!(
            ids(&merged),
            vec![vec!["a:al1", "b:al2"], vec!["a:al1"], vec!["b:al2"]]
        );
    }

    #[test]
    fn fills_gaps_from_duplicates() {
        let a = [track("1", "Song", "Artist", 200_000, None)];
        let b = [track("x", "Song", "Artist", 200_500, Some("USRC17607839"))];
        let merged = merge(&[("a", &a), ("b", &b)]);
        assert_eq!(merged[0].item.id, "1");
        assert_eq!(merged[0].item.isrc.as_deref(), Some("USRC17607839"));
    }

    #[test]
    fn normalises_titles_and_artists() {
        assert_eq!(normalize_title("Ｂｏｒｎ　ｔｏ　Ｒｕｎ"), "born to run");
        assert_eq!(normalize_title("Café del Mar [Live] (feat. X)"), "cafe del mar");
        assert_eq!(normalize_title("夜に駆ける【MV】"), "夜に駆ける");
        assert_eq!(normalize_title("Song - 2009 Remaster"), "song");
        assert_eq!(normalize_title("Song - Part 2"), "song part 2");
        assert_eq!(normalize_title("Song feat. Someone"), "song");
        assert_eq!(primary_artist("Beyoncé feat. JAY-Z"), "beyonce");
        assert_eq!(primary_artist("Simon & Garfunkel"), "simon");
        assert_eq!(primary_artist("宇多田ヒカル、椎名林檎"), "宇多田ヒカル");
        assert_eq!(primary_artist("AC/DC"), "ac dc");
    }
}

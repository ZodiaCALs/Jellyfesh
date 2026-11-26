//! The sources' likes and playlists (ADR-0024 §2–3; docs/ui.md,
//! "Favourites" and "Playlists"), from the core's `SourceLibrary` slice.
//! After the library's own, each signed-in source has a section with its
//! name and credit: Favourites shows its liked songs, albums and artists,
//! Playlists its playlists. The pages ask for fresh lists when they open,
//! and again when returned to after ten minutes; the core never polls.

use jf_core::{SourceLibrary, SourceLibraryList, SourceLibraryPart, SourceLibrarySnapshot};

/// How long a page's lists count as fresh before it asks again (ms).
pub const REFRESH_AFTER_MS: i64 = 10 * 60 * 1000;

/// Whether a page shown at `now_ms` asks for fresh lists, given when it
/// last asked.
pub fn should_refresh(last_ms: Option<i64>, now_ms: i64) -> bool {
    last_ms.is_none_or(|last| now_ms - last >= REFRESH_AFTER_MS)
}

/// A list worth showing: it has entries, or says why it has none.
fn shows(list: &SourceLibraryList) -> bool {
    !list.entries.is_empty() || list.error.is_some()
}

/// One source's section: the source and the lists the page shows.
pub type Section<'a> = (&'a SourceLibrary, Vec<&'a SourceLibraryList>);

fn sections(s: &SourceLibrarySnapshot, wanted: impl Fn(SourceLibraryPart) -> bool) -> Vec<Section<'_>> {
    s.sources
        .iter()
        .filter_map(|source| {
            let lists: Vec<&SourceLibraryList> = source
                .lists
                .iter()
                .filter(|l| wanted(l.part) && shows(l))
                .collect();
            (!lists.is_empty()).then_some((source, lists))
        })
        .collect()
}

/// Favourites: each source with its liked songs, albums and artists.
pub fn likes(s: &SourceLibrarySnapshot) -> Vec<Section<'_>> {
    sections(s, |part| part != SourceLibraryPart::Playlists)
}

/// Playlists: each source with its playlists.
pub fn playlists(s: &SourceLibrarySnapshot) -> Vec<Section<'_>> {
    sections(s, |part| part == SourceLibraryPart::Playlists)
}

/// What the person's lists say of a source's playlist: whether it is
/// their own, and whether its service lists its songs (when false, its page
/// says "{Service} lists this playlist's songs only in {Service}." and offers
/// no Save as Playlist…). `None` for a playlist not in their lists.
pub fn playlist_facts(s: &SourceLibrarySnapshot, plugin: &str, id: &str) -> Option<(bool, bool)> {
    s.sources
        .iter()
        .filter(|source| source.plugin == plugin)
        .flat_map(|source| source.lists.iter())
        .filter(|list| list.part == SourceLibraryPart::Playlists)
        .flat_map(|list| list.entries.iter())
        .find(|entry| entry.item.id == id)
        .map(|entry| (entry.owned, entry.songs_listed))
}

/// How long ago a cached list was loaded, for "Updated {time} ago".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Age {
    Minutes(u64),
    Hours(u64),
    Days(u64),
}

/// The age of a list loaded at `refreshed_at_ms`, at least a minute; `None`
/// while it loads or before it ever has.
pub fn age(list: &SourceLibraryList, now_ms: i64) -> Option<Age> {
    if list.loading {
        return None;
    }
    let minutes = (now_ms - list.refreshed_at_ms?).max(0) as u64 / 60_000;
    Some(match minutes {
        0..60 => Age::Minutes(minutes.max(1)),
        60..1440 => Age::Hours(minutes / 60),
        _ => Age::Days(minutes / 1440),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_core::{ItemKind, SourceItem, SourceLibraryEntry};

    fn entry(title: &str) -> SourceLibraryEntry {
        SourceLibraryEntry {
            item: SourceItem {
                source: "s".into(),
                kind: ItemKind::Track,
                id: title.into(),
                title: title.into(),
                subtitle: None,
                album: None,
                duration_ms: None,
                artwork: None,
                playable: true,
                unplayable_reason: None,
                track: None,
                also_in: Vec::new(),
                link: None,
                artist_id: None,
                album_id: None,
                downloaded: false,
            },
            owned: false,
            songs_listed: true,
        }
    }

    fn list(part: SourceLibraryPart, entries: Vec<SourceLibraryEntry>) -> SourceLibraryList {
        SourceLibraryList {
            part,
            entries,
            total: None,
            next: None,
            refreshed_at_ms: Some(0),
            loading: false,
            error: None,
        }
    }

    fn source(plugin: &str, lists: Vec<SourceLibraryList>) -> SourceLibrary {
        SourceLibrary {
            plugin: plugin.into(),
            name: plugin.into(),
            attribution: None,
            lists,
        }
    }

    #[test]
    fn asks_again_after_ten_minutes() {
        assert!(should_refresh(None, 5));
        assert!(!should_refresh(Some(0), REFRESH_AFTER_MS - 1));
        assert!(should_refresh(Some(0), REFRESH_AFTER_MS));
    }

    #[test]
    fn favourites_and_playlists_show_what_the_sources_keep() {
        let mut failed = list(SourceLibraryPart::LikedArtists, vec![]);
        failed.error = Some("Sign in again.".into());
        let s = SourceLibrarySnapshot {
            version: 1,
            sources: vec![
                source(
                    "a",
                    vec![
                        list(SourceLibraryPart::LikedSongs, vec![entry("x")]),
                        list(SourceLibraryPart::LikedAlbums, vec![]),
                        failed,
                        list(SourceLibraryPart::Playlists, vec![entry("p")]),
                    ],
                ),
                source("b", vec![list(SourceLibraryPart::Playlists, vec![])]),
            ],
        };
        let likes = likes(&s);
        assert_eq!(likes.len(), 1);
        let parts: Vec<SourceLibraryPart> = likes[0].1.iter().map(|l| l.part).collect();
        assert_eq!(
            parts,
            [SourceLibraryPart::LikedSongs, SourceLibraryPart::LikedArtists]
        );
        let playlists = playlists(&s);
        let plugins: Vec<&str> = playlists.iter().map(|(s, _)| s.plugin.as_str()).collect();
        assert_eq!(plugins, ["a"]);
    }

    #[test]
    fn knows_whether_a_playlists_songs_are_listed() {
        let mut followed = entry("p");
        followed.item.kind = ItemKind::Playlist;
        followed.songs_listed = false;
        let mut own = entry("q");
        own.item.kind = ItemKind::Playlist;
        own.owned = true;
        let s = SourceLibrarySnapshot {
            version: 1,
            sources: vec![source(
                "a",
                vec![list(SourceLibraryPart::Playlists, vec![followed, own])],
            )],
        };
        assert_eq!(playlist_facts(&s, "a", "p"), Some((false, false)));
        assert_eq!(playlist_facts(&s, "a", "q"), Some((true, true)));
        assert_eq!(playlist_facts(&s, "a", "x"), None);
        assert_eq!(playlist_facts(&s, "b", "p"), None);
    }

    #[test]
    fn cached_lists_say_how_old_they_are() {
        let mut l = list(SourceLibraryPart::LikedSongs, vec![]);
        l.refreshed_at_ms = Some(0);
        assert_eq!(age(&l, 10_000), Some(Age::Minutes(1)));
        assert_eq!(age(&l, 5 * 60_000), Some(Age::Minutes(5)));
        assert_eq!(age(&l, 3 * 3_600_000), Some(Age::Hours(3)));
        assert_eq!(age(&l, 50 * 3_600_000), Some(Age::Days(2)));
        l.loading = true;
        assert_eq!(age(&l, 5 * 60_000), None);
        l.loading = false;
        l.refreshed_at_ms = None;
        assert_eq!(age(&l, 5 * 60_000), None);
    }
}

//! A source's album, artist, playlist or folder page (docs/ui.md, "Album,
//! artist and playlist pages"; Jellyfesh Web's Browse page is the model,
//! docs/parity.md N18): what its header offers and what its list shows. The
//! core lists the items page by page and plays the container
//! (`Collection::Source`); this only reads what it listed.

use jf_core::{ItemKind, SourceItem};
use serde::{Deserialize, Serialize};

/// A source's album or artist to open: Go to Album and Go to Artist for a
/// source's songs and albums (`SourceItem::album_id` and `artist_id`; the
/// core browses them and describes them with `Details`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Container {
    pub plugin: String,
    pub kind: ItemKind,
    pub id: String,
    pub title: String,
    /// An album's artist, and the source's id for them.
    pub artist: Option<String>,
    pub artist_id: Option<String>,
}

impl Container {
    /// The container as the item its page shows.
    pub fn item(&self) -> SourceItem {
        SourceItem {
            source: self.plugin.clone(),
            kind: self.kind,
            id: self.id.clone(),
            title: self.title.clone(),
            subtitle: self.artist.clone(),
            album: None,
            duration_ms: None,
            artwork: None,
            playable: true,
            unplayable_reason: None,
            track: None,
            also_in: Vec::new(),
            link: None,
            artist_id: self.artist_id.clone(),
            album_id: None,
        }
    }
}

fn named(name: Option<&str>) -> Option<String> {
    name.map(str::trim).filter(|n| !n.is_empty()).map(str::to_owned)
}

/// Go to Album for a source's song: its album in the same source.
pub fn album_of(item: &SourceItem) -> Option<Container> {
    if item.source == crate::logic::search::LIBRARY || item.kind != ItemKind::Track {
        return None;
    }
    let id = item.album_id.clone().filter(|i| !i.is_empty())?;
    Some(Container {
        plugin: item.source.clone(),
        kind: ItemKind::Album,
        id,
        title: named(item.album.as_deref()).unwrap_or_default(),
        artist: named(item.subtitle.as_deref()),
        artist_id: item.artist_id.clone(),
    })
}

/// Go to Artist for a source's song or album: its artist in the same
/// source.
pub fn artist_of(item: &SourceItem) -> Option<Container> {
    if item.source == crate::logic::search::LIBRARY || !matches!(item.kind, ItemKind::Track | ItemKind::Album)
    {
        return None;
    }
    let id = item.artist_id.clone().filter(|i| !i.is_empty())?;
    Some(Container {
        plugin: item.source.clone(),
        kind: ItemKind::Artist,
        id,
        title: named(item.subtitle.as_deref()).unwrap_or_default(),
        artist: None,
        artist_id: None,
    })
}

/// What a container's page shows around its list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFacts {
    /// The artwork header with the kind, the facts, Play, Shuffle and More
    /// (albums, playlists and artists); a folder gets a title, as the root.
    pub header: bool,
    /// An album's songs are numbered.
    pub numbered: bool,
    /// The artist in accent text under the title (albums and playlists).
    pub artist_line: bool,
    /// Play and Shuffle from the start: the album, playlist or artist can
    /// play here. A folder offers them once it lists a song that can play
    /// ([`lists_playable`]).
    pub plays: bool,
}

/// The facts of the page for an item of `kind` with `subtitle` (an
/// album's artist) that can or cannot play here.
pub fn facts(kind: ItemKind, subtitle: Option<&str>, playable: bool) -> PageFacts {
    let has_artist = subtitle.is_some_and(|s| !s.trim().is_empty());
    let header = matches!(kind, ItemKind::Album | ItemKind::Playlist | ItemKind::Artist);
    PageFacts {
        header,
        numbered: kind == ItemKind::Album,
        artist_line: has_artist && matches!(kind, ItemKind::Album | ItemKind::Playlist),
        plays: playable && header,
    }
}

/// Whether a list (each item's kind and whether it can play here) holds a
/// song that can play.
pub fn lists_playable(items: impl IntoIterator<Item = (ItemKind, bool)>) -> bool {
    items
        .into_iter()
        .any(|(kind, playable)| kind == ItemKind::Track && playable)
}

/// Whether an album's songs are by more than one artist: then each row
/// names its artist, otherwise the album's artist says it once.
pub fn various_artists<'a>(artists: impl IntoIterator<Item = Option<&'a str>>) -> bool {
    let mut first: Option<Option<&str>> = None;
    for artist in artists {
        match first {
            None => first = Some(artist),
            Some(f) if f != artist => return true,
            Some(_) => {}
        }
    }
    false
}

/// The songs a page lists (each item's kind and length), as their lengths,
/// for "12 songs" and the "12 songs, 48 minutes" footer. `None` until every
/// page is loaded, so the count never shows a part of the list.
pub fn songs(
    items: impl IntoIterator<Item = (ItemKind, Option<u64>)>,
    complete: bool,
) -> Option<Vec<Option<u64>>> {
    let songs: Vec<Option<u64>> = items
        .into_iter()
        .filter(|(kind, _)| *kind == ItemKind::Track)
        .map(|(_, ms)| ms)
        .collect();
    (complete && !songs.is_empty()).then_some(songs)
}

/// The number shown on each row of a numbered list: its place among the
/// songs (1 for the first), none for anything else.
pub fn numbers(kinds: impl IntoIterator<Item = ItemKind>) -> Vec<Option<u32>> {
    let mut n = 0u32;
    kinds
        .into_iter()
        .map(|kind| {
            (kind == ItemKind::Track).then(|| {
                n += 1;
                n
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(source: &str) -> SourceItem {
        SourceItem {
            source: source.into(),
            kind: ItemKind::Track,
            id: "t1".into(),
            title: "Harbour Lights".into(),
            subtitle: Some("The Quiet Fleet".into()),
            album: Some("Lanterns".into()),
            duration_ms: None,
            artwork: None,
            playable: true,
            unplayable_reason: None,
            track: None,
            also_in: Vec::new(),
            link: None,
            artist_id: Some("ar9".into()),
            album_id: Some("al3".into()),
        }
    }

    #[test]
    fn a_source_songs_album_and_artist() {
        let album = album_of(&song("subsonic")).unwrap();
        assert_eq!(
            (album.kind, album.id.as_str(), album.title.as_str()),
            (ItemKind::Album, "al3", "Lanterns")
        );
        assert_eq!(album.artist_id.as_deref(), Some("ar9"));
        let page = album.item();
        assert_eq!(
            (page.source.as_str(), page.kind, page.subtitle.as_deref()),
            ("subsonic", ItemKind::Album, Some("The Quiet Fleet"))
        );
        let artist = artist_of(&song("subsonic")).unwrap();
        assert_eq!(
            (artist.kind, artist.id.as_str(), artist.title.as_str()),
            (ItemKind::Artist, "ar9", "The Quiet Fleet")
        );
        // The library's own items keep the library's pages.
        assert!(album_of(&song("library")).is_none());
        let mut unknown = song("subsonic");
        unknown.album_id = None;
        unknown.artist_id = Some(String::new());
        assert!(album_of(&unknown).is_none());
        assert!(artist_of(&unknown).is_none());
    }
    use ItemKind::{Album, Artist, Folder, Playlist, Track};

    #[test]
    fn albums_are_numbered_and_name_their_artist() {
        assert_eq!(
            facts(Album, Some("The Quiet Fleet"), true),
            PageFacts {
                header: true,
                numbered: true,
                artist_line: true,
                plays: true
            }
        );
        let playlist = facts(Playlist, Some("Mixed by Mira"), true);
        assert!(playlist.header && !playlist.numbered && playlist.artist_line && playlist.plays);
        // Artists have no artist line.
        let artist = facts(Artist, Some("12 albums"), true);
        assert!(artist.header && !artist.numbered && !artist.artist_line && artist.plays);
        assert!(!facts(Album, Some("  "), true).artist_line);
    }

    #[test]
    fn folders_get_a_title_and_play_once_they_list_songs() {
        let folder = facts(Folder, None, true);
        assert!(!folder.header && !folder.artist_line && !folder.plays);
        assert!(!lists_playable([(Folder, true), (Album, true)]));
        assert!(!lists_playable([(Track, false)]));
        assert!(lists_playable([(Folder, true), (Track, true)]));
    }

    #[test]
    fn only_playable_containers_play() {
        assert!(!facts(Album, None, false).plays);
        assert!(!facts(Track, None, true).plays);
    }

    #[test]
    fn rows_name_their_artist_only_when_it_varies() {
        assert!(!various_artists([Some("Glass Harbour"), Some("Glass Harbour")]));
        assert!(various_artists([Some("Glass Harbour"), Some("Tidal Choir")]));
        assert!(various_artists([Some("Glass Harbour"), None]));
        assert!(!various_artists([None, None]));
        assert!(!various_artists([]));
    }

    #[test]
    fn songs_count_once_the_list_is_complete() {
        let items = [(Track, Some(60_000)), (Folder, None), (Track, None)];
        assert_eq!(songs(items, false), None);
        assert_eq!(songs(items, true), Some(vec![Some(60_000), None]));
        assert_eq!(songs([(Folder, None)], true), None);
        assert_eq!(songs([], true), None);
    }

    #[test]
    fn songs_are_numbered_in_order() {
        assert_eq!(numbers([Track, Folder, Track]), [Some(1), None, Some(2)]);
    }
}

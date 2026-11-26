//! The Downloads page (docs/ui.md, "Downloads"; ADR-0025), from the core's
//! `Downloads` slice: In Progress (each "Waiting", its percentage, or why it
//! failed), why downloads wait when they do, then the downloaded songs by
//! album. The core keeps the downloads and their order; this only arranges
//! them.

use jf_core::{
    DownloadError, DownloadItem, DownloadState, DownloadTarget, DownloadsPaused, DownloadsSnapshot, ItemKind,
    SourceItem, TrackRef,
};

/// The downloads that have not finished, in the core's queue order.
pub fn in_progress(d: &DownloadsSnapshot) -> Vec<&DownloadItem> {
    d.items
        .iter()
        .filter(|i| i.state != DownloadState::Done)
        .collect()
}

/// The finished downloads, album by album as the core lists them.
pub fn finished(d: &DownloadsSnapshot) -> Vec<&DownloadItem> {
    d.items
        .iter()
        .filter(|i| i.state == DownloadState::Done)
        .collect()
}

/// Where a download in progress is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// "Waiting": its turn has not come, or the network keeps it.
    Waiting,
    /// Fetching: its percentage, when its size is known.
    Downloading(Option<u8>),
    /// It failed: the reason and Try Again.
    Failed(DownloadError),
}

pub fn progress(item: &DownloadItem) -> Progress {
    match &item.state {
        DownloadState::Queued | DownloadState::Done => Progress::Waiting,
        DownloadState::Downloading => Progress::Downloading(
            item.bytes_total
                .filter(|t| *t > 0)
                .map(|t| (item.bytes_done.min(t) * 100 / t) as u8),
        ),
        DownloadState::Failed { error } => Progress::Failed(error.clone()),
    }
}

/// Why queued downloads wait, while some do: Offline ("Downloads resume
/// when you're back online.") or Metered, with Pause on Metered Connections
/// on ("Downloads resume when you're off a metered connection.").
pub fn waiting_because(d: &DownloadsSnapshot) -> Option<DownloadsPaused> {
    d.paused
        .filter(|_| d.items.iter().any(|i| i.state == DownloadState::Queued))
}

/// The downloaded songs of one album.
#[derive(Debug, Clone, PartialEq)]
pub struct Album<'a> {
    /// `None` for songs the source gave no album.
    pub title: Option<String>,
    /// The artist all its songs share, if they do.
    pub artist: Option<String>,
    pub songs: Vec<&'a DownloadItem>,
}

/// The finished downloads by album: songs next to each other with the same
/// source and album make one.
pub fn albums(d: &DownloadsSnapshot) -> Vec<Album<'_>> {
    let mut albums: Vec<(String, Album<'_>)> = Vec::new();
    for item in finished(d) {
        let same = albums
            .last()
            .is_some_and(|(plugin, a)| *plugin == item.track.plugin && a.title == item.track.album);
        if same {
            let (_, album) = albums.last_mut().expect("an album");
            if album.artist != item.track.artist {
                album.artist = None;
            }
            album.songs.push(item);
        } else {
            albums.push((
                item.track.plugin.clone(),
                Album {
                    title: item.track.album.clone(),
                    artist: item.track.artist.clone(),
                    songs: vec![item],
                },
            ));
        }
    }
    albums.into_iter().map(|(_, a)| a).collect()
}

/// What Download downloads from a source's item, where the source offers
/// downloads: a song not downloaded yet, or an album's or a playlist's
/// songs.
pub fn target_of(item: &SourceItem, offered: bool) -> Option<DownloadTarget> {
    if !offered || item.source == crate::logic::search::LIBRARY {
        return None;
    }
    match item.kind {
        ItemKind::Track if !item.downloaded => match &item.track {
            Some(track @ TrackRef::Source(_)) => Some(DownloadTarget::Track { track: track.clone() }),
            _ => None,
        },
        ItemKind::Album | ItemKind::Playlist => Some(DownloadTarget::Collection {
            plugin: item.source.clone(),
            id: item.id.clone(),
            item_kind: item.kind,
        }),
        _ => None,
    }
}

/// What a downloaded song plays: its library track once the library has
/// it, else the source's song, which plays the file too (ADR-0025 §4).
pub fn track(item: &DownloadItem) -> TrackRef {
    match item.library_id {
        Some(id) => TrackRef::Library(id),
        None => TrackRef::Source(item.track.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_core::SourceTrack;
    use std::path::PathBuf;

    fn song(id: u64, plugin: &str, album: Option<&str>, artist: &str, state: DownloadState) -> DownloadItem {
        DownloadItem {
            id,
            track: SourceTrack {
                plugin: plugin.into(),
                id: format!("t{id}"),
                title: format!("Song {id}"),
                artist: Some(artist.into()),
                album: album.map(String::from),
                duration_ms: None,
                artwork_url: None,
                isrc: None,
            },
            state,
            bytes_done: 0,
            bytes_total: None,
            path: None,
            library_id: None,
            original: true,
        }
    }

    fn snapshot(items: Vec<DownloadItem>, paused: Option<DownloadsPaused>) -> DownloadsSnapshot {
        DownloadsSnapshot {
            version: 1,
            folder: PathBuf::from("/music"),
            items,
            bytes_used: 0,
            paused,
        }
    }

    #[test]
    fn splits_what_is_in_progress_from_what_is_downloaded() {
        let d = snapshot(
            vec![
                song(1, "j", Some("A"), "X", DownloadState::Downloading),
                song(2, "j", Some("A"), "X", DownloadState::Queued),
                song(3, "j", Some("B"), "Y", DownloadState::Done),
            ],
            None,
        );
        let ids = |v: Vec<&DownloadItem>| v.iter().map(|i| i.id).collect::<Vec<_>>();
        assert_eq!(ids(in_progress(&d)), [1, 2]);
        assert_eq!(ids(finished(&d)), [3]);
    }

    #[test]
    fn says_waiting_a_percentage_or_why_it_failed() {
        let mut item = song(1, "j", None, "X", DownloadState::Queued);
        assert_eq!(progress(&item), Progress::Waiting);
        item.state = DownloadState::Downloading;
        assert_eq!(progress(&item), Progress::Downloading(None));
        item.bytes_total = Some(400);
        item.bytes_done = 100;
        assert_eq!(progress(&item), Progress::Downloading(Some(25)));
        item.bytes_done = 900;
        assert_eq!(progress(&item), Progress::Downloading(Some(100)));
        let error = DownloadError::NotAllowed {
            service: "Jellyfin".into(),
        };
        item.state = DownloadState::Failed { error: error.clone() };
        assert_eq!(progress(&item), Progress::Failed(error));
    }

    #[test]
    fn says_why_downloads_wait_only_while_some_wait() {
        let queued = || vec![song(1, "j", None, "X", DownloadState::Queued)];
        assert_eq!(
            waiting_because(&snapshot(queued(), Some(DownloadsPaused::Metered))),
            Some(DownloadsPaused::Metered)
        );
        assert_eq!(waiting_because(&snapshot(queued(), None)), None);
        let done = vec![song(1, "j", None, "X", DownloadState::Done)];
        assert_eq!(
            waiting_because(&snapshot(done, Some(DownloadsPaused::Offline))),
            None
        );
    }

    #[test]
    fn groups_downloaded_songs_by_source_and_album() {
        let d = snapshot(
            vec![
                song(1, "j", Some("A"), "X", DownloadState::Done),
                song(2, "j", Some("A"), "X", DownloadState::Done),
                song(3, "s", Some("A"), "X", DownloadState::Done),
                song(4, "s", Some("B"), "X", DownloadState::Done),
                song(5, "s", Some("B"), "Z", DownloadState::Done),
                song(6, "s", None, "Z", DownloadState::Queued),
            ],
            None,
        );
        let albums = albums(&d);
        let shape: Vec<(Option<&str>, Option<&str>, Vec<u64>)> = albums
            .iter()
            .map(|a| {
                (
                    a.title.as_deref(),
                    a.artist.as_deref(),
                    a.songs.iter().map(|s| s.id).collect(),
                )
            })
            .collect();
        assert_eq!(
            shape,
            [
                (Some("A"), Some("X"), vec![1, 2]),
                (Some("A"), Some("X"), vec![3]),
                (Some("B"), None, vec![4, 5]),
            ]
        );
    }

    fn item(kind: ItemKind, downloaded: bool) -> SourceItem {
        let track = (kind == ItemKind::Track)
            .then(|| TrackRef::Source(song(1, "j", None, "X", DownloadState::Done).track));
        SourceItem {
            source: "j".into(),
            kind,
            id: "x".into(),
            title: "X".into(),
            subtitle: None,
            album: None,
            duration_ms: None,
            artwork: None,
            playable: true,
            unplayable_reason: None,
            track,
            also_in: Vec::new(),
            link: None,
            artist_id: None,
            album_id: None,
            downloaded,
        }
    }

    #[test]
    fn download_is_offered_for_songs_albums_and_playlists_of_sources_that_allow_it() {
        assert!(matches!(
            target_of(&item(ItemKind::Track, false), true),
            Some(DownloadTarget::Track { .. })
        ));
        assert_eq!(
            target_of(&item(ItemKind::Track, true), true),
            None,
            "downloaded already"
        );
        assert_eq!(
            target_of(&item(ItemKind::Track, false), false),
            None,
            "not offered"
        );
        assert_eq!(
            target_of(&item(ItemKind::Album, false), true),
            Some(DownloadTarget::Collection {
                plugin: "j".into(),
                id: "x".into(),
                item_kind: ItemKind::Album,
            })
        );
        assert!(target_of(&item(ItemKind::Playlist, false), true).is_some());
        assert_eq!(target_of(&item(ItemKind::Artist, false), true), None);
        let mut library = item(ItemKind::Album, false);
        library.source = crate::logic::search::LIBRARY.into();
        assert_eq!(target_of(&library, true), None);
    }

    #[test]
    fn a_downloaded_song_plays_from_the_library_once_it_is_there() {
        let mut item = song(1, "j", None, "X", DownloadState::Done);
        assert!(matches!(track(&item), TrackRef::Source(s) if s.id == "t1"));
        item.library_id = Some(42);
        assert_eq!(track(&item), TrackRef::Library(42));
    }
}

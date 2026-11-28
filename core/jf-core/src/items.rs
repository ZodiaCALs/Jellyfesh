//! Library items (playlist entries, favourites, play statistics) as queue
//! entries and as rows for track lists.

use std::path::Path;

use jf_library::{PlaylistItem, Track};
use jf_session::{SourceTrack, TrackRef};

use crate::types::{ArtworkRef, PlaylistSummary, TrackRow};

/// The library's item for a queue entry. A source track keeps its display
/// metadata (as JSON) so lists can show it without asking the plugin.
pub(crate) fn item_for(track: &TrackRef) -> PlaylistItem {
    match track {
        TrackRef::Library(id) => PlaylistItem::Library(*id),
        TrackRef::File(path) => PlaylistItem::File(path.clone()),
        TrackRef::Source(t) => PlaylistItem::Source {
            plugin: t.plugin.clone(),
            id: t.id.clone(),
            meta_json: serde_json::to_string(t).unwrap_or_default(),
        },
    }
}

/// The queue entry for a library item. A file that is in the catalogue plays
/// as its library track (with the catalogue's tags, artwork and ReplayGain).
pub(crate) fn track_for(item: &PlaylistItem, track: Option<&Track>) -> TrackRef {
    match (item, track) {
        (
            PlaylistItem::Source {
                plugin,
                id,
                meta_json,
            },
            _,
        ) => TrackRef::Source(source_track(plugin, id, meta_json)),
        (_, Some(t)) => TrackRef::Library(t.id),
        (PlaylistItem::Library(id), None) => TrackRef::Library(*id),
        (PlaylistItem::File(path), None) => TrackRef::File(path.clone()),
    }
}

/// The source track stored with a plugin item, or a bare one when the stored
/// metadata is not ours (an older or hand-edited entry).
fn source_track(plugin: &str, id: &str, meta_json: &str) -> SourceTrack {
    serde_json::from_str::<SourceTrack>(meta_json)
        .ok()
        .filter(|t| t.plugin == plugin && t.id == id)
        .unwrap_or_else(|| SourceTrack {
            plugin: plugin.to_owned(),
            id: id.to_owned(),
            title: id.to_owned(),
            artist: None,
            album: None,
            duration_ms: None,
            artwork_url: None,
            isrc: None,
            artist_id: None,
            album_id: None,
        })
}

/// Whether an item can be queued now: catalogue tracks and plugin items
/// always (plugins resolve them when they play), files while they exist.
fn playable(item: &PlaylistItem, track: Option<&Track>) -> bool {
    match (item, track) {
        (PlaylistItem::Source { .. }, _) | (_, Some(_)) => true,
        (PlaylistItem::File(path), None) => path.is_file(),
        (PlaylistItem::Library(_), None) => false,
    }
}

/// Queue entries for the playable items of a list, and the index among them
/// of the list's entry `start` (or of the first playable entry after it).
pub(crate) fn queue_entries<'a>(
    items: impl IntoIterator<Item = (&'a PlaylistItem, Option<&'a Track>)>,
    start: usize,
) -> (Vec<TrackRef>, usize) {
    let mut out = Vec::new();
    let mut start_at = None;
    for (i, (item, track)) in items.into_iter().enumerate() {
        if !playable(item, track) {
            continue;
        }
        if i >= start && start_at.is_none() {
            start_at = Some(out.len());
        }
        out.push(track_for(item, track));
    }
    let start_at = start_at.unwrap_or(out.len().saturating_sub(1));
    (out, start_at)
}

/// A row for a track list.
pub(crate) fn row(item: &PlaylistItem, track: Option<&Track>, favorite: bool) -> TrackRow {
    let reference = track_for(item, track);
    if let TrackRef::Source(t) = &reference {
        return TrackRow {
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            duration_ms: t.duration_ms,
            artwork: stored_artwork(t),
            artwork_hash: None,
            favorite,
            missing: false,
            downloaded: crate::downloads::is_downloaded(&t.plugin, &t.id),
            track: reference,
        };
    }
    match track {
        Some(t) => TrackRow {
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            duration_ms: t.duration_ms,
            artwork: None,
            artwork_hash: t.artwork_hash.clone(),
            favorite,
            missing: false,
            downloaded: false,
            track: reference,
        },
        None => {
            let (title, missing) = match item {
                PlaylistItem::File(path) => (file_title(path), !path.is_file()),
                _ => (String::new(), true),
            };
            TrackRow {
                title,
                artist: None,
                album: None,
                duration_ms: None,
                artwork: None,
                artwork_hash: None,
                favorite,
                missing,
                downloaded: false,
                track: reference,
            }
        }
    }
}

/// The picture a source track a list stored shows: the URL it was listed
/// with, or the core's reference to the item when its plugin answers
/// `artwork` (both fetched with `source_artwork`, in any later run too).
pub(crate) fn stored_artwork(track: &SourceTrack) -> Option<ArtworkRef> {
    crate::artwork::stored_picture(
        &track.plugin,
        jf_plugin_host::ItemKind::Track,
        &track.id,
        track.artwork_url.as_deref(),
    )
    .map(ArtworkRef::Url)
}

/// A file's name without its extension, as a title for a file without tags.
pub(crate) fn file_title(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub(crate) fn playlist_summary(p: jf_library::Playlist) -> PlaylistSummary {
    PlaylistSummary {
        id: p.id,
        name: p.name,
        entry_count: p.entry_count,
        duration_ms: p.duration_ms,
        updated_at_ms: p.updated_at_ms,
        artwork_hash: p.artwork_hash,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn source(id: &str) -> SourceTrack {
        SourceTrack {
            plugin: "org.example.source".into(),
            id: id.into(),
            title: format!("Song {id}"),
            artist: Some("Someone".into()),
            album: None,
            duration_ms: Some(180_000),
            artwork_url: Some("https://example.org/a.jpg".into()),
            isrc: None,
            artist_id: None,
            album_id: None,
        }
    }

    #[test]
    fn source_tracks_round_trip_through_items() {
        let t = TrackRef::Source(source("7"));
        let item = item_for(&t);
        assert_eq!(track_for(&item, None), t);
        let row = row(&item, None, true);
        assert_eq!(row.title, "Song 7");
        assert_eq!(
            row.artwork,
            Some(ArtworkRef::Url("https://example.org/a.jpg".into()))
        );
        assert!(row.favorite && !row.missing);
    }

    #[test]
    fn stored_source_tracks_keep_a_picture_the_core_can_fetch() {
        let listed = TrackRef::Source(source("8"));
        assert_eq!(
            row(&item_for(&listed), None, false).artwork,
            Some(ArtworkRef::Url("https://example.org/a.jpg".into())),
            "the URL it was listed with"
        );
        let bare = SourceTrack {
            plugin: "org.example.items-covers".into(),
            artwork_url: None,
            ..source("9")
        };
        let item = item_for(&TrackRef::Source(bare.clone()));
        assert_eq!(
            row(&item, None, false).artwork,
            None,
            "the plugin gives no artwork"
        );
        crate::artwork::note_covers("org.example.items-covers", true);
        let Some(ArtworkRef::Url(reference)) = row(&item, None, false).artwork else {
            panic!("a reference to the item")
        };
        assert!(
            reference.starts_with("jellyfesh-artwork://item?")
                && reference.contains("plugin=org.example.items-covers")
                && reference.contains("id=9"),
            "{reference}"
        );
        crate::artwork::note_covers("org.example.items-covers", false);
    }

    #[test]
    fn foreign_source_metadata_falls_back_to_the_id() {
        let item = PlaylistItem::Source {
            plugin: "p".into(),
            id: "x1".into(),
            meta_json: r#"{"title":"not ours"}"#.into(),
        };
        let TrackRef::Source(t) = track_for(&item, None) else {
            panic!("a source entry")
        };
        assert_eq!(
            (t.plugin.as_str(), t.id.as_str(), t.title.as_str()),
            ("p", "x1", "x1")
        );
    }

    #[test]
    fn missing_files_are_left_out_and_start_moves_to_the_next_entry() {
        let dir = tempfile::tempdir().unwrap();
        let here = dir.path().join("here.flac");
        std::fs::write(&here, b"").unwrap();
        let gone = dir.path().join("gone.flac");
        let items = [
            PlaylistItem::File(here.clone()),
            PlaylistItem::File(gone.clone()),
            PlaylistItem::Source {
                plugin: "p".into(),
                id: "1".into(),
                meta_json: String::new(),
            },
        ];
        let pairs = || items.iter().map(|i| (i, None));
        let (queue, start) = queue_entries(pairs(), 1);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0], TrackRef::File(here.clone()));
        assert_eq!(start, 1, "the missing entry 1 starts at the source entry");
        assert_eq!(queue_entries(pairs(), 0).1, 0);
        assert_eq!(queue_entries(pairs(), 9).1, 1, "past the end: the last entry");

        let row = row(&items[1], None, false);
        assert!(row.missing);
        assert_eq!(row.title, "gone");
        assert_eq!(row.track, TrackRef::File(PathBuf::from(&gone)));
    }
}

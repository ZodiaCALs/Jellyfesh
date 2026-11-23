//! Hearts, playlists and play statistics, kept in the browser (docs/ui.md:
//! "Hearts, playlists and history are kept in the browser"), with the rules
//! of `jf-library`'s playlists and statistics, which need SQLite and do not
//! build for the browser (`tests/mirrors.rs` compares the two):
//!
//! * a playlist's name is trimmed and may not be empty; names need not be
//!   unique, and the list is in name order (normalised, then by id);
//! * playlist and entry ids are never reused, even after a delete;
//! * entries keep their order: added at a position (appended when there is
//!   none or it is past the end), removed (the rest move up) or moved (to
//!   the last place when past the end); a change to the entries or the name
//!   moves the playlist's "updated" time;
//! * a heart keeps its first time when given again, and is later than every
//!   other heart, so Favourites keeps the order of the clicks; Favourites
//!   lists the latest first;
//! * a play counts or is a skip (`rules::play_counts`); Recently Played lists
//!   the latest counted plays first, Most Played the most counted plays (ties
//!   by the latest play);
//! * an item that is neither a favourite nor played or skipped any more is
//!   forgotten.
//!
//! Items are what the views show: a source's item with its plugin's id, the
//! library's songs under [`LIBRARY`] with their key as id. Only songs are
//! kept. The page stores [`Collections::state`] (IndexedDB) and loads it
//! back with [`Collections::from_state`].

use std::cmp::Reverse;

use jf_plugin_model::{Item, ItemKind};
use serde::{Deserialize, Serialize};

use crate::library::normalize;

/// The library's id where views take a source's id.
pub const LIBRARY: &str = "library";

/// An item of a source (or of the library, under [`LIBRARY`]), as the views
/// listed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    pub plugin: String,
    pub item: Item,
}

impl Saved {
    fn same(&self, other: &Saved) -> bool {
        self.plugin == other.plugin && self.item.id == other.item.id
    }
}

/// Why a change was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CollectionError {
    /// The name is empty once trimmed.
    EmptyName,
    /// No playlist has this id.
    NoPlaylist,
    /// The playlist has no entry with this id.
    NoEntry,
    /// Only songs go into playlists and Favourites.
    NotASong,
}

impl std::fmt::Display for CollectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CollectionError::EmptyName => "empty-name",
            CollectionError::NoPlaylist => "no-playlist",
            CollectionError::NoEntry => "no-entry",
            CollectionError::NotASong => "not-a-song",
        })
    }
}

impl std::error::Error for CollectionError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredEntry {
    id: i64,
    saved: Saved,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredPlaylist {
    id: i64,
    name: String,
    created_at_ms: i64,
    updated_at_ms: i64,
    entries: Vec<StoredEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Stats {
    id: i64,
    saved: Saved,
    favorite_at_ms: Option<i64>,
    play_count: u64,
    skip_count: u64,
    last_played_ms: Option<i64>,
}

/// A playlist as the Playlists page lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub entry_count: usize,
    /// The known lengths added up.
    pub duration_ms: u64,
    pub created_at_ms: i64,
    /// The last change to the name or the entries.
    pub updated_at_ms: i64,
    /// The first entry with artwork, for the playlist's cover.
    pub cover: Option<Saved>,
}

/// One entry of a playlist.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaylistEntry {
    /// Stable while the entry is in the playlist, and never reused.
    pub entry_id: i64,
    /// From 0, without gaps.
    pub position: usize,
    pub plugin: String,
    pub item: Item,
}

/// An item with its heart and plays: the rows of Favourites, Recently
/// Played and Most Played.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ItemStats {
    pub plugin: String,
    pub item: Item,
    pub favorite: bool,
    pub favorited_at_ms: Option<i64>,
    pub play_count: u64,
    pub skip_count: u64,
    pub last_played_ms: Option<i64>,
}

impl From<&Stats> for ItemStats {
    fn from(stats: &Stats) -> Self {
        ItemStats {
            plugin: stats.saved.plugin.clone(),
            item: stats.saved.item.clone(),
            favorite: stats.favorite_at_ms.is_some(),
            favorited_at_ms: stats.favorite_at_ms,
            play_count: stats.play_count,
            skip_count: stats.skip_count,
            last_played_ms: stats.last_played_ms,
        }
    }
}

/// Hearts, playlists and plays. See the module documentation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Collections {
    playlists: Vec<StoredPlaylist>,
    stats: Vec<Stats>,
    /// The last ids given out (ids are never reused).
    last_playlist: i64,
    last_entry: i64,
    last_stats: i64,
}

fn valid_name(name: &str) -> Result<&str, CollectionError> {
    let name = name.trim();
    if name.is_empty() {
        Err(CollectionError::EmptyName)
    } else {
        Ok(name)
    }
}

fn song(saved: &Saved) -> Result<(), CollectionError> {
    if saved.item.kind == ItemKind::Track {
        Ok(())
    } else {
        Err(CollectionError::NotASong)
    }
}

impl Collections {
    pub fn new() -> Collections {
        Collections::default()
    }

    /// Collections saved with [`Collections::state`]. What cannot be read
    /// starts empty, and ids stay above every id in use.
    pub fn from_state(state: &str) -> Collections {
        let mut loaded: Collections = serde_json::from_str(state).unwrap_or_default();
        let top_playlist = loaded.playlists.iter().map(|p| p.id).max().unwrap_or(0);
        let top_entry = loaded
            .playlists
            .iter()
            .flat_map(|p| p.entries.iter().map(|e| e.id))
            .max()
            .unwrap_or(0);
        let top_stats = loaded.stats.iter().map(|s| s.id).max().unwrap_or(0);
        loaded.last_playlist = loaded.last_playlist.max(top_playlist);
        loaded.last_entry = loaded.last_entry.max(top_entry);
        loaded.last_stats = loaded.last_stats.max(top_stats);
        loaded
    }

    /// Everything, for the page to store.
    pub fn state(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    // ---- playlists ----------------------------------------------------------------------------

    /// A new, empty playlist; returns its id.
    pub fn create_playlist(&mut self, name: &str, now_ms: i64) -> Result<i64, CollectionError> {
        let name = valid_name(name)?.to_owned();
        self.last_playlist += 1;
        let id = self.last_playlist;
        self.playlists.push(StoredPlaylist {
            id,
            name,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            entries: Vec::new(),
        });
        Ok(id)
    }

    pub fn rename_playlist(&mut self, id: i64, name: &str, now_ms: i64) -> Result<(), CollectionError> {
        let name = valid_name(name)?;
        let playlist = self.playlist_mut(id)?;
        if playlist.name != name {
            name.clone_into(&mut playlist.name);
            playlist.updated_at_ms = now_ms;
        }
        Ok(())
    }

    /// Deletes a playlist with its entries.
    pub fn delete_playlist(&mut self, id: i64) -> Result<(), CollectionError> {
        let before = self.playlists.len();
        self.playlists.retain(|p| p.id != id);
        if self.playlists.len() == before {
            Err(CollectionError::NoPlaylist)
        } else {
            Ok(())
        }
    }

    /// Every playlist, by name.
    pub fn playlists(&self) -> Vec<PlaylistSummary> {
        let mut list: Vec<(String, PlaylistSummary)> = self
            .playlists
            .iter()
            .map(|p| (normalize(&p.name), summary(p)))
            .collect();
        list.sort_by(|(a, x), (b, y)| a.cmp(b).then(x.id.cmp(&y.id)));
        list.into_iter().map(|(_, summary)| summary).collect()
    }

    pub fn playlist(&self, id: i64) -> Option<PlaylistSummary> {
        self.playlists.iter().find(|p| p.id == id).map(summary)
    }

    /// Up to `limit` entries in order from position `offset`; none for a
    /// playlist that does not exist.
    pub fn entries(&self, id: i64, offset: usize, limit: usize) -> Vec<PlaylistEntry> {
        let Some(playlist) = self.playlists.iter().find(|p| p.id == id) else {
            return Vec::new();
        };
        playlist
            .entries
            .iter()
            .enumerate()
            .skip(offset)
            .take(limit)
            .map(|(position, entry)| PlaylistEntry {
                entry_id: entry.id,
                position,
                plugin: entry.saved.plugin.clone(),
                item: entry.saved.item.clone(),
            })
            .collect()
    }

    /// Inserts songs, in order, at `at` (appended when `None` or past the
    /// end); returns their entry ids. All or nothing: anything but a song
    /// refuses the lot. A song may appear any number of times.
    pub fn add_to_playlist(
        &mut self,
        id: i64,
        items: Vec<Saved>,
        at: Option<usize>,
        now_ms: i64,
    ) -> Result<Vec<i64>, CollectionError> {
        self.playlist_mut(id)?;
        for saved in &items {
            song(saved)?;
        }
        let mut ids = Vec::with_capacity(items.len());
        let entries: Vec<StoredEntry> = items
            .into_iter()
            .map(|saved| {
                self.last_entry += 1;
                ids.push(self.last_entry);
                StoredEntry {
                    id: self.last_entry,
                    saved,
                }
            })
            .collect();
        let playlist = self.playlist_mut(id)?;
        if !entries.is_empty() {
            let at = at.map_or(playlist.entries.len(), |at| at.min(playlist.entries.len()));
            playlist.entries.splice(at..at, entries);
            playlist.updated_at_ms = now_ms;
        }
        Ok(ids)
    }

    /// Removes entries; ids that are not entries of this playlist are
    /// ignored. The entries after them move up.
    pub fn remove_from_playlist(
        &mut self,
        id: i64,
        entry_ids: &[i64],
        now_ms: i64,
    ) -> Result<(), CollectionError> {
        let playlist = self.playlist_mut(id)?;
        let before = playlist.entries.len();
        playlist.entries.retain(|e| !entry_ids.contains(&e.id));
        if playlist.entries.len() != before {
            playlist.updated_at_ms = now_ms;
        }
        Ok(())
    }

    /// Moves an entry to position `to` (the last position when past the
    /// end); the entries in between move by one.
    pub fn move_playlist_entry(
        &mut self,
        id: i64,
        entry_id: i64,
        to: usize,
        now_ms: i64,
    ) -> Result<(), CollectionError> {
        let playlist = self.playlist_mut(id)?;
        let from = playlist
            .entries
            .iter()
            .position(|e| e.id == entry_id)
            .ok_or(CollectionError::NoEntry)?;
        let to = to.min(playlist.entries.len() - 1);
        if to != from {
            let entry = playlist.entries.remove(from);
            playlist.entries.insert(to, entry);
            playlist.updated_at_ms = now_ms;
        }
        Ok(())
    }

    fn playlist_mut(&mut self, id: i64) -> Result<&mut StoredPlaylist, CollectionError> {
        self.playlists
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or(CollectionError::NoPlaylist)
    }

    // ---- hearts and plays ---------------------------------------------------------------------

    /// Marks a song as a favourite, or not. Giving a favourite the heart
    /// again keeps its time. Returns whether anything changed.
    pub fn set_favorite(
        &mut self,
        saved: Saved,
        favorite: bool,
        now_ms: i64,
    ) -> Result<bool, CollectionError> {
        song(&saved)?;
        let index = self.stats.iter().position(|s| s.saved.same(&saved));
        match (index, favorite) {
            (Some(i), true) if self.stats[i].favorite_at_ms.is_some() => {
                self.stats[i].saved = saved;
                Ok(false)
            }
            (None, false) => Ok(false),
            (Some(i), false) if self.stats[i].favorite_at_ms.is_none() => Ok(false),
            (_, true) => {
                // Later than every other favourite, so the list keeps the order of the
                // clicks even within one millisecond or when the clock steps back.
                let latest = self.stats.iter().filter_map(|s| s.favorite_at_ms).max();
                let at = latest.map_or(now_ms, |latest| now_ms.max(latest + 1));
                let i = self.upsert(saved);
                self.stats[i].favorite_at_ms = Some(at);
                Ok(true)
            }
            (Some(i), false) => {
                self.stats[i].favorite_at_ms = None;
                self.forget_unused(i);
                Ok(true)
            }
        }
    }

    /// Whether the item of `plugin` with `id` is a favourite.
    pub fn is_favorite(&self, plugin: &str, id: &str) -> bool {
        self.stats
            .iter()
            .any(|s| s.favorite_at_ms.is_some() && s.saved.plugin == plugin && s.saved.item.id == id)
    }

    /// The favourites, the latest first.
    pub fn favorites(&self, offset: usize, limit: usize) -> Vec<ItemStats> {
        let mut rows: Vec<&Stats> = self.stats.iter().filter(|s| s.favorite_at_ms.is_some()).collect();
        rows.sort_by_key(|s| (Reverse(s.favorite_at_ms), Reverse(s.id)));
        rows.into_iter()
            .skip(offset)
            .take(limit)
            .map(ItemStats::from)
            .collect()
    }

    /// How many favourites there are.
    pub fn favorite_count(&self) -> usize {
        self.stats.iter().filter(|s| s.favorite_at_ms.is_some()).count()
    }

    /// Records a play that started at `at_ms`: counted (played to its end or
    /// far enough, `rules::play_counts`) or a skip.
    pub fn record_play(&mut self, saved: Saved, at_ms: i64, counted: bool) -> Result<(), CollectionError> {
        song(&saved)?;
        let i = self.upsert(saved);
        let stats = &mut self.stats[i];
        if counted {
            stats.play_count += 1;
            stats.last_played_ms = Some(stats.last_played_ms.map_or(at_ms, |last| last.max(at_ms)));
        } else {
            stats.skip_count += 1;
        }
        Ok(())
    }

    /// The `limit` songs played (counted) most recently, the latest first.
    pub fn recently_played(&self, limit: usize) -> Vec<ItemStats> {
        let mut rows: Vec<&Stats> = self.stats.iter().filter(|s| s.last_played_ms.is_some()).collect();
        rows.sort_by_key(|s| (Reverse(s.last_played_ms), Reverse(s.id)));
        rows.into_iter().take(limit).map(ItemStats::from).collect()
    }

    /// The `limit` songs with the most counted plays; ties by the latest play.
    pub fn most_played(&self, limit: usize) -> Vec<ItemStats> {
        let mut rows: Vec<&Stats> = self.stats.iter().filter(|s| s.play_count > 0).collect();
        rows.sort_by_key(|s| (Reverse(s.play_count), Reverse(s.last_played_ms), s.id));
        rows.into_iter().take(limit).map(ItemStats::from).collect()
    }

    /// The row for `saved`, made if needed, with the item as last given.
    fn upsert(&mut self, saved: Saved) -> usize {
        if let Some(i) = self.stats.iter().position(|s| s.saved.same(&saved)) {
            self.stats[i].saved = saved;
            return i;
        }
        self.last_stats += 1;
        self.stats.push(Stats {
            id: self.last_stats,
            saved,
            favorite_at_ms: None,
            play_count: 0,
            skip_count: 0,
            last_played_ms: None,
        });
        self.stats.len() - 1
    }

    fn forget_unused(&mut self, index: usize) {
        let stats = &self.stats[index];
        if stats.favorite_at_ms.is_none() && stats.play_count == 0 && stats.skip_count == 0 {
            self.stats.remove(index);
        }
    }
}

fn summary(playlist: &StoredPlaylist) -> PlaylistSummary {
    PlaylistSummary {
        id: playlist.id,
        name: playlist.name.clone(),
        entry_count: playlist.entries.len(),
        duration_ms: playlist
            .entries
            .iter()
            .filter_map(|e| e.saved.item.duration_ms)
            .map(u64::from)
            .sum(),
        created_at_ms: playlist.created_at_ms,
        updated_at_ms: playlist.updated_at_ms,
        cover: playlist
            .entries
            .iter()
            .find(|e| e.saved.item.artwork.is_some())
            .map(|e| e.saved.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_model::ImageRef;

    fn track(plugin: &str, id: &str, title: &str) -> Saved {
        Saved {
            plugin: plugin.into(),
            item: Item {
                id: id.into(),
                kind: ItemKind::Track,
                title: title.into(),
                artist: Some("Glass Harbour".into()),
                album: None,
                album_id: None,
                artist_id: None,
                duration_ms: Some(180_000),
                track_number: None,
                disc_number: None,
                year: None,
                child_count: None,
                isrc: None,
                artwork: None,
                explicit: false,
                playable: true,
                unplayable: None,
            },
        }
    }

    fn titles(entries: &[PlaylistEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.item.title.as_str()).collect()
    }

    #[test]
    fn names_are_trimmed_and_listed_in_order() {
        let mut c = Collections::new();
        assert_eq!(c.create_playlist("   ", 1), Err(CollectionError::EmptyName));
        let b = c.create_playlist("  beach ", 1).unwrap();
        let a = c.create_playlist("Ärger", 2).unwrap();
        let z = c.create_playlist("Beach", 3).unwrap();
        let names: Vec<(i64, String)> = c.playlists().into_iter().map(|p| (p.id, p.name)).collect();
        assert_eq!(
            names,
            vec![(a, "Ärger".into()), (b, "beach".into()), (z, "Beach".into())],
            "by normalised name, then by id"
        );
        assert_eq!(c.rename_playlist(b, "", 4), Err(CollectionError::EmptyName));
        c.rename_playlist(b, "beach", 9).unwrap();
        assert_eq!(
            c.playlist(b).unwrap().updated_at_ms,
            1,
            "the same name changes nothing"
        );
        c.rename_playlist(b, " Calm ", 9).unwrap();
        assert_eq!(c.playlist(b).unwrap().name, "Calm");
        assert_eq!(c.playlist(b).unwrap().updated_at_ms, 9);
        assert_eq!(c.rename_playlist(99, "x", 9), Err(CollectionError::NoPlaylist));
    }

    #[test]
    fn ids_are_never_reused() {
        let mut c = Collections::new();
        let first = c.create_playlist("One", 1).unwrap();
        let entry = c
            .add_to_playlist(first, vec![track("p", "1", "A")], None, 1)
            .unwrap()[0];
        c.delete_playlist(first).unwrap();
        assert_eq!(c.delete_playlist(first), Err(CollectionError::NoPlaylist));
        let second = c.create_playlist("Two", 2).unwrap();
        assert!(second > first);
        let again = c
            .add_to_playlist(second, vec![track("p", "1", "A")], None, 2)
            .unwrap()[0];
        assert!(again > entry);
        // Also across a save: the counters come back, or are found from the ids in use.
        let restored = Collections::from_state(&c.state());
        assert_eq!(restored, c);
        let mut stripped =
            Collections::from_state(&c.state().replace("\"last_playlist\":2", "\"last_playlist\":0"));
        assert!(stripped.create_playlist("Three", 3).unwrap() > second);
    }

    #[test]
    fn entries_insert_move_and_remove_in_order() {
        let mut c = Collections::new();
        let id = c.create_playlist("Walk", 1).unwrap();
        let ids = c
            .add_to_playlist(id, vec![track("p", "1", "A"), track("p", "2", "B")], None, 2)
            .unwrap();
        c.add_to_playlist(id, vec![track("library", "1/x.flac", "C")], Some(1), 3)
            .unwrap();
        c.add_to_playlist(id, vec![track("p", "1", "A")], Some(99), 4)
            .unwrap();
        assert_eq!(
            titles(&c.entries(id, 0, 10)),
            ["A", "C", "B", "A"],
            "a song may appear twice"
        );
        assert_eq!(titles(&c.entries(id, 1, 2)), ["C", "B"]);
        assert_eq!(c.entries(id, 1, 2)[0].position, 1);
        c.move_playlist_entry(id, ids[0], 99, 5).unwrap();
        assert_eq!(titles(&c.entries(id, 0, 10)), ["C", "B", "A", "A"]);
        c.move_playlist_entry(id, ids[1], 0, 6).unwrap();
        assert_eq!(titles(&c.entries(id, 0, 10)), ["B", "C", "A", "A"]);
        assert_eq!(
            c.move_playlist_entry(id, 999, 0, 6),
            Err(CollectionError::NoEntry)
        );
        c.remove_from_playlist(id, &[ids[1], 12345], 7).unwrap();
        assert_eq!(titles(&c.entries(id, 0, 10)), ["C", "A", "A"]);
        let positions: Vec<usize> = c.entries(id, 0, 10).iter().map(|e| e.position).collect();
        assert_eq!(positions, [0, 1, 2], "no gaps");
        let summary = c.playlist(id).unwrap();
        assert_eq!(summary.entry_count, 3);
        assert_eq!(summary.duration_ms, 540_000);
        assert_eq!(summary.updated_at_ms, 7);
        c.remove_from_playlist(id, &[999], 8).unwrap();
        assert_eq!(
            c.playlist(id).unwrap().updated_at_ms,
            7,
            "nothing removed, nothing changed"
        );
        assert!(c.entries(42, 0, 10).is_empty());
    }

    #[test]
    fn only_songs_go_in_and_all_or_nothing() {
        let mut c = Collections::new();
        let id = c.create_playlist("Walk", 1).unwrap();
        let mut album = track("p", "9", "An Album");
        album.item.kind = ItemKind::Album;
        assert_eq!(
            c.add_to_playlist(id, vec![track("p", "1", "A"), album.clone()], None, 2),
            Err(CollectionError::NotASong)
        );
        assert!(c.entries(id, 0, 10).is_empty());
        assert_eq!(
            c.set_favorite(album.clone(), true, 1),
            Err(CollectionError::NotASong)
        );
        assert_eq!(c.record_play(album, 1, true), Err(CollectionError::NotASong));
        assert_eq!(
            c.add_to_playlist(77, vec![track("p", "1", "A")], None, 2),
            Err(CollectionError::NoPlaylist)
        );
    }

    #[test]
    fn the_cover_is_the_first_entry_with_artwork() {
        let mut c = Collections::new();
        let id = c.create_playlist("Walk", 1).unwrap();
        let mut pictured = track("p", "2", "B");
        pictured.item.artwork = Some(ImageRef {
            url: "https://music.example/cover/2.jpg".into(),
            headers: Vec::new(),
            width: None,
            height: None,
        });
        c.add_to_playlist(id, vec![track("p", "1", "A"), pictured.clone()], None, 2)
            .unwrap();
        assert_eq!(c.playlist(id).unwrap().cover, Some(pictured));
    }

    #[test]
    fn hearts_keep_their_time_and_order() {
        let mut c = Collections::new();
        assert!(c.set_favorite(track("p", "1", "A"), true, 100).unwrap());
        assert!(
            c.set_favorite(track("p", "2", "B"), true, 100).unwrap(),
            "the same millisecond"
        );
        assert!(
            c.set_favorite(track("p", "3", "C"), true, 50).unwrap(),
            "the clock stepped back"
        );
        let order: Vec<String> = c.favorites(0, 10).into_iter().map(|s| s.item.title).collect();
        assert_eq!(order, ["C", "B", "A"]);
        assert!(
            !c.set_favorite(track("p", "1", "A again"), true, 500).unwrap(),
            "a favourite keeps its time"
        );
        assert_eq!(
            c.favorites(2, 10)[0].item.title,
            "A again",
            "with the item as last given"
        );
        assert!(c.is_favorite("p", "2"));
        assert!(!c.is_favorite("q", "2"), "another source's song with the same id");
        assert_eq!(c.favorite_count(), 3);
        assert!(c.set_favorite(track("p", "2", "B"), false, 600).unwrap());
        assert!(!c.set_favorite(track("p", "2", "B"), false, 600).unwrap());
        assert!(!c.is_favorite("p", "2"));
        assert_eq!(c.favorite_count(), 2);
    }

    #[test]
    fn plays_count_or_skip_and_unused_rows_go() {
        let mut c = Collections::new();
        c.record_play(track("p", "1", "A"), 1_000, true).unwrap();
        c.record_play(track("p", "2", "B"), 2_000, true).unwrap();
        c.record_play(track("p", "2", "B"), 3_000, true).unwrap();
        c.record_play(track("p", "3", "C"), 4_000, false).unwrap();
        c.record_play(track("p", "1", "A"), 500, true).unwrap();
        let recent: Vec<String> = c.recently_played(10).into_iter().map(|s| s.item.title).collect();
        assert_eq!(
            recent,
            ["B", "A"],
            "skips are not plays; an older play keeps the latest"
        );
        let most: Vec<(String, u64)> = c
            .most_played(10)
            .into_iter()
            .map(|s| (s.item.title, s.play_count))
            .collect();
        assert_eq!(
            most,
            [("B".to_owned(), 2), ("A".to_owned(), 2)],
            "ties by the latest play"
        );
        assert_eq!(c.recently_played(1).len(), 1);
        // A skipped song keeps its row; a heart taken back from an unplayed song forgets it.
        c.set_favorite(track("p", "3", "C"), true, 5).unwrap();
        c.set_favorite(track("p", "3", "C"), false, 6).unwrap();
        c.set_favorite(track("p", "4", "D"), true, 7).unwrap();
        c.set_favorite(track("p", "4", "D"), false, 8).unwrap();
        let state: serde_json::Value = serde_json::from_str(&c.state()).unwrap();
        let kept: Vec<&str> = state["stats"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["saved"]["item"]["id"].as_str().unwrap())
            .collect();
        assert_eq!(kept, ["1", "2", "3"]);
    }

    #[test]
    fn a_damaged_state_starts_empty() {
        assert_eq!(Collections::from_state("not json"), Collections::new());
        assert_eq!(Collections::from_state("{}"), Collections::new());
    }
}

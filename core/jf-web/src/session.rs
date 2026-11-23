//! The play queue and what happens around it: what plays next, what Previous
//! does, when a play counts, how a transition sounds.
//!
//! The queue is `jf_session::Queue`, the same as in every app. Entries are
//! source tracks (`TrackRef::Source`, with the display data the source gave)
//! and files: the person's library (`TrackRef::File` under `/library/`) and
//! files dragged in (under `/dropped/`), whose tags this session keeps.
//! Dragged-in files live only as long as the page: [`Session::state`] leaves
//! them out. Library files are saved with the queue; after a restore the page
//! hands their tags back ([`Session::missing_files`]).

use std::collections::HashMap;
use std::path::PathBuf;

use jf_plugin_model::{ImageRef, Item, ItemKind};
use jf_session::{Advance, Queue, QueueError, QueueItem, QueueState, RepeatMode, SourceTrack, TrackRef};
use serde::{Deserialize, Serialize};

use crate::collections::{self, Saved};
use crate::rules::{self, CrossfadeSettings, ReplayGainInfo, Transition};

/// Entries the snapshot lists after the current one (as the core's slice).
pub const UP_NEXT_WINDOW: usize = 50;
/// Items remembered per generation, so a queued track can be handed back to
/// its plugin as the record it listed (as the core does).
const ITEM_GENERATION: usize = 10_000;
/// Where dragged-in files live in the queue's paths.
const DROPPED: &str = "/dropped/";
/// Where library files live in the queue's paths.
const LIBRARY: &str = "/library/";
/// The page's ids for library files start with this (`lib:<key>`).
pub const LIBRARY_ID: &str = "lib:";

/// The queue path of the file the page calls `id`.
fn file_path(id: &str) -> PathBuf {
    match id.strip_prefix(LIBRARY_ID) {
        Some(key) => PathBuf::from(format!("{LIBRARY}{key}")),
        None => PathBuf::from(format!("{DROPPED}{id}")),
    }
}

/// The page's id of the file at a queue path.
fn file_id(path: &std::path::Path) -> Option<String> {
    let path = path.to_str()?;
    if let Some(id) = path.strip_prefix(DROPPED) {
        return Some(id.to_owned());
    }
    path.strip_prefix(LIBRARY).map(|key| format!("{LIBRARY_ID}{key}"))
}
/// The album-key scope of dragged-in files.
const FILE_SCOPE: &str = "file";

/// A dragged-in file, as its tags describe it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileMeta {
    /// The page's id for the file.
    pub id: String,
    /// The file name.
    pub name: String,
    /// Title from the tags, else the file name without its extension.
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub replaygain: Option<ReplayGainInfo>,
}

/// Something to queue.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NewEntry {
    /// A track a source listed.
    Source {
        /// The plugin's id.
        plugin: String,
        /// The item as the plugin listed it.
        item: Box<Item>,
    },
    /// A file added with [`Session::add_file`].
    File {
        /// Its id.
        id: String,
    },
}

/// A queue entry as the views show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// Unique within the queue for its whole life.
    pub uid: u64,
    /// `source` or `file`.
    pub kind: &'static str,
    /// The source's plugin id (sources only).
    pub plugin: Option<String>,
    /// The source's id for the track, or the file's id.
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// Artwork as the source listed it.
    pub artwork_url: Option<String>,
    /// Tracks with the same key join gaplessly with album continuity on.
    pub album_key: Option<String>,
}

/// The queue as the views show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QueueSnapshot {
    /// Increases on every change.
    pub revision: u64,
    pub length: usize,
    /// Index of the current entry in playback order.
    pub position: Option<usize>,
    pub current: Option<Row>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// The next entries in playback order, at most [`UP_NEXT_WINDOW`].
    pub up_next: Vec<Row>,
}

/// One entry of the play history.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryRow {
    pub title: String,
    pub artist: Option<String>,
    pub plugin: Option<String>,
    pub played_at_ms: i64,
}

/// What Previous does.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PreviousAction {
    /// Seek the current track to its start.
    Restart,
    /// Play this entry (the queue moved back).
    Play { row: Box<Row> },
}

/// Items by (plugin, id) in two generations.
#[derive(Debug, Default)]
struct ItemMemory {
    current: HashMap<(String, String), Item>,
    previous: HashMap<(String, String), Item>,
}

impl ItemMemory {
    fn remember(&mut self, plugin: &str, item: &Item) {
        if self.current.len() >= ITEM_GENERATION {
            self.previous = std::mem::take(&mut self.current);
        }
        self.current
            .insert((plugin.to_owned(), item.id.clone()), item.clone());
    }

    fn get(&self, plugin: &str, id: &str) -> Option<&Item> {
        let key = (plugin.to_owned(), id.to_owned());
        self.current.get(&key).or_else(|| self.previous.get(&key))
    }
}

/// The play in progress, for the play-count rule.
#[derive(Debug, Clone, Copy)]
struct Play {
    uid: u64,
    duration_ms: Option<u64>,
    started_ms: i64,
}

/// A play that ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Finished {
    pub uid: u64,
    /// It counts as a play (`rules::play_counts`); otherwise it was a skip.
    pub counted: bool,
    /// When it started (wall clock, ms).
    pub started_ms: i64,
}

/// The queue and its rules. See the module documentation.
#[derive(Debug)]
pub struct Session {
    queue: Queue,
    files: HashMap<String, FileMeta>,
    items: ItemMemory,
    play: Option<Play>,
}

impl Session {
    /// An empty session whose shuffles derive from `seed`.
    pub fn new(seed: u64) -> Session {
        Session {
            queue: Queue::with_seed(seed),
            files: HashMap::new(),
            items: ItemMemory::default(),
            play: None,
        }
    }

    /// A session from a saved [`Session::state`].
    pub fn restore(state: QueueState) -> Result<Session, QueueError> {
        let mut queue = Queue::from_state(state)?;
        // Only source tracks and library files belong in a saved web queue.
        let dropped: Vec<u64> = queue
            .iter()
            .filter(|i| match &i.track {
                TrackRef::File(path) => !path.starts_with(LIBRARY),
                TrackRef::Library(_) => true,
                _ => false,
            })
            .map(|i| i.uid)
            .collect();
        for uid in dropped {
            queue.remove(uid)?;
        }
        Ok(Session {
            queue,
            files: HashMap::new(),
            items: ItemMemory::default(),
            play: None,
        })
    }

    /// The queue to save: everything but dragged-in files.
    pub fn state(&self) -> QueueState {
        let mut copy = self.queue.clone();
        let files: Vec<u64> = copy
            .iter()
            .filter(|i| matches!(&i.track, TrackRef::File(path) if path.starts_with(DROPPED)))
            .map(|i| i.uid)
            .collect();
        for uid in files {
            let _ = copy.remove(uid);
        }
        copy.to_state()
    }

    /// Adds a dragged-in file so it can be queued.
    pub fn add_file(&mut self, meta: FileMeta) {
        self.files.insert(meta.id.clone(), meta);
    }

    /// The tags of a dragged-in file.
    pub fn file(&self, id: &str) -> Option<&FileMeta> {
        self.files.get(id)
    }

    /// The ids of queued files whose tags this session does not have: library files of a
    /// restored queue, for the page to hand back with [`Session::add_file`].
    pub fn missing_files(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .queue
            .iter()
            .filter_map(|i| match &i.track {
                TrackRef::File(path) => file_id(path).filter(|id| !self.files.contains_key(id)),
                _ => None,
            })
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Remembers items a source listed, so the queue can hand them back.
    pub fn remember(&mut self, plugin: &str, items: &[Item]) {
        for item in items {
            self.items.remember(plugin, item);
        }
    }

    fn track_ref(&mut self, entry: NewEntry) -> Option<TrackRef> {
        match entry {
            NewEntry::Source { plugin, item } => {
                if item.kind != ItemKind::Track || !item.playable {
                    return None;
                }
                self.items.remember(&plugin, &item);
                Some(TrackRef::Source(source_track(&plugin, &item)))
            }
            NewEntry::File { id } => self
                .files
                .contains_key(&id)
                .then(|| TrackRef::File(file_path(&id))),
        }
    }

    /// Replaces the queue with `entries` (unplayable ones are left out) and
    /// makes `start` current; with `shuffle` the rest plays shuffled.
    pub fn play(&mut self, entries: Vec<NewEntry>, start: usize, shuffle: bool) -> Result<(), QueueError> {
        let mut tracks = Vec::with_capacity(entries.len());
        let mut start_at = None;
        for (index, entry) in entries.into_iter().enumerate() {
            if let Some(track) = self.track_ref(entry) {
                if index <= start {
                    start_at = Some(tracks.len());
                }
                tracks.push(track);
            }
        }
        if tracks.is_empty() {
            return Err(QueueError::InvalidState("nothing to play".into()));
        }
        let start = start_at.unwrap_or(0).min(tracks.len() - 1);
        self.queue.set_shuffle(shuffle);
        self.queue.set_items(tracks, Some(start))?;
        Ok(())
    }

    /// Play Next (`next`) or Play Last. Returns the new entries' uids.
    pub fn enqueue(&mut self, entries: Vec<NewEntry>, next: bool) -> Vec<u64> {
        let tracks: Vec<TrackRef> = entries.into_iter().filter_map(|e| self.track_ref(e)).collect();
        if next {
            self.queue.insert_next(tracks)
        } else {
            self.queue.append(tracks)
        }
    }

    /// Removes an entry; true when it was the current one.
    pub fn remove(&mut self, uid: u64) -> Result<bool, QueueError> {
        Ok(self.queue.remove(uid)?.was_current)
    }

    /// Moves an entry to `to` in playback order.
    pub fn move_item(&mut self, uid: u64, to: usize) -> Result<(), QueueError> {
        self.queue.move_item(uid, to)
    }

    /// Makes an entry current.
    pub fn jump_to(&mut self, uid: u64) -> Result<Row, QueueError> {
        let item = self.queue.jump_to(uid)?;
        Ok(self.row(&item))
    }

    /// Empties the queue (the history stays).
    pub fn clear(&mut self) {
        self.queue.clear();
        self.play = None;
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.queue.set_shuffle(on);
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.queue.set_repeat(mode);
    }

    /// Next: the entry to play, or `None` at the end of the queue (playback
    /// stops and the queue rewinds to its first entry, so Play starts again).
    pub fn next(&mut self, how: Advance) -> Option<Row> {
        match self.queue.next(how) {
            Some(item) => Some(self.row(&item)),
            None => {
                if let Some(first) = self.queue.item_at(0) {
                    let _ = self.queue.jump_to(first.uid);
                }
                None
            }
        }
    }

    /// Previous at `position_ms` into the current track (`active`: playing
    /// or paused): restart it after three seconds, else go back an entry
    /// (restart when there is none before it).
    pub fn previous(&mut self, position_ms: u64, active: bool) -> Option<PreviousAction> {
        if rules::previous_restarts(position_ms, active) {
            return Some(PreviousAction::Restart);
        }
        let before = self.queue.current_uid();
        let item = self.queue.previous()?;
        if Some(item.uid) == before {
            return Some(PreviousAction::Restart);
        }
        Some(PreviousAction::Play {
            row: Box::new(self.row(&item)),
        })
    }

    /// The current entry.
    pub fn current(&self) -> Option<Row> {
        self.queue.current().map(|i| self.row(&i))
    }

    /// The entry that plays next on its own (what to prepare for a gapless
    /// join or a crossfade).
    pub fn upcoming(&self) -> Option<Row> {
        self.queue.peek_next(Advance::Auto).map(|i| self.row(&i))
    }

    /// How the current entry hands over to `next` on its own. Durations
    /// come from the audio when the player knows them, else from the rows.
    pub fn transition(
        &self,
        settings: CrossfadeSettings,
        next_uid: u64,
        current_ms: Option<u64>,
        next_ms: Option<u64>,
    ) -> Transition {
        let current = self.current();
        let next = self.queue.get(next_uid).map(|track| {
            self.row(&QueueItem {
                uid: next_uid,
                track: track.clone(),
            })
        });
        let (Some(current), Some(next)) = (current, next) else {
            return Transition::Gapless;
        };
        rules::transition(
            settings,
            current.album_key.as_deref(),
            next.album_key.as_deref(),
            current_ms.or(current.duration_ms),
            next_ms.or(next.duration_ms),
        )
    }

    /// The track behind an entry, for its plugin: the item as listed, or one
    /// rebuilt from the queue's display data (a restored queue).
    pub fn source_item(&self, uid: u64) -> Option<(String, Item)> {
        let TrackRef::Source(track) = self.queue.get(uid)? else {
            return None;
        };
        let item = self
            .items
            .get(&track.plugin, &track.id)
            .cloned()
            .unwrap_or_else(|| item_from(track));
        Some((track.plugin.clone(), item))
    }

    /// The dragged-in file behind an entry.
    pub fn file_of(&self, uid: u64) -> Option<&FileMeta> {
        let TrackRef::File(path) = self.queue.get(uid)? else {
            return None;
        };
        self.files.get(&file_id(path)?)
    }

    /// An entry started playing at `now_ms` (wall clock): it goes into the
    /// history, and its play is timed for the play-count rule.
    pub fn started(&mut self, uid: u64, now_ms: i64, duration_ms: Option<u64>) {
        let Some(track) = self.queue.get(uid).cloned() else {
            return;
        };
        let duration_ms = duration_ms.or_else(|| self.row_for(uid, &track).duration_ms);
        self.queue.record_played(track, now_ms);
        self.play = Some(Play {
            uid,
            duration_ms,
            started_ms: now_ms,
        });
    }

    /// The play in progress ended after `played_ms`: `natural` when it
    /// reached its end. Returns whether it counts as a play (else a skip),
    /// or `None` when nothing was playing.
    pub fn finish(&mut self, played_ms: u64, natural: bool) -> Option<Finished> {
        let play = self.play.take()?;
        Some(Finished {
            uid: play.uid,
            counted: rules::play_counts(played_ms, play.duration_ms, natural),
            started_ms: play.started_ms,
        })
    }

    /// What an entry is for hearts, playlists and plays: a source's song with
    /// its plugin, a library song under the library's id (its key as id), or
    /// `None` for a dropped file, which is kept nowhere.
    pub fn saved(&self, uid: u64) -> Option<Saved> {
        match self.queue.get(uid)? {
            TrackRef::Source(_) => {
                let (plugin, item) = self.source_item(uid)?;
                Some(Saved { plugin, item })
            }
            TrackRef::File(path) => {
                let id = file_id(path)?;
                let key = id.strip_prefix(LIBRARY_ID)?;
                let meta = self.files.get(&id);
                let item = item_from(&SourceTrack {
                    plugin: collections::LIBRARY.to_owned(),
                    id: key.to_owned(),
                    title: meta.map_or_else(|| key.to_owned(), |m| m.title.clone()),
                    artist: meta.and_then(|m| m.artist.clone()),
                    album: meta.and_then(|m| m.album.clone()),
                    duration_ms: meta.and_then(|m| m.duration_ms),
                    artwork_url: None,
                    isrc: None,
                });
                Some(Saved {
                    plugin: collections::LIBRARY.to_owned(),
                    item,
                })
            }
            _ => None,
        }
    }

    /// The queue for the views.
    pub fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            revision: self.queue.revision(),
            length: self.queue.len(),
            position: self.queue.position(),
            current: self.current(),
            shuffle: self.queue.is_shuffled(),
            repeat: self.queue.repeat(),
            up_next: self
                .queue
                .up_next(UP_NEXT_WINDOW)
                .iter()
                .map(|i| self.row(i))
                .collect(),
        }
    }

    /// A window of the playback order, for paged lists.
    pub fn window(&self, offset: usize, limit: usize) -> Vec<Row> {
        self.queue
            .window(offset, limit)
            .iter()
            .map(|i| self.row(i))
            .collect()
    }

    /// The play history, newest first, at most `limit` entries.
    pub fn history(&self, limit: usize) -> Vec<HistoryRow> {
        let entries: Vec<_> = self.queue.history().collect();
        entries
            .iter()
            .rev()
            .take(limit)
            .map(|entry| {
                let row = self.row_for(0, &entry.track);
                HistoryRow {
                    title: row.title,
                    artist: row.artist,
                    plugin: row.plugin,
                    played_at_ms: entry.played_at_ms,
                }
            })
            .collect()
    }

    fn row(&self, item: &QueueItem) -> Row {
        self.row_for(item.uid, &item.track)
    }

    fn row_for(&self, uid: u64, track: &TrackRef) -> Row {
        match track {
            TrackRef::Source(t) => Row {
                uid,
                kind: "source",
                plugin: Some(t.plugin.clone()),
                id: t.id.clone(),
                title: t.title.clone(),
                artist: t.artist.clone(),
                album: t.album.clone(),
                duration_ms: t.duration_ms,
                artwork_url: t.artwork_url.clone(),
                album_key: rules::album_key(Some(&t.plugin), t.album.as_deref(), t.artist.as_deref()),
            },
            TrackRef::File(path) => {
                let id = file_id(path).unwrap_or_default();
                let meta = self.files.get(&id);
                Row {
                    uid,
                    kind: "file",
                    plugin: None,
                    title: meta.map_or_else(|| id.clone(), |m| m.title.clone()),
                    artist: meta.and_then(|m| m.artist.clone()),
                    album: meta.and_then(|m| m.album.clone()),
                    duration_ms: meta.and_then(|m| m.duration_ms),
                    artwork_url: None,
                    album_key: meta.and_then(|m| {
                        rules::album_key(Some(FILE_SCOPE), m.album.as_deref(), m.artist.as_deref())
                    }),
                    id,
                }
            }
            TrackRef::Library(id) => Row {
                uid,
                kind: "file",
                plugin: None,
                id: id.to_string(),
                title: id.to_string(),
                artist: None,
                album: None,
                duration_ms: None,
                artwork_url: None,
                album_key: None,
            },
        }
    }
}

/// The queue entry for a source's item (as the core makes it).
pub fn source_track(plugin: &str, item: &Item) -> SourceTrack {
    SourceTrack {
        plugin: plugin.to_owned(),
        id: item.id.clone(),
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_ms.map(u64::from),
        artwork_url: item.artwork.as_ref().map(|a| a.url.clone()),
        isrc: item.isrc.clone(),
    }
}

/// A track record rebuilt from the queue's display data.
fn item_from(track: &SourceTrack) -> Item {
    Item {
        id: track.id.clone(),
        kind: ItemKind::Track,
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        album_id: None,
        artist_id: None,
        duration_ms: track.duration_ms.and_then(|d| u32::try_from(d).ok()),
        track_number: None,
        disc_number: None,
        year: None,
        child_count: None,
        isrc: track.isrc.clone(),
        artwork: track.artwork_url.clone().map(|url| ImageRef {
            url,
            headers: Vec::new(),
            width: None,
            height: None,
        }),
        explicit: false,
        playable: true,
        unplayable: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, album: &str, duration_ms: u32) -> Item {
        Item {
            id: id.into(),
            kind: ItemKind::Track,
            title: format!("Song {id}"),
            artist: Some("Artist".into()),
            album: Some(album.into()),
            album_id: None,
            artist_id: None,
            duration_ms: Some(duration_ms),
            track_number: None,
            disc_number: None,
            year: None,
            child_count: None,
            isrc: None,
            artwork: None,
            explicit: false,
            playable: true,
            unplayable: None,
        }
    }

    fn source(id: &str, album: &str) -> NewEntry {
        NewEntry::Source {
            plugin: "org.example.p".into(),
            item: Box::new(item(id, album, 200_000)),
        }
    }

    fn titles(rows: &[Row]) -> Vec<String> {
        rows.iter().map(|r| r.id.clone()).collect()
    }

    #[test]
    fn plays_and_advances_like_the_core() {
        let mut s = Session::new(7);
        s.play(
            vec![source("a", "X"), source("b", "X"), source("c", "Y")],
            1,
            false,
        )
        .unwrap();
        assert_eq!(s.current().unwrap().id, "b");
        assert_eq!(s.upcoming().unwrap().id, "c");
        assert_eq!(s.next(Advance::Manual).unwrap().id, "c");
        // The end of the queue rewinds to the first entry.
        assert_eq!(s.next(Advance::Auto), None);
        assert_eq!(s.current().unwrap().id, "a");
        assert_eq!(titles(&s.snapshot().up_next), ["b", "c"]);
    }

    #[test]
    fn unplayable_items_never_reach_the_queue() {
        let mut s = Session::new(1);
        let mut locked = item("locked", "X", 1000);
        locked.playable = false;
        let mut album = item("al", "X", 1000);
        album.kind = ItemKind::Album;
        s.play(
            vec![
                NewEntry::Source {
                    plugin: "p".into(),
                    item: Box::new(locked),
                },
                NewEntry::Source {
                    plugin: "p".into(),
                    item: Box::new(album),
                },
                source("ok", "X"),
            ],
            2,
            false,
        )
        .unwrap();
        assert_eq!(s.snapshot().length, 1);
        assert_eq!(s.current().unwrap().id, "ok");
        assert!(s.play(vec![], 0, false).is_err());
    }

    #[test]
    fn previous_restarts_after_three_seconds() {
        let mut s = Session::new(1);
        s.play(vec![source("a", "X"), source("b", "X")], 1, false)
            .unwrap();
        assert_eq!(s.previous(4_000, true), Some(PreviousAction::Restart));
        let PreviousAction::Play { row } = s.previous(1_000, true).unwrap() else {
            panic!("expected to go back");
        };
        assert_eq!(row.id, "a");
        assert_eq!(
            s.previous(0, true),
            Some(PreviousAction::Restart),
            "nothing before"
        );
    }

    #[test]
    fn transitions_use_album_keys() {
        let mut s = Session::new(1);
        s.play(
            vec![source("a", "X"), source("b", "X"), source("c", "Y")],
            0,
            false,
        )
        .unwrap();
        let on = CrossfadeSettings {
            enabled: true,
            ..CrossfadeSettings::default()
        };
        let next = s.upcoming().unwrap();
        assert_eq!(s.transition(on, next.uid, None, None), Transition::Gapless);
        s.next(Advance::Auto);
        let next = s.upcoming().unwrap();
        assert_eq!(
            s.transition(on, next.uid, Some(200_000), None),
            Transition::Crossfade { overlap_ms: 6_000 }
        );
    }

    #[test]
    fn counts_plays_and_keeps_history() {
        let mut s = Session::new(1);
        s.play(vec![source("a", "X")], 0, false).unwrap();
        let uid = s.current().unwrap().uid;
        s.started(uid, 1_700_000_000_000, None);
        let skipped = Finished {
            uid,
            counted: false,
            started_ms: 1_700_000_000_000,
        };
        assert_eq!(s.finish(99_000, false), Some(skipped));
        assert_eq!(s.finish(0, true), None, "nothing playing");
        s.started(uid, 1_700_000_100_000, None);
        let counted = Finished {
            uid,
            counted: true,
            started_ms: 1_700_000_100_000,
        };
        assert_eq!(s.finish(100_000, false), Some(counted));
        let history = s.history(10);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].played_at_ms, 1_700_000_100_000);
    }

    #[test]
    fn entries_say_what_they_are_for_hearts_and_plays() {
        let mut s = Session::new(1);
        let meta = FileMeta {
            id: "lib:3/Album/01 Tone.flac".into(),
            name: "01 Tone.flac".into(),
            title: "Tone".into(),
            artist: Some("Aurora".into()),
            album: Some("Sky".into()),
            duration_ms: Some(5_000),
            replaygain: None,
        };
        s.add_file(meta.clone());
        s.add_file(FileMeta {
            id: "f1".into(),
            ..meta.clone()
        });
        s.play(
            vec![
                source("a", "X"),
                NewEntry::File { id: meta.id.clone() },
                NewEntry::File { id: "f1".into() },
            ],
            0,
            false,
        )
        .unwrap();
        let rows = s.window(0, 3);
        let source = s.saved(rows[0].uid).unwrap();
        assert_eq!(
            (source.plugin.as_str(), source.item.id.as_str()),
            ("org.example.p", "a")
        );
        let library = s.saved(rows[1].uid).unwrap();
        assert_eq!(library.plugin, "library");
        assert_eq!(library.item.id, "3/Album/01 Tone.flac", "the library's key");
        assert_eq!(library.item.title, "Tone");
        assert_eq!(library.item.duration_ms, Some(5_000));
        assert_eq!(library.item.kind, ItemKind::Track);
        assert!(s.saved(rows[2].uid).is_none(), "a dropped file is kept nowhere");
        assert!(s.saved(9_999).is_none());
    }

    #[test]
    fn library_files_are_saved_and_their_tags_handed_back() {
        let meta = FileMeta {
            id: "lib:3/Album/01 Tone.flac".into(),
            name: "01 Tone.flac".into(),
            title: "Tone".into(),
            artist: Some("Aurora".into()),
            album: Some("Sky".into()),
            duration_ms: Some(5_000),
            replaygain: None,
        };
        let mut s = Session::new(1);
        s.add_file(meta.clone());
        s.add_file(FileMeta {
            id: "f1".into(),
            ..meta.clone()
        });
        s.play(
            vec![
                NewEntry::File { id: meta.id.clone() },
                NewEntry::File { id: "f1".into() },
            ],
            0,
            false,
        )
        .unwrap();
        assert!(s.missing_files().is_empty());
        let saved = s.state();
        assert_eq!(saved.items.len(), 1, "the dropped file is not saved");
        let mut restored = Session::restore(saved).unwrap();
        assert_eq!(restored.missing_files(), std::slice::from_ref(&meta.id));
        let uid = restored.window(0, 1)[0].uid;
        assert!(restored.file_of(uid).is_none());
        restored.add_file(meta.clone());
        assert!(restored.missing_files().is_empty());
        assert_eq!(restored.file_of(uid), Some(&meta));
        assert_eq!(restored.current().unwrap().title, "Tone");
    }

    #[test]
    fn files_play_but_are_not_saved() {
        let mut s = Session::new(1);
        s.add_file(FileMeta {
            id: "f1".into(),
            name: "tone.flac".into(),
            title: "Tone".into(),
            artist: None,
            album: Some("Tests".into()),
            duration_ms: Some(5_000),
            replaygain: None,
        });
        s.play(
            vec![source("a", "X"), NewEntry::File { id: "f1".into() }],
            1,
            false,
        )
        .unwrap();
        let current = s.current().unwrap();
        assert_eq!((current.kind, current.title.as_str()), ("file", "Tone"));
        assert_eq!(s.file_of(current.uid).unwrap().name, "tone.flac");
        assert!(s.source_item(current.uid).is_none());
        let saved = s.state();
        assert_eq!(saved.items.len(), 1);
        let restored = Session::restore(saved).unwrap();
        assert_eq!(restored.snapshot().length, 1);
        let uid = restored.window(0, 1)[0].uid;
        let (plugin, item) = restored.source_item(uid).unwrap();
        assert_eq!((plugin.as_str(), item.id.as_str()), ("org.example.p", "a"));
        assert_eq!(item.duration_ms, Some(200_000));
    }
}

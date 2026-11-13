//! Persistence of the session in SQLite: the queue, the playback position
//! to restore, and small settings stored as JSON.
//!
//! The session database is separate from the library catalogue, so a
//! library rebuild never loses the queue, and the session can be written
//! often (debounced by the caller) without contending with scans.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::queue::{HistoryEntry, Queue, QueueError, QueueItem, QueueState, RepeatMode, TrackRef};

/// Current schema version (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i32 = 2;

/// Errors from the session store.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("session setting is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Queue(#[from] QueueError),
    #[error("session database schema version {found} is newer than this build supports ({supported})")]
    TooNew { found: i32, supported: i32 },
    #[error("corrupt session data: {0}")]
    Corrupt(String),
}

/// Where playback was when the session was saved.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlaybackState {
    /// The queue entry that was current.
    pub uid: Option<u64>,
    /// Position inside that entry, in milliseconds.
    pub position_ms: u64,
    /// Whether it was playing (restores paused either way unless the
    /// listener asks to resume on launch).
    pub was_playing: bool,
}

/// The session database.
#[derive(Debug)]
pub struct SessionStore {
    conn: Connection,
}

impl SessionStore {
    /// Opens (creating and migrating) the session database at `path`.
    pub fn open(path: &Path) -> Result<Self, SessionError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    /// A store that lives in memory, for tests and one-shot CLI runs.
    pub fn open_in_memory() -> Result<Self, SessionError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, SessionError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(SessionError::TooNew {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        if version < 1 {
            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE IF NOT EXISTS queue_items (
                     uid           INTEGER PRIMARY KEY,
                     canonical_pos INTEGER NOT NULL,
                     order_pos     INTEGER NOT NULL,
                     kind          TEXT    NOT NULL,
                     library_id    INTEGER,
                     path          BLOB
                 );
                 CREATE TABLE IF NOT EXISTS queue_meta (
                     key   TEXT PRIMARY KEY,
                     value INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS history (
                     seq          INTEGER PRIMARY KEY AUTOINCREMENT,
                     kind         TEXT    NOT NULL,
                     library_id   INTEGER,
                     path         BLOB,
                     played_at_ms INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS settings (
                     key   TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 PRAGMA user_version = 1;
                 COMMIT;",
            )?;
        }
        if version < 2 {
            // v2: tracks from online sources, stored as JSON.
            conn.execute_batch(
                "BEGIN;
                 ALTER TABLE queue_items ADD COLUMN source TEXT;
                 ALTER TABLE history ADD COLUMN source TEXT;
                 PRAGMA user_version = 2;
                 COMMIT;",
            )?;
        }
        Ok(SessionStore { conn })
    }

    /// Saves the whole queue in one transaction, replacing what was stored.
    pub fn save_queue(&mut self, queue: &Queue) -> Result<(), SessionError> {
        let state = queue.to_state();
        let order_pos: std::collections::HashMap<u64, usize> =
            state.order.iter().enumerate().map(|(i, u)| (*u, i)).collect();
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM queue_items", [])?;
        tx.execute("DELETE FROM queue_meta", [])?;
        tx.execute("DELETE FROM history", [])?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO queue_items (uid, canonical_pos, order_pos, kind, library_id, path, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (ci, item) in state.items.iter().enumerate() {
                let oi = order_pos
                    .get(&item.uid)
                    .copied()
                    .ok_or_else(|| SessionError::Corrupt(format!("uid {} not in order", item.uid)))?;
                let (kind, library_id, path, source) = encode_track(&item.track)?;
                insert.execute(params![
                    u64_to_i64(item.uid),
                    ci as i64,
                    oi as i64,
                    kind,
                    library_id,
                    path,
                    source
                ])?;
            }
            let mut meta = tx.prepare("INSERT INTO queue_meta (key, value) VALUES (?1, ?2)")?;
            let position = state.position.map_or(-1, |p| p as i64);
            let repeat = match state.repeat {
                RepeatMode::Off => 0,
                RepeatMode::All => 1,
                RepeatMode::One => 2,
            };
            for (key, value) in [
                ("position", position),
                ("shuffle", i64::from(state.shuffle)),
                ("repeat", repeat),
                ("next_uid", u64_to_i64(state.next_uid)),
                ("seed", u64_to_i64(state.seed)),
                ("shuffles", u64_to_i64(state.shuffles)),
            ] {
                meta.execute(params![key, value])?;
            }
            let mut hist = tx.prepare(
                "INSERT INTO history (kind, library_id, path, source, played_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for entry in &state.history {
                let (kind, library_id, path, source) = encode_track(&entry.track)?;
                hist.execute(params![kind, library_id, path, source, entry.played_at_ms])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Loads the saved queue, or `None` when nothing was saved yet.
    pub fn load_queue(&self) -> Result<Option<Queue>, SessionError> {
        let meta = |key: &str| -> Result<Option<i64>, SessionError> {
            Ok(self
                .conn
                .query_row("SELECT value FROM queue_meta WHERE key = ?1", [key], |r| r.get(0))
                .optional()?)
        };
        let Some(next_uid) = meta("next_uid")? else {
            return Ok(None);
        };
        let position = meta("position")?.unwrap_or(-1);
        let repeat = match meta("repeat")?.unwrap_or(0) {
            0 => RepeatMode::Off,
            1 => RepeatMode::All,
            2 => RepeatMode::One,
            other => return Err(SessionError::Corrupt(format!("repeat mode {other}"))),
        };

        let mut stmt = self.conn.prepare(
            "SELECT uid, order_pos, kind, library_id, path, source FROM queue_items ORDER BY canonical_pos",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<Vec<u8>>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?;
        let mut items = Vec::new();
        let mut by_order = Vec::new();
        for row in rows {
            let (uid, order_pos, kind, library_id, path, source) = row?;
            let uid = i64_to_u64(uid);
            items.push(QueueItem {
                uid,
                track: decode_track(&kind, library_id, path, source)?,
            });
            by_order.push((order_pos, uid));
        }
        by_order.sort_unstable();
        let order = by_order.into_iter().map(|(_, u)| u).collect();

        let mut stmt = self
            .conn
            .prepare("SELECT kind, library_id, path, source, played_at_ms FROM history ORDER BY seq")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<Vec<u8>>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        let mut history = Vec::new();
        for row in rows {
            let (kind, library_id, path, source, played_at_ms) = row?;
            history.push(HistoryEntry {
                track: decode_track(&kind, library_id, path, source)?,
                played_at_ms,
            });
        }

        let state = QueueState {
            items,
            order,
            position: usize::try_from(position).ok(),
            shuffle: meta("shuffle")?.unwrap_or(0) != 0,
            repeat,
            history,
            next_uid: i64_to_u64(next_uid),
            seed: i64_to_u64(meta("seed")?.unwrap_or(0)),
            shuffles: i64_to_u64(meta("shuffles")?.unwrap_or(0)),
        };
        Ok(Some(Queue::from_state(state)?))
    }

    /// Saves where playback is, for restore on the next launch.
    pub fn save_playback(&self, state: &PlaybackState) -> Result<(), SessionError> {
        self.set_setting("playback", state)
    }

    pub fn load_playback(&self) -> Result<Option<PlaybackState>, SessionError> {
        self.setting("playback")
    }

    /// Stores a small setting as JSON (volume, EQ, ReplayGain mode, …).
    pub fn set_setting<T: Serialize + ?Sized>(&self, key: &str, value: &T) -> Result<(), SessionError> {
        let json = serde_json::to_string(value)?;
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, json],
        )?;
        Ok(())
    }

    /// Reads a setting stored with [`SessionStore::set_setting`].
    pub fn setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, SessionError> {
        let json: Option<String> = self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?;
        json.map(|j| serde_json::from_str(&j))
            .transpose()
            .map_err(SessionError::from)
    }

    pub fn remove_setting(&self, key: &str) -> Result<(), SessionError> {
        self.conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }
}

type EncodedTrack = (&'static str, Option<i64>, Option<Vec<u8>>, Option<String>);

fn encode_track(track: &TrackRef) -> Result<EncodedTrack, SessionError> {
    Ok(match track {
        TrackRef::Library(id) => ("library", Some(*id), None, None),
        TrackRef::File(path) => ("file", None, Some(path_to_bytes(path)), None),
        TrackRef::Source(source) => ("source", None, None, Some(serde_json::to_string(source)?)),
    })
}

fn decode_track(
    kind: &str,
    library_id: Option<i64>,
    path: Option<Vec<u8>>,
    source: Option<String>,
) -> Result<TrackRef, SessionError> {
    match (kind, library_id, path, source) {
        ("library", Some(id), _, _) => Ok(TrackRef::Library(id)),
        ("file", _, Some(bytes), _) => Ok(TrackRef::File(path_from_bytes(&bytes))),
        ("source", _, _, Some(json)) => Ok(TrackRef::Source(serde_json::from_str(&json)?)),
        (kind, _, _, _) => Err(SessionError::Corrupt(format!(
            "track of kind {kind:?} without its value"
        ))),
    }
}

/// uids, seeds and counters are `u64`; SQLite integers are `i64`. The bit
/// pattern is stored as is and restored exactly.
fn u64_to_i64(v: u64) -> i64 {
    i64::from_ne_bytes(v.to_ne_bytes())
}

fn i64_to_u64(v: i64) -> u64 {
    u64::from_ne_bytes(v.to_ne_bytes())
}

// Paths are stored as their exact OS bytes so names that are not valid
// UTF-8 survive a round trip.

#[cfg(unix)]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(windows)]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    let wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    PathBuf::from(std::ffi::OsString::from_wide(&wide))
}

#[cfg(not(any(unix, windows)))]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

#[cfg(not(any(unix, windows)))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::Advance;

    fn sample_queue() -> Queue {
        let mut q = Queue::with_seed(21);
        q.set_items((1..=12).map(TrackRef::Library).collect(), Some(3))
            .unwrap();
        q.append(vec![TrackRef::File(PathBuf::from("/music/Ünïcode 曲.flac"))]);
        q.append(vec![TrackRef::Source(crate::queue::SourceTrack {
            plugin: "org.example.test".into(),
            id: "track-7".into(),
            title: "An Invented Title".into(),
            artist: Some("Test Artist".into()),
            album: None,
            duration_ms: Some(201_000),
            artwork_url: Some("https://music.example/art/7.jpg".into()),
            isrc: None,
        })]);
        q.set_shuffle(true);
        q.set_repeat(RepeatMode::One);
        q.next(Advance::Manual);
        q.record_played(TrackRef::Library(4), 1_700_000_000_000);
        q.record_played(TrackRef::File(PathBuf::from("/music/x.mp3")), 1_700_000_100_000);
        q
    }

    #[test]
    fn empty_store_has_no_queue() {
        let store = SessionStore::open_in_memory().unwrap();
        assert!(store.load_queue().unwrap().is_none());
        assert!(store.load_playback().unwrap().is_none());
    }

    #[test]
    fn queue_round_trips_through_sqlite() {
        let mut store = SessionStore::open_in_memory().unwrap();
        let q = sample_queue();
        store.save_queue(&q).unwrap();
        let loaded = store.load_queue().unwrap().unwrap();
        assert_eq!(loaded.to_state(), q.to_state());

        // Saving again replaces the previous queue.
        let mut q2 = Queue::with_seed(1);
        q2.set_items(vec![TrackRef::Library(99)], None).unwrap();
        store.save_queue(&q2).unwrap();
        assert_eq!(store.load_queue().unwrap().unwrap().to_state(), q2.to_state());
    }

    #[test]
    fn survives_reopening_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.sqlite");
        let q = sample_queue();
        {
            let mut store = SessionStore::open(&path).unwrap();
            store.save_queue(&q).unwrap();
            store
                .save_playback(&PlaybackState {
                    uid: q.current_uid(),
                    position_ms: 83_250,
                    was_playing: true,
                })
                .unwrap();
            store.set_setting("volume", &0.8_f32).unwrap();
        }
        let store = SessionStore::open(&path).unwrap();
        assert_eq!(store.load_queue().unwrap().unwrap().to_state(), q.to_state());
        let pb = store.load_playback().unwrap().unwrap();
        assert_eq!(pb.position_ms, 83_250);
        assert_eq!(pb.uid, q.current_uid());
        assert_eq!(store.setting::<f32>("volume").unwrap(), Some(0.8));
        store.remove_setting("volume").unwrap();
        assert_eq!(store.setting::<f32>("volume").unwrap(), None);
    }

    #[test]
    fn migrates_a_version_1_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.sqlite");
        {
            // The v1 schema as v0.0.2 created it, with one saved entry.
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE queue_items (uid INTEGER PRIMARY KEY, canonical_pos INTEGER NOT NULL,
                     order_pos INTEGER NOT NULL, kind TEXT NOT NULL, library_id INTEGER, path BLOB);
                 CREATE TABLE queue_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
                 CREATE TABLE history (seq INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL,
                     library_id INTEGER, path BLOB, played_at_ms INTEGER NOT NULL);
                 CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO queue_items VALUES (1, 0, 0, 'library', 42, NULL);
                 INSERT INTO queue_meta VALUES ('position', 0), ('shuffle', 0), ('repeat', 0),
                     ('next_uid', 2), ('seed', 5), ('shuffles', 0);
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let mut store = SessionStore::open(&path).unwrap();
        let q = store.load_queue().unwrap().unwrap();
        assert_eq!(q.current().unwrap().track, TrackRef::Library(42));
        store.save_queue(&sample_queue()).unwrap();
        assert_eq!(
            store.load_queue().unwrap().unwrap().to_state(),
            sample_queue().to_state()
        );
    }

    #[test]
    fn rejects_a_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        assert!(matches!(
            SessionStore::open(&path),
            Err(SessionError::TooNew { found: 99, .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_round_trip() {
        use std::os::unix::ffi::OsStrExt;
        let raw = std::ffi::OsStr::from_bytes(b"/music/\xff\xfe-bad.flac");
        let mut q = Queue::with_seed(1);
        q.set_items(vec![TrackRef::File(PathBuf::from(raw))], Some(0))
            .unwrap();
        let mut store = SessionStore::open_in_memory().unwrap();
        store.save_queue(&q).unwrap();
        let loaded = store.load_queue().unwrap().unwrap();
        assert_eq!(
            loaded.current().unwrap().track,
            TrackRef::File(PathBuf::from(raw))
        );
    }

    #[test]
    fn extreme_ids_round_trip() {
        assert_eq!(i64_to_u64(u64_to_i64(u64::MAX)), u64::MAX);
        assert_eq!(i64_to_u64(u64_to_i64(0)), 0);
        assert_eq!(i64_to_u64(u64_to_i64(1 << 63)), 1 << 63);
    }
}

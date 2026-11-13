//! [`PlaylistItem`]s as stored by playlists and by favourites and statistics.
//!
//! Library tracks and files are stored by absolute path, which is also their
//! key: a library entry is joined to `tracks` by path when read, so it finds
//! its track again after the file was removed and scanned back in under a new
//! id. Plugin items are keyed by (plugin, id).

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Row};

use crate::util::clean_path;
use crate::{LibraryError, PlaylistItem, Track, TrackId};

/// Stored `kind` of a library track.
pub(crate) const LIBRARY: i64 = 0;
/// Stored `kind` of a file.
pub(crate) const FILE: i64 = 1;
/// Stored `kind` of a plugin item.
pub(crate) const SOURCE: i64 = 2;

/// An item ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredItem {
    pub kind: i64,
    /// Library tracks and files.
    pub path: Option<String>,
    /// Library tracks: the id the track has now.
    pub track_id: Option<TrackId>,
    pub plugin: Option<String>,
    pub source_id: Option<String>,
    pub meta_json: Option<String>,
}

impl StoredItem {
    pub(crate) fn library(path: String, track_id: TrackId) -> Self {
        Self {
            kind: LIBRARY,
            path: Some(path),
            track_id: Some(track_id),
            plugin: None,
            source_id: None,
            meta_json: None,
        }
    }

    pub(crate) fn file(path: String) -> Self {
        Self {
            kind: FILE,
            path: Some(path),
            track_id: None,
            plugin: None,
            source_id: None,
            meta_json: None,
        }
    }

    /// The stable key: the path of a track or file, `plugin:id` for a plugin item.
    pub(crate) fn key(&self) -> String {
        match (&self.path, &self.plugin, &self.source_id) {
            (Some(path), _, _) => path.clone(),
            (None, Some(plugin), Some(id)) => format!("{plugin}:{id}"),
            _ => String::new(),
        }
    }
}

/// Resolves `item` for storage. A library item's track must exist.
pub(crate) fn resolve(conn: &Connection, item: &PlaylistItem) -> Result<StoredItem, LibraryError> {
    Ok(match item {
        PlaylistItem::Library(id) => {
            let path: String = conn
                .prepare_cached("SELECT path FROM tracks WHERE id = ?1")?
                .query_row([id], |row| row.get(0))
                .optional()?
                .ok_or(LibraryError::TrackNotFound(*id))?;
            StoredItem::library(path, *id)
        }
        PlaylistItem::File(path) => StoredItem::file(file_key(path)?),
        PlaylistItem::Source {
            plugin,
            id,
            meta_json,
        } => StoredItem {
            kind: SOURCE,
            path: None,
            track_id: None,
            plugin: Some(plugin.clone()),
            source_id: Some(id.clone()),
            meta_json: Some(meta_json.clone()),
        },
    })
}

/// A file's key: its absolute path with `.` and `..` resolved, as text.
pub(crate) fn file_key(path: &Path) -> Result<String, LibraryError> {
    clean_path(path)?
        .into_os_string()
        .into_string()
        .map_err(|_| LibraryError::NonUtf8Path(path.to_path_buf()))
}

/// The stored item columns of table alias `x`, as [`item_at`] reads them.
pub(crate) const ITEM_COLUMNS: &str = "x.kind, x.path, x.track_id, x.plugin, x.source_id, x.meta_json";
/// How many columns [`ITEM_COLUMNS`] has.
pub(crate) const ITEM_COLUMN_COUNT: usize = 6;

/// The item at column `base`. A library item takes the id of `track`, the
/// track its path joins to now. Without one (the file left the catalogue) it
/// reads as the file at its path: the id it was stored with may belong to
/// another track by now, and the path keeps its favourite and counts.
pub(crate) fn item_at(row: &Row<'_>, base: usize, track: Option<&Track>) -> rusqlite::Result<PlaylistItem> {
    let kind: i64 = row.get(base)?;
    Ok(match kind {
        LIBRARY => match track {
            Some(track) => PlaylistItem::Library(track.id),
            None => PlaylistItem::File(PathBuf::from(row.get::<_, String>(base + 1)?)),
        },
        FILE => PlaylistItem::File(PathBuf::from(row.get::<_, String>(base + 1)?)),
        _ => PlaylistItem::Source {
            plugin: row.get(base + 3)?,
            id: row.get(base + 4)?,
            meta_json: row.get::<_, Option<String>>(base + 5)?.unwrap_or_default(),
        },
    })
}

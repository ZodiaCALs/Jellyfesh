//! The library's error type.

use std::io;
use std::path::PathBuf;

use crate::TrackId;
use crate::watch::WatchErrorKind;

/// Errors returned by [`crate::Library`] and [`crate::LibraryWatcher`].
///
/// Problems with single files during a scan are not errors: they are listed in
/// [`crate::ScanReport::errors`] and the scan carries on.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LibraryError {
    /// The SQLite database failed.
    #[error("library database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// A file or directory could not be read or written.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    /// The database was written by a newer build with a schema this build does not know.
    #[error("library database has schema version {found}; this build supports up to {supported}")]
    SchemaTooNew {
        /// The `user_version` found in the database.
        found: i64,
        /// The newest schema version this build can migrate to.
        supported: i64,
    },
    /// [`crate::Library::remove_root`] was given a path that is not a library root.
    #[error("not a library root: {}", .0.display())]
    RootNotFound(PathBuf),
    /// Paths are stored as text; this one is not valid UTF-8.
    #[error("path is not valid UTF-8: {}", .0.display())]
    NonUtf8Path(PathBuf),
    /// A track's file exists but its tags could not be parsed.
    #[error("cannot read tags from {}: {message}", path.display())]
    Tags {
        /// The audio file.
        path: PathBuf,
        /// What the tag reader reported.
        message: String,
    },
    /// No track has this id.
    #[error("no track with id {0}")]
    TrackNotFound(TrackId),
    /// No playlist has this id.
    #[error("no playlist with id {0}")]
    PlaylistNotFound(i64),
    /// The playlist has no entry with this id.
    #[error("playlist {playlist_id} has no entry {entry_id}")]
    PlaylistEntryNotFound {
        /// The playlist.
        playlist_id: i64,
        /// The entry that is not in it.
        entry_id: i64,
    },
    /// A playlist name must have more than white space.
    #[error("a playlist needs a name")]
    EmptyPlaylistName,
    /// A cached cover could not be decoded, or its thumbnail encoded.
    #[error("cannot make a thumbnail of artwork {hash}: {message}")]
    Artwork {
        /// The artwork's blake3 hex.
        hash: String,
        /// What the image codec reported.
        message: String,
    },
    /// Folder watching could not start or follow a folder.
    #[error("cannot watch {}: {message}", path.as_ref().map_or("folders".into(), |p| p.display().to_string()))]
    Watch {
        /// What kind of failure, for a translated message in the UI.
        kind: WatchErrorKind,
        /// The folder concerned, when the failure is about one folder.
        path: Option<PathBuf>,
        /// An English explanation, with a remedy where there is one.
        message: String,
    },
}

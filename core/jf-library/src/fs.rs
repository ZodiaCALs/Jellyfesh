//! The filesystem behind the library.
//!
//! Desktop builds use [`StdFileSystem`]. Android implements [`FileSystem`] over the
//! Storage Access Framework, so the scanner never touches `std::fs` for music files.
//! (The artwork cache is app-private storage and always uses `std::fs`.)

use std::fs;
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

/// A seekable reader, as returned by [`FileSystem::open`].
pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek + ?Sized> ReadSeek for T {}

/// One regular file found by [`FileSystem::walk`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileEntry {
    /// Full path of the file.
    pub path: PathBuf,
    /// Size in bytes.
    pub size: u64,
    /// Modification time in nanoseconds since the Unix epoch (negative before it).
    pub mtime_ns: i64,
}

/// What [`FileSystem::stat`] found at a path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PathKind {
    /// A regular file, with its size and modification time.
    File(FileEntry),
    /// A folder.
    Dir,
    /// Anything else: a device, a socket, a pipe. Never music.
    Other,
}

/// Where the library finds music files.
pub trait FileSystem: Send {
    /// Lists every regular file below `root`, recursively.
    ///
    /// Errors for single entries are yielded as items; the walk continues after them.
    fn walk(&self, root: &Path) -> Box<dyn Iterator<Item = io::Result<FileEntry>> + '_>;

    /// Opens a file for reading.
    fn open(&self, path: &Path) -> io::Result<Box<dyn ReadSeek>>;

    /// What is at `path`, following symbolic links.
    ///
    /// Fails with [`io::ErrorKind::NotFound`] when nothing is there (or
    /// [`io::ErrorKind::NotADirectory`] when a parent is a file):
    /// [`crate::Library::scan_paths`] then removes the tracks below `path`. Any
    /// other error keeps them.
    fn stat(&self, path: &Path) -> io::Result<PathKind>;
}

/// [`FileSystem`] over `std::fs`, walking directories with `walkdir`.
///
/// Entries come in file-name order, so scans are deterministic. Symbolic links
/// are followed (walkdir detects loops). Hidden entries, whose
/// names start with a dot, are skipped: they are mostly `.Trash`, `.AppleDouble`
/// and macOS `._song.mp3` resource forks that look like audio but are not.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn walk(&self, root: &Path) -> Box<dyn Iterator<Item = io::Result<FileEntry>> + '_> {
        let iter = walkdir::WalkDir::new(root)
            .follow_links(true)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| entry.depth() == 0 || !is_hidden(entry.file_name()))
            .filter_map(|item| match item {
                Ok(entry) if entry.file_type().is_file() => Some(match entry.metadata() {
                    Ok(meta) => Ok(FileEntry {
                        size: meta.len(),
                        mtime_ns: mtime_ns(&meta),
                        path: entry.into_path(),
                    }),
                    Err(err) => Err(walk_error(err)),
                }),
                Ok(_) => None,
                Err(err) => Some(Err(walk_error(err))),
            });
        Box::new(iter)
    }

    fn open(&self, path: &Path) -> io::Result<Box<dyn ReadSeek>> {
        Ok(Box::new(fs::File::open(path)?))
    }

    fn stat(&self, path: &Path) -> io::Result<PathKind> {
        let meta = fs::metadata(path)?;
        Ok(if meta.is_file() {
            PathKind::File(FileEntry {
                path: path.to_path_buf(),
                size: meta.len(),
                mtime_ns: mtime_ns(&meta),
            })
        } else if meta.is_dir() {
            PathKind::Dir
        } else {
            PathKind::Other
        })
    }
}

/// Whether a file or folder name is hidden (starts with a dot). The scanner
/// never catalogues anything below a hidden name.
pub(crate) fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

/// Keeps the failing path in the message; `io::Error` has no path field.
fn walk_error(err: walkdir::Error) -> io::Error {
    let kind = err.io_error().map_or(io::ErrorKind::Other, io::Error::kind);
    match err.path() {
        Some(path) => io::Error::new(kind, format!("{}: {err}", path.display())),
        None => io::Error::new(kind, err.to_string()),
    }
}

/// Modification time of `meta` in nanoseconds since the Unix epoch.
pub(crate) fn mtime_ns(meta: &fs::Metadata) -> i64 {
    match meta.modified() {
        Ok(time) => match time.duration_since(UNIX_EPOCH) {
            Ok(after) => i64::try_from(after.as_nanos()).unwrap_or(i64::MAX),
            Err(before) => i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, |n| -n),
        },
        Err(_) => 0,
    }
}

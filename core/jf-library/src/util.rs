//! Small helpers shared by the modules: the clock, paths and atomic file writes.

use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Milliseconds since the Unix epoch.
pub(crate) fn unix_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// `path` made absolute against the current directory, without `.` components
/// or a trailing separator: how library roots are stored. `..` stays.
pub(crate) fn absolute_components(path: &Path) -> io::Result<PathBuf> {
    Ok(std::path::absolute(path)?.components().collect())
}

/// `path` made absolute against the current directory, with `.` and `..`
/// resolved lexically (symbolic links are not looked at).
pub(crate) fn clean_path(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut clean = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    Ok(clean)
}

/// Writes `bytes` to `path` through a hidden temporary file in the same folder
/// and a rename, so a reader sees the old file or the new one, never part of
/// one. `durable` also flushes the data to the disk before the rename.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8], durable: bool) -> io::Result<()> {
    // Unique per process and call, so concurrent writers never share a temporary file.
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let temp = path.with_file_name(format!(
        ".{}.{}-{}.part",
        name.to_string_lossy(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
        file.write_all(bytes)?;
        if durable {
            file.sync_all()?;
        }
        drop(file);
        fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn paths_are_cleaned_lexically() {
        let clean = clean_path(Path::new("/music/./a/../b/song.flac")).unwrap();
        assert_eq!(clean, PathBuf::from("/music/b/song.flac"));
        assert_eq!(clean_path(Path::new("/../x")).unwrap(), PathBuf::from("/x"));
        assert!(clean_path(Path::new("relative")).unwrap().is_absolute());
    }

    #[test]
    fn atomic_writes_replace_and_leave_no_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.txt");
        write_atomic(&path, b"first", false).unwrap();
        write_atomic(&path, b"second", true).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(write_atomic(&dir.path().join("missing/out.txt"), b"x", false).is_err());
    }
}

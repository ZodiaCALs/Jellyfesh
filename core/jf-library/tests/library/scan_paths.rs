//! `Library::scan_paths`: rescanning given files and folders only.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use jf_library::{FileEntry, FileSystem, Library, PathKind, ReadSeek, ScanReport, StdFileSystem};
use lofty::tag::Accessor;

use crate::fixtures::{self, Fixture, standard};

/// What the scanner asked the file system for.
#[derive(Debug, Clone, Default)]
struct Calls {
    walks: Arc<Mutex<Vec<PathBuf>>>,
    stats: Arc<Mutex<Vec<PathBuf>>>,
}

impl Calls {
    fn take(&self) -> (Vec<PathBuf>, Vec<PathBuf>) {
        (
            std::mem::take(&mut *self.walks.lock().unwrap()),
            std::mem::take(&mut *self.stats.lock().unwrap()),
        )
    }
}

/// [`StdFileSystem`] that records walks and stats.
struct Recording {
    inner: StdFileSystem,
    calls: Calls,
}

impl FileSystem for Recording {
    fn walk(&self, root: &Path) -> Box<dyn Iterator<Item = io::Result<FileEntry>> + '_> {
        self.calls.walks.lock().unwrap().push(root.to_path_buf());
        self.inner.walk(root)
    }

    fn open(&self, path: &Path) -> io::Result<Box<dyn ReadSeek>> {
        self.inner.open(path)
    }

    fn stat(&self, path: &Path) -> io::Result<PathKind> {
        self.calls.stats.lock().unwrap().push(path.to_path_buf());
        self.inner.stat(path)
    }
}

fn recording_fixture() -> (Fixture, Calls) {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    fs::create_dir_all(&music).unwrap();
    let calls = Calls::default();
    let fs = Recording {
        inner: StdFileSystem,
        calls: calls.clone(),
    };
    let lib = Library::open_with_fs(
        &dir.path().join("db").join("library.db"),
        &dir.path().join("cache"),
        Box::new(fs),
    )
    .unwrap();
    lib.add_root(&music).unwrap();
    (Fixture { dir, music, lib }, calls)
}

fn scan_paths(fx: &Fixture, paths: &[PathBuf]) -> ScanReport {
    fx.lib.scan_paths(paths, &mut |_| {}).unwrap()
}

fn counts(report: &ScanReport) -> (u64, u64, u64, u64) {
    (report.added, report.updated, report.removed, report.unchanged)
}

#[test]
fn only_the_given_paths_are_read_and_the_rest_is_left_alone() {
    let (fx, calls) = recording_fixture();
    standard::build(&fx);
    fx.scan();
    let before = fx.all_tracks();
    let revision = fx.lib.revision().unwrap();

    // Three changes; only two are passed.
    fixtures::tag(&fx.path(standard::COVER_B), |t| {
        t.set_title("Blue Moon (live)".to_owned())
    });
    fixtures::touch(&fx.path(standard::COVER_B), 60);
    fs::remove_file(fx.path(standard::CJK)).unwrap();
    for name in ["1", "2"] {
        let path = fx.path(&format!("new/album/{name}.wav"));
        fixtures::wav(&path, 100);
        fixtures::tag(&path, |t| fixtures::basic(t, name, "Newcomer", "Debut", 1));
    }
    fixtures::wav(&fx.path("elsewhere/unseen.wav"), 100);
    calls.take();

    let mut progress = Vec::new();
    let report = fx
        .lib
        .scan_paths(&[fx.path(standard::COVER_B), fx.path("new")], &mut |p| {
            progress.push(p.current)
        })
        .unwrap();
    assert_eq!(counts(&report), (2, 1, 0, 0), "{report:?}");
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(progress.len(), 3);
    let (walks, stats) = calls.take();
    assert_eq!(walks, [fx.path("new")], "the roots are not walked");
    assert_eq!(stats, [fx.path(standard::COVER_B), fx.path("new")]);
    assert!(fx.lib.revision().unwrap() > revision);

    let after = fx.all_tracks();
    for track in &before {
        let now = after.iter().find(|t| t.path == track.path);
        if track.path == fx.path(standard::COVER_B) {
            let now = now.unwrap();
            assert_eq!((now.id, now.title.as_str()), (track.id, "Blue Moon (live)"));
        } else {
            assert_eq!(now, Some(track), "untouched: {}", track.path.display());
        }
    }
    assert!(
        after.iter().any(|t| t.path == fx.path(standard::CJK)),
        "a deleted file that was not passed keeps its track"
    );
    assert!(!after.iter().any(|t| t.path == fx.path("elsewhere/unseen.wav")));
    assert_eq!(after.len(), before.len() + 2);

    // Its own turn removes the deleted file; an unchanged file changes nothing.
    assert_eq!(counts(&scan_paths(&fx, &[fx.path(standard::CJK)])), (0, 0, 1, 0));
    let revision = fx.lib.revision().unwrap();
    let report = scan_paths(&fx, &[fx.path(standard::COVER_A), fx.path(standard::COVER_A)]);
    assert_eq!(counts(&report), (0, 0, 0, 1));
    assert_eq!(scan_paths(&fx, &[]), ScanReport::default());
    assert_eq!(fx.lib.revision().unwrap(), revision);
    fx.assert_search_index_consistent();
}

#[test]
fn folders_renames_and_unreadable_files() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let stats = fx.lib.stats().unwrap();

    // A deleted folder takes its tracks, album, artist and artwork with it.
    fs::remove_dir_all(fx.path("Artist A")).unwrap();
    let report = scan_paths(&fx, &[fx.path("Artist A"), fx.path("Artist A/Album One")]);
    assert_eq!(counts(&report), (0, 0, 2, 0));
    let after = fx.lib.stats().unwrap();
    assert_eq!(
        (after.tracks, after.albums, after.artists, after.artwork),
        (stats.tracks - 2, stats.albums - 1, stats.artists - 1, 0)
    );

    // A rename is the old path gone and the new one added.
    let old = fx.track_at(standard::RG);
    fs::rename(fx.path(standard::RG), fx.path("wav/renamed.wav")).unwrap();
    let report = scan_paths(&fx, &[fx.path(standard::RG), fx.path("wav/renamed.wav")]);
    assert_eq!(counts(&report), (1, 0, 1, 0));
    assert_eq!(fx.track_at("wav/renamed.wav").title, old.title);
    assert!(!fx.all_tracks().iter().any(|t| t.path == old.path));

    // A known file that stops parsing keeps its track and is reported.
    let arabic = fx.track_at(standard::ARABIC);
    fs::write(fx.path(standard::ARABIC), b"no longer audio").unwrap();
    let report = scan_paths(&fx, &[fx.path(standard::ARABIC)]);
    assert_eq!(counts(&report), (0, 0, 0, 0));
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(fx.track_at(standard::ARABIC), arabic);

    // Hidden paths and other files are ignored, like in the full scan.
    fixtures::wav(&fx.path(".Trash/old.wav"), 50);
    fixtures::wav(&fx.path("dir/.hidden/inner.wav"), 50);
    let report = scan_paths(
        &fx,
        &[fx.path(".Trash/old.wav"), fx.path("dir"), fx.path("notes.txt")],
    );
    assert_eq!(report, ScanReport::default());
    fx.assert_search_index_consistent();
}

#[test]
fn roots_paths_outside_them_and_missing_roots() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let tracks = fx.lib.stats().unwrap().tracks;

    // A path above the root stands for the root; one outside is reported.
    fixtures::wav(&fx.path("new.wav"), 50);
    let outside = fx.dir.path().join("outside.wav");
    fixtures::wav(&outside, 50);
    let report = scan_paths(&fx, &[fx.dir.path().to_path_buf(), outside.clone()]);
    assert_eq!(counts(&report).0, 1, "{report:?}");
    assert_eq!(report.unchanged, standard::GOOD_FILES);
    assert!(
        report
            .errors
            .iter()
            .any(|(path, reason)| *path == outside && reason.contains("not in a library folder")),
        "{:?}",
        report.errors
    );
    assert_eq!(fx.lib.stats().unwrap().tracks, tracks + 1);

    // An unmounted drive: its paths look deleted, but the root is gone too.
    let away = fx.dir.path().join("unmounted");
    fs::rename(&fx.music, &away).unwrap();
    let report = scan_paths(&fx, &[fx.path(standard::COVER_A), fx.music.clone()]);
    assert_eq!(counts(&report), (0, 0, 0, 0));
    assert!(
        report.errors.iter().any(|(path, _)| *path == fx.music),
        "{:?}",
        report.errors
    );
    assert_eq!(fx.lib.stats().unwrap().tracks, tracks + 1, "nothing removed");
    fs::rename(&away, &fx.music).unwrap();

    // A file under a nested root belongs to it.
    let inner = fx.path("cjk");
    fx.lib.add_root(&inner).unwrap();
    fixtures::wav(&inner.join("inner.wav"), 50);
    assert_eq!(scan_paths(&fx, &[inner.join("inner.wav")]).added, 1);
    fx.lib.remove_root(&inner).unwrap();
    assert!(!fx.all_tracks().iter().any(|t| t.path == inner.join("inner.wav")));

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let odd = fx.music.join(std::ffi::OsStr::from_bytes(b"\xff.wav"));
        let report = scan_paths(&fx, std::slice::from_ref(&odd));
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].0, odd);
    }
}

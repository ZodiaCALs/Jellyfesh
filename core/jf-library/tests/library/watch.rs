//! `LibraryWatcher` end to end: real files changed in a temporary folder, the
//! watcher's reports fed to `Library::scan_paths`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use jf_library::{LibraryWatcher, WatchErrorKind};
use lofty::tag::Accessor;

use crate::fixtures::{self, Fixture, standard};

const DEBOUNCE: Duration = Duration::from_millis(150);
/// How long to wait for a report that must come.
const PATIENCE: Duration = Duration::from_secs(20);

fn watch(roots: Vec<PathBuf>) -> (LibraryWatcher, Receiver<Vec<PathBuf>>) {
    let (tx, rx) = mpsc::channel();
    let watcher = LibraryWatcher::new(roots, DEBOUNCE, move |paths| {
        let _ = tx.send(paths);
    })
    .unwrap();
    (watcher, rx)
}

/// Feeds reports to `scan_paths` until `done` holds; returns the reported paths.
fn scan_until(
    fx: &Fixture,
    rx: &Receiver<Vec<PathBuf>>,
    what: &str,
    done: impl Fn(&Fixture) -> bool,
) -> Vec<PathBuf> {
    let deadline = Instant::now() + PATIENCE;
    let mut reported = Vec::new();
    while !done(fx) {
        let left = deadline.saturating_duration_since(Instant::now());
        let paths = rx
            .recv_timeout(left)
            .unwrap_or_else(|err| panic!("{what}: no report ({err}); had {reported:?}"));
        fx.lib.scan_paths(&paths, &mut |_| {}).unwrap();
        reported.extend(paths);
    }
    reported
}

/// Whether `reported` names `file` or a folder holding it: the watcher may
/// report a folder (a new root's own creation, a rescan), which is then
/// scanned whole.
fn covers(reported: &[PathBuf], file: &Path) -> bool {
    reported.iter().any(|path| file.starts_with(path))
}

/// No report names a path under `root` for a while; reports for other roots
/// may still come (macOS and Windows can report a change again late).
fn assert_nothing_under(rx: &Receiver<Vec<PathBuf>>, root: &Path, what: &str) {
    let end = Instant::now() + DEBOUNCE * 5;
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(paths) => assert!(!paths.iter().any(|p| p.starts_with(root)), "{what}: {paths:?}"),
            Err(_) => break,
        }
    }
}

/// Lets reports of changes made before the watch began go by: FSEvents (macOS) can deliver
/// events from just before a stream starts, such as the fixture's own files.
fn settle(rx: &Receiver<Vec<PathBuf>>) {
    let end = Instant::now() + PATIENCE;
    while Instant::now() < end && rx.recv_timeout(DEBOUNCE * 5).is_ok() {}
}

fn assert_quiet(rx: &Receiver<Vec<PathBuf>>, what: &str) {
    if let Ok(paths) = rx.recv_timeout(DEBOUNCE * 5) {
        panic!("{what}: unexpected report {paths:?}");
    }
}

fn titles_under(fx: &Fixture, folder: &str) -> Vec<String> {
    let folder = fx.path(folder);
    let mut titles: Vec<String> = fx
        .all_tracks()
        .into_iter()
        .filter(|t| t.path.starts_with(&folder))
        .map(|t| t.title)
        .collect();
    titles.sort();
    titles
}

fn has_track(fx: &Fixture, relative: &str) -> bool {
    let path = fx.path(relative);
    fx.all_tracks().iter().any(|t| t.path == path)
}

fn tagged_wav(path: &Path, title: &str) {
    fixtures::wav(path, 100);
    fixtures::tag(path, |t| fixtures::basic(t, title, "Watcher", "Live Album", 1));
}

#[test]
fn created_modified_renamed_and_deleted_files_reach_the_catalogue() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let untouched = fx.track_at(standard::COVER_A);
    let (watcher, rx) = watch(vec![fx.music.clone()]);
    assert!(watcher.unwatched().is_empty());
    assert_eq!(watcher.roots(), std::slice::from_ref(&fx.music));

    // Create: a new folder with two tracks.
    tagged_wav(&fx.path("live/1.wav"), "One");
    tagged_wav(&fx.path("live/2.wav"), "Two");
    let reported = scan_until(&fx, &rx, "create", |fx| {
        titles_under(fx, "live") == ["One", "Two"]
    });
    assert!(reported.iter().all(|p| p.starts_with(&fx.music)), "{reported:?}");

    // Modify: new tags.
    fixtures::tag(&fx.path("live/1.wav"), |t| t.set_title("One (edit)".to_owned()));
    scan_until(&fx, &rx, "modify", |fx| {
        titles_under(fx, "live") == ["One (edit)", "Two"]
    });

    // Rename a file, then the folder.
    fs::rename(fx.path("live/2.wav"), fx.path("live/2 renamed.wav")).unwrap();
    scan_until(&fx, &rx, "rename", |fx| {
        has_track(fx, "live/2 renamed.wav") && !has_track(fx, "live/2.wav")
    });
    assert_eq!(fx.track_at("live/2 renamed.wav").title, "Two");
    fs::rename(fx.path("live"), fx.path("concert")).unwrap();
    scan_until(&fx, &rx, "folder rename", |fx| {
        titles_under(fx, "concert") == ["One (edit)", "Two"] && titles_under(fx, "live").is_empty()
    });

    // Changes inside the renamed folder are still seen.
    tagged_wav(&fx.path("concert/3.wav"), "Three");
    scan_until(&fx, &rx, "create in a renamed folder", |fx| {
        titles_under(fx, "concert").len() == 3
    });

    // Delete a file and a folder.
    fs::remove_file(fx.path(standard::RG)).unwrap();
    fs::remove_dir_all(fx.path("concert")).unwrap();
    scan_until(&fx, &rx, "delete", |fx| {
        !has_track(fx, standard::RG) && titles_under(fx, "concert").is_empty()
    });
    assert_eq!(fx.lib.stats().unwrap().tracks, standard::GOOD_FILES - 1);
    assert_eq!(fx.track_at(standard::COVER_A), untouched);
    fx.assert_search_index_consistent();
}

#[test]
fn reads_hidden_folders_and_other_files_are_not_reported() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let (_watcher, rx) = watch(vec![fx.music.clone()]);
    settle(&rx);

    // Reading files, as the player and the scanner do, changes nothing.
    fs::read(fx.path(standard::COVER_A)).unwrap();
    let id = fx.track_at(standard::COVER_A).id;
    fx.lib.embedded_lyrics(id).unwrap();
    assert_eq!(fx.scan().unchanged, standard::GOOD_FILES);
    // Hidden folders (here in the root and in an existing folder) and files
    // that are not audio are ignored.
    fixtures::wav(&fx.path(".Trash/old.wav"), 50);
    fixtures::wav(&fx.path("wav/.part/song.wav"), 50);
    fs::write(fx.path("notes.txt"), b"edited").unwrap();
    fs::write(fx.path("cover.jpg"), b"not an image").unwrap();
    assert_quiet(&rx, "reads, hidden folders and other files");
}

#[test]
fn each_root_reports_on_its_own_and_roots_can_change() {
    let fx = Fixture::new();
    let second = fx.dir.path().join("second");
    fs::create_dir_all(&second).unwrap();
    let later = fx.dir.path().join("later");
    let (mut watcher, rx) = watch(vec![second.clone(), fx.music.clone(), later.clone()]);
    assert_eq!(watcher.unwatched(), [(later.clone(), WatchErrorKind::NotFound)]);
    let mut roots = vec![fx.music.clone(), second.clone(), later.clone()];
    roots.sort();
    assert_eq!(watcher.roots(), roots);

    fixtures::wav(&fx.path("a.wav"), 50);
    fixtures::wav(&second.join("b.wav"), 50);
    let mut reported: Vec<PathBuf> = Vec::new();
    let deadline = Instant::now() + PATIENCE;
    while !(covers(&reported, &fx.path("a.wav")) && covers(&reported, &second.join("b.wav"))) {
        let paths = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|err| panic!("no report ({err}); had {reported:?}"));
        let in_music = paths.iter().all(|p| p.starts_with(&fx.music));
        let in_second = paths.iter().all(|p| p.starts_with(&second));
        assert!(in_music ^ in_second, "one root per report: {paths:?}");
        reported.extend(paths);
    }

    // The missing root appears; the second root is dropped. macOS may report
    // the new root's own creation (the whole folder), and macOS and Windows may
    // report the first root's files again a little later; nothing may come
    // from the dropped root.
    fs::create_dir_all(&later).unwrap();
    watcher.set_roots(vec![fx.music.clone(), later.clone()]).unwrap();
    assert!(watcher.unwatched().is_empty());
    fixtures::wav(&second.join("c.wav"), 50);
    fixtures::wav(&later.join("d.wav"), 50);
    let deadline = Instant::now() + PATIENCE;
    let mut seen: Vec<Vec<PathBuf>> = Vec::new();
    loop {
        let paths = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|err| panic!("a report for the new root ({err}); had {seen:?}"));
        assert!(
            paths
                .iter()
                .all(|p| p.starts_with(&later) || p.starts_with(&fx.music)),
            "a report from a root no longer watched: {paths:?}"
        );
        let done = covers(&paths, &later.join("d.wav"));
        seen.push(paths);
        if done {
            break;
        }
    }
    assert_nothing_under(&rx, &second, "a root no longer watched");
}

#[test]
fn dropping_the_watcher_stops_its_thread() {
    let dir = tempfile::tempdir().unwrap();
    let (watcher, rx) = watch(vec![dir.path().to_path_buf()]);
    fixtures::wav(&dir.path().join("pending.wav"), 50);
    drop(watcher);
    // The callback, and with it the sender, is gone once the thread has ended.
    let deadline = Instant::now() + PATIENCE;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(_) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => panic!("the watcher thread is still running"),
        }
    }

    let (empty, _rx) = watch(Vec::new());
    assert!(empty.roots().is_empty() && empty.unwatched().is_empty());
    assert!(empty.take_error().is_none());
}

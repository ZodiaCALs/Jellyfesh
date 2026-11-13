//! The idle CPU budget: a `LibraryWatcher` with nothing to report never wakes.
//!
//! A test binary of its own, so the threads it counts are the watcher's only.

#[cfg(target_os = "linux")]
#[test]
fn an_idle_watcher_never_wakes_up() {
    use std::collections::BTreeSet;
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    use jf_library::LibraryWatcher;

    fn threads() -> BTreeSet<String> {
        fs::read_dir("/proc/self/task")
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect()
    }

    /// Voluntary and involuntary context switches: each wake-up adds one.
    fn switches(tid: &str) -> u64 {
        fs::read_to_string(format!("/proc/self/task/{tid}/status"))
            .unwrap()
            .lines()
            .filter(|line| line.contains("ctxt_switches"))
            .map(|line| line.split_whitespace().last().unwrap().parse::<u64>().unwrap())
            .sum()
    }

    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("album")).unwrap();
    let before = threads();
    let watcher =
        LibraryWatcher::new(vec![dir.path().to_path_buf()], Duration::from_millis(50), |_| {}).unwrap();
    // A change, reported and done with, then quiet.
    fs::write(dir.path().join("album").join("song.flac"), b"not really").unwrap();
    sleep(Duration::from_millis(500));

    let ours: Vec<String> = threads().difference(&before).cloned().collect();
    let names: Vec<String> = ours
        .iter()
        .map(|tid| fs::read_to_string(format!("/proc/self/task/{tid}/comm")).unwrap())
        .collect();
    assert_eq!(ours.len(), 2, "the watcher's thread and notify's: {names:?}");
    let start: Vec<u64> = ours.iter().map(|tid| switches(tid)).collect();
    sleep(Duration::from_secs(2));
    let end: Vec<u64> = ours.iter().map(|tid| switches(tid)).collect();
    assert_eq!(start, end, "{names:?} woke while idle");
    drop(watcher);
}

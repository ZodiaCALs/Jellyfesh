//! Folder watching: [`LibraryWatcher`] reports changed paths below the library
//! roots, coalesced per root after a quiet period, for
//! [`crate::Library::scan_paths`].
//!
//! It is built on `notify`'s recommended watcher for the platform: inotify on
//! Linux and Android, FSEvents on macOS, `ReadDirectoryChangesW` on Windows.
//! Each blocks in the kernel, and so does this module's thread: it sleeps on
//! its channel while nothing is pending and wakes only for an event or the end
//! of a quiet period. Nothing polls.
//!
//! Reported: files and folders created, written, renamed (both names),
//! deleted, or with changed metadata (a touched modification time); every root
//! after the system dropped events (an inotify queue overflow); a folder that
//! appeared but could not be watched (see [`LibraryWatcher::take_error`]), so
//! it is scanned once at least.
//!
//! Not reported: files being opened or read (the scanner reads them itself),
//! anything below a hidden name (`.Trash`, `.part` downloads, `._` forks, the
//! same rule as the scan), existing files that are not audio, and paths outside
//! the roots.

use std::collections::BTreeSet;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::event::{AccessKind, AccessMode, MetadataKind, ModifyKind};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::LibraryError;
use crate::fs::is_hidden;
use crate::tags::is_audio_path;
use crate::util::absolute_components;

/// Why folder watching failed, for a translated message in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum WatchErrorKind {
    /// The system's limit on watched folders or on watchers is reached. On
    /// Linux: `fs.inotify.max_user_watches` or `fs.inotify.max_user_instances`.
    LimitReached,
    /// The system cannot report file changes (no inotify, for example).
    Unavailable,
    /// The folder does not exist.
    NotFound,
    /// The folder cannot be read.
    PermissionDenied,
    /// Anything else; the message says more.
    Other,
}

/// Paths pending for one root beyond this are replaced by the root itself.
const MAX_PENDING_PATHS: usize = 10_000;

/// Longer debounce periods are shortened to this.
const MAX_DEBOUNCE: Duration = Duration::from_secs(3600);

/// Watches the library roots and reports changed paths.
///
/// `on_change` runs on the watcher's own thread, once per root that had
/// changes, when that root has been quiet for the debounce period: a long copy
/// into one root does not hold back changes in another. The paths are
/// absolute and below that root; a folder stands for everything in it (its
/// changed descendants are left out). Hand them to
/// [`crate::Library::scan_paths`] on the library's thread; keep `on_change`
/// short, since it holds up the next report.
///
/// Dropping the watcher stops it and waits for a running `on_change` to return.
pub struct LibraryWatcher {
    /// `None` only while dropping.
    watcher: Option<RecommendedWatcher>,
    /// The roots as given (absolute, without `.` components), sorted.
    roots: Vec<PathBuf>,
    /// Outermost roots with a recursive watch. Only these are ever unwatched:
    /// unwatching a folder inside one would drop part of its watch.
    watched: Vec<PathBuf>,
    /// Outermost roots that could not be watched, with the reason.
    unwatched: Vec<(PathBuf, WatchErrorKind)>,
    tx: Sender<Msg>,
    thread: Option<JoinHandle<()>>,
    error: Arc<Mutex<Option<LibraryError>>>,
}

impl std::fmt::Debug for LibraryWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryWatcher")
            .field("roots", &self.roots)
            .field("unwatched", &self.unwatched)
            .finish_non_exhaustive()
    }
}

enum Msg {
    Event(notify::Result<Event>),
    Roots(Vec<RootPaths>),
    Stop,
}

impl LibraryWatcher {
    /// Starts watching `roots`, recursively, reporting changes to `on_change`
    /// after `debounce` (at most an hour) without further changes in a root.
    ///
    /// Fails, with [`LibraryError::Watch`], when the system cannot watch at
    /// all ([`WatchErrorKind::Unavailable`]) or its limits are reached
    /// ([`WatchErrorKind::LimitReached`]). A root that is missing or unreadable
    /// is not an error: it is listed by [`LibraryWatcher::unwatched`] and tried
    /// again by [`LibraryWatcher::set_roots`].
    pub fn new(
        roots: Vec<PathBuf>,
        debounce: Duration,
        on_change: impl Fn(Vec<PathBuf>) + Send + 'static,
    ) -> Result<Self, LibraryError> {
        let (tx, rx) = mpsc::channel();
        let error = Arc::new(Mutex::new(None));
        let events = tx.clone();
        let watcher = guard(|| {
            RecommendedWatcher::new(
                move |event| {
                    let _ = events.send(Msg::Event(event));
                },
                notify::Config::default(),
            )
        })
        .map_err(|err| watch_error(&err, None))?;
        let thread = {
            let error = Arc::clone(&error);
            let debounce = debounce.min(MAX_DEBOUNCE);
            thread::Builder::new()
                .name("jf-library-watch".to_owned())
                .spawn(move || run(&rx, debounce, &on_change, &error))?
        };
        let mut this = Self {
            watcher: Some(watcher),
            roots: Vec::new(),
            watched: Vec::new(),
            unwatched: Vec::new(),
            tx,
            thread: Some(thread),
            error,
        };
        this.set_roots(roots)?;
        Ok(this)
    }

    /// Replaces the watched roots and re-establishes every watch, so calling it
    /// again with the same roots also recovers a root whose folder came back
    /// (a drive mounted again).
    ///
    /// Every root that can be watched is watched. Fails like
    /// [`LibraryWatcher::new`] when a limit is reached or watching is
    /// unavailable; the roots that could be watched stay watched.
    pub fn set_roots(&mut self, roots: Vec<PathBuf>) -> Result<(), LibraryError> {
        let mut wanted: Vec<PathBuf> = Vec::new();
        let mut unwatched = Vec::new();
        for root in roots {
            match absolute_components(&root) {
                Ok(clean) => wanted.push(clean),
                Err(err) => {
                    tracing::warn!(root = %root.display(), %err, "cannot watch library folder");
                    unwatched.push((root, WatchErrorKind::Other));
                }
            }
        }
        wanted.sort();
        wanted.dedup();
        // Nested roots are covered by the recursive watch of the outer root.
        let outermost: Vec<PathBuf> = wanted
            .iter()
            .filter(|root| {
                !wanted
                    .iter()
                    .any(|other| other != *root && root.starts_with(other))
            })
            .cloned()
            .collect();

        let Some(watcher) = self.watcher.as_mut() else {
            return Ok(());
        };
        // Before the new watches, so their first events find the roots known.
        let _ = self.tx.send(Msg::Roots(
            wanted.iter().map(|root| RootPaths::new(root)).collect(),
        ));
        for old in self.watched.drain(..) {
            let _ = guard(|| watcher.unwatch(&old));
        }
        let mut fatal = None;
        for root in outermost {
            match guard(|| watcher.watch(&root, RecursiveMode::Recursive)) {
                Ok(()) => self.watched.push(root),
                Err(err) => {
                    // A recursive watch that failed part way keeps the folders it reached.
                    let _ = guard(|| watcher.unwatch(&root));
                    let error = watch_error(&err, Some(&root));
                    tracing::warn!(%error, "library folder is not watched");
                    if let LibraryError::Watch { kind, .. } = &error {
                        unwatched.push((root, *kind));
                        if matches!(kind, WatchErrorKind::LimitReached | WatchErrorKind::Unavailable) {
                            fatal.get_or_insert(error);
                        }
                    }
                }
            }
        }
        self.roots = wanted;
        self.unwatched = unwatched;
        fatal.map_or(Ok(()), Err)
    }

    /// The roots, as given to [`LibraryWatcher::set_roots`] (absolute), sorted.
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Roots that are not watched, with the reason: a missing or unreadable
    /// folder, or a limit reached. [`LibraryWatcher::set_roots`] tries them again.
    pub fn unwatched(&self) -> &[(PathBuf, WatchErrorKind)] {
        &self.unwatched
    }

    /// The first problem since the last call that happened while watching,
    /// after `new` or `set_roots` returned: a folder created later that could
    /// not be watched because the limit was reached, for example. Its folder
    /// was still reported to `on_change`, but later changes in it are not.
    pub fn take_error(&self) -> Option<LibraryError> {
        self.error.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

impl Drop for LibraryWatcher {
    fn drop(&mut self) {
        if let Some(watcher) = self.watcher.take() {
            // Stops notify's thread, which drops its sender; see `guard`.
            let _ = catch_unwind(AssertUnwindSafe(move || drop(watcher)));
        }
        let _ = self.tx.send(Msg::Stop);
        if let Some(thread) = self.thread.take()
            && thread.thread().id() != thread::current().id()
        {
            let _ = thread.join();
        }
    }
}

/// notify unwraps internally if its own thread has died; such a panic must not
/// reach the caller.
fn guard<T>(call: impl FnOnce() -> notify::Result<T>) -> notify::Result<T> {
    catch_unwind(AssertUnwindSafe(call))
        .unwrap_or_else(|_| Err(notify::Error::generic("the system file watcher stopped")))
}

const LIMIT_WATCHES: &str =
    "the system limit on watched folders is reached (on Linux, raise fs.inotify.max_user_watches)";
// Only Linux and Android report this limit apart (see classify_io).
#[cfg(any(target_os = "linux", target_os = "android"))]
const LIMIT_INSTANCES: &str =
    "too many file watchers are open (on Linux, raise fs.inotify.max_user_instances)";

/// A [`LibraryError::Watch`] for `err`, about its own path or else `path`.
pub(crate) fn watch_error(err: &notify::Error, path: Option<&Path>) -> LibraryError {
    let path = err.paths.first().cloned().or_else(|| path.map(Path::to_path_buf));
    let (kind, message) = match &err.kind {
        notify::ErrorKind::MaxFilesWatch => (WatchErrorKind::LimitReached, LIMIT_WATCHES.to_owned()),
        notify::ErrorKind::PathNotFound => (WatchErrorKind::NotFound, "the folder does not exist".to_owned()),
        notify::ErrorKind::Io(io) => classify_io(io),
        // notify's Windows backend reports a folder that does not exist as a generic error.
        notify::ErrorKind::Generic(_) if path.as_deref().is_some_and(is_missing) => {
            (WatchErrorKind::NotFound, "the folder does not exist".to_owned())
        }
        notify::ErrorKind::Generic(message) => (WatchErrorKind::Other, message.clone()),
        _ => (WatchErrorKind::Other, err.to_string()),
    };
    LibraryError::Watch { kind, path, message }
}

/// Whether nothing is at `path` (a dangling link counts as nothing).
fn is_missing(path: &Path) -> bool {
    matches!(std::fs::metadata(path), Err(err) if err.kind() == io::ErrorKind::NotFound)
}

fn classify_io(err: &io::Error) -> (WatchErrorKind, String) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // From asm-generic/errno-base.h, the same on every Linux architecture.
        const ENFILE: i32 = 23;
        const EMFILE: i32 = 24;
        const ENOSPC: i32 = 28;
        match err.raw_os_error() {
            Some(EMFILE) => return (WatchErrorKind::LimitReached, LIMIT_INSTANCES.to_owned()),
            Some(ENFILE) => {
                return (
                    WatchErrorKind::LimitReached,
                    "the system has too many open files".to_owned(),
                );
            }
            Some(ENOSPC) => return (WatchErrorKind::LimitReached, LIMIT_WATCHES.to_owned()),
            _ => {}
        }
    }
    match err.kind() {
        io::ErrorKind::NotFound => (WatchErrorKind::NotFound, "the folder does not exist".to_owned()),
        io::ErrorKind::PermissionDenied => (WatchErrorKind::PermissionDenied, "permission denied".to_owned()),
        io::ErrorKind::Unsupported => (
            WatchErrorKind::Unavailable,
            "this system cannot watch folders for changes".to_owned(),
        ),
        _ => (WatchErrorKind::Other, err.to_string()),
    }
}

/// A root, and the real path the system may report for it (FSEvents reports
/// `/private/var/…` for `/var/…`, for example).
#[derive(Debug, Clone, PartialEq, Eq)]
struct RootPaths {
    root: PathBuf,
    canonical: Option<PathBuf>,
}

impl RootPaths {
    fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            canonical: std::fs::canonicalize(root).ok().filter(|real| real != root),
        }
    }

    /// `path` below this root, spelled with the root as given.
    fn translate(&self, path: &Path) -> Option<PathBuf> {
        if path.starts_with(&self.root) {
            return Some(path.to_path_buf());
        }
        let below = path.strip_prefix(self.canonical.as_ref()?).ok()?;
        Some(self.root.join(below))
    }
}

/// Paths changed in one root since its last report.
#[derive(Debug)]
struct Pending {
    root: PathBuf,
    paths: BTreeSet<PathBuf>,
    deadline: Instant,
}

/// The state of the watcher thread.
#[derive(Debug)]
struct Debouncer {
    debounce: Duration,
    roots: Vec<RootPaths>,
    pending: Vec<Pending>,
}

impl Debouncer {
    fn next_deadline(&self) -> Option<Instant> {
        self.pending.iter().map(|p| p.deadline).min()
    }

    fn set_roots(&mut self, roots: Vec<RootPaths>) {
        self.pending
            .retain(|p| roots.iter().any(|root| root.root == p.root));
        self.roots = roots;
    }

    /// The deepest root containing `path`, and `path` spelled under it; `None`
    /// outside the roots and below a hidden name.
    fn locate(&self, path: &Path) -> Option<(PathBuf, PathBuf)> {
        let (root, path) = self
            .roots
            .iter()
            .filter_map(|root| Some((&root.root, root.translate(path)?)))
            .max_by_key(|(root, _)| root.components().count())?;
        let below = path.strip_prefix(root).ok()?;
        let hidden = below
            .components()
            .any(|c| matches!(c, Component::Normal(name) if is_hidden(name)));
        (!hidden).then(|| (root.clone(), path))
    }

    fn event(&mut self, event: &Event, now: Instant) {
        if event.need_rescan() {
            let roots: Vec<PathBuf> = self.roots.iter().map(|root| root.root.clone()).collect();
            for root in roots {
                self.mark(&root.clone(), root, now);
            }
        }
        if !is_change(&event.kind) {
            return;
        }
        for path in &event.paths {
            if let Some((root, path)) = self.locate(path) {
                self.mark(&root, path, now);
            }
        }
    }

    /// Paths of a watching error are scanned, so their content is caught up at least once.
    fn error(&mut self, err: &notify::Error, now: Instant) {
        for path in &err.paths {
            if let Some((root, path)) = self.locate(path) {
                self.mark(&root, path, now);
            }
        }
    }

    fn mark(&mut self, root: &Path, path: PathBuf, now: Instant) {
        let deadline = now + self.debounce;
        let index = match self.pending.iter().position(|p| p.root == root) {
            Some(index) => index,
            None => {
                self.pending.push(Pending {
                    root: root.to_path_buf(),
                    paths: BTreeSet::new(),
                    deadline,
                });
                self.pending.len() - 1
            }
        };
        let pending = &mut self.pending[index];
        pending.deadline = deadline;
        // A pending folder already covers everything below it.
        let covered = path
            .ancestors()
            .skip(1)
            .take_while(|ancestor| ancestor.starts_with(root))
            .any(|ancestor| pending.paths.contains(ancestor));
        if covered {
            return;
        }
        pending.paths.insert(path);
        if pending.paths.len() > MAX_PENDING_PATHS {
            pending.paths = BTreeSet::from([root.to_path_buf()]);
        }
    }

    /// Reports every root whose quiet period is over.
    fn flush(&mut self, now: Instant, on_change: &dyn Fn(Vec<PathBuf>)) {
        let (due, waiting): (Vec<Pending>, Vec<Pending>) =
            self.pending.drain(..).partition(|p| p.deadline <= now);
        self.pending = waiting;
        for pending in due {
            let paths = coalesce(pending.paths);
            if paths.is_empty() {
                continue;
            }
            if catch_unwind(AssertUnwindSafe(|| on_change(paths))).is_err() {
                tracing::error!(root = %pending.root.display(), "the library watcher's on_change panicked");
            }
        }
    }
}

/// Whether an event kind can change what the catalogue holds. Opening and
/// reading are not changes; closing after writing is.
fn is_change(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime)) => false,
        _ => true,
    }
}

/// Paths in order, without those below another path in the set, and without
/// existing files that are not audio (folders and removed paths stay).
fn coalesce(paths: BTreeSet<PathBuf>) -> Vec<PathBuf> {
    // Paths sort by component, so a folder's descendants follow it directly.
    let mut out: Vec<PathBuf> = Vec::new();
    for path in paths {
        if out.last().is_some_and(|folder| path.starts_with(folder)) {
            continue;
        }
        out.push(path);
    }
    out.retain(|path| match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => is_audio_path(path),
        _ => true,
    });
    out
}

fn run(
    rx: &Receiver<Msg>,
    debounce: Duration,
    on_change: &dyn Fn(Vec<PathBuf>),
    error: &Mutex<Option<LibraryError>>,
) {
    let mut state = Debouncer {
        debounce,
        roots: Vec::new(),
        pending: Vec::new(),
    };
    loop {
        // Sleep until a message arrives, or until the next quiet period ends.
        let received = match state.next_deadline() {
            None => rx.recv().ok(),
            Some(deadline) => match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(msg) => Some(msg),
                Err(RecvTimeoutError::Timeout) => {
                    state.flush(Instant::now(), on_change);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => None,
            },
        };
        match received {
            None | Some(Msg::Stop) => return,
            Some(Msg::Roots(roots)) => state.set_roots(roots),
            Some(Msg::Event(Ok(event))) => state.event(&event, Instant::now()),
            Some(Msg::Event(Err(err))) => {
                let reported = watch_error(&err, None);
                tracing::warn!(error = %reported, "library folder watching");
                error
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get_or_insert(reported);
                state.error(&err, Instant::now());
            }
        }
        // Events for a busy root must not hold back a root that went quiet.
        state.flush(Instant::now(), on_change);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, Flag, RemoveKind, RenameMode};

    fn debouncer(roots: &[&str]) -> Debouncer {
        Debouncer {
            debounce: Duration::from_millis(100),
            roots: roots
                .iter()
                .map(|root| RootPaths {
                    root: PathBuf::from(root),
                    canonical: None,
                })
                .collect(),
            pending: Vec::new(),
        }
    }

    fn paths(d: &Debouncer, root: &str) -> Vec<PathBuf> {
        d.pending
            .iter()
            .find(|p| p.root == Path::new(root))
            .map(|p| p.paths.iter().cloned().collect())
            .unwrap_or_default()
    }

    #[test]
    fn events_are_grouped_by_the_deepest_root() {
        let mut d = debouncer(&["/m", "/m/inner", "/n"]);
        let now = Instant::now();
        let create =
            |path: &str| Event::new(EventKind::Create(CreateKind::File)).add_path(PathBuf::from(path));
        d.event(&create("/m/a.flac"), now);
        d.event(&create("/m/inner/b.flac"), now);
        d.event(&create("/n/c.flac"), now);
        d.event(&create("/elsewhere/d.flac"), now);
        d.event(&create("/m/.Trash/e.flac"), now);
        d.event(&create("/m/x/._f.flac"), now);
        assert_eq!(paths(&d, "/m"), [PathBuf::from("/m/a.flac")]);
        assert_eq!(paths(&d, "/m/inner"), [PathBuf::from("/m/inner/b.flac")]);
        assert_eq!(paths(&d, "/n"), [PathBuf::from("/n/c.flac")]);
        assert_eq!(d.pending.len(), 3);
    }

    #[test]
    fn reads_are_ignored_and_writes_and_renames_kept() {
        assert!(!is_change(&EventKind::Access(AccessKind::Open(AccessMode::Any))));
        assert!(!is_change(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(is_change(&EventKind::Access(AccessKind::Close(
            AccessMode::Write
        ))));
        assert!(is_change(&EventKind::Modify(ModifyKind::Name(RenameMode::Both))));
        assert!(is_change(&EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Any
        ))));
        assert!(!is_change(&EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::AccessTime
        ))));
        assert!(is_change(&EventKind::Remove(RemoveKind::Any)));
        assert!(is_change(&EventKind::Any));
    }

    #[test]
    fn folders_cover_their_contents_and_overflow_rescans_every_root() {
        let mut d = debouncer(&["/m", "/n"]);
        let now = Instant::now();
        let event = |path: &str| Event::new(EventKind::Any).add_path(PathBuf::from(path));
        d.event(&event("/m/album"), now);
        d.event(&event("/m/album/1.flac"), now);
        d.event(&event("/m/single.flac"), now);
        assert_eq!(
            paths(&d, "/m"),
            [PathBuf::from("/m/album"), PathBuf::from("/m/single.flac")]
        );
        let set: BTreeSet<PathBuf> = ["/m/b/1.flac", "/m/b", "/m/bc.flac", "/m/b/c/2.flac"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(
            coalesce(set),
            [PathBuf::from("/m/b"), PathBuf::from("/m/bc.flac")]
        );

        d.event(&Event::new(EventKind::Other).set_flag(Flag::Rescan), now);
        assert!(paths(&d, "/m").contains(&PathBuf::from("/m")));
        assert_eq!(paths(&d, "/n"), [PathBuf::from("/n")]);
    }

    #[test]
    fn each_root_reports_after_its_own_quiet_period() {
        let mut d = debouncer(&["/m", "/n"]);
        let start = Instant::now();
        let event = |path: &str| Event::new(EventKind::Any).add_path(PathBuf::from(path));
        d.event(&event("/m/a.flac"), start);
        d.event(&event("/n/b.flac"), start + Duration::from_millis(80));
        let reports = std::cell::RefCell::new(Vec::new());
        let record = |paths: Vec<PathBuf>| reports.borrow_mut().push(paths);

        d.flush(start + Duration::from_millis(99), &record);
        assert!(reports.borrow().is_empty());
        d.flush(start + Duration::from_millis(100), &record);
        assert_eq!(*reports.borrow(), [vec![PathBuf::from("/m/a.flac")]]);
        assert_eq!(d.next_deadline(), Some(start + Duration::from_millis(180)));
        d.flush(start + Duration::from_millis(180), &record);
        assert_eq!(reports.borrow().len(), 2);
        assert_eq!(d.next_deadline(), None);
    }

    #[test]
    fn real_paths_are_spelled_with_the_root_as_given() {
        let root = RootPaths {
            root: PathBuf::from("/var/music"),
            canonical: Some(PathBuf::from("/private/var/music")),
        };
        assert_eq!(
            root.translate(Path::new("/private/var/music/a.flac")),
            Some(PathBuf::from("/var/music/a.flac"))
        );
        assert_eq!(
            root.translate(Path::new("/var/music/a.flac")),
            Some(PathBuf::from("/var/music/a.flac"))
        );
        assert_eq!(root.translate(Path::new("/private/var/other")), None);
    }

    #[test]
    fn errors_name_their_kind_and_remedy() {
        let limit = watch_error(
            &notify::Error::new(notify::ErrorKind::MaxFilesWatch),
            Some(Path::new("/m")),
        );
        match &limit {
            LibraryError::Watch { kind, path, message } => {
                assert_eq!(*kind, WatchErrorKind::LimitReached);
                assert_eq!(path.as_deref(), Some(Path::new("/m")));
                assert!(message.contains("max_user_watches"));
            }
            other => panic!("{other:?}"),
        }
        assert!(limit.to_string().starts_with("cannot watch /m: "));

        let kind_of = |err: notify::Error| match watch_error(&err, None) {
            LibraryError::Watch { kind, .. } => kind,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            kind_of(notify::Error::path_not_found().add_path(PathBuf::from("/x"))),
            WatchErrorKind::NotFound
        );
        assert_eq!(
            kind_of(notify::Error::io(io::Error::from(
                io::ErrorKind::PermissionDenied
            ))),
            WatchErrorKind::PermissionDenied
        );
        assert_eq!(
            kind_of(notify::Error::io(io::Error::from(io::ErrorKind::Unsupported))),
            WatchErrorKind::Unavailable
        );
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            assert_eq!(
                kind_of(notify::Error::io(io::Error::from_raw_os_error(24))),
                WatchErrorKind::LimitReached
            );
            let instances = watch_error(&notify::Error::io(io::Error::from_raw_os_error(24)), None);
            assert!(
                instances.to_string().contains("max_user_instances"),
                "{instances}"
            );
            assert!(instances.to_string().starts_with("cannot watch folders: "));
        }
        assert_eq!(kind_of(notify::Error::generic("boom")), WatchErrorKind::Other);
        // As notify reports a missing folder on Windows: by what is (not) at the path.
        let dir = tempfile::tempdir().unwrap();
        let generic = |path: PathBuf| {
            notify::Error::generic("Input watch path is neither a file nor a directory.").add_path(path)
        };
        assert_eq!(
            kind_of(generic(dir.path().join("gone"))),
            WatchErrorKind::NotFound
        );
        assert_eq!(kind_of(generic(dir.path().to_path_buf())), WatchErrorKind::Other);
        match watch_error(&notify::Error::generic("boom"), Some(&dir.path().join("gone"))) {
            LibraryError::Watch { kind, .. } => assert_eq!(kind, WatchErrorKind::NotFound),
            other => panic!("{other:?}"),
        }
    }
}

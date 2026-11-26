//! Source pictures through the core (ADR-0022 §5).
//!
//! Apps show a source's pictures (list rows, Discover shelves, details,
//! comment avatars) by passing the `ArtworkRef::Url` the core gave them to
//! [`crate::Core::source_artwork`]. The core fetches each picture once, with
//! the headers its plugin gave for it (the plugin host has already checked
//! the URL against the plugin's allowlist and moved `http` to `https` where
//! the allowlist allows), makes a JPEG of about the size asked for and keeps
//! both in `<cache>/source-artwork/<key>/`. Requests for the same picture
//! are answered together, a few pictures are fetched at a time, and the
//! oldest pictures go when the folder grows past its limit.
//!
//! Only pictures a plugin gave can be fetched: the URLs are remembered, with
//! their headers, as plugin items become rows. Items listed without a
//! picture, from a plugin that answers `artwork` (Subsonic's covers, whose
//! URLs would hold the sign-in, come only that way), get a reference of the
//! core's own instead, `jellyfesh-artwork://item?plugin=…&kind=…&id=…`: the
//! core asks the plugin's `artwork` for it and fetches what that gives, with
//! its headers, into the same cache.
//!
//! Lists the library keeps (Recently Played, playlists, favourites) store a
//! source track's picture URL from an earlier run, whose headers are gone:
//! [`stored_picture`] notes whose it is, and the core fetches it through
//! the plugin's `artwork` answer for the item, else as it is when the
//! plugin's allowlist still covers it. Now Playing keeps its picture under
//! the same name, so those rows find it in the cache.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, SystemTime};

use crossbeam_channel::Sender;
use jf_plugin_host::{CallError, Header, ImageRef, ItemKind as HostKind};

use crate::actor::Msg;
use crate::sources::Ctx;
use crate::types::ArtworkReady;

/// Pictures fetched at once.
const WORKERS: usize = 4;
/// Picture URLs remembered with their headers.
const KNOWN: usize = 8192;
/// The folder is trimmed to three quarters of this when it grows past it.
const MAX_BYTES: u64 = 256 * 1024 * 1024;
/// How many pictures are stored between checks of the folder's size.
const CHECK_EVERY: u32 = 64;
/// The size of Now Playing's picture.
const NOW_PLAYING_PX: u32 = 1024;
/// How long a plugin's `artwork` may take.
const ARTWORK_WAIT: Duration = Duration::from_secs(25);
/// A picture served from the cache is marked as used again after this long,
/// so the least recently used ones go first.
const TOUCH_AFTER: Duration = Duration::from_secs(60 * 60);
/// The scheme of the core's references to an item's picture.
const ITEM_SCHEME: &str = "jellyfesh-artwork";

/// The URLs plugins gave pictures under, with the headers to send. Shared
/// by the whole process: rows are made on many threads.
static KNOWN_IMAGES: Mutex<Option<Known>> = Mutex::new(None);

/// The plugins that answer `artwork`, as their rows are made.
static COVERS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Whose item a stored picture URL is (lists the library keeps): URL →
/// (plugin, kind, id).
static OWNERS: Mutex<Option<Owners>> = Mutex::new(None);

#[derive(Debug, Default)]
struct Owners {
    items: HashMap<String, (String, HostKind, String)>,
    order: VecDeque<String>,
}

/// The picture to show for a source item a list stored (a playlist entry,
/// Recently Played): the URL it was listed with, which the core can fetch
/// in any later run through its plugin; or, without one, the core's
/// reference to the item when its plugin answers `artwork`; else none.
pub(crate) fn stored_picture(plugin: &str, kind: HostKind, id: &str, stored: Option<&str>) -> Option<String> {
    match stored.filter(|url| !url.is_empty()) {
        Some(url) => {
            if !url.starts_with(ITEM_SCHEME) {
                let mut guard = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
                let owners = guard.get_or_insert_with(Owners::default);
                if owners
                    .items
                    .insert(url.to_owned(), (plugin.to_owned(), kind, id.to_owned()))
                    .is_none()
                {
                    owners.order.push_back(url.to_owned());
                    while owners.order.len() > KNOWN {
                        if let Some(old) = owners.order.pop_front() {
                            owners.items.remove(&old);
                        }
                    }
                }
            }
            Some(url.to_owned())
        }
        None => covers(plugin).then(|| item_ref(plugin, kind, id)),
    }
}

/// Whose item a stored picture URL is.
fn owner_of(url: &str) -> Option<(String, HostKind, String)> {
    OWNERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()?
        .items
        .get(url)
        .cloned()
}

/// Notes whether a plugin answers `artwork` (the plugins as last published).
pub(crate) fn note_covers(plugin: &str, answers: bool) {
    let mut guard = COVERS.lock().unwrap_or_else(|e| e.into_inner());
    let covers = guard.get_or_insert_with(HashSet::new);
    if answers {
        covers.insert(plugin.to_owned());
    } else {
        covers.remove(plugin);
    }
}

/// Whether a plugin answers `artwork`.
pub(crate) fn covers(plugin: &str) -> bool {
    COVERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|c| c.contains(plugin))
}

fn kind_name(kind: HostKind) -> &'static str {
    match kind {
        HostKind::Track => "track",
        HostKind::Album => "album",
        HostKind::Artist => "artist",
        HostKind::Playlist => "playlist",
        HostKind::Folder => "folder",
    }
}

fn kind_of(name: &str) -> Option<HostKind> {
    Some(match name {
        "track" => HostKind::Track,
        "album" => HostKind::Album,
        "artist" => HostKind::Artist,
        "playlist" => HostKind::Playlist,
        "folder" => HostKind::Folder,
        _ => return None,
    })
}

/// The core's reference to an item's picture, which its plugin's `artwork`
/// gives.
pub(crate) fn item_ref(plugin: &str, kind: HostKind, id: &str) -> String {
    let mut url = url::Url::parse(&format!("{ITEM_SCHEME}://item")).expect("a valid base");
    url.query_pairs_mut()
        .append_pair("plugin", plugin)
        .append_pair("kind", kind_name(kind))
        .append_pair("id", id);
    url.into()
}

/// The plugin, kind and id of an item reference.
fn parse_item_ref(text: &str) -> Option<(String, HostKind, String)> {
    let url = url::Url::parse(text).ok()?;
    if url.scheme() != ITEM_SCHEME {
        return None;
    }
    let (mut plugin, mut kind, mut id) = (None, None, None);
    for (key, value) in url.query_pairs() {
        match &*key {
            "plugin" => plugin = Some(value.into_owned()),
            "kind" => kind = kind_of(&value),
            "id" => id = Some(value.into_owned()),
            _ => {}
        }
    }
    Some((plugin?, kind?, id?))
}

/// The picture for a row: the one the plugin listed, or, when it listed none
/// and answers `artwork`, the core's reference for the item.
pub(crate) fn row_picture(
    plugin: &str,
    kind: HostKind,
    id: &str,
    listed: Option<&ImageRef>,
) -> Option<String> {
    match listed {
        Some(image) => {
            remember(image);
            Some(image.url.clone())
        }
        None => covers(plugin).then(|| item_ref(plugin, kind, id)),
    }
}

#[derive(Debug, Default)]
struct Known {
    headers: HashMap<String, Vec<Header>>,
    order: VecDeque<String>,
}

fn known() -> std::sync::MutexGuard<'static, Option<Known>> {
    KNOWN_IMAGES.lock().unwrap_or_else(|e| e.into_inner())
}

/// Remembers a picture a plugin gave, so the core may fetch it.
pub(crate) fn remember(image: &ImageRef) {
    let mut guard = known();
    let known = guard.get_or_insert_with(Known::default);
    if known
        .headers
        .insert(image.url.clone(), image.headers.clone())
        .is_none()
    {
        known.order.push_back(image.url.clone());
        while known.order.len() > KNOWN {
            if let Some(old) = known.order.pop_front() {
                known.headers.remove(&old);
            }
        }
    }
}

/// The picture to fetch for `url`, when a plugin gave it.
fn image_for(url: &str) -> Option<ImageRef> {
    let guard = known();
    let headers = guard.as_ref()?.headers.get(url)?.clone();
    Some(ImageRef {
        url: url.to_owned(),
        headers,
        width: None,
        height: None,
    })
}

/// The folder for one picture.
fn folder(dir: &Path, url: &str) -> PathBuf {
    dir.join(&blake3::hash(url.as_bytes()).to_hex()[..32])
}

fn thumbnail_path(dir: &Path, url: &str, size_px: u32) -> PathBuf {
    folder(dir, url).join(format!("{}.jpg", jf_library::thumbnail_size(size_px)))
}

fn original_path(dir: &Path, url: &str) -> PathBuf {
    folder(dir, url).join("original")
}

/// Keeps a picture's bytes (fetched elsewhere, such as Now Playing's) and
/// makes its large JPEG, which it returns.
pub(crate) fn store_now_playing(dir: &Path, url: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    crate::sources::write_atomic(&original_path(dir, url), bytes).map_err(|e| e.to_string())?;
    let path = thumbnail_path(dir, url, NOW_PLAYING_PX);
    jf_library::make_thumbnail(bytes, NOW_PLAYING_PX, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The picture to fetch for an item reference: what the plugin's `artwork`
/// answers for the item (passed back whole when it was listed).
fn image_from_plugin(ctx: Option<&Ctx>, reference: &str, size_px: u32) -> Result<ImageRef, String> {
    let (plugin, kind, id) = parse_item_ref(reference).ok_or("not a picture a source gave")?;
    let ctx = ctx.ok_or("online sources are off")?;
    plugin_artwork(ctx, &plugin, kind, &id, size_px)
}

/// What the plugin's `artwork` answers for an item.
fn plugin_artwork(
    ctx: &Ctx,
    plugin: &str,
    kind: HostKind,
    id: &str,
    size_px: u32,
) -> Result<ImageRef, String> {
    let item = ctx.item(plugin, id, kind);
    let size = jf_library::thumbnail_size(size_px);
    let answer = ctx
        .host
        .artwork(plugin, item, size)
        .recv_timeout(ARTWORK_WAIT)
        .unwrap_or(Err(CallError::Timeout));
    match answer {
        Ok(Some(image)) => Ok(image),
        Ok(None) => Err("the source has no picture for it".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// The picture to fetch for a URL a list stored (from an earlier run, its
/// headers gone): the plugin's `artwork` answer for its item, else the URL
/// itself while the plugin's allowlist covers it.
fn image_for_stored(ctx: Option<&Ctx>, url: &str, size_px: u32) -> Result<ImageRef, String> {
    let (plugin, kind, id) = owner_of(url).ok_or("not a picture a source gave")?;
    let ctx = ctx.ok_or("online sources are off")?;
    if covers(&plugin)
        && let Ok(image) = plugin_artwork(ctx, &plugin, kind, &id, size_px)
    {
        return Ok(image);
    }
    ctx.host
        .checked_image(&plugin, url)
        .ok_or_else(|| format!("{url} is not on its source's allowlist"))
}

/// Fetches (or reads the kept original of) a picture and makes its JPEG.
fn make(dir: &Path, ctx: Option<&Ctx>, url: &str, size_px: u32) -> Result<PathBuf, String> {
    let path = thumbnail_path(dir, url, size_px);
    if path.is_file() {
        return Ok(path);
    }
    let original = original_path(dir, url);
    let bytes = match std::fs::read(&original) {
        Ok(bytes) => bytes,
        Err(_) => {
            let image = match image_for(url) {
                Some(image) => image,
                None if parse_item_ref(url).is_some() => image_from_plugin(ctx, url, size_px)?,
                None => image_for_stored(ctx, url, size_px)?,
            };
            let bytes = crate::sources::fetch_image(&image)?;
            crate::sources::write_atomic(&original, &bytes).map_err(|e| e.to_string())?;
            bytes
        }
    };
    jf_library::make_thumbnail(&bytes, size_px, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Marks a cached picture as used now, at most once in [`TOUCH_AFTER`].
pub(crate) fn touch(path: &Path) {
    let now = SystemTime::now();
    let stale = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .is_ok_and(|at| now.duration_since(at).is_ok_and(|age| age > TOUCH_AFTER));
    if stale && let Ok(file) = std::fs::File::options().write(true).open(path) {
        let _ = file.set_modified(now);
    }
}

/// Requests waiting for a fetcher; the newest go first (the rows on screen).
#[derive(Debug, Default)]
struct Waiting {
    jobs: Mutex<(Vec<(String, u32)>, bool)>,
    ready: Condvar,
}

impl Waiting {
    fn push(&self, job: (String, u32)) {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).0.push(job);
        self.ready.notify_one();
    }

    /// The newest request, or `None` once the cache is gone.
    fn pop(&self) -> Option<(String, u32)> {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if jobs.1 {
                return None;
            }
            if let Some(job) = jobs.0.pop() {
                return Some(job);
            }
            jobs = self.ready.wait(jobs).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn close(&self) {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).1 = true;
        self.ready.notify_all();
    }
}

/// Deletes the least recently made pictures while the folder is over its
/// limit, down to three quarters of it.
fn trim(dir: &Path, max_bytes: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut folders: Vec<(std::time::SystemTime, PathBuf, u64)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| {
            let path = e.path();
            let (mut bytes, mut newest) = (0, std::time::UNIX_EPOCH);
            for file in std::fs::read_dir(&path)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
            {
                if let Ok(meta) = file.metadata() {
                    bytes += meta.len();
                    newest = newest.max(meta.modified().unwrap_or(std::time::UNIX_EPOCH));
                }
            }
            (newest, path, bytes)
        })
        .collect();
    let mut total: u64 = folders.iter().map(|f| f.2).sum();
    if total <= max_bytes {
        return;
    }
    folders.sort();
    for (_, path, bytes) in folders {
        if total <= max_bytes / 4 * 3 {
            break;
        }
        if std::fs::remove_dir_all(&path).is_ok() {
            total = total.saturating_sub(bytes);
        }
    }
}

/// The source-picture cache and its fetchers.
#[derive(Debug)]
pub(crate) struct Artwork {
    dir: PathBuf,
    pending: Arc<Mutex<HashSet<(String, u32)>>>,
    waiting: Arc<Waiting>,
}

impl Drop for Artwork {
    fn drop(&mut self) {
        self.waiting.close();
    }
}

impl Artwork {
    /// The cache in `<cache_dir>/source-artwork`, with its fetchers, which
    /// ask `ctx`'s plugins for item pictures and tell the actor when a
    /// picture is ready.
    pub(crate) fn new(cache_dir: &Path, ctx: Option<Arc<Ctx>>, actor: Sender<Msg>) -> Artwork {
        let dir = cache_dir.join("source-artwork");
        let pending: Arc<Mutex<HashSet<(String, u32)>>> = Arc::default();
        let waiting = Arc::new(Waiting::default());
        let stored = Arc::new(AtomicU32::new(0));
        // Weak: the context holds the shared state that holds this cache.
        let ctx: Option<Weak<Ctx>> = ctx.as_ref().map(Arc::downgrade);
        for n in 0..WORKERS {
            let (waiting, dir, pending, actor, stored, ctx) = (
                waiting.clone(),
                dir.clone(),
                pending.clone(),
                actor.clone(),
                stored.clone(),
                ctx.clone(),
            );
            let spawned = std::thread::Builder::new()
                .name(format!("jf-artwork-{n}"))
                .spawn(move || {
                    while let Some((url, size_px)) = waiting.pop() {
                        let ctx = ctx.as_ref().and_then(Weak::upgrade);
                        let result = make(&dir, ctx.as_deref(), &url, size_px);
                        if let Err(error) = &result {
                            tracing::info!(%error, "a source picture could not be had");
                        }
                        pending
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(&(url.clone(), size_px));
                        let ready = ArtworkReady {
                            url,
                            size_px,
                            path: result.ok(),
                        };
                        let _ = actor.send(Msg::Artwork(ready));
                        if stored.fetch_add(1, Ordering::Relaxed) % CHECK_EVERY == CHECK_EVERY - 1 {
                            trim(&dir, MAX_BYTES);
                        }
                    }
                });
            if let Err(e) = spawned {
                tracing::warn!("could not start an artwork fetcher: {e}");
            }
        }
        Artwork {
            dir,
            pending,
            waiting,
        }
    }

    /// The folder pictures are kept in.
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// The picture's file when it is ready; otherwise starts making it (once
    /// for any number of requests, the newest requests first) and returns
    /// `None`: `Event::Artwork` follows.
    pub(crate) fn get(&self, url: &str, size_px: u32) -> Option<PathBuf> {
        let path = thumbnail_path(&self.dir, url, size_px);
        if path.is_file() {
            touch(&path);
            return Some(path);
        }
        let key = (url.to_owned(), size_px);
        if self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone())
        {
            self.waiting.push(key);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pictures_a_plugin_gave_are_known_with_their_headers() {
        let url = "https://img.example.com/known-picture.jpg";
        assert!(image_for(url).is_none());
        remember(&ImageRef {
            url: url.into(),
            headers: vec![Header {
                name: "referer".into(),
                value: "https://example.com/".into(),
            }],
            width: None,
            height: None,
        });
        let image = image_for(url).unwrap();
        assert_eq!(image.headers[0].name, "referer");
        assert!(image_for("https://img.example.com/other.jpg").is_none());
    }

    #[test]
    fn item_references_round_trip_and_follow_the_plugins() {
        let reference = item_ref("org.example.covers", HostKind::Album, "al:1/2 & 3");
        assert!(reference.starts_with("jellyfesh-artwork://item?"), "{reference}");
        assert_eq!(
            parse_item_ref(&reference),
            Some(("org.example.covers".into(), HostKind::Album, "al:1/2 & 3".into()))
        );
        assert_eq!(parse_item_ref("https://img.example.com/a.jpg"), None);
        assert_eq!(
            parse_item_ref("jellyfesh-artwork://item?plugin=x&kind=nope&id=1"),
            None
        );
        let listed = ImageRef {
            url: "https://img.example.com/listed.jpg".into(),
            headers: Vec::new(),
            width: None,
            height: None,
        };
        assert_eq!(
            row_picture("org.example.nocovers", HostKind::Track, "t1", Some(&listed)).as_deref(),
            Some("https://img.example.com/listed.jpg")
        );
        assert_eq!(
            row_picture("org.example.nocovers", HostKind::Track, "t1", None),
            None
        );
        note_covers("org.example.covers", true);
        assert!(
            row_picture("org.example.covers", HostKind::Track, "t1", None)
                .is_some_and(|r| r.starts_with("jellyfesh-artwork:"))
        );
        note_covers("org.example.covers", false);
        assert_eq!(
            row_picture("org.example.covers", HostKind::Track, "t1", None),
            None
        );
    }

    #[test]
    fn stored_pictures_name_their_item() {
        let url = "https://img.example.com/stored-picture.jpg";
        assert_eq!(
            stored_picture("org.example.nocovers", HostKind::Track, "t9", Some(url)).as_deref(),
            Some(url),
            "a stored URL stays the row's picture"
        );
        assert_eq!(
            owner_of(url),
            Some(("org.example.nocovers".into(), HostKind::Track, "t9".into()))
        );
        assert_eq!(
            stored_picture("org.example.nocovers", HostKind::Track, "t9", None),
            None,
            "nothing to show without a URL or the plugin's artwork"
        );
        note_covers("org.example.storedcovers", true);
        let reference = stored_picture("org.example.storedcovers", HostKind::Track, "t9", None).unwrap();
        assert_eq!(
            parse_item_ref(&reference),
            Some(("org.example.storedcovers".into(), HostKind::Track, "t9".into()))
        );
        note_covers("org.example.storedcovers", false);
    }

    #[test]
    fn the_newest_request_goes_first() {
        let waiting = Waiting::default();
        waiting.push(("a".into(), 1));
        waiting.push(("b".into(), 1));
        assert_eq!(waiting.pop(), Some(("b".into(), 1)));
        assert_eq!(waiting.pop(), Some(("a".into(), 1)));
        waiting.close();
        assert_eq!(waiting.pop(), None);
    }

    #[test]
    fn trimming_keeps_the_newest_pictures() {
        let dir = tempfile::tempdir().unwrap();
        for (n, name) in ["a", "b", "c", "d"].iter().enumerate() {
            let folder = dir.path().join(name);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("original"), vec![0u8; 1000]).unwrap();
            let file = std::fs::File::options()
                .write(true)
                .open(folder.join("original"))
                .unwrap();
            let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000 + n as u64 * 100);
            file.set_modified(at).unwrap();
        }
        // 4000 bytes against a limit of 3000: the oldest go until 2250 is left.
        trim(dir.path(), 3000);
        let left: Vec<bool> = ["a", "b", "c", "d"]
            .iter()
            .map(|n| dir.path().join(n).exists())
            .collect();
        assert_eq!(left, [false, false, true, true]);
        trim(dir.path(), 3000);
        assert!(dir.path().join("c").exists(), "under the limit nothing goes");
    }
}

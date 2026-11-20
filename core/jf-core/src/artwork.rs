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
//! their headers, as plugin items become rows.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crossbeam_channel::{Sender, unbounded};
use jf_plugin_host::{Header, ImageRef};

use crate::actor::Msg;
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

/// The URLs plugins gave pictures under, with the headers to send. Shared
/// by the whole process: rows are made on many threads.
static KNOWN_IMAGES: Mutex<Option<Known>> = Mutex::new(None);

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

/// Fetches (or reads the kept original of) a picture and makes its JPEG.
fn make(dir: &Path, url: &str, size_px: u32) -> Result<PathBuf, String> {
    let path = thumbnail_path(dir, url, size_px);
    if path.is_file() {
        return Ok(path);
    }
    let original = original_path(dir, url);
    let bytes = match std::fs::read(&original) {
        Ok(bytes) => bytes,
        Err(_) => {
            let image = image_for(url).ok_or("not a picture a source gave")?;
            let bytes = crate::sources::fetch_image(&image)?;
            crate::sources::write_atomic(&original, &bytes).map_err(|e| e.to_string())?;
            bytes
        }
    };
    jf_library::make_thumbnail(&bytes, size_px, &path).map_err(|e| e.to_string())?;
    Ok(path)
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
    jobs: Sender<(String, u32)>,
}

impl Artwork {
    /// The cache in `<cache_dir>/source-artwork`, with its fetchers, which
    /// tell the actor when a picture is ready.
    pub(crate) fn new(cache_dir: &Path, actor: Sender<Msg>) -> Artwork {
        Artwork::with_limit(cache_dir, actor, MAX_BYTES)
    }

    fn with_limit(cache_dir: &Path, actor: Sender<Msg>, max_bytes: u64) -> Artwork {
        let dir = cache_dir.join("source-artwork");
        let pending: Arc<Mutex<HashSet<(String, u32)>>> = Arc::default();
        let (jobs, rx) = unbounded::<(String, u32)>();
        let stored = Arc::new(AtomicU32::new(0));
        for n in 0..WORKERS {
            let (rx, dir, pending, actor, stored) = (
                rx.clone(),
                dir.clone(),
                pending.clone(),
                actor.clone(),
                stored.clone(),
            );
            let spawned = std::thread::Builder::new()
                .name(format!("jf-artwork-{n}"))
                .spawn(move || {
                    for (url, size_px) in rx.iter() {
                        let result = make(&dir, &url, size_px);
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
                            trim(&dir, max_bytes);
                        }
                    }
                });
            if let Err(e) = spawned {
                tracing::warn!("could not start an artwork fetcher: {e}");
            }
        }
        Artwork { dir, pending, jobs }
    }

    /// The folder pictures are kept in.
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// The picture's file when it is ready; otherwise starts making it (once
    /// for any number of requests) and returns `None`: `Event::Artwork`
    /// follows.
    pub(crate) fn get(&self, url: &str, size_px: u32) -> Option<PathBuf> {
        let path = thumbnail_path(&self.dir, url, size_px);
        if path.is_file() {
            return Some(path);
        }
        let key = (url.to_owned(), size_px);
        if self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone())
        {
            let _ = self.jobs.send(key);
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

//! Artwork loading off the main thread.
//!
//! List rows and cards ask for artwork by content hash and size; a worker
//! thread gets the core's thumbnail (`Core::artwork_thumbnail`), decodes and
//! scales it, and hands back a texture. Requests are served newest first, so
//! the rows on screen win over rows scrolled past, and decoded textures are
//! kept in a byte-budget LRU.
//!
//! Until the core's thumbnails land (it answers "not available in this build
//! yet"), the worker scales the original cached artwork
//! (`<cache>/artwork/<hash>.<ext>`, the content-addressed path the core
//! writes) itself.
//!
//! A source's pictures (`ArtworkRef::Url`) come through the core too, never
//! from the app (ADR-0022 §5, docs/core-api.md "Covers"): the worker asks
//! `Core::source_artwork`; a path is the cached picture, `None` means the
//! core is fetching it, and the request waits for `Event::Artwork`, which
//! [`Thumbs::artwork_ready`] answers.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;

use gtk::prelude::*;
use gtk::{gdk, gdk_pixbuf, glib};
use jf_core::Core;

use crate::logic::lru::Lru;
use crate::ui::state::try_app;

/// Decoded artwork kept in memory.
const CACHE_BYTES: usize = 64 * 1024 * 1024;
/// Requests beyond this many waiting are dropped (oldest first).
const MAX_QUEUED: usize = 256;

/// What to load.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArtKey {
    /// Library artwork by content hash.
    Hash(String),
    /// An image file (Now Playing artwork, baked backdrops).
    File(PathBuf),
    /// A source's picture, which the core fetches and caches.
    Url(String),
}

type Request = (ArtKey, u32);
type Waiter = Box<dyn FnOnce(Option<&gdk::Texture>)>;

/// What the worker made of a request.
enum Loaded {
    Done(Option<gdk::Texture>),
    /// The core is fetching the source's picture: wait for `Event::Artwork`.
    Fetching,
}

pub struct Thumbs {
    cache: RefCell<Lru<Request, gdk::Texture>>,
    waiting: RefCell<HashMap<Request, Vec<Waiter>>>,
    /// Source pictures the core could not get: not asked for again.
    missing: RefCell<std::collections::HashSet<Request>>,
    tx: RefCell<Option<crossbeam_channel::Sender<Request>>>,
    thread: RefCell<Option<JoinHandle<()>>>,
}

impl std::fmt::Debug for Thumbs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Thumbs").finish_non_exhaustive()
    }
}

/// The thumbnail size to ask the core for: the smallest of its sizes that
/// covers `px` device pixels.
pub fn size_for(px: i32) -> u32 {
    [64, 128, 256, 512, 1024]
        .into_iter()
        .find(|s| *s as i32 >= px)
        .unwrap_or(1024)
}

impl Thumbs {
    pub fn start(core: Arc<Core>, cache_dir: PathBuf) -> std::io::Result<Thumbs> {
        let (tx, rx) = crossbeam_channel::unbounded::<Request>();
        let (done_tx, done_rx) = async_channel::unbounded::<(Request, Loaded)>();
        let dir = cache_dir.clone();
        let thread = std::thread::Builder::new()
            .name("jf-gtk-artwork".into())
            .spawn(move || {
                let mut stack: Vec<Request> = Vec::new();
                loop {
                    if stack.is_empty() {
                        match rx.recv() {
                            Ok(r) => stack.push(r),
                            Err(_) => break,
                        }
                    }
                    stack.extend(rx.try_iter());
                    if stack.len() > MAX_QUEUED {
                        let excess = stack.len() - MAX_QUEUED;
                        stack.drain(..excess);
                    }
                    let Some(req) = stack.pop() else { continue };
                    stack.retain(|r| *r != req);
                    let texture = load(&core, &dir, &req);
                    if done_tx.send_blocking((req, texture)).is_err() {
                        break;
                    }
                }
            })?;
        glib::spawn_future_local(async move {
            while let Ok((req, loaded)) = done_rx.recv().await {
                if let (Some(app), Loaded::Done(texture)) = (try_app(), loaded) {
                    app.thumbs.deliver(req, texture);
                }
            }
        });
        Ok(Thumbs {
            cache: RefCell::new(Lru::new(CACHE_BYTES)),
            waiting: RefCell::new(HashMap::new()),
            missing: RefCell::new(std::collections::HashSet::new()),
            tx: RefCell::new(Some(tx)),
            thread: RefCell::new(Some(thread)),
        })
    }

    /// The core cached a source's picture (or cannot get it): the views
    /// waiting for it load it now, or keep their placeholder.
    pub fn artwork_ready(&self, ready: &jf_core::ArtworkReady) {
        let req = (ArtKey::Url(ready.url.clone()), ready.size_px);
        if ready.path.is_none() {
            self.missing.borrow_mut().insert(req.clone());
            self.deliver(req, None);
            return;
        }
        if self.waiting.borrow().contains_key(&req)
            && let Some(tx) = self.tx.borrow().as_ref()
        {
            let _ = tx.send(req);
        }
    }

    /// The texture if it is decoded already.
    pub fn cached(&self, key: &ArtKey, size: u32) -> Option<gdk::Texture> {
        self.cache.borrow_mut().get(&(key.clone(), size))
    }

    /// Loads artwork; `done` runs on the main thread, at once when cached.
    pub fn load(&self, key: ArtKey, size: u32, done: impl FnOnce(Option<&gdk::Texture>) + 'static) {
        let req = (key, size);
        if let Some(t) = self.cache.borrow_mut().get(&req) {
            done(Some(&t));
            return;
        }
        if self.missing.borrow().contains(&req) {
            done(None);
            return;
        }
        let mut waiting = self.waiting.borrow_mut();
        let first = !waiting.contains_key(&req);
        waiting.entry(req.clone()).or_default().push(Box::new(done));
        drop(waiting);
        if first && let Some(tx) = self.tx.borrow().as_ref() {
            let _ = tx.send(req);
        }
    }

    fn deliver(&self, req: Request, texture: Option<gdk::Texture>) {
        if let Some(t) = &texture {
            let bytes = (t.width().max(1) as usize) * (t.height().max(1) as usize) * 4;
            self.cache.borrow_mut().put(req.clone(), t.clone(), bytes);
        }
        let waiters = self.waiting.borrow_mut().remove(&req).unwrap_or_default();
        for w in waiters {
            w(texture.as_ref());
        }
    }

    /// Stops the worker (at shutdown).
    pub fn stop(&self) {
        self.tx.borrow_mut().take();
        self.waiting.borrow_mut().clear();
        if let Some(t) = self.thread.borrow_mut().take() {
            let _ = t.join();
        }
    }
}

fn load(core: &Core, cache_dir: &Path, (key, size): &Request) -> Loaded {
    let path = match key {
        ArtKey::Hash(hash) => match core.artwork_thumbnail(hash, *size) {
            Ok(Some(path)) => Some(path),
            _ => original_artwork(cache_dir, hash),
        },
        ArtKey::File(path) => Some(path.clone()),
        ArtKey::Url(url) => match core.source_artwork(url, *size) {
            Some(path) => Some(path),
            None => return Loaded::Fetching,
        },
    };
    Loaded::Done(path.and_then(|path| decode(&path, *size)))
}

fn decode(path: &Path, size: u32) -> Option<gdk::Texture> {
    let side = i32::try_from(size).unwrap_or(1024);
    let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_scale(path, side, side, true).ok()?;
    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

/// The cached original of artwork `hash`, as the library stores it.
fn original_artwork(cache_dir: &Path, hash: &str) -> Option<PathBuf> {
    if hash.is_empty() || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let dir = cache_dir.join("artwork");
    ["jpg", "png", "webp", "gif", "bmp", "img"]
        .iter()
        .map(|ext| dir.join(format!("{hash}.{ext}")))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_cover_the_request() {
        assert_eq!(size_for(40), 64);
        assert_eq!(size_for(64), 64);
        assert_eq!(size_for(80), 128);
        assert_eq!(size_for(400), 512);
        assert_eq!(size_for(5_000), 1024);
    }

    #[test]
    fn finds_cached_originals_by_hash_only() {
        let dir = std::env::temp_dir().join(format!("jf-gtk-thumbs-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("artwork")).unwrap();
        std::fs::write(dir.join("artwork/abc123.png"), b"x").unwrap();
        assert_eq!(
            original_artwork(&dir, "abc123"),
            Some(dir.join("artwork/abc123.png"))
        );
        assert_eq!(original_artwork(&dir, "../etc"), None);
        assert_eq!(original_artwork(&dir, "def"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

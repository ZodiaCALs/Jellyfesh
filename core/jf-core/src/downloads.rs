//! Downloads from the person's own servers (ADR-0025): a queue in the core.
//!
//! * Only plugins with the `download` capability download, through their
//!   `download` export (where to fetch the file, never a stream URL).
//! * The queue survives a restart (`<data_dir>/downloads.json`), resumes
//!   with HTTP ranges where the server allows (`<file>.part`), retries
//!   network errors with backoff, and waits while the network is off, or
//!   metered with Wi-Fi Only on. At most `at_once` run together.
//! * Files are named by the template, made safe, and never overwrite another
//!   (" (2)"). A converted file gets the song's tags and cover; an original
//!   stays byte for byte unless Add Missing Cover and Lyrics is on.
//! * Finished files join the library (the downloads folder is a library
//!   root), linked to their source songs: those play the file, offline too,
//!   and rows show the Downloaded mark ([`downloaded`]).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use jf_plugin_host::filenames::{NameFields, render, unique_file_name};
use jf_plugin_host::{CallError, PluginError};
use jf_session::SourceTrack;
use serde::{Deserialize, Serialize};

use crate::actor::Msg;
use crate::sources::Ctx;
use crate::types::{
    DownloadError, DownloadItem, DownloadQuality, DownloadSettings, DownloadState, DownloadsPaused,
};

/// Attempts at a download before it fails (network errors only).
const ATTEMPTS: u32 = 5;
/// The first wait before trying again; it doubles each time.
const BACKOFF: Duration = Duration::from_secs(1);
/// Longest wait for the plugin's answer.
const CALL_WAIT: Duration = Duration::from_secs(25);
/// Progress goes to the actor at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// Redirect hops followed (same origin only, so the headers stay there).
const MAX_REDIRECTS: usize = 3;

/// Finished downloads by (plugin, item id), with their files.
static FINISHED: RwLock<Option<HashMap<(String, String), PathBuf>>> = RwLock::new(None);

/// The file of a source song's finished download, when it is still there:
/// the song plays it (offline too).
pub(crate) fn downloaded(plugin: &str, id: &str) -> Option<PathBuf> {
    FINISHED
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()?
        .get(&(plugin.to_owned(), id.to_owned()))
        .filter(|path| path.is_file())
        .cloned()
}

/// Whether a source song has a finished download (the Downloaded mark).
pub(crate) fn is_downloaded(plugin: &str, id: &str) -> bool {
    FINISHED
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|map| map.contains_key(&(plugin.to_owned(), id.to_owned())))
}

/// Why a download failed, as kept on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Failure {
    NotAllowed { service: String },
    NotOffered { service: String },
    Network { message: String },
    Storage { message: String },
    Other { message: String },
}

impl Failure {
    fn error(&self) -> DownloadError {
        match self.clone() {
            Failure::NotAllowed { service } => DownloadError::NotAllowed { service },
            Failure::NotOffered { service } => DownloadError::NotOffered { service },
            Failure::Network { message } => DownloadError::Network { message },
            Failure::Storage { message } => DownloadError::Storage { message },
            Failure::Other { message } => DownloadError::Other { message },
        }
    }
}

/// Where a download is, as kept on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum Status {
    Queued,
    /// Running; queued again after a restart (it resumes).
    Downloading,
    Done,
    Failed {
        failure: Failure,
    },
}

/// One download.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Entry {
    id: u64,
    track: SourceTrack,
    status: Status,
    bytes_done: u64,
    bytes_total: Option<u64>,
    /// The file: where it goes while it downloads (`.part` beside it), and
    /// where it is once finished.
    path: Option<PathBuf>,
    library_id: Option<i64>,
    original: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Stored {
    next_id: u64,
    items: Vec<Entry>,
}

/// What a download's thread tells the actor.
#[derive(Debug)]
pub(crate) enum DownloadMsg {
    /// The file it goes to, and whether it is the server's original.
    Started {
        id: u64,
        path: PathBuf,
        original: bool,
    },
    Progress {
        id: u64,
        done: u64,
        total: Option<u64>,
    },
    Finished {
        id: u64,
        path: PathBuf,
    },
    Failed {
        id: u64,
        failure: Failure,
    },
    /// Cancelled, or paused (the network went): it stops where it is.
    Stopped {
        id: u64,
    },
}

/// A download for its thread.
struct Job {
    id: u64,
    plugin: String,
    service: String,
    item: jf_plugin_host::Item,
    track: SourceTrack,
    settings: DownloadSettings,
    folder: PathBuf,
    /// The file chosen before (a restart resumes it).
    path: Option<PathBuf>,
    stop: Arc<AtomicBool>,
}

/// The download queue.
#[derive(Debug)]
pub(crate) struct Downloads {
    file: PathBuf,
    next_id: u64,
    items: Vec<Entry>,
    /// Running downloads' stop flags.
    running: HashMap<u64, Arc<AtomicBool>>,
}

impl Downloads {
    /// The queue kept in `file`; downloads that were running are queued
    /// again (they resume).
    pub(crate) fn load(file: PathBuf) -> Downloads {
        let stored: Stored = std::fs::read(&file)
            .ok()
            .and_then(|bytes| match serde_json::from_slice(&bytes) {
                Ok(stored) => Some(stored),
                Err(e) => {
                    tracing::warn!(path = %file.display(), "ignoring an unreadable download queue: {e}");
                    None
                }
            })
            .unwrap_or_default();
        let mut items = stored.items;
        for entry in &mut items {
            if entry.status == Status::Downloading {
                entry.status = Status::Queued;
            }
        }
        let downloads = Downloads {
            file,
            next_id: stored.next_id.max(1),
            items,
            running: HashMap::new(),
        };
        downloads.note_finished();
        downloads
    }

    fn save(&self) {
        let stored = Stored {
            next_id: self.next_id,
            items: self.items.clone(),
        };
        let written = serde_json::to_vec(&stored)
            .map_err(std::io::Error::other)
            .and_then(|bytes| crate::sources::write_atomic(&self.file, &bytes));
        if let Err(e) = written {
            tracing::warn!("could not keep the download queue: {e}");
        }
    }

    /// Updates the finished downloads the rows and playback see.
    fn note_finished(&self) {
        let map: HashMap<(String, String), PathBuf> = self
            .items
            .iter()
            .filter(|e| e.status == Status::Done)
            .filter_map(|e| Some(((e.track.plugin.clone(), e.track.id.clone()), e.path.clone()?)))
            .collect();
        *FINISHED.write().unwrap_or_else(|e| e.into_inner()) = Some(map);
    }

    /// Queues songs; one already queued, running or done is left as it is.
    /// Returns how many were added.
    pub(crate) fn add(&mut self, tracks: Vec<SourceTrack>) -> usize {
        let mut added = 0;
        for track in tracks {
            let known = self
                .items
                .iter_mut()
                .find(|e| e.track.plugin == track.plugin && e.track.id == track.id);
            match known {
                Some(entry) => {
                    if matches!(entry.status, Status::Failed { .. }) {
                        entry.status = Status::Queued;
                        added += 1;
                    }
                }
                None => {
                    self.items.push(Entry {
                        id: self.next_id,
                        track,
                        status: Status::Queued,
                        bytes_done: 0,
                        bytes_total: None,
                        path: None,
                        library_id: None,
                        original: false,
                    });
                    self.next_id += 1;
                    added += 1;
                }
            }
        }
        self.save();
        added
    }

    /// Starts queued downloads while fewer than `at_once` run (none while
    /// `paused`).
    pub(crate) fn pump(
        &mut self,
        ctx: Option<&Arc<Ctx>>,
        settings: &DownloadSettings,
        folder: &Path,
        paused: bool,
        tx: &Sender<Msg>,
    ) {
        let Some(ctx) = ctx else {
            return;
        };
        if paused {
            return;
        }
        let at_once = settings.at_once.clamp(1, 4) as usize;
        while self.running.len() < at_once {
            let Some(entry) = self.items.iter_mut().find(|e| e.status == Status::Queued) else {
                break;
            };
            let stop = Arc::new(AtomicBool::new(false));
            let job = Job {
                id: entry.id,
                plugin: entry.track.plugin.clone(),
                service: ctx
                    .host
                    .plugin(&entry.track.plugin)
                    .map_or_else(|| entry.track.plugin.clone(), |info| info.name),
                item: ctx.track_item(&entry.track),
                track: entry.track.clone(),
                settings: settings.clone(),
                folder: folder.to_path_buf(),
                path: entry.path.clone(),
                stop: stop.clone(),
            };
            entry.status = Status::Downloading;
            self.running.insert(entry.id, stop);
            let (ctx, tx) = (ctx.clone(), tx.clone());
            let spawned = std::thread::Builder::new()
                .name("jf-download".into())
                .spawn(move || run(&ctx, job, &tx));
            if let Err(e) = spawned {
                let id = entry.id;
                entry.status = Status::Failed {
                    failure: Failure::Other {
                        message: e.to_string(),
                    },
                };
                self.running.remove(&id);
                break;
            }
        }
    }

    /// A message from a download's thread; `true` when a file finished.
    pub(crate) fn on_msg(&mut self, msg: DownloadMsg, offline: bool) -> bool {
        let id = match &msg {
            DownloadMsg::Started { id, .. }
            | DownloadMsg::Progress { id, .. }
            | DownloadMsg::Finished { id, .. }
            | DownloadMsg::Failed { id, .. }
            | DownloadMsg::Stopped { id } => *id,
        };
        let Some(entry) = self.items.iter_mut().find(|e| e.id == id) else {
            // Cancelled meanwhile.
            self.running.remove(&id);
            return false;
        };
        let mut finished = false;
        match msg {
            DownloadMsg::Started { path, original, .. } => {
                entry.path = Some(path);
                entry.original = original;
            }
            DownloadMsg::Progress { done, total, .. } => {
                entry.bytes_done = done;
                entry.bytes_total = total;
                return false; // not kept on disk each time
            }
            DownloadMsg::Finished { path, .. } => {
                self.running.remove(&id);
                entry.status = Status::Done;
                entry.bytes_total = Some(entry.bytes_done.max(file_size(&path)));
                entry.bytes_done = entry.bytes_total.unwrap_or_default();
                entry.path = Some(path);
                finished = true;
            }
            DownloadMsg::Failed { failure, .. } => {
                self.running.remove(&id);
                // The network went: it waits for it rather than failing.
                entry.status = if offline && matches!(failure, Failure::Network { .. }) {
                    Status::Queued
                } else {
                    Status::Failed { failure }
                };
            }
            DownloadMsg::Stopped { .. } => {
                self.running.remove(&id);
                if entry.status == Status::Downloading {
                    entry.status = Status::Queued;
                }
            }
        }
        self.save();
        if finished {
            self.note_finished();
        }
        finished
    }

    /// Notes the library tracks of finished files (after the library scanned
    /// them), with `id_of`; `true` when one was found.
    pub(crate) fn link(&mut self, id_of: impl Fn(&Path) -> Option<i64>) -> bool {
        let mut changed = false;
        for entry in &mut self.items {
            if entry.status == Status::Done
                && entry.library_id.is_none()
                && let Some(path) = &entry.path
                && let Some(id) = id_of(path)
            {
                entry.library_id = Some(id);
                changed = true;
            }
        }
        if changed {
            self.save();
        }
        changed
    }

    /// Stops the running downloads: they wait, and resume on the next pump.
    pub(crate) fn pause_all(&mut self) {
        for stop in self.running.values() {
            stop.store(true, Ordering::Relaxed);
        }
    }

    /// Cancels a download that has not finished; its partial file goes.
    pub(crate) fn cancel(&mut self, id: u64) -> Result<(), String> {
        let at = self
            .items
            .iter()
            .position(|e| e.id == id && e.status != Status::Done)
            .ok_or("no such download in progress")?;
        let entry = self.items.remove(at);
        if let Some(stop) = self.running.remove(&id) {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some(path) = &entry.path {
            let _ = std::fs::remove_file(partial(path));
        }
        self.save();
        Ok(())
    }

    /// Queues a failed download again.
    pub(crate) fn retry(&mut self, id: u64) -> Result<(), String> {
        let entry = self
            .items
            .iter_mut()
            .find(|e| e.id == id && matches!(e.status, Status::Failed { .. }))
            .ok_or("no such failed download")?;
        entry.status = Status::Queued;
        self.save();
        Ok(())
    }

    /// Removes a finished download and deletes its file (and `.lrc`);
    /// returns the file for the library to forget.
    pub(crate) fn remove(&mut self, id: u64) -> Result<Option<PathBuf>, String> {
        let at = self
            .items
            .iter()
            .position(|e| e.id == id && e.status == Status::Done)
            .ok_or("no such finished download")?;
        let entry = self.items.remove(at);
        let path = entry.path.clone();
        if let Some(path) = &path {
            delete_file(path)?;
        }
        self.save();
        self.note_finished();
        Ok(path)
    }

    /// Remove All Downloads: stops the running ones and deletes every file;
    /// returns the finished files for the library to forget.
    pub(crate) fn remove_all(&mut self) -> Result<Vec<PathBuf>, String> {
        for stop in self.running.values() {
            stop.store(true, Ordering::Relaxed);
        }
        self.running.clear();
        let mut removed = Vec::new();
        let mut failed = None;
        for entry in std::mem::take(&mut self.items) {
            let Some(path) = entry.path else { continue };
            if entry.status == Status::Done {
                match delete_file(&path) {
                    Ok(()) => removed.push(path),
                    Err(e) => failed = Some(e),
                }
            } else {
                let _ = std::fs::remove_file(partial(&path));
            }
        }
        self.save();
        self.note_finished();
        match failed {
            Some(error) => Err(error),
            None => Ok(removed),
        }
    }

    /// The slice's items: downloads in progress in queue order, then the
    /// finished ones by album; and the space the finished ones take.
    pub(crate) fn items(&self) -> (Vec<DownloadItem>, u64) {
        let item = |e: &Entry| DownloadItem {
            id: e.id,
            track: e.track.clone(),
            state: match &e.status {
                Status::Queued => DownloadState::Queued,
                Status::Downloading => DownloadState::Downloading,
                Status::Done => DownloadState::Done,
                Status::Failed { failure } => DownloadState::Failed {
                    error: failure.error(),
                },
            },
            bytes_done: e.bytes_done,
            bytes_total: e.bytes_total,
            path: e.path.clone().filter(|_| e.status == Status::Done),
            library_id: e.library_id,
            original: e.original,
        };
        let mut items: Vec<DownloadItem> = self
            .items
            .iter()
            .filter(|e| e.status != Status::Done)
            .map(item)
            .collect();
        let mut done: Vec<&Entry> = self.items.iter().filter(|e| e.status == Status::Done).collect();
        done.sort_by(|a, b| {
            (a.track.album.as_deref(), a.track.title.as_str())
                .cmp(&(b.track.album.as_deref(), b.track.title.as_str()))
        });
        let used = done.iter().filter_map(|e| e.path.as_deref()).map(file_size).sum();
        items.extend(done.into_iter().map(item));
        (items, used)
    }
}

/// Why queued downloads cannot run now.
pub(crate) fn paused(online: bool, metered: bool, settings: &DownloadSettings) -> Option<DownloadsPaused> {
    if !online {
        Some(DownloadsPaused::Offline)
    } else if metered && settings.wifi_only {
        Some(DownloadsPaused::Metered)
    } else {
        None
    }
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Where a file downloads to before it is finished.
fn partial(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

fn delete_file(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("could not delete {}: {e}", path.display())),
    }
    let _ = std::fs::remove_file(path.with_extension("lrc"));
    Ok(())
}

/// The plugin API's quality for a download.
fn host_quality(quality: DownloadQuality) -> jf_plugin_host::DownloadQuality {
    match quality {
        DownloadQuality::Original => jf_plugin_host::DownloadQuality::Original,
        DownloadQuality::High => jf_plugin_host::DownloadQuality::High,
        DownloadQuality::Normal => jf_plugin_host::DownloadQuality::Normal,
        DownloadQuality::Low => jf_plugin_host::DownloadQuality::Low,
    }
}

/// The file a song downloads to: the template's folders and name under
/// `folder`, never another file's name.
pub(crate) fn target(
    folder: &Path,
    template: &str,
    track: &SourceTrack,
    item: &jf_plugin_host::Item,
    service: &str,
    container: &str,
) -> PathBuf {
    let fields = NameFields {
        artist: track.artist.clone(),
        album_artist: track.artist.clone(),
        album: track.album.clone(),
        title: track.title.clone(),
        track: item.track_number,
        disc: item.disc_number,
        year: item.year,
        source: Some(service.to_owned()),
    };
    let mut parts = render(template, &fields);
    let stem = parts.pop().unwrap_or_else(|| track.title.clone());
    let dir = parts
        .iter()
        .fold(folder.to_path_buf(), |dir, part| dir.join(part));
    let name = unique_file_name(&stem, container, |name| {
        let path = dir.join(name);
        path.exists() || partial(&path).exists()
    });
    dir.join(name)
}

/// Runs one download on its own thread.
fn run(ctx: &Ctx, job: Job, tx: &Sender<Msg>) {
    let send = |msg: DownloadMsg| {
        let _ = tx.send(Msg::Download(msg));
    };
    match download(ctx, &job, &send) {
        Ok(path) => send(DownloadMsg::Finished { id: job.id, path }),
        Err(None) => send(DownloadMsg::Stopped { id: job.id }),
        Err(Some(failure)) => send(DownloadMsg::Failed { id: job.id, failure }),
    }
}

/// Fetches, names and tags one file; `Err(None)` when it was stopped.
fn download(ctx: &Ctx, job: &Job, send: &dyn Fn(DownloadMsg)) -> Result<PathBuf, Option<Failure>> {
    let quality = host_quality(job.settings.quality);
    let info = ctx
        .host
        .download(&job.plugin, job.item.clone(), quality)
        .recv_timeout(CALL_WAIT)
        .unwrap_or(Err(CallError::Timeout))
        .map_err(|error| {
            tracing::debug!(plugin = %job.plugin, %error, "no download from the plugin");
            Some(match error {
                CallError::Plugin(PluginError::Unsupported) => Failure::NotAllowed {
                    service: job.service.clone(),
                },
                CallError::NotDeclared(_) => Failure::NotOffered {
                    service: job.service.clone(),
                },
                CallError::Plugin(PluginError::Network(message)) => Failure::Network { message },
                error => Failure::Other {
                    message: error.to_string(),
                },
            })
        })?;
    let path = match &job.path {
        // A restart resumes the file chosen before (same kind of file).
        Some(path) if path.extension().and_then(|e| e.to_str()) == Some(info.container.as_str()) => {
            path.clone()
        }
        _ => target(
            &job.folder,
            &job.settings.name_template,
            &job.track,
            &job.item,
            &job.service,
            &info.container,
        ),
    };
    send(DownloadMsg::Started {
        id: job.id,
        path: path.clone(),
        original: info.original,
    });
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Some(storage(&e)))?;
    }
    let part = partial(&path);
    let mut wait = BACKOFF;
    for attempt in 1..=ATTEMPTS {
        match fetch(&info, &part, job, send) {
            Ok(()) => break,
            Err(Fetch::Stopped) => return Err(None),
            Err(Fetch::Failed(failure)) => return Err(Some(failure)),
            Err(Fetch::Network(message)) => {
                if attempt == ATTEMPTS {
                    return Err(Some(Failure::Network { message }));
                }
                tracing::debug!(id = job.id, attempt, %message, "download interrupted; trying again");
                let until = Instant::now() + wait;
                while Instant::now() < until {
                    if job.stop.load(Ordering::Relaxed) {
                        return Err(None);
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                wait *= 2;
            }
        }
    }
    std::fs::rename(&part, &path).map_err(|e| Some(storage(&e)))?;
    tag(ctx, job, &path, info.original);
    Ok(path)
}

fn storage(error: &std::io::Error) -> Failure {
    Failure::Storage {
        message: error.to_string(),
    }
}

enum Fetch {
    Stopped,
    Network(String),
    Failed(Failure),
}

/// Fetches the file into `part`, resuming what is there when the server
/// takes ranges.
fn fetch(
    info: &jf_plugin_host::DownloadInfo,
    part: &Path,
    job: &Job,
    send: &dyn Fn(DownloadMsg),
) -> Result<(), Fetch> {
    let have = file_size(part);
    let mut url = url::Url::parse(&info.url).map_err(|e| {
        Fetch::Failed(Failure::Other {
            message: e.to_string(),
        })
    })?;
    let mut response = None;
    for _ in 0..=MAX_REDIRECTS {
        let mut request = crate::sources::http_agent().get(url.as_str());
        for header in &info.headers {
            request = request.header(header.name.as_str(), header.value.as_str());
        }
        if have > 0 {
            request = request.header("range", format!("bytes={have}-"));
        }
        let answer = request.call().map_err(|e| Fetch::Network(e.to_string()))?;
        let status = answer.status().as_u16();
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            let next = answer
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .and_then(|location| url.join(location).ok())
                .filter(|next| matches!(next.scheme(), "http" | "https") && next.origin() == url.origin())
                .ok_or_else(|| {
                    Fetch::Failed(Failure::Other {
                        message: "the server redirected the download elsewhere".into(),
                    })
                })?;
            url = next;
            continue;
        }
        response = Some(answer);
        break;
    }
    let mut response = response.ok_or_else(|| {
        Fetch::Failed(Failure::Other {
            message: "the server redirected the download too often".into(),
        })
    })?;
    let status = response.status().as_u16();
    let resumed = status == 206 && have > 0;
    match status {
        200 | 206 => {}
        401 | 403 => {
            return Err(Fetch::Failed(Failure::NotAllowed {
                service: job.service.clone(),
            }));
        }
        // Gone past the end: the part is the whole file.
        416 if have > 0 => return Ok(()),
        500..=599 => return Err(Fetch::Network(format!("the server answered {status}"))),
        _ => {
            return Err(Fetch::Failed(Failure::Other {
                message: format!("the server answered {status}"),
            }));
        }
    }
    // A refusal in the body is no file: Subsonic answers 200 with its JSON
    // error 50 when the server decides the account may not download.
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ["application/json", "application/xml", "text/"]
        .iter()
        .any(|kind| content_type.starts_with(kind))
    {
        return Err(Fetch::Failed(Failure::NotAllowed {
            service: job.service.clone(),
        }));
    }
    let length = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let total = match (resumed, length) {
        (true, Some(rest)) => Some(have + rest),
        (false, Some(all)) => Some(all),
        _ => info.size,
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(part)
        .map_err(|e| Fetch::Failed(storage(&e)))?;
    let mut done = if resumed { have } else { 0 };
    let mut body = response.body_mut().as_reader();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut last = Instant::now();
    loop {
        if job.stop.load(Ordering::Relaxed) {
            return Err(Fetch::Stopped);
        }
        let read = body
            .read(&mut buffer)
            .map_err(|e| Fetch::Network(e.to_string()))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|e| Fetch::Failed(storage(&e)))?;
        done += read as u64;
        if last.elapsed() >= PROGRESS_EVERY {
            last = Instant::now();
            send(DownloadMsg::Progress {
                id: job.id,
                done,
                total,
            });
        }
    }
    file.flush().map_err(|e| Fetch::Failed(storage(&e)))?;
    if let Some(total) = total
        && done < total
    {
        return Err(Fetch::Network(format!(
            "the connection ended at {done} of {total} bytes"
        )));
    }
    send(DownloadMsg::Progress {
        id: job.id,
        done,
        total,
    });
    Ok(())
}

/// Tags a finished file: a converted one gets the song's tags and cover; an
/// original only a missing cover and lyrics, with Add Missing Cover and
/// Lyrics on. Synced lyrics also go in a `.lrc` beside it when asked.
fn tag(ctx: &Ctx, job: &Job, path: &Path, original: bool) {
    let settings = &job.settings;
    let wants_tags = !original || settings.add_missing_cover_and_lyrics;
    let lyrics = ((settings.lyrics_in_tags && wants_tags) || settings.save_lrc_files)
        .then(|| ctx.lyrics(&job.plugin, &job.item))
        .flatten();
    let synced = lyrics
        .as_ref()
        .is_some_and(|(doc, synced)| *synced && doc.timing != jf_lyrics::Timing::Unsynced);
    let text = lyrics.as_ref().map(|(doc, _)| lyrics_text(doc, synced));
    if settings.save_lrc_files
        && synced
        && let Some(text) = &text
        && let Err(e) = std::fs::write(path.with_extension("lrc"), text)
    {
        tracing::warn!(path = %path.display(), "could not write the .lrc file: {e}");
    }
    if !wants_tags {
        return;
    }
    let listed = job.item.artwork.clone();
    let tags = jf_library::SongTags {
        title: job.track.title.clone(),
        artist: job.track.artist.clone(),
        album: job.track.album.clone(),
        album_artist: job.track.artist.clone(),
        track: job.item.track_number,
        disc: job.item.disc_number,
        year: job.item.year,
        cover: ctx.artwork(&job.plugin, &job.item, listed),
        lyrics: settings.lyrics_in_tags.then_some(text).flatten(),
    };
    if let Err(e) = jf_library::write_song_tags(path, &tags, original) {
        tracing::warn!(path = %path.display(), "could not tag the download: {e}");
    }
}

/// Lyrics as text for tags and `.lrc` files: LRC lines when synced, else
/// the plain lines.
fn lyrics_text(doc: &jf_lyrics::LyricDoc, synced: bool) -> String {
    doc.lines
        .iter()
        .map(|line| match (synced, line.start_ms) {
            (true, Some(ms)) => format!(
                "[{:02}:{:02}.{:02}]{}",
                ms / 60_000,
                ms / 1_000 % 60,
                ms / 10 % 100,
                line.text
            ),
            _ => line.text.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synced_lyrics_become_lrc_lines() {
        let doc = jf_lyrics::parse("[00:01.50]First\n[01:02.03]Second\n", None).unwrap();
        assert_eq!(lyrics_text(&doc, true), "[00:01.50]First\n[01:02.03]Second");
        assert_eq!(lyrics_text(&doc, false), "First\nSecond");
    }

    fn song(id: &str) -> SourceTrack {
        SourceTrack {
            plugin: "org.example.server".into(),
            id: id.into(),
            title: format!("Song {id}"),
            artist: Some("Someone".into()),
            album: Some("Album".into()),
            duration_ms: None,
            artwork_url: None,
            isrc: None,
            artist_id: None,
            album_id: None,
        }
    }

    #[test]
    fn the_queue_survives_a_restart_and_running_downloads_resume() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("downloads.json");
        let mut queue = Downloads::load(file.clone());
        assert_eq!(queue.add(vec![song("a"), song("b")]), 2);
        assert_eq!(queue.add(vec![song("a")]), 0, "once");
        queue.items[0].status = Status::Downloading;
        let done = dir.path().join("Song b.flac");
        std::fs::write(&done, b"fLaC").unwrap();
        queue.items[1].status = Status::Done;
        queue.items[1].path = Some(done.clone());
        queue.save();
        queue.note_finished();
        assert!(is_downloaded("org.example.server", "b"));
        assert_eq!(downloaded("org.example.server", "b"), Some(done.clone()));

        let again = Downloads::load(file);
        assert_eq!(again.items[0].status, Status::Queued, "resumes");
        let (items, used) = again.items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].state, DownloadState::Queued);
        assert_eq!(items[1].state, DownloadState::Done);
        assert_eq!(used, 4);
    }

    #[test]
    fn files_follow_the_template_and_never_clash() {
        let dir = tempfile::tempdir().unwrap();
        let mut item = crate::sources::bare_item("t1", jf_plugin_host::ItemKind::Track);
        item.track_number = Some(7);
        let first = target(dir.path(), "", &song("t1"), &item, "Server", "flac");
        assert_eq!(
            first,
            dir.path().join("Someone").join("Album").join("07 Song t1.flac")
        );
        std::fs::create_dir_all(first.parent().unwrap()).unwrap();
        std::fs::write(&first, b"x").unwrap();
        let second = target(dir.path(), "", &song("t1"), &item, "Server", "flac");
        assert_eq!(
            second,
            dir.path()
                .join("Someone")
                .join("Album")
                .join("07 Song t1 (2).flac")
        );
        assert_eq!(
            target(
                dir.path(),
                "{source}/{title}",
                &song("x/y"),
                &item,
                "My: Server",
                "opus"
            ),
            dir.path().join("My_ Server").join("Song x_y.opus")
        );
    }

    #[test]
    fn downloads_wait_for_the_network() {
        let wifi_only = DownloadSettings::default();
        assert_eq!(paused(false, false, &wifi_only), Some(DownloadsPaused::Offline));
        assert_eq!(paused(true, true, &wifi_only), Some(DownloadsPaused::Metered));
        assert_eq!(paused(true, false, &wifi_only), None);
        let any = DownloadSettings {
            wifi_only: false,
            ..DownloadSettings::default()
        };
        assert_eq!(paused(true, true, &any), None);
    }
}

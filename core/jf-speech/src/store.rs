//! Downloading, verifying and deleting models.
//!
//! In the store's folder, a model `ggml-base.bin` has up to three files:
//!
//! * `ggml-base.bin.part`: a download in progress or interrupted. The next
//!   download continues it with an HTTP range request; a server that answers
//!   the whole file (200 instead of 206) starts it again.
//! * `ggml-base.bin`: the model, renamed from the `.part` file only after its
//!   SHA-1 (and SHA-256, once the catalogue has it) matched.
//! * `ggml-base.bin.verified`: the file's size, modification time and
//!   digests, so [`ModelStore::path`] hashes a file again only after it changed.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, UNIX_EPOCH};

use sha2::Digest;

use crate::{ModelInfo, SpeechError, catalogue};

/// Bytes read at a time while downloading or hashing; cancellation is checked in between.
const CHUNK: usize = 256 * 1024;
/// Progress is reported each time this many more bytes arrived (and at the end).
const PROGRESS_STEP: u64 = 256 * 1024;
/// Connections one download may open again after one broke having brought bytes.
const MAX_RESUMES: u32 = 3;

/// The folder where speech models are downloaded, and where they are found.
///
/// Nothing is downloaded unless [`ModelStore::download`] is called. Methods
/// may run on any thread; a download blocks its caller until it ends.
#[derive(Debug, Clone)]
pub struct ModelStore {
    dir: PathBuf,
    /// Replaces the catalogue's URLs with `{base_url}/{file}` (tests).
    base_url: Option<String>,
    models: &'static [ModelInfo],
}

impl ModelStore {
    /// A store in `dir` (created on the first download) for the [`catalogue`]'s models.
    pub fn new(dir: impl Into<PathBuf>) -> ModelStore {
        ModelStore {
            dir: dir.into(),
            base_url: None,
            models: catalogue(),
        }
    }

    /// A store that downloads `{base_url}/{file}` instead of the catalogue's
    /// URLs, such as a local test server.
    pub fn with_base_url(dir: impl Into<PathBuf>, base_url: &str) -> ModelStore {
        ModelStore {
            dir: dir.into(),
            base_url: Some(base_url.trim_end_matches('/').to_owned()),
            models: catalogue(),
        }
    }

    /// The model file, once it is downloaded and verified.
    ///
    /// The first call after a file appeared or changed hashes it (seconds for
    /// the largest model) and records the result next to it; later calls only
    /// compare its size and modification time. `None` for unknown ids.
    pub fn path(&self, id: &str) -> Option<PathBuf> {
        let model = self.info(id)?;
        let file = self.dir.join(model.file);
        let meta = fs::metadata(&file).ok().filter(fs::Metadata::is_file)?;
        let stamp = Stamp::of(&meta);
        let marker = self.marker_path(model);
        let digests = match stamp.as_ref().and_then(|stamp| read_marker(&marker, stamp)) {
            Some(cached) => cached,
            None => {
                let digests = hash_file(&file).ok()?;
                if let Some(stamp) = &stamp {
                    write_marker(&marker, stamp, &digests);
                }
                digests
            }
        };
        digests.mismatch(model).is_none().then_some(file)
    }

    /// Downloads the model `id` unless it is already here, and answers its path.
    ///
    /// Continues an interrupted download. `progress` receives the bytes the
    /// file has so far and its size (the catalogue's approximate size until
    /// the server tells). Setting `cancel` stops the download with
    /// [`SpeechError::Cancelled`] and keeps what arrived for the next call.
    /// A file that fails verification is deleted. One download per model at a
    /// time in this process; unknown ids answer [`SpeechError::Download`].
    pub fn download(
        &self,
        id: &str,
        progress: &mut dyn FnMut(u64, u64),
        cancel: &AtomicBool,
    ) -> Result<PathBuf, SpeechError> {
        let model = self
            .info(id)
            .ok_or_else(|| SpeechError::Download(format!("there is no speech model {id:?}")))?;
        let file = self.dir.join(model.file);
        let _busy = Busy::claim(&file)?;
        if let Some(path) = self.path(id) {
            let size = fs::metadata(&path).map_or(model.approx_bytes, |m| m.len());
            progress(size, size);
            return Ok(path);
        }
        fs::create_dir_all(&self.dir).map_err(|e| disk("creating the model folder", &e))?;
        let part = self.part_path(model);
        let fetched = Transfer::open(&part, cancel)
            .and_then(|transfer| transfer.run(&self.url(model), model.approx_bytes, progress, cancel));
        let digests = match fetched {
            Ok(digests) => digests,
            Err(e) => {
                // An attempt that brought nothing leaves nothing behind.
                if fs::metadata(&part).is_ok_and(|m| m.len() == 0) {
                    let _ = fs::remove_file(&part);
                }
                return Err(e);
            }
        };
        if let Some(mismatch) = digests.mismatch(model) {
            // Continuing this file can never fix it.
            let _ = fs::remove_file(&part);
            return Err(SpeechError::Download(format!(
                "{} failed verification: {mismatch}",
                model.file
            )));
        }
        fs::rename(&part, &file).map_err(|e| disk("saving the model", &e))?;
        if let Some(stamp) = fs::metadata(&file).ok().as_ref().and_then(Stamp::of) {
            write_marker(&self.marker_path(model), &stamp, &digests);
        }
        tracing::info!("speech model {} downloaded and verified", model.id);
        Ok(file)
    }

    /// Deletes the model `id`: its file, a partial download and the
    /// verification record. Deleting what is not there succeeds; a model
    /// downloading in this process is refused (cancel it first).
    pub fn delete(&self, id: &str) -> io::Result<()> {
        let model = self.info(id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("there is no speech model {id:?}"),
            )
        })?;
        let file = self.dir.join(model.file);
        if Busy::holds(&file) {
            return Err(io::Error::new(
                io::ErrorKind::ResourceBusy,
                "the speech model is downloading; cancel the download first",
            ));
        }
        for path in [file, self.part_path(model), self.marker_path(model)] {
            match fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    /// The models this store offers: the [`catalogue`]'s.
    pub fn models(&self) -> &'static [ModelInfo] {
        self.models
    }

    /// The model with this id, as this store offers it.
    pub fn model(&self, id: &str) -> Option<&'static ModelInfo> {
        self.info(id)
    }

    /// A store for `models` instead of the catalogue's, downloading
    /// `{base_url}/{file}`: tests verify small files of their own with it.
    #[cfg(any(test, feature = "test-support"))]
    pub fn with_models(dir: impl Into<PathBuf>, base_url: &str, models: &'static [ModelInfo]) -> ModelStore {
        ModelStore {
            dir: dir.into(),
            base_url: Some(base_url.trim_end_matches('/').to_owned()),
            models,
        }
    }

    fn info(&self, id: &str) -> Option<&'static ModelInfo> {
        self.models.iter().find(|m| m.id == id)
    }

    fn url(&self, model: &ModelInfo) -> String {
        match &self.base_url {
            Some(base) => format!("{base}/{}", model.file),
            None => model.url.to_owned(),
        }
    }

    fn part_path(&self, model: &ModelInfo) -> PathBuf {
        self.dir.join(format!("{}.part", model.file))
    }

    fn marker_path(&self, model: &ModelInfo) -> PathBuf {
        self.dir.join(format!("{}.verified", model.file))
    }
}

/// One download into a `.part` file, over as many connections as it takes.
struct Transfer {
    file: File,
    hasher: Hasher,
    /// Bytes in the file, all hashed.
    have: u64,
}

/// Why copying a response body stopped early.
enum Stop {
    Cancelled,
    Network(io::Error),
    Disk(io::Error),
}

impl Transfer {
    /// Opens the `.part` file and hashes what an earlier attempt left in it,
    /// so the finished file needs no second pass.
    fn open(part: &Path, cancel: &AtomicBool) -> Result<Transfer, SpeechError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(part)
            .map_err(|e| disk("opening the partial download", &e))?;
        let mut hasher = Hasher::new();
        let have = hasher.read_all(&mut file, cancel)?;
        Ok(Transfer { file, hasher, have })
    }

    /// Empties the file to download from the first byte.
    fn restart(&mut self) -> Result<(), SpeechError> {
        self.file
            .set_len(0)
            .and_then(|()| self.file.seek(SeekFrom::Start(0)))
            .map_err(|e| disk("restarting the download", &e))?;
        self.hasher = Hasher::new();
        self.have = 0;
        Ok(())
    }

    fn run(
        mut self,
        url: &str,
        approx_bytes: u64,
        progress: &mut dyn FnMut(u64, u64),
        cancel: &AtomicBool,
    ) -> Result<Digests, SpeechError> {
        let mut restarted = false;
        let mut resumes = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(SpeechError::Cancelled);
            }
            let mut request = agent().get(url).header("Accept-Encoding", "identity");
            if self.have > 0 {
                request = request.header("Range", format!("bytes={}-", self.have));
            }
            let mut response = request.call().map_err(|e| SpeechError::Download(e.to_string()))?;
            let status = response.status().as_u16();
            let headers = response.headers();
            let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
            let length = header("content-length").and_then(|v| v.trim().parse::<u64>().ok());
            let range = header("content-range").and_then(parse_content_range);
            let total = match status {
                206 => match range {
                    Some(range) if range.start == Some(self.have) => {
                        range.total.or(length.map(|length| self.have + length))
                    }
                    _ => {
                        // Another range than asked for: start again, once.
                        if restarted {
                            return Err(SpeechError::Download(
                                "the server answered a range that was not asked for".into(),
                            ));
                        }
                        tracing::debug!(
                            "the model server answered another range; downloading from the start"
                        );
                        restarted = true;
                        self.restart()?;
                        continue;
                    }
                },
                200 => {
                    if self.have > 0 {
                        tracing::debug!("the model server ignores ranges; downloading from the start");
                        self.restart()?;
                    }
                    length
                }
                // Nothing at or after the byte asked for: the partial file may be whole already.
                416 if self.have > 0 => {
                    if range.and_then(|range| range.total) == Some(self.have) {
                        progress(self.have, self.have);
                        return self.finish();
                    }
                    if restarted {
                        return Err(SpeechError::Download("the server refused every range".into()));
                    }
                    restarted = true;
                    self.restart()?;
                    continue;
                }
                _ => return Err(refused(status)),
            };
            let shown = |have: u64| total.unwrap_or(approx_bytes).max(have);
            progress(self.have, shown(self.have));
            let before = self.have;
            let mut body = response.body_mut().as_reader();
            let ended = self.copy(&mut body, &mut |have| progress(have, shown(have)), cancel);
            let broke = match ended {
                Ok(()) if total.is_some_and(|total| self.have < total) => {
                    io::Error::from(io::ErrorKind::UnexpectedEof)
                }
                Ok(()) => {
                    progress(self.have, shown(self.have));
                    return self.finish();
                }
                Err(Stop::Cancelled) => return Err(SpeechError::Cancelled),
                Err(Stop::Disk(e)) => return Err(disk("writing the model", &e)),
                Err(Stop::Network(e)) => e,
            };
            if self.have > before && resumes < MAX_RESUMES {
                resumes += 1;
                tracing::debug!(
                    "the model download broke at byte {} ({broke}); continuing",
                    self.have
                );
                continue;
            }
            return Err(SpeechError::Download(format!("the connection broke: {broke}")));
        }
    }

    /// Appends a response body to the file.
    fn copy(
        &mut self,
        body: &mut dyn Read,
        progress: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<(), Stop> {
        let mut buf = vec![0; CHUNK];
        let mut reported = self.have;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Stop::Cancelled);
            }
            let n = match body.read(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(Stop::Network(e)),
            };
            self.file.write_all(&buf[..n]).map_err(Stop::Disk)?;
            self.hasher.update(&buf[..n]);
            self.have += n as u64;
            if self.have - reported >= PROGRESS_STEP {
                reported = self.have;
                progress(self.have);
            }
        }
    }

    fn finish(self) -> Result<Digests, SpeechError> {
        self.file.sync_all().map_err(|e| disk("writing the model", &e))?;
        Ok(self.hasher.finish())
    }
}

/// SHA-1 and SHA-256 at once.
struct Hasher {
    sha1: sha1::Sha1,
    sha256: sha2::Sha256,
}

impl Hasher {
    fn new() -> Hasher {
        Hasher {
            sha1: sha1::Sha1::new(),
            sha256: sha2::Sha256::new(),
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        self.sha1.update(bytes);
        self.sha256.update(bytes);
    }

    /// Hashes the rest of `reader`; answers the bytes read.
    fn read_all(&mut self, reader: &mut dyn Read, cancel: &AtomicBool) -> Result<u64, SpeechError> {
        let mut buf = vec![0; CHUNK];
        let mut read = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(SpeechError::Cancelled);
            }
            match reader.read(&mut buf) {
                Ok(0) => return Ok(read),
                Ok(n) => {
                    self.update(&buf[..n]);
                    read += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(disk("reading the partial download", &e)),
            }
        }
    }

    fn finish(self) -> Digests {
        Digests {
            sha1: hex(&self.sha1.finalize()),
            sha256: hex(&self.sha256.finalize()),
        }
    }
}

/// A file's sums, lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Digests {
    sha1: String,
    sha256: String,
}

impl Digests {
    /// How these differ from the model's published sums, if they do.
    fn mismatch(&self, model: &ModelInfo) -> Option<String> {
        if !self.sha1.eq_ignore_ascii_case(model.sha1) {
            return Some(format!("its SHA-1 is {}, not {}", self.sha1, model.sha1));
        }
        match model.sha256 {
            Some(expected) if !self.sha256.eq_ignore_ascii_case(expected) => {
                Some(format!("its SHA-256 is {}, not {expected}", self.sha256))
            }
            _ => None,
        }
    }
}

fn hash_file(path: &Path) -> Result<Digests, SpeechError> {
    let mut file = File::open(path).map_err(|e| disk("reading the model", &e))?;
    let mut hasher = Hasher::new();
    hasher.read_all(&mut file, &AtomicBool::new(false))?;
    Ok(hasher.finish())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0xf)]));
    }
    out
}

/// What identifies a file's contents cheaply: size and modification time.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    size: u64,
    mtime_ns: u128,
}

impl Stamp {
    /// `None` where the file system keeps no modification time: then every
    /// [`ModelStore::path`] call hashes the file.
    fn of(meta: &fs::Metadata) -> Option<Stamp> {
        let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Stamp {
            size: meta.len(),
            mtime_ns: mtime.as_nanos(),
        })
    }
}

/// The digests recorded for a file, if the record is for this size and time.
fn read_marker(path: &Path, stamp: &Stamp) -> Option<Digests> {
    let text = fs::read_to_string(path).ok()?;
    let field = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix(' '))
            .map(str::trim)
    };
    let recorded = Stamp {
        size: field("size")?.parse().ok()?,
        mtime_ns: field("mtime-ns")?.parse().ok()?,
    };
    if recorded != *stamp {
        return None;
    }
    Some(Digests {
        sha1: field("sha1")?.to_owned(),
        sha256: field("sha256")?.to_owned(),
    })
}

fn write_marker(path: &Path, stamp: &Stamp, digests: &Digests) {
    let text = format!(
        "size {}\nmtime-ns {}\nsha1 {}\nsha256 {}\n",
        stamp.size, stamp.mtime_ns, digests.sha1, digests.sha256
    );
    // Without the record the next call hashes the file again; nothing else is lost.
    if let Err(e) = fs::write(path, text) {
        tracing::debug!("recording a verified speech model: {e}");
    }
}

/// Models downloading in this process, by file.
static BUSY: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// A claim on downloading one model file, released when dropped.
struct Busy(PathBuf);

impl Busy {
    fn claim(file: &Path) -> Result<Busy, SpeechError> {
        let mut busy = BUSY.lock().unwrap_or_else(PoisonError::into_inner);
        if busy.iter().any(|p| p == file) {
            return Err(SpeechError::Download(
                "this speech model is downloading already".into(),
            ));
        }
        busy.push(file.to_owned());
        Ok(Busy(file.to_owned()))
    }

    fn holds(file: &Path) -> bool {
        BUSY.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|p| p == file)
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        BUSY.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|p| *p != self.0);
    }
}

/// A `Content-Range` header: `bytes START-END/TOTAL`, or `bytes */TOTAL`
/// (with 416); TOTAL may be `*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContentRange {
    start: Option<u64>,
    total: Option<u64>,
}

fn parse_content_range(value: &str) -> Option<ContentRange> {
    let spec = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = spec.split_once('/')?;
    let total = match total.trim() {
        "*" => None,
        total => Some(total.parse().ok()?),
    };
    let start = match range.trim() {
        "*" => None,
        range => Some(range.split_once('-')?.0.trim().parse().ok()?),
    };
    Some(ContentRange { start, total })
}

fn refused(status: u16) -> SpeechError {
    SpeechError::Download(match status {
        404 | 410 => format!("the model is not on the server (HTTP {status})"),
        401 | 403 => format!("the server refused the download (HTTP {status})"),
        408 | 429 | 500..=599 => format!("the server is unavailable (HTTP {status}); try again later"),
        _ => format!("the server answered HTTP {status}"),
    })
}

fn disk(what: &str, e: &io::Error) -> SpeechError {
    SpeechError::Download(format!("{what}: {e}"))
}

/// The download agent: rustls with the `ring` provider and the operating
/// system's root certificates (as jf-engine's), proxies from the environment,
/// and redirects followed (Hugging Face answers with its CDN). Timeouts fail
/// a stalled connection; a download as a whole may take as long as it needs.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};
        let native = rustls_native_certs::load_native_certs();
        for err in &native.errors {
            tracing::debug!("loading root certificates: {err}");
        }
        let roots: Vec<Certificate<'static>> = native
            .certs
            .iter()
            .map(|der| Certificate::from_der(der.as_ref()).to_owned())
            .collect();
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .root_certs(RootCerts::new_with_certs(&roots))
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        ureq::Agent::config_builder()
            .tls_config(tls)
            .http_status_as_error(false)
            .user_agent(concat!("jellyfesh/", env!("CARGO_PKG_VERSION")))
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_per_read(Some(Duration::from_secs(30)))
            .build()
            .new_agent()
    })
}

#[cfg(test)]
mod tests;

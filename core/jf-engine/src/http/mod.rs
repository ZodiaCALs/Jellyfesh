//! HTTP range streaming (architecture §9).
//!
//! An [`HttpStream`] downloads one resource on its own thread with range requests and keeps the
//! bytes in a [`cache::Cache`]. [`HttpReader`]s give Symphonia a seekable view of it: a read of
//! cached bytes returns at once; a read past them asks the downloader for that position and
//! waits. The downloader stays about [`READAHEAD_SECS`] of audio ahead of the decoder's reader
//! (in bytes, from the stream's average bit rate once known), reconnects with exponential
//! backoff when a connection fails or ends early, and asks the [`StreamResolver`] for a fresh
//! URL shortly before `expires_at_ms` and after a 401, 403 or 410.
//!
//! The engine thread never waits on the network while playing: before decoding it checks
//! [`HttpStream::ready`] and, when too little is buffered, waits in short steps it can
//! interrupt (and reports buffering once the output has run dry).

mod cache;

use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::{Condvar, Mutex};

use self::cache::{BLOCK, Cache, SPILL_CAP};
use crate::types::{HttpSource, StreamResolver};

/// Read-ahead target, in seconds of audio.
pub(crate) const READAHEAD_SECS: u64 = 10;
/// Read-ahead before the bit rate is known.
const DEFAULT_READAHEAD: u64 = 1 << 20;
const MIN_READAHEAD: u64 = 256 << 10;
const MAX_READAHEAD: u64 = 64 << 20;
/// The least read-ahead playback resumes with after the output ran dry.
const RESUME_MIN: u64 = 64 << 10;
/// Bytes read from a connection at a time.
const CHUNK: usize = 32 << 10;
/// Ask for a new URL this long before it expires.
const REFRESH_MARGIN_MS: i64 = 30_000;
/// Retries of a failing request before the stream fails (about 26 s in all); delays double from
/// `BACKOFF_START` up to `BACKOFF_MAX`.
const MAX_RETRIES: u32 = 10;
const BACKOFF_START: Duration = Duration::from_millis(100);
const BACKOFF_MAX: Duration = Duration::from_secs(5);
/// An open connection with nothing to fetch is closed after this long.
const IDLE_CONNECTION: Duration = Duration::from_secs(20);
/// Re-resolves after a refusal (401, 403, 410) before giving up.
const MAX_REFUSAL_RESOLVES: u32 = 2;

/// Shared between the downloader thread, the readers and the engine.
#[derive(Debug)]
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Debug)]
struct State {
    cache: Cache,
    /// Resource length, once known.
    total: Option<u64>,
    /// Position of the decoder's reader; the read-ahead counts from here.
    primary_pos: u64,
    /// A position another reader waits for (header scans).
    scan_wanted: Option<u64>,
    readahead: u64,
    /// The stream failed for good: reads past the cached bytes fail with this.
    error: Option<String>,
    closed: bool,
    downloader_idle: bool,
    source: HttpSource,
    content_type: Option<String>,
    stats: StreamStats,
}

impl State {
    /// Where the downloader should fetch next, if anywhere.
    fn next_fetch(&self) -> Option<u64> {
        if let Some(pos) = self.scan_wanted {
            return self.cache.missing_from(pos, pos + 4 * BLOCK, self.total);
        }
        let limit = self.primary_pos.saturating_add(self.readahead);
        self.cache.missing_from(self.primary_pos, limit, self.total)
    }

    fn at_end(&self, pos: u64) -> bool {
        self.total.is_some_and(|total| pos >= total)
    }
}

/// Counters for tests and logs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StreamStats {
    /// Start offsets of the requests made, in order.
    pub requests: Vec<u64>,
    /// Requests that failed or connections that ended early.
    pub failures: u32,
    /// Calls to the resolver.
    pub resolves: u32,
    /// Bytes received.
    pub bytes: u64,
}

/// What a stream needs besides its source.
#[derive(Clone)]
pub(crate) struct StreamOptions {
    pub id: u64,
    pub cache_dir: Option<PathBuf>,
    pub resolver: Option<Arc<dyn StreamResolver>>,
}

/// A downloading resource. Cloning gives another handle to the same stream; the download stops
/// when the last handle (and reader) is gone.
#[derive(Debug, Clone)]
pub(crate) struct HttpStream {
    handle: Arc<Handle>,
}

#[derive(Debug)]
struct Handle {
    shared: Arc<Shared>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock();
        state.closed = true;
        self.shared.changed.notify_all();
    }
}

impl HttpStream {
    /// Starts downloading `source` from its beginning.
    pub(crate) fn open(source: HttpSource, options: StreamOptions) -> std::io::Result<Self> {
        let memory_cap = DEFAULT_READAHEAD * 2 + (2 << 20);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                cache: Cache::new(memory_cap, options.cache_dir.clone(), SPILL_CAP),
                total: source.size,
                primary_pos: 0,
                scan_wanted: None,
                readahead: DEFAULT_READAHEAD,
                error: None,
                closed: false,
                downloader_idle: false,
                content_type: None,
                source,
                stats: StreamStats::default(),
            }),
            changed: Condvar::new(),
        });
        let thread_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("jf-http".into())
            .spawn(move || Downloader::new(thread_shared, options).run())?;
        Ok(Self {
            handle: Arc::new(Handle { shared }),
        })
    }

    fn shared(&self) -> &Shared {
        &self.handle.shared
    }

    /// A reader from the start of the resource. The `primary` reader is the decoder's: the
    /// read-ahead follows it.
    pub(crate) fn reader(&self, primary: bool) -> HttpReader {
        HttpReader {
            stream: self.clone(),
            pos: 0,
            primary,
        }
    }

    /// Whether enough is buffered to resume after the output ran dry: a quarter of the
    /// read-ahead (about 2.5 s of audio), the rest of the resource, or a failure (which the
    /// decoder then reports). This keeps a slow connection from stuttering.
    pub(crate) fn ready_to_resume(&self) -> bool {
        let readahead = self.shared().state.lock().readahead;
        self.buffered_at_least((readahead / 4).max(RESUME_MIN))
    }

    fn buffered_at_least(&self, min_bytes: u64) -> bool {
        let state = self.shared().state.lock();
        let pos = state.primary_pos;
        let ahead = state.cache.contiguous_from(pos);
        state.error.is_some() || ahead >= min_bytes || state.total.is_some_and(|total| pos + ahead >= total)
    }

    /// Sets the read-ahead from the stream's average byte rate.
    pub(crate) fn set_byte_rate(&self, bytes_per_second: u64) {
        let readahead = (bytes_per_second.saturating_mul(READAHEAD_SECS)).clamp(MIN_READAHEAD, MAX_READAHEAD);
        let mut state = self.shared().state.lock();
        state.readahead = readahead;
        state.cache.set_memory_cap(readahead * 2 + (2 << 20));
        self.shared().changed.notify_all();
    }

    /// Resource length, when known.
    pub(crate) fn total(&self) -> Option<u64> {
        self.shared().state.lock().total
    }

    /// The MIME type: the source's, or the server's `Content-Type`.
    pub(crate) fn mime(&self) -> Option<String> {
        let state = self.shared().state.lock();
        state.source.mime.clone().or_else(|| state.content_type.clone())
    }

    /// The URL without its query, for messages.
    pub(crate) fn describe(&self) -> String {
        self.shared().state.lock().source.redacted_url().to_string()
    }

    /// The extension of the URL's path, as a probe hint.
    pub(crate) fn extension(&self) -> Option<String> {
        let state = self.shared().state.lock();
        let path = state.source.redacted_url();
        let name = path.rsplit('/').next()?;
        let (_, ext) = name.rsplit_once('.')?;
        (!ext.is_empty() && ext.len() <= 5).then(|| ext.to_ascii_lowercase())
    }

    /// The failure that ended the stream, if any.
    pub(crate) fn error(&self) -> Option<String> {
        self.shared().state.lock().error.clone()
    }
}

/// A seekable view of an [`HttpStream`] for Symphonia.
#[derive(Debug)]
pub(crate) struct HttpReader {
    stream: HttpStream,
    pos: u64,
    primary: bool,
}

impl Read for HttpReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let shared = self.stream.shared();
        let mut state = shared.state.lock();
        loop {
            if state.at_end(self.pos) {
                return Ok(0);
            }
            let n = state.cache.read_at(self.pos, buf)?;
            if n > 0 {
                self.pos += n as u64;
                if self.primary {
                    state.primary_pos = self.pos;
                    if state.downloader_idle {
                        shared.changed.notify_all();
                    }
                } else if state.scan_wanted.is_some() {
                    state.scan_wanted = None;
                    shared.changed.notify_all();
                }
                return Ok(n);
            }
            if let Some(error) = &state.error {
                return Err(io::Error::other(error.clone()));
            }
            if state.closed {
                return Err(io::Error::other("the stream was closed"));
            }
            if self.primary {
                state.primary_pos = self.pos;
            } else {
                state.scan_wanted = Some(self.pos);
            }
            shared.changed.notify_all();
            shared.changed.wait_for(&mut state, Duration::from_millis(100));
        }
    }
}

impl Drop for HttpReader {
    fn drop(&mut self) {
        if !self.primary {
            let shared = self.stream.shared();
            let mut state = shared.state.lock();
            if state.scan_wanted.take().is_some() {
                shared.changed.notify_all();
            }
        }
    }
}

impl Seek for HttpReader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let shared = self.stream.shared();
        let mut state = shared.state.lock();
        let target = match to {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta),
            SeekFrom::End(delta) => match state.total {
                Some(total) => total.checked_add_signed(delta),
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "stream length unknown",
                    ));
                }
            },
        };
        let target =
            target.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;
        self.pos = target;
        if self.primary && state.primary_pos != target {
            state.primary_pos = target;
            shared.changed.notify_all();
        }
        Ok(target)
    }
}

impl symphonia::core::io::MediaSource for HttpReader {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.stream.total()
    }
}

/// An open response body and the resource position of its next byte.
struct Connection {
    body: Box<dyn Read + Send>,
    pos: u64,
    /// The server ignored the range and sends the whole resource.
    whole: bool,
}

enum Failure {
    /// Worth retrying after a pause (network error, 5xx, connection ended early).
    Retry(String),
    /// The server refused the URL (401, 403, 410): resolve it again.
    Refused(String),
    /// Retrying will not help (404, other 4xx, a bad URL).
    Fatal(String),
}

struct Downloader {
    shared: Arc<Shared>,
    options: StreamOptions,
    connection: Option<Connection>,
    retries: u32,
    refusal_resolves: u32,
}

impl Downloader {
    fn new(shared: Arc<Shared>, options: StreamOptions) -> Self {
        Self {
            shared,
            options,
            connection: None,
            retries: 0,
            refusal_resolves: 0,
        }
    }

    fn run(mut self) {
        let mut buf = vec![0u8; CHUNK];
        loop {
            let Some(target) = self.next_target() else {
                let state = self.shared.state.lock();
                let stats = &state.stats;
                tracing::debug!(
                    "stream {} closed: {} request(s) from {:?}, {} failure(s), {} resolve(s), {} bytes, {} spilled",
                    self.options.id,
                    stats.requests.len(),
                    stats.requests.first(),
                    stats.failures,
                    stats.resolves,
                    stats.bytes,
                    state.cache.spilled_bytes()
                );
                return;
            };
            if !self.connection.as_ref().is_some_and(|c| c.serves(target)) {
                self.connection = None;
                match self.connect(target) {
                    Ok(Some(connection)) => self.connection = Some(connection),
                    // The resource ends before `target`.
                    Ok(None) => {}
                    Err(Failure::Fatal(message)) => self.fail(message),
                    Err(Failure::Refused(message)) => {
                        if self.refusal_resolves < MAX_REFUSAL_RESOLVES && self.resolve() {
                            self.refusal_resolves += 1;
                        } else {
                            self.fail(message);
                        }
                    }
                    Err(Failure::Retry(message)) => self.retry_later(message),
                }
                continue;
            }
            let Some(connection) = self.connection.as_mut() else {
                continue;
            };
            match connection.body.read(&mut buf) {
                Ok(0) => {
                    let end = connection.pos;
                    self.connection = None;
                    let mut state = self.shared.state.lock();
                    match state.total {
                        Some(total) if end < total => {
                            drop(state);
                            self.retry_later(format!("the connection ended at byte {end} of {total}"));
                        }
                        _ => {
                            state.total = Some(end);
                            self.shared.changed.notify_all();
                        }
                    }
                }
                Ok(n) => {
                    let pos = connection.pos;
                    connection.pos += n as u64;
                    let whole = connection.whole;
                    self.retries = 0;
                    let mut state = self.shared.state.lock();
                    state.stats.bytes += n as u64;
                    state.cache.write_at(pos, &buf[..n]);
                    let keep_from = state.primary_pos.saturating_sub(BLOCK * 4);
                    let keep_to = state.primary_pos.saturating_add(state.readahead);
                    let total = state.total;
                    state.cache.evict(keep_from, keep_to, total);
                    // A ranged connection that no longer continues where the reader needs data
                    // (it seeked, or the bytes ahead are cached) is dropped; one that only ran
                    // into the end of the read-ahead window is kept for when the reader moves on.
                    let next = pos + n as u64;
                    if !whole && state.next_fetch().is_some_and(|wanted| wanted != next) {
                        self.connection = None;
                    }
                    self.shared.changed.notify_all();
                }
                Err(err) => {
                    self.connection = None;
                    self.retry_later(format!("reading the response: {err}"));
                }
            }
        }
    }

    /// Waits until there is something to fetch; `None` when the stream is closed. An open
    /// connection is kept while waiting (TCP holds the server back) until it has been idle for
    /// [`IDLE_CONNECTION`].
    fn next_target(&mut self) -> Option<u64> {
        let shared = Arc::clone(&self.shared);
        let mut state = shared.state.lock();
        let mut idle_since: Option<Instant> = None;
        loop {
            if state.closed {
                return None;
            }
            if state.error.is_none()
                && let Some(target) = state.next_fetch()
            {
                return Some(target);
            }
            state.downloader_idle = true;
            if self.connection.is_some() {
                let since = *idle_since.get_or_insert_with(Instant::now);
                let left = IDLE_CONNECTION.saturating_sub(since.elapsed());
                if left.is_zero() {
                    self.connection = None;
                } else {
                    let _ = shared.changed.wait_for(&mut state, left);
                }
            } else {
                shared.changed.wait(&mut state);
            }
            state.downloader_idle = false;
        }
    }

    fn fail(&mut self, message: String) {
        tracing::warn!("stream {}: {message}", self.options.id);
        self.connection = None;
        let mut state = self.shared.state.lock();
        state.error = Some(message);
        self.shared.changed.notify_all();
    }

    /// Counts a failure and sleeps with exponential backoff; fails the stream after too many.
    fn retry_later(&mut self, message: String) {
        self.retries += 1;
        {
            let mut state = self.shared.state.lock();
            state.stats.failures += 1;
        }
        if self.retries > MAX_RETRIES {
            self.fail(format!("{message} (gave up after {MAX_RETRIES} retries)"));
            return;
        }
        let delay = BACKOFF_START
            .saturating_mul(1 << (self.retries - 1).min(16))
            .min(BACKOFF_MAX);
        tracing::debug!("stream {}: {message}; retrying in {delay:?}", self.options.id);
        let deadline = Instant::now() + delay;
        let mut state = self.shared.state.lock();
        while !state.closed && Instant::now() < deadline {
            let _ = self.shared.changed.wait_until(&mut state, deadline);
        }
    }

    /// Asks the resolver for a fresh source. Returns whether it gave one.
    fn resolve(&mut self) -> bool {
        let Some(resolver) = self.options.resolver.clone() else {
            return false;
        };
        self.shared.state.lock().stats.resolves += 1;
        match resolver.resolve(self.options.id) {
            Ok(source) => {
                tracing::debug!(
                    "stream {}: resolved again to {}",
                    self.options.id,
                    source.redacted_url()
                );
                let mut state = self.shared.state.lock();
                if let Some(size) = source.size
                    && state.total.is_none()
                {
                    state.total = Some(size);
                }
                state.source = source;
                true
            }
            Err(message) => {
                tracing::warn!(
                    "stream {}: resolving the URL again failed: {message}",
                    self.options.id
                );
                false
            }
        }
    }

    /// Opens a request for the resource from `pos`. `Ok(None)` when `pos` is past the end.
    fn connect(&mut self, pos: u64) -> Result<Option<Connection>, Failure> {
        let expires = self.shared.state.lock().source.expires_at_ms;
        if let Some(expires) = expires
            && now_ms() + REFRESH_MARGIN_MS >= expires
        {
            self.resolve();
        }
        let source = {
            let mut state = self.shared.state.lock();
            state.stats.requests.push(pos);
            state.source.clone()
        };
        let mut request = agent()
            .get(&source.url)
            .header("Range", format!("bytes={pos}-"))
            .header("Accept-Encoding", "identity");
        for (name, value) in &source.headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::BadUri(message)) => return Err(Failure::Fatal(format!("bad URL: {message}"))),
            Err(err) => return Err(Failure::Retry(err.to_string())),
        };
        let status = response.status().as_u16();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let content_range = header("content-range");
        let content_length = header("content-length").and_then(|v| v.trim().parse::<u64>().ok());
        let content_type = header("content-type");
        match status {
            206 => {
                let (start, total) = content_range
                    .as_deref()
                    .and_then(parse_content_range)
                    .ok_or_else(|| Failure::Retry("206 without a usable Content-Range".into()))?;
                let mut state = self.shared.state.lock();
                if let Some(total) = total {
                    state.total = Some(total);
                }
                if state.content_type.is_none() {
                    state.content_type = content_type;
                }
                drop(state);
                if start > pos {
                    return Err(Failure::Retry(format!("asked for byte {pos}, got {start}")));
                }
                let mut connection = Connection {
                    body: Box::new(response.into_body().into_reader()),
                    pos: start,
                    whole: false,
                };
                // A server may start a little earlier than asked; skip to `pos`.
                connection
                    .skip_to(pos)
                    .map_err(|e| Failure::Retry(e.to_string()))?;
                Ok(Some(connection))
            }
            200 => {
                let mut state = self.shared.state.lock();
                if let Some(length) = content_length {
                    state.total = Some(length);
                }
                if state.content_type.is_none() {
                    state.content_type = content_type;
                }
                drop(state);
                if pos > 0 {
                    tracing::debug!(
                        "stream {}: the server ignores ranges; reading from the start",
                        self.options.id
                    );
                }
                Ok(Some(Connection {
                    body: Box::new(response.into_body().into_reader()),
                    pos: 0,
                    whole: true,
                }))
            }
            416 => {
                let total = content_range
                    .as_deref()
                    .and_then(|v| v.rsplit('/').next())
                    .and_then(|t| t.trim().parse::<u64>().ok());
                let mut state = self.shared.state.lock();
                if let Some(total) = total {
                    state.total = Some(total);
                } else if state.total.is_none() {
                    state.total = Some(pos);
                }
                self.shared.changed.notify_all();
                Ok(None)
            }
            401 | 403 | 410 => Err(Failure::Refused(format!(
                "the server refused the URL (HTTP {status})"
            ))),
            404 => Err(Failure::Fatal("not found (HTTP 404)".into())),
            408 | 425 | 429 | 500..=599 => Err(Failure::Retry(format!("HTTP {status}"))),
            _ => Err(Failure::Fatal(format!("HTTP {status}"))),
        }
    }
}

impl Connection {
    /// Whether reading on gives the bytes at `target` without a new request.
    fn serves(&self, target: u64) -> bool {
        self.pos == target || (self.whole && self.pos <= target)
    }

    /// Reads and drops bytes until `pos` (a range response that starts early).
    fn skip_to(&mut self, pos: u64) -> io::Result<()> {
        if pos > self.pos {
            let skip = pos - self.pos;
            let copied = io::copy(&mut (&mut self.body).take(skip), &mut io::sink())?;
            self.pos += copied;
            if copied < skip {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
        }
        Ok(())
    }
}

/// `bytes START-END/TOTAL` (TOTAL may be `*`): the start and, when given, the total.
fn parse_content_range(value: &str) -> Option<(u64, Option<u64>)> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = rest.split_once('/')?;
    let (start, _end) = range.split_once('-')?;
    let start = start.trim().parse().ok()?;
    let total = total.trim().parse().ok();
    Some((start, total))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// The process-wide HTTP agent: rustls with the `ring` provider and the operating system's root
/// certificates (no bundled root store), proxies from the environment, and timeouts that fail a
/// stalled connection so the stream can reconnect.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};
        let native = rustls_native_certs::load_native_certs();
        for err in &native.errors {
            tracing::debug!("loading root certificates: {err}");
        }
        if native.certs.is_empty() {
            tracing::warn!("no root certificates found; HTTPS streams will fail");
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
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_response(Some(Duration::from_secs(15)))
            .timeout_per_read(Some(Duration::from_secs(15)))
            .build()
            .new_agent()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_ranges_parse() {
        assert_eq!(parse_content_range("bytes 0-99/1000"), Some((0, Some(1000))));
        assert_eq!(parse_content_range("bytes 500-999/*"), Some((500, None)));
        assert_eq!(parse_content_range(" bytes  7-8/9 "), Some((7, Some(9))));
        assert_eq!(parse_content_range("items 0-1/2"), None);
        assert_eq!(parse_content_range("bytes */1000"), None);
    }
}

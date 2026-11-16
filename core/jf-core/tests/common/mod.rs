//! Helpers for the online-source tests: building the test plugins into
//! components and packages, a local HTTP test server, generated audio and
//! pictures, and a core in a temporary folder that trusts the test key.
//! Everything the "services" answer is made up here.

#![allow(dead_code, reason = "each test file uses a different part of this module")]

use std::f64::consts::PI;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use flacenc::component::BitRepr;
use flacenc::error::Verify;
use jf_core::{
    Command, Config, Core, Event, HostOptions, Limits, MemorySecretStore, OutputKind, PluginSummary,
    SecretStore, SourcesSnapshot, TrustList,
};
use jf_plugin_host::package::PackageBuilder;
use jf_plugin_host::trust::test_key_seed;

pub const ECHO: &str = "org.jellyfesh.test.echo";
pub const SPIN: &str = "org.jellyfesh.test.spin";
pub const CATALOG: &str = "org.jellyfesh.test.catalog";

/// Test plugin crates by short name.
const PLUGINS: &[(&str, &str)] = &[
    ("echo", "jf-test-echo"),
    ("spin", "jf-test-spin"),
    ("catalog", "jf-core-test-catalog"),
    ("remote", "jf-test-remote"),
];

/// The cargo target directory this test binary was built in.
pub fn target_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    // <target>/<profile>/deps/<test binary>
    exe.ancestors().nth(3).expect("target directory").to_path_buf()
}

/// A test plugin as a component, built once per test binary and cached in
/// `target/test-plugins/` (shared with the plugin host's tests).
pub fn component(name: &str) -> Vec<u8> {
    build_all();
    std::fs::read(target_dir().join("test-plugins").join(format!("{name}.wasm"))).expect("cached component")
}

fn build_all() {
    static BUILT: OnceLock<()> = OnceLock::new();
    BUILT.get_or_init(|| {
        let started = Instant::now();
        let target = target_dir();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Process::new(cargo);
        command
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args([
                "build",
                "--quiet",
                "--target",
                "wasm32-unknown-unknown",
                "--target-dir",
            ])
            .arg(&target)
            .env(
                "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS",
                "--cfg getrandom_backend=\"custom\"",
            );
        for (_, package) in PLUGINS {
            command.args(["--package", package]);
        }
        let status = command.status().expect("running cargo");
        assert!(
            status.success(),
            "building the test plugins failed (they need the wasm32-unknown-unknown target)"
        );
        let cache = target.join("test-plugins");
        std::fs::create_dir_all(&cache).unwrap();
        for (name, package) in PLUGINS {
            let module_path = target
                .join("wasm32-unknown-unknown/debug")
                .join(format!("{}.wasm", package.replace('-', "_")));
            let cached = cache.join(format!("{name}.wasm"));
            let fresh = match (std::fs::metadata(&module_path), std::fs::metadata(&cached)) {
                (Ok(module), Ok(component)) => component.modified().unwrap() >= module.modified().unwrap(),
                _ => false,
            };
            if !fresh {
                let module = std::fs::read(&module_path).unwrap();
                let component = wit_component::ComponentEncoder::default()
                    .module(&module)
                    .unwrap()
                    .validate(true)
                    .encode()
                    .unwrap_or_else(|e| panic!("componentising {name}: {e:#}"));
                std::fs::write(&cached, component).unwrap();
            }
        }
        eprintln!("test plugins ready in {:?}", started.elapsed());
    });
}

/// `settings.schema.json` of the test plugins.
pub const SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "server_url": { "type": "string", "format": "uri", "title": "Server" }
    }
}"#;

/// A manifest for a test plugin; the allowlist is the configured server.
pub fn manifest(id: &str, capabilities: &[&str], auth: &[&str]) -> String {
    let list = |items: &[&str]| {
        items
            .iter()
            .map(|i| format!("{i:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut text = format!(
        "id = {id:?}\nname = \"Test {id}\"\nversion = \"1.0.0\"\napi_version = \"1.0\"\n\
         author = \"Jellyfesh tests\"\ndescription = \"A plugin for tests\"\n\
         capabilities = [{}]\nplayback_mode = \"stream\"\n",
        list(capabilities)
    );
    if !auth.is_empty() {
        text += &format!("auth = [{}]\n", list(auth));
    }
    text += "\n[network]\nallow = [\"${settings.server_url}\"]\nmax_rps = 20\n";
    text += "\n[settings]\nschema = \"settings.schema.json\"\n";
    text
}

pub fn echo_manifest() -> String {
    manifest(
        ECHO,
        &[
            "auth",
            "search",
            "browse",
            "resolve_stream",
            "lyrics",
            "artwork",
            "recommendations",
        ],
        &["qr", "oauth", "password-exchange", "api-key"],
    )
}

pub fn spin_manifest() -> String {
    manifest(SPIN, &["search", "browse", "lyrics", "recommendations"], &[])
}

pub fn catalog_manifest() -> String {
    manifest(
        CATALOG,
        &["search", "browse", "resolve_stream", "lyrics", "artwork"],
        &[],
    )
}

/// The manifest with another version.
pub fn with_version(manifest: &str, version: &str) -> String {
    assert!(manifest.contains("version = \"1.0.0\"\n"));
    manifest.replacen("version = \"1.0.0\"\n", &format!("version = {version:?}\n"), 1)
}

/// Writes a `.jfpkg` named `file` (signed with the published test key when
/// `signed`).
pub fn package_as(dir: &Path, file: &str, name: &str, manifest: &str, signed: bool) -> PathBuf {
    let mut builder = PackageBuilder::new()
        .manifest(manifest)
        .wasm(component(name))
        .file("settings.schema.json", SCHEMA.as_bytes().to_vec());
    if signed {
        builder = builder.sign(test_key_seed());
    }
    let path = dir.join(file);
    std::fs::write(&path, builder.build().unwrap()).unwrap();
    path
}

/// Writes a `.jfpkg` (signed with the published test key when `signed`).
pub fn package(dir: &Path, name: &str, manifest: &str, signed: bool, icon: Option<Vec<u8>>) -> PathBuf {
    let mut builder = PackageBuilder::new()
        .manifest(manifest)
        .wasm(component(name))
        .file("settings.schema.json", SCHEMA.as_bytes().to_vec());
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }
    if signed {
        builder = builder.sign(test_key_seed());
    }
    let path = dir.join(format!(
        "{name}-{}.jfpkg",
        if signed { "signed" } else { "unsigned" }
    ));
    std::fs::write(&path, builder.build().unwrap()).unwrap();
    path
}

/// A small PNG of one colour.
pub fn png(rgb: [u8; 3], size: u32) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(size, size, image::Rgb(rgb));
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

pub const RATE: u32 = 44_100;

/// A sine as 16-bit stereo FLAC.
pub fn sine_flac(freq: f64, seconds: f64) -> Vec<u8> {
    let frames = (f64::from(RATE) * seconds) as usize;
    let samples: Vec<i32> = (0..frames)
        .flat_map(|n| {
            let v = (0.25 * (2.0 * PI * freq * n as f64 / f64::from(RATE)).sin() * 32767.0).round() as i32;
            [v, v]
        })
        .collect();
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .expect("flac config");
    let source = flacenc::source::MemSource::from_samples(&samples, 2, 16, RATE as usize);
    let block_size = config.block_size;
    let mut stream = flacenc::encode_with_fixed_block_size(&config, source, block_size).expect("flac encode");
    // State the fixed block size, as libFLAC does (see the engine's tests).
    stream
        .stream_info_mut()
        .set_block_sizes(block_size, block_size)
        .expect("block sizes");
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).expect("flac write");
    sink.as_slice().to_vec()
}

/// A 16-bit stereo sine WAV file.
pub fn sine_wav(path: &Path, freq: f32, seconds: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let frames = (RATE as f32 * seconds) as u32;
    for i in 0..frames {
        let v = ((std::f32::consts::TAU * freq * i as f32 / RATE as f32).sin() * 0.25 * f32::from(i16::MAX))
            as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}

/// Reads a rendered 32-bit float WAV: (rate, left channel).
pub fn read_wav(path: &Path) -> (u32, Vec<f32>) {
    let mut reader = hound::WavReader::open(path).unwrap();
    let rate = reader.spec().sample_rate;
    let channels = usize::from(reader.spec().channels);
    let samples: Vec<f32> = reader.samples::<f32>().map(Result::unwrap).collect();
    (rate, samples.iter().step_by(channels).copied().collect())
}

/// The frequency of a sine from its zero crossings.
pub fn frequency(x: &[f32], rate: u32) -> f64 {
    let crossings = x.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
    crossings as f64 / 2.0 / (x.len() as f64 / f64::from(rate))
}

// ---- the core -------------------------------------------------------------

/// A core in a temporary folder whose plugin host trusts the test key and
/// keeps its secrets in memory.
pub struct Env {
    pub dir: tempfile::TempDir,
    pub core: Core,
    pub events: Receiver<Event>,
    pub secrets: Arc<MemorySecretStore>,
    pub out: PathBuf,
}

/// Limits for tests: production CPU and memory, shorter deadlines.
pub fn test_limits() -> Limits {
    Limits {
        wall: Duration::from_secs(5),
        wall_login: Duration::from_secs(8),
        ..Limits::default()
    }
}

impl Env {
    pub fn new() -> Env {
        Env::with(|_| {})
    }

    /// A core whose config `change` adjusts first.
    pub fn with(change: impl FnOnce(&mut Config)) -> Env {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemorySecretStore::new());
        let out = dir.path().join("out.wav");
        let config = config(dir.path(), &out, secrets.clone(), change);
        let core = Core::start(config).unwrap();
        let events = core.subscribe();
        Env {
            dir,
            core,
            events,
            secrets,
            out,
        }
    }

    /// Stops the core and starts another on the same folders.
    pub fn restart(self) -> Env {
        self.restart_with(|_| {})
    }

    /// Stops the core and starts another on the same folders, with its
    /// config adjusted by `change`.
    pub fn restart_with(self, change: impl FnOnce(&mut Config)) -> Env {
        let Env {
            dir,
            core,
            secrets,
            out,
            ..
        } = self;
        core.shutdown();
        let core = Core::start(config(dir.path(), &out, secrets.clone(), change)).unwrap();
        let events = core.subscribe();
        Env {
            dir,
            core,
            events,
            secrets,
            out,
        }
    }

    pub fn incoming(&self) -> PathBuf {
        let dir = self.dir.path().join("incoming");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Sends a command and waits until `done` sees an event; fails on the
    /// command's `CommandFailed`.
    pub fn run(&self, command: Command, done: impl FnMut(&Event) -> bool) -> Vec<Event> {
        let id = self.core.send(command);
        let mut done = done;
        wait_for(&self.events, Duration::from_secs(60), |e| {
            if let Event::CommandFailed { id: failed, error } = e
                && *failed == id
            {
                panic!("command failed: {error}");
            }
            done(e)
        })
    }

    /// Sends a command that must fail and returns its error.
    pub fn fails(&self, command: Command) -> String {
        let id = self.core.send(command);
        let events = wait_for(
            &self.events,
            Duration::from_secs(60),
            |e| matches!(e, Event::CommandFailed { id: failed, .. } if *failed == id),
        );
        match events.last() {
            Some(Event::CommandFailed { error, .. }) => error.clone(),
            _ => unreachable!(),
        }
    }

    /// Installs a test plugin package and returns its summary once the
    /// `Sources` slice shows it.
    pub fn install(
        &self,
        name: &str,
        manifest: &str,
        id: &str,
        signed: bool,
        icon: Option<Vec<u8>>,
    ) -> PluginSummary {
        let path = package(&self.incoming(), name, manifest, signed, icon);
        let events = self.run(Command::InstallPlugin { path }, |e| has_plugin(e, id));
        plugin_in(events.last().unwrap(), id).unwrap()
    }

    /// Points a plugin at a server.
    pub fn set_server(&self, id: &str, server: &str) {
        let settings_json = serde_json::json!({ "server_url": server }).to_string();
        self.run(
            Command::SetPluginSettings {
                id: id.into(),
                settings_json,
            },
            |e| matches!(e, Event::Sources(s) if s.plugins.iter().any(|p| p.id == id && p.settings_json.contains(server))),
        );
    }

    /// Installs a signed test plugin and points it at `server`.
    pub fn plugin(&self, name: &str, manifest: &str, id: &str, server: &str) -> PluginSummary {
        self.install(name, manifest, id, true, None);
        self.set_server(id, server);
        self.summary(id)
    }

    pub fn sources(&self) -> Arc<SourcesSnapshot> {
        self.core.snapshot().sources
    }

    pub fn summary(&self, id: &str) -> PluginSummary {
        self.sources()
            .plugins
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .unwrap_or_else(|| panic!("{id} is not installed"))
    }
}

/// The test config: a WAV file output rendered as fast as possible, no
/// session, the test key trusted, secrets in `secrets`.
pub fn config(
    dir: &Path,
    out: &Path,
    secrets: Arc<MemorySecretStore>,
    change: impl FnOnce(&mut Config),
) -> Config {
    let mut c = Config::new(dir.join("data"), dir.join("cache"));
    c.output = OutputKind::WavFile {
        path: out.to_path_buf(),
        realtime: false,
    };
    c.persist_session = false;
    c.restore_session = false;
    c.watch_folders = false;
    c.sources.secrets = Some(secrets as Arc<dyn SecretStore>);
    c.sources.host = HostOptions {
        trust: TrustList::builtin().with_test_key(),
        limits: test_limits(),
        ..HostOptions::default()
    };
    change(&mut c);
    c
}

pub fn has_plugin(event: &Event, id: &str) -> bool {
    matches!(event, Event::Sources(s) if s.plugins.iter().any(|p| p.id == id))
}

pub fn plugin_in(event: &Event, id: &str) -> Option<PluginSummary> {
    match event {
        Event::Sources(s) => s.plugins.iter().find(|p| p.id == id).cloned(),
        _ => None,
    }
}

/// Collects events until `done` returns true or the timeout passes.
pub fn wait_for(rx: &Receiver<Event>, timeout: Duration, mut done: impl FnMut(&Event) -> bool) -> Vec<Event> {
    let end = Instant::now() + timeout;
    let mut seen = Vec::new();
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(ev) => {
                let stop = done(&ev);
                seen.push(ev);
                if stop {
                    return seen;
                }
            }
            Err(_) => break,
        }
    }
    let summary: Vec<String> = seen.iter().map(short).collect();
    panic!("timed out; events seen: {summary:#?}");
}

/// One line per event, for failure messages.
pub fn short(event: &Event) -> String {
    match event {
        Event::Playback(p) => format!("Playback {:?}", p.status),
        Event::Queue(q) => format!("Queue len {} at {:?}", q.len, q.position),
        Event::NowPlaying(n) => format!("NowPlaying {:?}", n.as_ref().map(|n| n.title.clone())),
        Event::SearchResults(s) => format!(
            "SearchResults {} from {} ({} items, done {}, error {:?})",
            s.request.0,
            s.source,
            s.items.len(),
            s.done,
            s.error
        ),
        Event::Sources(s) => format!(
            "Sources v{}: {:?}",
            s.version,
            s.plugins
                .iter()
                .map(|p| format!("{} on={} account={:?}", p.id, p.enabled, p.account))
                .collect::<Vec<_>>()
        ),
        other => format!("{other:?}").chars().take(300).collect(),
    }
}

// ---- the test server ---------------------------------------------------------

/// A request the test server received.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    /// Path with query.
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// When it arrived.
    pub at: Instant,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }

    pub fn query(&self, key: &str) -> Option<String> {
        let url = url::Url::parse(&format!("http://x{}", self.target)).ok()?;
        url.query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    }

    pub fn form(&self, key: &str) -> Option<String> {
        url::form_urlencoded::parse(&self.body)
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    }
}

/// What the test server answers.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub delay: Duration,
}

impl Response {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Response {
        Response {
            status,
            headers: Vec::new(),
            body: body.into(),
            delay: Duration::ZERO,
        }
    }

    pub fn json(value: serde_json::Value) -> Response {
        Response::new(200, value.to_string()).header("content-type", "application/json")
    }

    pub fn header(mut self, name: &str, value: &str) -> Response {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// `bytes` served with range requests, as an audio server does.
    pub fn ranged(request: &Request, bytes: &[u8], content_type: &str) -> Response {
        let start = request
            .header("range")
            .and_then(|r| r.strip_prefix("bytes="))
            .and_then(|r| r.split('-').next())
            .and_then(|s| s.parse::<usize>().ok());
        match start {
            Some(start) if start < bytes.len() => Response::new(206, bytes[start..].to_vec())
                .header("content-type", content_type)
                .header("accept-ranges", "bytes")
                .header(
                    "content-range",
                    &format!("bytes {start}-{}/{}", bytes.len() - 1, bytes.len()),
                ),
            Some(_) => Response::new(416, "").header("content-range", &format!("bytes */{}", bytes.len())),
            None => Response::new(200, bytes.to_vec())
                .header("content-type", content_type)
                .header("accept-ranges", "bytes"),
        }
    }
}

type Handler = dyn Fn(&Request) -> Response + Send + Sync;

/// A tiny threaded HTTP/1.1 server on 127.0.0.1. Every connection is
/// answered once and closed.
pub struct TestServer {
    addr: SocketAddr,
    log: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl TestServer {
    pub fn start(handler: impl Fn(&Request) -> Response + Send + Sync + 'static) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handler: Arc<Handler> = Arc::new(handler);
        let thread = {
            let (log, stop) = (log.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let (log, handler) = (log.clone(), handler.clone());
                            std::thread::spawn(move || serve(stream, &log, &*handler));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(5)),
                    }
                }
            })
        };
        TestServer {
            addr,
            log,
            stop,
            thread: Some(thread),
        }
    }

    /// `http://127.0.0.1:<port>`.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn requests(&self) -> Vec<Request> {
        self.log.lock().unwrap().clone()
    }

    /// Requests whose path starts with `prefix`.
    pub fn with_path(&self, prefix: &str) -> Vec<Request> {
        self.requests()
            .into_iter()
            .filter(|r| r.path().starts_with(prefix))
            .collect()
    }

    pub fn count(&self, prefix: &str) -> usize {
        self.with_path(prefix).len()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(stream: TcpStream, log: &Mutex<Vec<Request>>, handler: &Handler) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return;
    }
    let mut parts = line.split_whitespace();
    let (method, target) = (
        parts.next().unwrap_or_default().to_string(),
        parts.next().unwrap_or_default().to_string(),
    );
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.trim_end().split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    let length: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    let request = Request {
        method,
        target,
        headers,
        body,
        at: Instant::now(),
    };
    log.lock().unwrap().push(request.clone());
    let response = handler(&request);
    std::thread::sleep(response.delay);
    let mut stream = reader.into_inner();
    let _ = stream.set_nodelay(true);
    let mut head = format!("HTTP/1.1 {} Test\r\nconnection: close\r\n", response.status);
    for (name, value) in &response.headers {
        head += &format!("{name}: {value}\r\n");
    }
    head += &format!("content-length: {}\r\n\r\n", response.body.len());
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    for chunk in response.body.chunks(64 * 1024) {
        if stream.write_all(chunk).is_err() {
            return;
        }
    }
    let _ = stream.flush();
}

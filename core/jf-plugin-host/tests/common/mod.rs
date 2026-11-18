//! Shared helpers for the plugin-host integration tests: building the test
//! plugins, packaging them, a local HTTP test server, an injectable DNS
//! resolver, and a host in a temporary directory.

#![allow(dead_code, reason = "each test file uses a different part of this module")]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use jf_plugin_host::package::PackageBuilder;
use jf_plugin_host::trust::{TrustList, test_key_seed};
use jf_plugin_host::{
    CallError, HostConfig, HostOptions, Item, ItemKind, Limits, MemorySecretStore, PageRequest, PluginHost,
    Reply, Resolve, SearchFilters,
};

/// Every test plugin crate, by short name.
pub const PLUGINS: &[&str] = &[
    "echo",
    "spin",
    "memory-bomb",
    "net-attacker",
    "secret-thief",
    "panicker",
    "api-mismatch",
    "remote",
    "compat-1-0",
    "compat-1-1",
    "maker",
    "appearance",
    "discover",
];

/// The cargo target directory this test binary was built in.
pub fn target_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    // <target>/<profile>/deps/<test binary>
    exe.ancestors().nth(3).expect("target directory").to_path_buf()
}

/// The core module of a test plugin (before componentisation).
pub fn core_module(name: &str) -> Vec<u8> {
    build_all();
    let file = format!("jf_test_{}.wasm", name.replace('-', "_"));
    std::fs::read(target_dir().join("wasm32-unknown-unknown/debug").join(file)).expect("built module")
}

/// The component of a test plugin. Built once per test binary (cargo is
/// incremental, so repeated runs are quick) and cached in
/// `target/test-plugins/`.
pub fn component(name: &str) -> Vec<u8> {
    build_all();
    std::fs::read(target_dir().join("test-plugins").join(format!("{name}.wasm"))).expect("cached component")
}

fn build_all() {
    static BUILT: OnceLock<()> = OnceLock::new();
    BUILT.get_or_init(|| {
        let started = Instant::now();
        let target = target_dir();
        check_wasm_target();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(cargo);
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
        for name in PLUGINS {
            command.args(["--package", &format!("jf-test-{name}")]);
        }
        let status = command.status().expect("running cargo");
        assert!(status.success(), "building the test plugins failed");
        let cache = target.join("test-plugins");
        std::fs::create_dir_all(&cache).unwrap();
        for name in PLUGINS {
            let module_path = target
                .join("wasm32-unknown-unknown/debug")
                .join(format!("jf_test_{}.wasm", name.replace('-', "_")));
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

fn check_wasm_target() {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--print", "target-libdir", "--target", "wasm32-unknown-unknown"])
        .output()
        .expect("running rustc");
    let libdir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(
        output.status.success() && Path::new(&libdir).is_dir(),
        "the plugin-host tests build plugins for wasm32-unknown-unknown; install it with \
         `rustup target add wasm32-unknown-unknown`"
    );
}

/// `settings.schema.json` of the test plugins.
pub const SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "server_url": { "type": "string", "format": "uri", "title": "Server" }
    }
}"#;

/// A manifest for a test plugin.
pub fn manifest(id: &str, capabilities: &[&str], auth: &[&str], allow: &[&str], max_rps: u32) -> String {
    let list = |items: &[&str]| {
        items
            .iter()
            .map(|i| format!("{i:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut text = format!(
        "id = {id:?}\nname = \"Test {id}\"\nversion = \"1.0.0\"\napi_version = \"1.0\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [{}]\nplayback_mode = \"stream\"\n",
        list(capabilities)
    );
    if !auth.is_empty() {
        text += &format!("auth = [{}]\n", list(auth));
    }
    text += &format!("\n[network]\nallow = [{}]\nmax_rps = {max_rps}\n", list(allow));
    text += "\n[settings]\nschema = \"settings.schema.json\"\n";
    if auth.contains(&"cookie-import") {
        text += "\n[cookie_import]\nurl = \"${settings.server_url}\"\nnames = [\"session\"]\n";
    }
    text
}

/// The echo plugin's id and manifest.
pub const ECHO: &str = "org.jellyfesh.test.echo";

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
        &["qr", "oauth", "cookie-import", "password-exchange", "api-key"],
        &["${settings.server_url}"],
        20,
    )
}

/// Writes a `.jfpkg` (signed with the test key when `signed`).
pub fn package(dir: &Path, file: &str, manifest: &str, wasm: Vec<u8>, signed: bool) -> PathBuf {
    let mut builder = PackageBuilder::new()
        .manifest(manifest)
        .wasm(wasm)
        .file("settings.schema.json", SCHEMA.as_bytes().to_vec());
    if signed {
        builder = builder.sign(test_key_seed());
    }
    let path = dir.join(file);
    std::fs::write(&path, builder.build().unwrap()).unwrap();
    path
}

/// Writes a `.jfpkg` with its own settings schema and extra entries
/// (translations), signed with the test key.
pub fn package_with(
    dir: &Path,
    file: &str,
    manifest: &str,
    wasm: Vec<u8>,
    schema: &str,
    extra: &[(&str, &str)],
) -> PathBuf {
    let mut builder = PackageBuilder::new()
        .manifest(manifest)
        .wasm(wasm)
        .file("settings.schema.json", schema.as_bytes().to_vec());
    for (name, text) in extra {
        builder = builder.file(name, text.as_bytes().to_vec());
    }
    let path = dir.join(file);
    std::fs::write(&path, builder.sign(test_key_seed()).build().unwrap()).unwrap();
    path
}

/// A host in a temporary directory, trusting the test key.
pub struct Env {
    pub dir: tempfile::TempDir,
    pub host: PluginHost,
    pub secrets: Arc<MemorySecretStore>,
    pub resolver: Arc<TestResolver>,
}

/// Limits for tests: production CPU and memory, shorter deadlines.
pub fn test_limits() -> Limits {
    Limits {
        wall: Duration::from_secs(3),
        wall_login: Duration::from_secs(5),
        ..Limits::default()
    }
}

impl Env {
    pub fn new() -> Env {
        Env::with_limits(test_limits())
    }

    pub fn with_limits(limits: Limits) -> Env {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemorySecretStore::new());
        let resolver = Arc::new(TestResolver::default());
        let host = Env::open(dir.path(), &secrets, &resolver, limits);
        Env {
            dir,
            host,
            secrets,
            resolver,
        }
    }

    /// Opens another host on the same directory and secret store.
    pub fn reopen(&self) -> PluginHost {
        Env::open(self.dir.path(), &self.secrets, &self.resolver, test_limits())
    }

    fn open(
        dir: &Path,
        secrets: &Arc<MemorySecretStore>,
        resolver: &Arc<TestResolver>,
        limits: Limits,
    ) -> PluginHost {
        let config = HostConfig {
            dir: dir.join("plugins"),
            cache_dir: dir.join("cache"),
            secrets: secrets.clone(),
            allow_community: false,
        };
        let options = HostOptions {
            trust: TrustList::builtin().with_test_key(),
            resolver: resolver.clone(),
            limits,
            ..HostOptions::default()
        };
        PluginHost::with_options(config, options).unwrap()
    }

    /// Where packages are written before installing.
    pub fn incoming(&self) -> PathBuf {
        let dir = self.dir.path().join("incoming");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Installs a signed test plugin with its own settings schema and extra
    /// entries.
    pub fn install_with(&self, plugin: &str, manifest: &str, id: &str, schema: &str, extra: &[(&str, &str)]) {
        let path = package_with(
            &self.incoming(),
            &format!("{id}.jfpkg"),
            manifest,
            component(plugin),
            schema,
            extra,
        );
        self.host
            .install(&path)
            .unwrap_or_else(|e| panic!("installing {id}: {e}"));
    }

    /// Installs a signed test plugin and points it at `server`.
    pub fn install(&self, plugin: &str, manifest: &str, id: &str, server: Option<&str>) {
        let path = package(
            &self.incoming(),
            &format!("{id}.jfpkg"),
            manifest,
            component(plugin),
            true,
        );
        self.host
            .install(&path)
            .unwrap_or_else(|e| panic!("installing {id}: {e}"));
        if let Some(server) = server {
            self.host
                .set_settings(id, &serde_json::json!({ "server_url": server }).to_string())
                .unwrap();
        }
    }
}

/// Waits for a reply (at most 30 s).
pub fn recv<T>(reply: Reply<T>) -> Result<T, CallError> {
    reply
        .recv_timeout(Duration::from_secs(30))
        .expect("no reply within 30 s")
}

/// A first page of `limit` items.
pub fn page(limit: u32) -> PageRequest {
    PageRequest { cursor: None, limit }
}

pub fn any() -> SearchFilters {
    SearchFilters { kinds: Vec::new() }
}

/// A track item for calls that take one.
pub fn track(id: &str, title: &str) -> Item {
    Item {
        id: id.into(),
        kind: ItemKind::Track,
        title: title.into(),
        artist: Some("Artist".into()),
        album: None,
        album_id: None,
        artist_id: None,
        duration_ms: Some(200_000),
        track_number: None,
        disc_number: None,
        year: None,
        child_count: None,
        isrc: None,
        artwork: None,
        explicit: false,
        playable: true,
        unplayable: None,
    }
}

/// The titles of a search result.
pub fn titles(host: &PluginHost, id: &str, query: &str) -> Vec<String> {
    let page =
        recv(host.search(id, query, any(), page(50))).unwrap_or_else(|e| panic!("search {query}: {e}"));
    page.items.into_iter().map(|i| i.title).collect()
}

/// A request the test server received.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    /// Path with query.
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
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
    /// Wait this long before answering.
    pub delay: Duration,
    /// Send the body without `content-length` (until the connection closes).
    pub chunked: bool,
}

impl Response {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Response {
        Response {
            status,
            headers: Vec::new(),
            body: body.into(),
            delay: Duration::ZERO,
            chunked: false,
        }
    }

    pub fn json(value: serde_json::Value) -> Response {
        Response::new(200, value.to_string()).header("content-type", "application/json")
    }

    pub fn header(mut self, name: &str, value: &str) -> Response {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn redirect(location: &str) -> Response {
        Response::new(302, "").header("location", location)
    }
}

type Handler = dyn Fn(&Request) -> Response + Send + Sync;

/// A tiny threaded HTTP/1.1 server on 127.0.0.1 for tests. Every
/// connection is answered once and closed.
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

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn requests(&self) -> Vec<Request> {
        self.log.lock().unwrap().clone()
    }

    /// Requests whose path starts with `prefix`.
    pub fn count(&self, prefix: &str) -> usize {
        self.requests()
            .iter()
            .filter(|r| r.path().starts_with(prefix))
            .count()
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
    };
    log.lock().unwrap().push(request.clone());
    let response = handler(&request);
    std::thread::sleep(response.delay);
    let mut stream = reader.into_inner();
    // Small answers go out in one write, without Nagle's delay, so timings
    // measure the host rather than TCP's delayed acknowledgements.
    let _ = stream.set_nodelay(true);
    let mut head = format!("HTTP/1.1 {} Test\r\nconnection: close\r\n", response.status);
    for (name, value) in &response.headers {
        head += &format!("{name}: {value}\r\n");
    }
    if !response.chunked {
        head += &format!("content-length: {}\r\n", response.body.len());
    }
    head += "\r\n";
    let mut first = head.into_bytes();
    let split = response.body.len().min(64 * 1024);
    first.extend_from_slice(&response.body[..split]);
    if stream.write_all(&first).is_err() {
        return;
    }
    for chunk in response.body[split..].chunks(64 * 1024) {
        if stream.write_all(chunk).is_err() {
            return;
        }
    }
    let _ = stream.flush();
}

/// A resolver whose answers the test sets per host name. Each lookup takes
/// the next answer in the list (the last one repeats), so a test can make a
/// name change its address between lookups (DNS rebinding).
#[derive(Debug, Default)]
pub struct TestResolver {
    answers: Mutex<HashMap<String, Vec<Vec<IpAddr>>>>,
    calls: Mutex<HashMap<String, usize>>,
}

impl TestResolver {
    pub fn answer(&self, host: &str, answers: &[&[&str]]) {
        let answers = answers
            .iter()
            .map(|a| a.iter().map(|ip| ip.parse().unwrap()).collect())
            .collect();
        self.answers.lock().unwrap().insert(host.to_string(), answers);
    }

    pub fn calls(&self, host: &str) -> usize {
        self.calls.lock().unwrap().get(host).copied().unwrap_or(0)
    }
}

impl Resolve for TestResolver {
    fn lookup(&self, host: &str, _port: u16) -> std::io::Result<Vec<IpAddr>> {
        let call = {
            let mut calls = self.calls.lock().unwrap();
            let n = calls.entry(host.to_string()).or_insert(0);
            *n += 1;
            *n - 1
        };
        let answers = self.answers.lock().unwrap();
        match answers.get(host) {
            Some(list) if !list.is_empty() => Ok(list[call.min(list.len() - 1)].clone()),
            _ if host == "localhost" => Ok(vec!["127.0.0.1".parse().unwrap()]),
            _ => Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no such host")),
        }
    }
}

/// Runs `f` and returns its result with the time it took.
pub fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let value = f();
    (value, started.elapsed())
}

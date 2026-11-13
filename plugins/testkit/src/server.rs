//! A tiny HTTP/1.1 server on 127.0.0.1 that answers with a handler, for
//! mocking a service in tests, and a tiny client to fetch the URLs a plugin
//! hands out (streams, artwork) the way the engine would.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// A request the server received.
#[derive(Debug, Clone)]
pub struct Request {
    /// `GET`, `POST`, …
    pub method: String,
    /// Path with query, as sent.
    pub target: String,
    /// Headers, names in lowercase.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl Request {
    /// The first header called `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The path without the query.
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }

    /// Query parameters, decoded, in order.
    pub fn query_pairs(&self) -> Vec<(String, String)> {
        match self.target.split_once('?') {
            Some((_, query)) => url::form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect(),
            None => Vec::new(),
        }
    }

    /// The first query parameter called `key`.
    pub fn query(&self, key: &str) -> Option<String> {
        self.query_pairs()
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Form fields of an `application/x-www-form-urlencoded` body.
    pub fn form_pairs(&self) -> Vec<(String, String)> {
        url::form_urlencoded::parse(&self.body).into_owned().collect()
    }

    /// Query parameters followed by form fields: what a Subsonic-style API
    /// reads from either place.
    pub fn params(&self) -> Vec<(String, String)> {
        let mut all = self.query_pairs();
        if self
            .header("content-type")
            .is_some_and(|t| t.starts_with("application/x-www-form-urlencoded"))
        {
            all.extend(self.form_pairs());
        }
        all
    }

    /// The first parameter called `key` from [`Request::params`].
    pub fn param(&self, key: &str) -> Option<String> {
        self.params().into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The body parsed as JSON.
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }
}

/// What the server answers.
#[derive(Debug, Clone)]
pub struct Response {
    /// Status code.
    pub status: u16,
    /// Headers.
    pub headers: Vec<(String, String)>,
    /// Body.
    pub body: Vec<u8>,
}

impl Response {
    /// A response with a body and no headers.
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Response {
        Response {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    /// `200` with a JSON body.
    pub fn json(value: &serde_json::Value) -> Response {
        Response::new(200, value.to_string()).header("content-type", "application/json; charset=utf-8")
    }

    /// `status` with a plain-text body.
    pub fn text(status: u16, text: &str) -> Response {
        Response::new(status, text).header("content-type", "text/plain; charset=utf-8")
    }

    /// `204 No Content`.
    pub fn no_content() -> Response {
        Response::new(204, Vec::new())
    }

    /// Adds a header.
    pub fn header(mut self, name: &str, value: &str) -> Response {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
}

type Handler = dyn Fn(&Request) -> Response + Send + Sync;

/// A threaded HTTP/1.1 server on 127.0.0.1. Each connection gets one answer
/// and is closed; every request is logged.
pub struct TestServer {
    addr: SocketAddr,
    log: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for TestServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestServer").field("addr", &self.addr).finish()
    }
}

impl TestServer {
    /// Starts a server that answers every request with `handler`.
    pub fn start(handler: impl Fn(&Request) -> Response + Send + Sync + 'static) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("binding 127.0.0.1");
        listener.set_nonblocking(true).expect("non-blocking listener");
        let addr = listener.local_addr().expect("listener address");
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
                        Err(_) => std::thread::sleep(Duration::from_millis(2)),
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

    /// Every request so far.
    pub fn requests(&self) -> Vec<Request> {
        self.log.lock().expect("request log").clone()
    }

    /// Requests whose path is `path`.
    pub fn requests_to(&self, path: &str) -> Vec<Request> {
        self.requests().into_iter().filter(|r| r.path() == path).collect()
    }

    /// How many requests went to `path`.
    pub fn count(&self, path: &str) -> usize {
        self.requests_to(path).len()
    }

    /// Forgets the requests so far.
    pub fn clear(&self) {
        self.log.lock().expect("request log").clear();
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

/// The query parameters of a URL a plugin handed out, decoded, by name (the
/// last value wins when a name repeats).
pub fn url_query(url: &str) -> std::collections::HashMap<String, String> {
    url::Url::parse(url)
        .map(|parsed| parsed.query_pairs().into_owned().collect())
        .unwrap_or_default()
}

/// A local address where nothing listens: a server that is down.
pub fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("binding 127.0.0.1");
    let addr = listener.local_addr().expect("listener address");
    drop(listener);
    format!("http://{addr}")
}

fn serve(stream: TcpStream, log: &Mutex<Vec<Request>>, handler: &Handler) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Some(request) = read_request(&stream) else {
        return;
    };
    log.lock().expect("request log").push(request.clone());
    let response = handler(&request);
    let mut stream = stream;
    let _ = stream.set_nodelay(true);
    let reason = match response.status {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Status",
    };
    let mut head = format!("HTTP/1.1 {} {reason}\r\nconnection: close\r\n", response.status);
    for (name, value) in &response.headers {
        head += &format!("{name}: {value}\r\n");
    }
    head += &format!("content-length: {}\r\n\r\n", response.body.len());
    let mut bytes = head.into_bytes();
    if request.method != "HEAD" {
        bytes.extend_from_slice(&response.body);
    }
    let _ = stream.write_all(&bytes);
    let _ = stream.flush();
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
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
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        target,
        headers,
        body,
    })
}

/// A fetched response.
#[derive(Debug, Clone)]
pub struct Fetched {
    /// Status code.
    pub status: u16,
    /// Headers, names in lowercase.
    pub headers: Vec<(String, String)>,
    /// Body.
    pub body: Vec<u8>,
}

impl Fetched {
    /// The first header called `name`.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// `GET url` with `headers` over plain HTTP (no redirects, no TLS), as the
/// engine fetches a stream or the host an image.
pub fn fetch(url: &str, headers: &[(String, String)]) -> std::io::Result<Fetched> {
    let parsed = url::Url::parse(url).map_err(|e| std::io::Error::other(e.to_string()))?;
    let host = parsed.host_str().unwrap_or_default().to_string();
    let port = parsed.port_or_known_default().unwrap_or(80);
    let mut stream = TcpStream::connect((host.as_str(), port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let target = match parsed.query() {
        Some(query) => format!("{}?{query}", parsed.path()),
        None => parsed.path().to_string(),
    };
    let mut request = format!("GET {target} HTTP/1.1\r\nhost: {host}:{port}\r\nconnection: close\r\n");
    for (name, value) in headers {
        request += &format!("{name}: {value}\r\n");
    }
    request += "\r\n";
    stream.write_all(request.as_bytes())?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| std::io::Error::other("no end of headers"))?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| std::io::Error::other("no status line"))?;
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    Ok(Fetched {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
    })
}

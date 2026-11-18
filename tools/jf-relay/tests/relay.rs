//! The relay end to end on loopback: a browser-like client on one side (raw HTTP/1.1, as
//! Jellyfesh Web's requests arrive), a mock service on the other, and the checks in between:
//! origin, token and host; the headers a page may not set forwarded; the answer wrapped; nothing
//! secret logged.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use jf_relay::{Allow, Config, Relay};
use zeroize::Zeroizing;

const TOKEN: &str = "test-token-0123456789abcdef";
const ORIGIN: &str = "http://localhost:4173";

/// One request the mock service received.
#[derive(Debug, Clone, Default)]
struct Seen {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A service on 127.0.0.1 that records requests and answers per path.
struct Service {
    port: u16,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Service {
    fn start() -> Service {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&seen);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let record = Arc::clone(&record);
                thread::spawn(move || serve(stream, &record));
            }
        });
        Service { port, seen }
    }

    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

fn serve(stream: TcpStream, record: &Mutex<Vec<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    if reader.read_line(&mut first).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').unwrap();
        headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
    }
    let length: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .map_or(0, |(_, v)| v.parse().unwrap());
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let seen = Seen {
        method,
        target: target.clone(),
        headers,
        body,
    };
    let answer = match target.as_str() {
        "/redirect" => {
            "HTTP/1.1 302 Found\r\nLocation: https://elsewhere.example/next\r\nContent-Length: 0\r\n\r\n"
                .to_owned()
        }
        "/range" => {
            let range = seen.header("range").unwrap_or("none").to_owned();
            format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-3/10\r\nContent-Type: audio/wav\r\nContent-Length: 4\r\nX-Asked: {range}\r\n\r\nRIFF"
            )
        }
        _ => {
            let body = "secret-body-of-the-service";
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nSet-Cookie: MUSIC_U=session-cookie-value; Max-Age=100; Path=/\r\nX-Name: Zo\u{eb}\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
        }
    };
    record.lock().unwrap().push(seen);
    let mut stream = stream;
    let _ = stream.write_all(answer.as_bytes());
}

/// A relay on 127.0.0.1 for `allow`, with its log lines.
struct Running {
    port: u16,
    log: Arc<Mutex<Vec<String>>>,
}

fn relay(allow: Allow, verbose: bool) -> Running {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let log = Arc::new(Mutex::new(Vec::new()));
    let lines = Arc::clone(&log);
    let relay = Relay::new(
        Config {
            token: Zeroizing::new(TOKEN.to_owned()),
            origins: vec![ORIGIN.to_owned()],
            allow,
            verbose,
        },
        Arc::new(move |line: &str| lines.lock().unwrap().push(line.to_owned())),
    );
    thread::spawn(move || relay.serve(listener));
    Running { port, log }
}

/// What came back to the "browser".
#[derive(Debug)]
struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn upstream_headers(&self) -> Vec<(String, String)> {
        serde_json::from_str(self.header("jf-relay-headers").expect("wrapped headers")).unwrap()
    }
}

/// Sends a raw request and reads the whole answer (chunked or not).
fn send(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Answer {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").expect("a head");
    let head = String::from_utf8(raw[..split].to_vec()).unwrap();
    let mut rest = raw[split + 4..].to_vec();
    let mut lines = head.lines();
    let status = lines.next().unwrap().split(' ').nth(1).unwrap().parse().unwrap();
    let headers: Vec<(String, String)> = lines
        .map(|l| {
            let (n, v) = l.split_once(':').unwrap();
            (n.to_owned(), v.trim().to_owned())
        })
        .collect();
    let chunked = headers
        .iter()
        .any(|(n, v)| n.eq_ignore_ascii_case("transfer-encoding") && v == "chunked");
    if chunked {
        let mut body = Vec::new();
        loop {
            let end = rest.windows(2).position(|w| w == b"\r\n").unwrap();
            let size = usize::from_str_radix(std::str::from_utf8(&rest[..end]).unwrap(), 16).unwrap();
            rest.drain(..end + 2);
            if size == 0 {
                break;
            }
            body.extend_from_slice(&rest[..size]);
            rest.drain(..size + 2);
        }
        rest = body;
    }
    Answer {
        status,
        headers,
        body: rest,
    }
}

fn bearer() -> String {
    format!("Bearer {TOKEN}")
}

/// A forward request with every check passing, to `url`.
fn forward(port: u16, url: &str, method: &str, wrapped: &str, body: &[u8]) -> Answer {
    send(
        port,
        "POST",
        "/v1/forward",
        &[
            ("Origin", ORIGIN),
            ("Authorization", &bearer()),
            ("Jf-Relay-Url", url),
            ("Jf-Relay-Method", method),
            ("Jf-Relay-Headers", wrapped),
        ],
        body,
    )
}

fn allowing(service: &Service) -> Allow {
    let mut allow = Allow::bundled().unwrap();
    allow.add(&service.origin()).unwrap();
    allow
}

#[test]
fn forwards_the_headers_a_page_may_not_set_and_wraps_the_answer() {
    let service = Service::start();
    let running = relay(allowing(&service), true);
    let wrapped = r#"[["Cookie","uin=7000001; qm_keyst=W_X_key"],["Referer","https://y.qq.com/"],["User-Agent","Mozilla/5.0 (Windows NT 10.0)"],["Origin","https://y.qq.com"],["Content-Type","application/x-www-form-urlencoded"]]"#;
    let answer = forward(
        running.port,
        &format!("{}/u.y.qq.com/cgi-bin/musics.fcg?sign=zzb1", service.origin()),
        "POST",
        wrapped,
        b"{\"comm\":{}}",
    );
    assert_eq!(answer.status, 200);
    assert_eq!(answer.header("jf-relay-status"), Some("200"));
    assert_eq!(answer.header("access-control-allow-origin"), Some(ORIGIN));
    assert!(
        answer
            .header("access-control-expose-headers")
            .unwrap()
            .contains("Jf-Relay-Headers")
    );
    assert_eq!(answer.body, b"secret-body-of-the-service");
    // Set-Cookie, which a page could never read, comes back wrapped; non-ASCII stays intact.
    let upstream = answer.upstream_headers();
    assert!(upstream.contains(&(
        "set-cookie".into(),
        "MUSIC_U=session-cookie-value; Max-Age=100; Path=/".into()
    )));
    assert!(upstream.contains(&("x-name".into(), "Zo\u{eb}".into())));
    assert!(answer.header("jf-relay-headers").unwrap().is_ascii());

    let seen = service.seen();
    assert_eq!(seen.len(), 1);
    let request = &seen[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/u.y.qq.com/cgi-bin/musics.fcg?sign=zzb1");
    assert_eq!(request.header("cookie"), Some("uin=7000001; qm_keyst=W_X_key"));
    assert_eq!(request.header("referer"), Some("https://y.qq.com/"));
    assert_eq!(
        request.header("user-agent"),
        Some("Mozilla/5.0 (Windows NT 10.0)")
    );
    assert_eq!(
        request.header("origin"),
        Some("https://y.qq.com"),
        "the plugin's, not the page's"
    );
    assert_eq!(
        request.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(request.body, b"{\"comm\":{}}");
    assert_eq!(
        request.header("authorization"),
        None,
        "the relay's token stays with the relay"
    );
    assert!(request.header("jf-relay-url").is_none());

    // The log says what happened, never with the path, query, headers, body or token.
    let log = running.log.lock().unwrap().join("\n");
    assert!(log.contains("POST 127.0.0.1 -> 200"), "{log}");
    for secret in [
        TOKEN,
        "qm_keyst",
        "W_X_key",
        "musics.fcg",
        "sign=",
        "secret-body",
        "session-cookie",
        "comm",
    ] {
        assert!(!log.contains(secret), "the log holds {secret}: {log}");
    }
}

#[test]
fn needs_an_allowed_origin() {
    let service = Service::start();
    let running = relay(allowing(&service), false);
    let url = format!("{}/x", service.origin());
    for origin in [Some("https://evil.example"), Some("null"), None] {
        let mut headers = vec![("Authorization", bearer()), ("Jf-Relay-Url", url.clone())];
        if let Some(origin) = origin {
            headers.push(("Origin", origin.to_owned()));
        }
        let headers: Vec<(&str, &str)> = headers.iter().map(|(n, v)| (*n, v.as_str())).collect();
        let answer = send(running.port, "POST", "/v1/forward", &headers, b"");
        assert_eq!(answer.status, 403, "{origin:?}");
        assert_eq!(answer.header("jf-relay-error"), Some("origin"));
        assert_eq!(
            answer.header("access-control-allow-origin"),
            None,
            "other pages cannot read it"
        );
    }
    // A preflight from another page is refused too; Jellyfesh Web's is answered.
    let preflight = |origin: &str| {
        send(
            running.port,
            "OPTIONS",
            "/v1/forward",
            &[
                ("Origin", origin),
                ("Access-Control-Request-Method", "POST"),
                ("Access-Control-Request-Headers", "authorization, jf-relay-url"),
                ("Access-Control-Request-Private-Network", "true"),
            ],
            b"",
        )
    };
    assert_eq!(preflight("https://evil.example").status, 403);
    let ok = preflight(ORIGIN);
    assert_eq!(ok.status, 204);
    assert_eq!(ok.header("access-control-allow-origin"), Some(ORIGIN));
    assert!(
        ok.header("access-control-allow-headers")
            .unwrap()
            .contains("authorization")
    );
    assert_eq!(ok.header("access-control-allow-private-network"), Some("true"));
    assert!(service.seen().is_empty(), "nothing reached the service");
}

#[test]
fn needs_the_token() {
    let service = Service::start();
    let running = relay(allowing(&service), false);
    let url = format!("{}/x", service.origin());
    for authorization in [
        None,
        Some("Bearer wrong-token-0123456789"),
        Some("Bearer test-token-0123456789abcdeF"),
        Some(TOKEN),
    ] {
        let mut headers = vec![("Origin", ORIGIN.to_owned()), ("Jf-Relay-Url", url.clone())];
        if let Some(value) = authorization {
            headers.push(("Authorization", value.to_owned()));
        }
        let headers: Vec<(&str, &str)> = headers.iter().map(|(n, v)| (*n, v.as_str())).collect();
        let answer = send(running.port, "POST", "/v1/forward", &headers, b"");
        assert_eq!(answer.status, 401, "{authorization:?}");
        assert_eq!(answer.header("jf-relay-error"), Some("token"));
        // Jellyfesh Web can read why, to say so in Settings.
        assert_eq!(answer.header("access-control-allow-origin"), Some(ORIGIN));
    }
    let ping = send(
        running.port,
        "GET",
        "/v1/ping",
        &[("Origin", ORIGIN), ("Authorization", &bearer())],
        b"",
    );
    assert_eq!(ping.status, 200);
    let body: serde_json::Value = serde_json::from_slice(&ping.body).unwrap();
    assert_eq!(body["relay"], "jf-relay");
    assert_eq!(body["protocol"], 1);
    assert!(service.seen().is_empty());
}

#[test]
fn forwards_only_to_the_allowlist() {
    let service = Service::start();
    // Without --allow for the mock's origin: the bundled hosts only.
    let running = relay(Allow::bundled().unwrap(), false);
    for url in [
        format!("{}/x", service.origin()),
        "https://evil.example/".to_owned(),
        "http://u.y.qq.com/cgi-bin/musics.fcg".to_owned(),
        "https://u.y.qq.com.evil.example/".to_owned(),
        "file:///etc/passwd".to_owned(),
    ] {
        let answer = forward(running.port, &url, "GET", "[]", b"");
        assert!(matches!(answer.status, 400 | 403), "{url}: {}", answer.status);
        assert!(
            matches!(answer.header("jf-relay-error"), Some("host" | "request")),
            "{url}"
        );
    }
    assert!(service.seen().is_empty(), "nothing reached the service");
}

#[test]
fn refuses_headers_the_host_sets_and_bad_requests() {
    let service = Service::start();
    let running = relay(allowing(&service), false);
    let url = format!("{}/x", service.origin());
    for (method, wrapped) in [
        ("GET", r#"[["Host","evil.example"]]"#),
        ("GET", r#"[["Content-Length","5"]]"#),
        ("GET", r#"[["Transfer-Encoding","chunked"]]"#),
        ("GET", r#"[["Bad Name","x"]]"#),
        ("GET", r#"{"cookie":"a=b"}"#),
        ("CONNECT", "[]"),
    ] {
        let answer = forward(running.port, &url, method, wrapped, b"");
        assert_eq!(answer.status, 400, "{wrapped}");
        assert_eq!(answer.header("jf-relay-error"), Some("request"));
    }
    let with_user = forward(
        running.port,
        &format!("http://user:pw@127.0.0.1:{}/x", service.port),
        "GET",
        "[]",
        b"",
    );
    assert_eq!(with_user.status, 400);
    assert!(service.seen().is_empty());
}

#[test]
fn passes_redirects_and_ranges_back_without_following() {
    let service = Service::start();
    let running = relay(allowing(&service), false);
    let redirect = forward(
        running.port,
        &format!("{}/redirect", service.origin()),
        "GET",
        "[]",
        b"",
    );
    assert_eq!(redirect.status, 200);
    assert_eq!(redirect.header("jf-relay-status"), Some("302"));
    assert!(
        redirect
            .upstream_headers()
            .contains(&("location".into(), "https://elsewhere.example/next".into()))
    );
    // The media element's Range passes through when the page wrapped none.
    let range = send(
        running.port,
        "POST",
        "/v1/forward",
        &[
            ("Origin", ORIGIN),
            ("Authorization", &bearer()),
            ("Jf-Relay-Url", &format!("{}/range", service.origin())),
            ("Jf-Relay-Method", "GET"),
            ("Range", "bytes=0-3"),
        ],
        b"",
    );
    assert_eq!(range.header("jf-relay-status"), Some("206"));
    assert_eq!(range.body, b"RIFF");
    let upstream = range.upstream_headers();
    assert!(upstream.contains(&("content-range".into(), "bytes 0-3/10".into())));
    assert!(upstream.contains(&("x-asked".into(), "bytes=0-3".into())));
    assert_eq!(service.seen().len(), 2, "one request each, no redirect followed");
}

#[test]
fn answers_only_its_endpoints() {
    let service = Service::start();
    let running = relay(allowing(&service), false);
    let other = send(
        running.port,
        "GET",
        "/",
        &[("Origin", ORIGIN), ("Authorization", &bearer())],
        b"",
    );
    assert_eq!(other.status, 404);
    let get_forward = send(
        running.port,
        "GET",
        "/v1/forward",
        &[("Origin", ORIGIN), ("Authorization", &bearer())],
        b"",
    );
    assert_eq!(get_forward.status, 405);
    let mut stream = TcpStream::connect(("127.0.0.1", running.port)).unwrap();
    stream
        .write_all(b"POST /v1/forward HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n")
        .unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    assert!(raw.starts_with("HTTP/1.1 501"), "{raw}");
}

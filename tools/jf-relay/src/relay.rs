//! Answering Jellyfesh Web: the checks every request passes, and the forwarded request.
//!
//! The protocol (`docs/web-relay.md`, and `platforms/web/src/lib/plugins/relay.ts` on the page's
//! side):
//!
//! * `OPTIONS` (a CORS preflight) to any path: allowed origins get the CORS answer, others 403.
//! * `GET /v1/ping`: 200 with `{"relay": "jf-relay", "version": …, "protocol": 1}` once the
//!   origin and token pass, for Settings › Sources › Relay to check the connection.
//! * `POST /v1/forward`: the request to make is wrapped in headers, because a page may not set
//!   `Cookie`, `Referer`, `User-Agent` or `Origin` itself:
//!   * `Jf-Relay-Url`: the absolute URL;
//!   * `Jf-Relay-Method`: `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE` or `OPTIONS`;
//!   * `Jf-Relay-Headers`: the request's headers as a JSON array of `[name, value]` pairs
//!     (checked with the plugin hosts' rules: at most 64, 64 KiB, none of the headers the host
//!     sets);
//!   * the body is the request's body; `Range` and `If-Range` pass through when the wrapped
//!     headers have none (a media element asks for ranges).
//!
//!   The answer is always `200` with `Jf-Relay-Status` (the service's status, redirects
//!   included: the relay never follows one, the page checks each hop) and `Jf-Relay-Headers`
//!   (the service's headers as JSON, `Set-Cookie` and `Location` included, which a page could
//!   not read otherwise), and the service's body.
//! * Everything the relay refuses is a 4xx or 5xx with `Jf-Relay-Error` (`origin`, `token`,
//!   `host`, `address`, `request`, `dns`, `upstream`, `timeout`, `busy`) and a JSON body with a
//!   message; with the CORS headers when the origin is allowed, so the page can read why.
//!
//! Every request needs an allowed `Origin` and `Authorization: Bearer <token>`; the URL must be
//! on the allowlist ([`crate::allow`]), and the address its host resolves to is checked again
//! when connecting. Nothing is logged of a request but its method, host and status, and only
//! with `--verbose`: never a path, query, header, body or the token.

use std::io::{BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use jf_plugin_model::Header;
use ureq::http;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};
use url::Url;
use zeroize::Zeroizing;

use crate::allow::{Allow, permitted};
use crate::http::{Body, ReadError, Request, Response, read_request, write_response};

/// The forwarding endpoint.
pub const FORWARD: &str = "/v1/forward";
/// The connection check.
pub const PING: &str = "/v1/ping";
/// The protocol version `ping` reports.
pub const PROTOCOL: u32 = 1;

/// Request headers.
pub const URL_HEADER: &str = "jf-relay-url";
pub const METHOD_HEADER: &str = "jf-relay-method";
pub const HEADERS_HEADER: &str = "jf-relay-headers";
/// Response headers.
pub const STATUS_HEADER: &str = "Jf-Relay-Status";
pub const UPSTREAM_HEADERS_HEADER: &str = "Jf-Relay-Headers";
pub const ERROR_HEADER: &str = "Jf-Relay-Error";

/// Most connections handled at once; more get `busy`.
const MAX_CONNECTIONS: usize = 64;
/// Largest request head (the wrapped headers can be 64 KiB of JSON).
const MAX_HEAD: usize = 160 * 1024;
/// Most service headers passed back, and their size as JSON.
const MAX_UPSTREAM_HEADERS: usize = 256;
const MAX_UPSTREAM_HEADERS_JSON: usize = 96 * 1024;
const METHODS: &[&str] = &["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];

/// Where the relay's lines go (standard error, or a test's buffer).
pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

/// How the relay runs.
#[derive(Debug)]
pub struct Config {
    /// The token every request must carry.
    pub token: Zeroizing<String>,
    /// The origins Jellyfesh Web is opened at (`https://jellyfesh.example`), as browsers send
    /// them in `Origin`.
    pub origins: Vec<String>,
    pub allow: Allow,
    /// Log each request's method, host and status.
    pub verbose: bool,
}

/// The shortest token the relay accepts.
pub const MIN_TOKEN: usize = 16;

/// An origin as browsers send it: `scheme://host[:port]`, lowercase, without a default port or
/// a path; `None` when `text` is not one.
pub fn normalize_origin(text: &str) -> Option<String> {
    let url = Url::parse(text.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

/// A new random token: 32 hex digits from the system's random source.
pub fn new_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("no random numbers: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compares in time that depends only on the lengths.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Why the resolver refused a host; travels through ureq as `Error::Other`.
#[derive(Debug)]
enum Refused {
    NotAllowed(String),
    Address(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::NotAllowed(m) | Refused::Address(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Refused {}

/// Resolves names with the system's resolver, then checks every address it gave: a fixed host
/// only at public addresses, an `--allow` origin also on the local network. The connection goes
/// only to the addresses checked here.
#[derive(Debug)]
struct CheckedResolver {
    allow: Arc<Allow>,
    inner: DefaultResolver,
}

impl Resolver for CheckedResolver {
    fn resolve(
        &self,
        uri: &http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let refuse = |r: Refused| ureq::Error::Other(Box::new(r));
        let scheme = uri.scheme_str().unwrap_or_default();
        let host = uri.host().unwrap_or_default();
        let port = uri.port_u16().unwrap_or(if scheme == "https" { 443 } else { 80 });
        let bare = host.trim_start_matches('[').trim_end_matches(']');
        let access = self.allow.access(scheme, bare, port).ok_or_else(|| {
            refuse(Refused::NotAllowed(format!(
                "{host}:{port} is not on the relay's allowlist"
            )))
        })?;
        let found = self.inner.resolve(uri, config, timeout)?;
        for address in found.iter() {
            permitted(access, address.ip())
                .map_err(|why| refuse(Refused::Address(format!("{host}: {why}"))))?;
        }
        Ok(found)
    }
}

/// The relay.
pub struct Relay {
    config: Config,
    agent: ureq::Agent,
    log: Log,
    connections: AtomicUsize,
}

impl std::fmt::Debug for Relay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Relay")
            .field("origins", &self.config.origins)
            .finish_non_exhaustive()
    }
}

/// An error answer.
fn refusal(status: u16, code: &str, message: &str) -> Response {
    let body = serde_json::json!({ "error": code, "message": message }).to_string();
    Response::new(status)
        .header(ERROR_HEADER, code)
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-store")
        .body(Body::Bytes(body.into_bytes()))
}

/// JSON with every non-ASCII character escaped: header values are bytes, and browsers read them
/// as Latin-1.
pub fn ascii_json(value: &serde_json::Value) -> String {
    let text = value.to_string();
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

impl Relay {
    /// A relay for `config`, writing its lines to `log`.
    pub fn new(config: Config, log: Log) -> Arc<Relay> {
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::Rustls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        let agent_config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            // Each hop goes back to the page, which checks it against the plugin's allowlist.
            .max_redirects(0)
            // A proxy would resolve names itself, past the address checks.
            .proxy(None)
            .user_agent("Jellyfesh")
            .tls_config(tls)
            .max_response_header_size(64 * 1024)
            .timeout_resolve(Some(Duration::from_secs(10)))
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_send_request(Some(Duration::from_secs(30)))
            .timeout_send_body(Some(Duration::from_secs(60)))
            // A long poll (WeChat's sign-in) waits up to the plugin's 30 s.
            .timeout_recv_response(Some(Duration::from_secs(35)))
            .timeout_per_read(Some(Duration::from_secs(60)))
            .build();
        let resolver = CheckedResolver {
            allow: Arc::new(config.allow.clone()),
            inner: DefaultResolver::default(),
        };
        Arc::new(Relay {
            agent: ureq::Agent::with_parts(agent_config, DefaultConnector::new(), resolver),
            config,
            log,
            connections: AtomicUsize::new(0),
        })
    }

    /// Accepts connections until the listener fails, each on a thread of its own.
    pub fn serve(self: &Arc<Self>, listener: TcpListener) -> std::io::Result<()> {
        for stream in listener.incoming() {
            let stream = match stream {
                Ok(stream) => stream,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionAborted => continue,
                Err(error) => return Err(error),
            };
            let relay = Arc::clone(self);
            let taken = relay.connections.fetch_add(1, Ordering::SeqCst);
            let spawned = std::thread::Builder::new()
                .name("jf-relay".into())
                .spawn(move || {
                    relay.connection(stream, taken >= MAX_CONNECTIONS);
                    relay.connections.fetch_sub(1, Ordering::SeqCst);
                });
            if spawned.is_err() {
                self.connections.fetch_sub(1, Ordering::SeqCst);
            }
        }
        Ok(())
    }

    fn connection(&self, mut stream: TcpStream, busy: bool) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
        let _ = stream.set_nodelay(true);
        let Ok(reading) = stream.try_clone() else { return };
        let mut reader = BufReader::new(reading);
        let request = read_request(&mut reader, MAX_HEAD, jf_plugin_model::http::MAX_REQUEST_BODY);
        let response = match request {
            Ok(request) if busy => {
                let origin = self.allowed_origin(&request);
                self.with_cors(
                    refusal(503, "busy", "The relay is busy. Try again in a moment."),
                    origin.as_deref(),
                )
            }
            Ok(request) => self.handle(&request),
            Err(ReadError::Closed | ReadError::Io(_)) => return,
            Err(ReadError::Bad(why)) => refusal(400, "request", &why),
            Err(ReadError::TooLarge(what)) => refusal(413, "request", &format!("{what} is too large")),
            Err(ReadError::Unsupported(why)) => refusal(501, "request", &why),
        };
        let _ = write_response(&mut stream, response);
        let _ = stream.flush();
        let _ = stream.shutdown(Shutdown::Both);
    }

    fn allowed_origin(&self, request: &Request) -> Option<String> {
        let origin = request.header("origin")?;
        let normal = normalize_origin(origin)?;
        self.config.origins.contains(&normal).then(|| origin.to_owned())
    }

    fn with_cors(&self, mut response: Response, origin: Option<&str>) -> Response {
        if let Some(origin) = origin {
            response.headers.extend([
                ("Access-Control-Allow-Origin".to_owned(), origin.to_owned()),
                (
                    "Access-Control-Expose-Headers".to_owned(),
                    format!("{STATUS_HEADER}, {UPSTREAM_HEADERS_HEADER}, {ERROR_HEADER}"),
                ),
            ]);
        }
        response.headers.push(("Vary".to_owned(), "Origin".to_owned()));
        response
    }

    /// The answer to one request (the connection's checks done).
    pub fn handle(&self, request: &Request) -> Response {
        let origin = self.allowed_origin(request);
        let path = request.path.as_str();
        if request.method == "OPTIONS" {
            return self.preflight(request, origin.as_deref());
        }
        let Some(origin) = origin else {
            self.note(&format!(
                "refused a request from {}: not an origin given with --origin",
                request.header("origin").unwrap_or("a page that sent no origin")
            ));
            return self.with_cors(
                refusal(
                    403,
                    "origin",
                    "The relay does not accept requests from this page.",
                ),
                None,
            );
        };
        let authorized = request
            .header("authorization")
            .and_then(|value| value.strip_prefix("Bearer "))
            .is_some_and(|token| same(token.trim().as_bytes(), self.config.token.as_bytes()));
        if !authorized {
            self.note("refused a request: missing or wrong token");
            return self.with_cors(
                refusal(401, "token", "The relay refused the token."),
                Some(&origin),
            );
        }
        let response = match (request.method.as_str(), path) {
            ("GET" | "POST", PING) => Response::new(200)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "no-store")
                .body(Body::Bytes(
                    serde_json::json!({
                        "relay": "jf-relay",
                        "version": env!("CARGO_PKG_VERSION"),
                        "protocol": PROTOCOL,
                    })
                    .to_string()
                    .into_bytes(),
                )),
            ("POST", FORWARD) => self.forward(request),
            (_, PING | FORWARD) => refusal(405, "request", "Use POST to forward a request."),
            _ => refusal(404, "request", "The relay has no such endpoint."),
        };
        self.with_cors(response, Some(&origin))
    }

    fn preflight(&self, request: &Request, origin: Option<&str>) -> Response {
        let Some(origin) = origin else {
            self.note(&format!(
                "refused a preflight from {}: not an origin given with --origin",
                request.header("origin").unwrap_or("a page that sent no origin")
            ));
            return refusal(
                403,
                "origin",
                "The relay does not accept requests from this page.",
            );
        };
        let mut response = Response::new(204)
            .header("Access-Control-Allow-Origin", origin)
            .header("Access-Control-Allow-Methods", "GET, POST")
            .header(
                "Access-Control-Allow-Headers",
                "authorization, content-type, range, if-range, jf-relay-url, jf-relay-method, jf-relay-headers",
            )
            .header("Access-Control-Max-Age", "7200")
            .header("Vary", "Origin, Access-Control-Request-Headers");
        // Chromium's earlier Private Network Access preflights ask for this; a relay on this
        // computer or the local network is exactly that.
        if request.header("access-control-request-private-network") == Some("true") {
            response = response.header("Access-Control-Allow-Private-Network", "true");
        }
        response
    }

    fn note(&self, line: &str) {
        if self.config.verbose {
            (self.log)(line);
        }
    }

    /// Forwards a wrapped request and wraps the answer.
    fn forward(&self, request: &Request) -> Response {
        if request.count(URL_HEADER) != 1
            || request.count(METHOD_HEADER) > 1
            || request.count(HEADERS_HEADER) > 1
        {
            return refusal(
                400,
                "request",
                "Give Jf-Relay-Url once, and Jf-Relay-Method and Jf-Relay-Headers at most once.",
            );
        }
        let method = request
            .header(METHOD_HEADER)
            .unwrap_or("GET")
            .to_ascii_uppercase();
        if !METHODS.contains(&method.as_str()) {
            return refusal(
                400,
                "request",
                "Jf-Relay-Method is not a method the relay forwards.",
            );
        }
        let mut headers: Vec<Header> = match request.header(HEADERS_HEADER) {
            None => Vec::new(),
            Some(text) => match serde_json::from_str::<Vec<(String, String)>>(text) {
                Ok(pairs) => pairs
                    .into_iter()
                    .map(|(name, value)| Header { name, value })
                    .collect(),
                Err(_) => {
                    return refusal(
                        400,
                        "request",
                        "Jf-Relay-Headers is not a JSON list of [name, value] pairs.",
                    );
                }
            },
        };
        for passed in ["range", "if-range"] {
            if !headers.iter().any(|h| h.name.eq_ignore_ascii_case(passed))
                && let Some(value) = request.header(passed)
            {
                headers.push(Header {
                    name: passed.to_owned(),
                    value: value.to_owned(),
                });
            }
        }
        let url = request.header(URL_HEADER).unwrap_or_default();
        let target =
            match jf_plugin_model::http::check_request(url, &headers, request.body.len(), method == "HEAD") {
                Ok(target) => target,
                Err(why) => return refusal(400, "request", &why),
            };
        let host = target.host_str().unwrap_or_default().to_owned();
        // Checked here for a clear answer, and again by the resolver for the address.
        if self.config.allow.check(&target).is_err() {
            self.note(&format!("refused {method} {host}: not on the allowlist"));
            return refusal(
                403,
                "host",
                &format!(
                    "The relay does not forward to {}. Add it with --allow if it is yours.",
                    origin_of(&target)
                ),
            );
        }
        let mut builder = http::Request::builder()
            .method(method.as_str())
            .uri(target.as_str());
        for header in &headers {
            builder = builder.header(header.name.as_str(), header.value.as_str());
        }
        let upstream = match builder.body(request.body.clone()) {
            Ok(upstream) => upstream,
            Err(error) => return refusal(400, "request", &error.to_string()),
        };
        let started = Instant::now();
        let result = self.agent.run(upstream);
        let elapsed = started.elapsed().as_millis();
        match result {
            Ok(answer) => {
                let status = answer.status().as_u16();
                self.note(&format!("{method} {host} -> {status} ({elapsed} ms)"));
                let mut pairs = Vec::new();
                let mut size = 2;
                for (name, value) in answer.headers().iter().take(MAX_UPSTREAM_HEADERS) {
                    let pair = serde_json::json!([name.as_str(), String::from_utf8_lossy(value.as_bytes())]);
                    size += pair.to_string().len() + 1;
                    if size > MAX_UPSTREAM_HEADERS_JSON {
                        break;
                    }
                    pairs.push(pair);
                }
                let body = if method == "HEAD" {
                    Body::Empty
                } else {
                    Body::Stream(Box::new(answer.into_body().into_with_config().reader()))
                };
                Response::new(200)
                    .header(STATUS_HEADER, status.to_string())
                    .header(
                        UPSTREAM_HEADERS_HEADER,
                        ascii_json(&serde_json::Value::Array(pairs)),
                    )
                    .header("Content-Type", "application/octet-stream")
                    .header("Cache-Control", "no-store")
                    .header("X-Content-Type-Options", "nosniff")
                    .body(body)
            }
            Err(error) => {
                let (status, code, message) = describe_error(error);
                self.note(&format!("{method} {host} -> {code} ({elapsed} ms)"));
                refusal(status, code, &message)
            }
        }
    }
}

fn origin_of(url: &Url) -> String {
    url.origin().ascii_serialization()
}

/// A failed exchange as an answer: status, `Jf-Relay-Error` and a message.
fn describe_error(error: ureq::Error) -> (u16, &'static str, String) {
    match error {
        ureq::Error::Other(inner) => match inner.downcast::<Refused>() {
            Ok(refused) => match *refused {
                Refused::NotAllowed(m) => (403, "host", m),
                Refused::Address(m) => (403, "address", m),
            },
            Err(other) => (502, "upstream", other.to_string()),
        },
        ureq::Error::Timeout(_) => (504, "timeout", "The service did not answer in time.".into()),
        ureq::Error::HostNotFound => (
            502,
            "dns",
            "The service's host name could not be resolved.".into(),
        ),
        ureq::Error::Io(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            (504, "timeout", "The service did not answer in time.".into())
        }
        ureq::Error::Io(e) => (502, "upstream", format!("Could not reach the service: {e}")),
        other => (502, "upstream", format!("Could not reach the service: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_are_normalized_as_browsers_send_them() {
        assert_eq!(
            normalize_origin("https://Jellyfesh.Example").as_deref(),
            Some("https://jellyfesh.example")
        );
        assert_eq!(
            normalize_origin("https://jellyfesh.example:443/").as_deref(),
            Some("https://jellyfesh.example")
        );
        assert_eq!(
            normalize_origin("http://localhost:4173").as_deref(),
            Some("http://localhost:4173")
        );
        for bad in [
            "null",
            "jellyfesh.example",
            "https://jellyfesh.example/app/",
            "file:///x",
            "https://u:p@a.example",
        ] {
            assert_eq!(normalize_origin(bad), None, "{bad}");
        }
    }

    #[test]
    fn tokens_compare_whole() {
        assert!(same(b"0123456789abcdef", b"0123456789abcdef"));
        assert!(!same(b"0123456789abcdef", b"0123456789abcdeF"));
        assert!(!same(b"0123456789abcdef", b"0123456789abcde"));
        let token = new_token().unwrap();
        assert_eq!(token.len(), 32);
        assert_ne!(token, new_token().unwrap());
    }

    #[test]
    fn json_in_headers_is_ascii() {
        let value = serde_json::json!([["x-name", "Zoë 🎵"]]);
        let text = ascii_json(&value);
        assert!(text.is_ascii());
        assert_eq!(serde_json::from_str::<serde_json::Value>(&text).unwrap(), value);
    }
}

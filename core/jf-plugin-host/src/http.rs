//! The `http` import: requests limited to the plugin's allowlist.
//!
//! Each plugin has its own [`HttpClient`]: a `ureq` agent (connection pool),
//! a token-bucket rate limiter and the plugin's cookie jar. What it enforces
//! is listed in the WIT (`plugins/wit/imports.wit`); the parts that matter
//! most:
//!
//! * ureq never follows redirects itself; every hop goes back through the
//!   allowlist check here and through the resolver below;
//! * the resolver ([`SandboxResolver`]) looks the name up once, refuses the
//!   whole answer if any address may not be used, and gives the connector
//!   only the checked addresses, so the connection goes where the check said;
//! * proxies from the environment are ignored: a proxy would resolve names
//!   itself and defeat the address checks;
//! * bodies are read with a hard limit, after checking `content-length`.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use ureq::http;
use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};
use url::Url;

use crate::allow::{Access, Allowlist};
use crate::bindings::jellyfesh::plugin::http::{HttpError, Method, Request, Response};
use crate::bindings::jellyfesh::plugin::types::Header;
use crate::cookies::CookieJar;
use crate::net::{AddressClass, Resolve, classify};

// The limits are the model's, shared with Jellyfesh Web's plugin workers.
pub use jf_plugin_model::http::{
    MAX_HEADER_BYTES, MAX_HEADERS, MAX_PER_CALL, MAX_REDIRECTS, MAX_REQUEST_BODY, MAX_RESPONSE, MAX_TIMEOUT,
};
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);

/// What one plugin call may still spend on HTTP.
#[derive(Debug, Clone)]
pub(crate) struct NetBudget {
    /// The call's wall-clock deadline.
    pub deadline: Instant,
    /// Response bytes received so far in this call.
    pub received: u64,
    /// After a `maker.make` call read the song's audio: the only origins its
    /// requests may go to (ADR-0019 §4).
    pub only: Option<Vec<String>>,
}

/// Why the sandbox refused a connection; travels through ureq as
/// `ureq::Error::Other`.
#[derive(Debug, thiserror::Error)]
enum SandboxError {
    #[error("{0}")]
    NotAllowed(String),
    #[error("{0}")]
    Blocked(String),
    #[error("{0}")]
    Dns(String),
}

/// A plugin's HTTP client.
#[derive(Debug)]
pub(crate) struct HttpClient {
    plugin_id: String,
    agent: ureq::Agent,
    allowlist: Arc<RwLock<Allowlist>>,
    limiter: Mutex<TokenBucket>,
    cookies: Arc<CookieJar>,
}

impl HttpClient {
    pub(crate) fn new(
        plugin_id: &str,
        allowlist: Arc<RwLock<Allowlist>>,
        max_rps: u32,
        cookies: Arc<CookieJar>,
        resolve: Arc<dyn Resolve>,
    ) -> HttpClient {
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::Rustls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .proxy(None)
            .user_agent("Jellyfesh")
            .tls_config(tls)
            .max_idle_connections(8)
            .max_idle_connections_per_host(4)
            .max_response_header_size(MAX_HEADER_BYTES)
            .build();
        let resolver = SandboxResolver {
            allowlist: allowlist.clone(),
            resolve,
        };
        HttpClient {
            plugin_id: plugin_id.to_string(),
            agent: ureq::Agent::with_parts(config, DefaultConnector::new(), resolver),
            allowlist,
            limiter: Mutex::new(TokenBucket::new(max_rps)),
            cookies,
        }
    }

    /// Whether `url` is covered by the allowlist (used for URLs the plugin
    /// hands back: stream URLs, artwork, browser pages).
    pub(crate) fn allows(&self, url: &Url) -> bool {
        self.allowlist.read().check(url).is_ok()
    }

    /// Sends a request, following redirects if asked.
    pub(crate) fn send(&self, request: Request, budget: &mut NetBudget) -> Result<Response, HttpError> {
        let started = Instant::now();
        let timeout = request.timeout_ms.map_or(MAX_TIMEOUT, |ms| {
            Duration::from_millis(u64::from(ms)).min(MAX_TIMEOUT)
        });
        let deadline = budget.deadline.min(started + timeout);

        if request.body.len() > MAX_REQUEST_BODY {
            return Err(HttpError::InvalidRequest(
                "the request body is larger than 16 MiB".into(),
            ));
        }
        let mut headers = check_headers(&request.headers)?;
        let mut method = request.method;
        let mut body = request.body;
        if method == Method::Head && !body.is_empty() {
            return Err(HttpError::InvalidRequest(
                "a HEAD request cannot have a body".into(),
            ));
        }
        let mut url = Url::parse(&request.url)
            .map_err(|e| HttpError::InvalidRequest(format!("bad URL {:?}: {e}", request.url)))?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(HttpError::InvalidRequest(
                "URLs with user information are not allowed".into(),
            ));
        }
        url.set_fragment(None);

        for hop in 0..=MAX_REDIRECTS {
            if let Some(only) = &budget.only {
                let origin = url.origin().ascii_serialization();
                if !only.contains(&origin) {
                    return Err(HttpError::NotAllowed(format!(
                        "{origin}: this call read the song's audio, which may only go where the person allowed"
                    )));
                }
            }
            self.allowlist.read().check(&url).map_err(HttpError::NotAllowed)?;
            self.wait_for_token(deadline)?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(HttpError::Timeout);
            }

            let mut builder = http::Request::builder()
                .method(http_method(method))
                .uri(url.as_str());
            let mut cookie_header = self.cookies.header_for(&url);
            for (name, value) in &headers {
                if name == http::header::COOKIE {
                    let merged = match cookie_header.take() {
                        Some(jar) => format!("{jar}; {}", value.to_str().unwrap_or_default()),
                        None => value.to_str().unwrap_or_default().to_string(),
                    };
                    cookie_header = Some(merged);
                } else {
                    builder = builder.header(name, value);
                }
            }
            if let Some(cookie) = cookie_header {
                builder = builder.header(http::header::COOKIE, cookie);
            }
            let http_request = builder
                .body(body.clone())
                .map_err(|e| HttpError::InvalidRequest(e.to_string()))?;
            let http_request = self
                .agent
                .configure_request(http_request)
                .timeout_global(Some(remaining))
                .build();
            let mut response = self.agent.run(http_request).map_err(map_error)?;

            let status = response.status().as_u16();
            let set_cookie: Vec<String> = response
                .headers()
                .get_all(http::header::SET_COOKIE)
                .iter()
                .filter_map(|v| v.to_str().ok().map(str::to_string))
                .collect();
            self.cookies.store_response(&url, &set_cookie);

            let location = response
                .headers()
                .get(http::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            if request.follow_redirects
                && matches!(status, 301 | 302 | 303 | 307 | 308)
                && let Some(location) = location
            {
                if hop == MAX_REDIRECTS {
                    return Err(HttpError::TooManyRedirects);
                }
                let next = url
                    .join(&location)
                    .map_err(|e| HttpError::InvalidRequest(format!("bad redirect {location:?}: {e}")))?;
                if status == 303 || (matches!(status, 301 | 302) && method == Method::Post) {
                    method = Method::Get;
                    body = Vec::new();
                    headers.retain(|(name, _)| {
                        name != http::header::CONTENT_TYPE && name != http::header::CONTENT_ENCODING
                    });
                }
                if next.origin() != url.origin() {
                    // Credentials the plugin set for one origin never travel to another.
                    headers.retain(|(name, _)| {
                        name != http::header::AUTHORIZATION && name != http::header::COOKIE
                    });
                }
                tracing::debug!(plugin = %self.plugin_id, from = %url, to = %next, "following a redirect");
                url = next;
                continue;
            }

            let left = MAX_PER_CALL.saturating_sub(budget.received);
            let limit = MAX_RESPONSE.min(left);
            if response
                .headers()
                .get(http::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
                .is_some_and(|len| len > limit)
            {
                return Err(HttpError::TooLarge);
            }
            let response_headers = collect_headers(response.headers());
            let data = response
                .body_mut()
                .with_config()
                .limit(limit)
                .read_to_vec()
                .map_err(|e| match e {
                    ureq::Error::BodyExceedsLimit(_) => HttpError::TooLarge,
                    other => map_error(other),
                })?;
            budget.received += data.len() as u64;
            return Ok(Response {
                status,
                headers: response_headers,
                body: data,
                url: url.to_string(),
            });
        }
        Err(HttpError::TooManyRedirects)
    }

    fn wait_for_token(&self, deadline: Instant) -> Result<(), HttpError> {
        loop {
            let wait = self.limiter.lock().take(Instant::now());
            match wait {
                None => return Ok(()),
                Some(wait) => {
                    if Instant::now() + wait > deadline {
                        return Err(HttpError::RateLimited);
                    }
                    std::thread::sleep(wait);
                }
            }
        }
    }
}

fn http_method(method: Method) -> http::Method {
    match method {
        Method::Get => http::Method::GET,
        Method::Head => http::Method::HEAD,
        Method::Post => http::Method::POST,
        Method::Put => http::Method::PUT,
        Method::Patch => http::Method::PATCH,
        Method::Delete => http::Method::DELETE,
        Method::Options => http::Method::OPTIONS,
    }
}

/// Validates the plugin's request headers.
fn check_headers(headers: &[Header]) -> Result<Vec<(http::HeaderName, http::HeaderValue)>, HttpError> {
    if headers.len() > MAX_HEADERS {
        return Err(HttpError::InvalidRequest(format!(
            "more than {MAX_HEADERS} headers"
        )));
    }
    let total: usize = headers.iter().map(|h| h.name.len() + h.value.len()).sum();
    if total > MAX_HEADER_BYTES {
        return Err(HttpError::InvalidRequest(
            "the headers are larger than 64 KiB".into(),
        ));
    }
    let mut out = Vec::with_capacity(headers.len());
    for header in headers {
        let name = http::HeaderName::from_bytes(header.name.as_bytes())
            .map_err(|_| HttpError::InvalidRequest(format!("bad header name {:?}", header.name)))?;
        let lower = name.as_str();
        if jf_plugin_model::http::host_sets_header(lower) {
            return Err(HttpError::InvalidRequest(format!(
                "the host sets the {lower} header"
            )));
        }
        let value = http::HeaderValue::from_str(&header.value)
            .map_err(|_| HttpError::InvalidRequest(format!("bad value for header {lower}")))?;
        out.push((name, value));
    }
    Ok(out)
}

fn collect_headers(headers: &http::HeaderMap) -> Vec<Header> {
    headers
        .iter()
        .take(256)
        .map(|(name, value)| Header {
            name: name.as_str().to_string(),
            value: String::from_utf8_lossy(value.as_bytes()).into_owned(),
        })
        .collect()
}

fn map_error(error: ureq::Error) -> HttpError {
    match error {
        ureq::Error::Other(inner) => match inner.downcast::<SandboxError>() {
            Ok(sandbox) => match *sandbox {
                SandboxError::NotAllowed(m) => HttpError::NotAllowed(m),
                SandboxError::Blocked(m) => HttpError::BlockedAddress(m),
                SandboxError::Dns(m) => HttpError::Dns(m),
            },
            Err(other) => HttpError::Connection(other.to_string()),
        },
        ureq::Error::Timeout(_) => HttpError::Timeout,
        ureq::Error::HostNotFound => HttpError::Dns("host not found".into()),
        ureq::Error::Io(e) if e.kind() == std::io::ErrorKind::TimedOut => HttpError::Timeout,
        ureq::Error::BodyExceedsLimit(_) | ureq::Error::LargeResponseHeader(..) => HttpError::TooLarge,
        ureq::Error::BadUri(m) => HttpError::InvalidRequest(m),
        ureq::Error::TooManyRedirects => HttpError::TooManyRedirects,
        other => HttpError::Connection(other.to_string()),
    }
}

/// Resolves once, checks every address, and returns only checked ones.
#[derive(Debug)]
struct SandboxResolver {
    allowlist: Arc<RwLock<Allowlist>>,
    resolve: Arc<dyn Resolve>,
}

impl Resolver for SandboxResolver {
    fn resolve(
        &self,
        uri: &http::Uri,
        _config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let fail = |e: SandboxError| ureq::Error::Other(Box::new(e));
        let scheme = uri.scheme_str().unwrap_or_default();
        let host = uri.host().unwrap_or_default();
        let port = uri.port_u16().unwrap_or(if scheme == "https" { 443 } else { 80 });
        // Checked again here, so the resolver never resolves a name the
        // allowlist does not cover, whoever calls it.
        let access = self
            .allowlist
            .read()
            .check_origin(scheme, host.trim_start_matches('[').trim_end_matches(']'), port)
            .ok_or_else(|| {
                fail(SandboxError::NotAllowed(format!(
                    "{host}:{port} is not on the allowlist"
                )))
            })?;

        let literal = host.trim_start_matches('[').trim_end_matches(']');
        let ips = match literal.parse::<IpAddr>() {
            Ok(ip) => vec![ip],
            Err(_) => {
                let wait = if timeout.after.is_not_happening() {
                    RESOLVE_TIMEOUT
                } else {
                    (*timeout.after).min(RESOLVE_TIMEOUT)
                };
                lookup_with_timeout(self.resolve.clone(), host.to_string(), port, wait)
                    .map_err(|e| fail(SandboxError::Dns(e)))?
            }
        };
        if ips.is_empty() {
            return Err(fail(SandboxError::Dns(format!("{host} has no addresses"))));
        }
        for ip in &ips {
            match (classify(*ip), access) {
                (AddressClass::Public, _) | (AddressClass::Local, Access::UserServer) => {}
                (AddressClass::Local, Access::Fixed) => {
                    return Err(fail(SandboxError::Blocked(format!(
                        "{host} resolved to {ip}, a private address; only your own server may use those"
                    ))));
                }
                (AddressClass::Forbidden, _) => {
                    return Err(fail(SandboxError::Blocked(format!(
                        "{host} resolved to {ip}, which is never allowed"
                    ))));
                }
            }
        }
        let mut out = self.empty();
        // ResolvedSocketAddrs holds at most 16 addresses.
        for ip in ips.into_iter().take(16) {
            out.push(SocketAddr::new(ip, port));
        }
        Ok(out)
    }
}

fn lookup_with_timeout(
    resolve: Arc<dyn Resolve>,
    host: String,
    port: u16,
    wait: Duration,
) -> Result<Vec<IpAddr>, String> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    let name = host.clone();
    std::thread::Builder::new()
        .name("jf-plugin-dns".into())
        .spawn(move || {
            let _ = tx.send(resolve.lookup(&name, port));
        })
        .map_err(|e| e.to_string())?;
    match rx.recv_timeout(wait) {
        Ok(Ok(ips)) => Ok(ips),
        Ok(Err(e)) => Err(format!("cannot resolve {host}: {e}")),
        Err(_) => Err(format!("resolving {host} timed out")),
    }
}

/// A token bucket: bursts of `rate` requests, refilled at `rate` per second.
#[derive(Debug)]
struct TokenBucket {
    rate: f64,
    tokens: f64,
    last: Option<Instant>,
}

impl TokenBucket {
    fn new(rate: u32) -> TokenBucket {
        let rate = f64::from(rate.max(1));
        TokenBucket {
            rate,
            tokens: rate,
            last: None,
        }
    }

    /// Takes a token, or says how long to wait for one.
    fn take(&mut self, now: Instant) -> Option<Duration> {
        if let Some(last) = self.last {
            let elapsed = now.saturating_duration_since(last).as_secs_f64();
            self.tokens = (self.tokens + elapsed * self.rate).min(self.rate);
        }
        self.last = Some(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            None
        } else {
            Some(Duration::from_secs_f64((1.0 - self.tokens) / self.rate))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_bucket_allows_bursts_then_paces() {
        let mut bucket = TokenBucket::new(4);
        let t0 = Instant::now();
        for _ in 0..4 {
            assert_eq!(bucket.take(t0), None);
        }
        let wait = bucket.take(t0).unwrap();
        assert!(
            wait > Duration::from_millis(200) && wait <= Duration::from_millis(250),
            "{wait:?}"
        );
        assert_eq!(bucket.take(t0 + Duration::from_millis(260)), None);
    }

    #[test]
    fn rejects_headers_the_host_owns() {
        for name in [
            "Host",
            "content-length",
            "Transfer-Encoding",
            "Proxy-Authorization",
            "connection",
            "expect",
        ] {
            let header = Header {
                name: name.into(),
                value: "x".into(),
            };
            assert!(check_headers(&[header]).is_err(), "{name}");
        }
        let injected = Header {
            name: "x-test".into(),
            value: "a\r\nhost: evil".into(),
        };
        assert!(check_headers(&[injected]).is_err());
        let fine = Header {
            name: "Authorization".into(),
            value: "Bearer abc".into(),
        };
        assert_eq!(check_headers(&[fine]).unwrap().len(), 1);
    }
}

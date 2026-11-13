//! HTTP through the host's `http` import, with JSON helpers.
//!
//! Only URLs on the manifest's allowlist work; the host follows redirects
//! (re-checking each hop), keeps the plugin's cookies and enforces size,
//! rate and time limits. See `plugins/wit/imports.wit` for the details.
//!
//! ```no_run
//! # use jf_plugin_sdk::prelude::*;
//! # fn f() -> Result<()> {
//! #[derive(serde::Deserialize)]
//! struct Ping { version: String }
//! let ping: Ping = http::Request::get("https://api.example.com/ping")
//!     .bearer("token")
//!     .send_json()?;
//! # Ok(()) }
//! ```

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::bindings::jellyfesh::plugin::http as wit;
use crate::{Header, PluginError, Result};

pub use wit::{HttpError, Method};

/// An outgoing request, built step by step.
#[derive(Debug, Clone)]
pub struct Request {
    inner: wit::Request,
}

impl Request {
    /// A request with no headers or body that follows redirects.
    pub fn new(method: Method, url: impl Into<String>) -> Request {
        Request {
            inner: wit::Request {
                method,
                url: url.into(),
                headers: Vec::new(),
                body: Vec::new(),
                timeout_ms: None,
                follow_redirects: true,
            },
        }
    }

    /// `GET url`.
    pub fn get(url: impl Into<String>) -> Request {
        Request::new(Method::Get, url)
    }

    /// `POST url`.
    pub fn post(url: impl Into<String>) -> Request {
        Request::new(Method::Post, url)
    }

    /// `PUT url`.
    pub fn put(url: impl Into<String>) -> Request {
        Request::new(Method::Put, url)
    }

    /// `PATCH url`.
    pub fn patch(url: impl Into<String>) -> Request {
        Request::new(Method::Patch, url)
    }

    /// `DELETE url`.
    pub fn delete(url: impl Into<String>) -> Request {
        Request::new(Method::Delete, url)
    }

    /// `HEAD url`.
    pub fn head(url: impl Into<String>) -> Request {
        Request::new(Method::Head, url)
    }

    /// Adds a header.
    pub fn header(mut self, name: &str, value: &str) -> Request {
        self.inner.headers.push(Header::new(name, value));
        self
    }

    /// Adds `Authorization: Bearer <token>`.
    pub fn bearer(self, token: &str) -> Request {
        self.header("authorization", &format!("Bearer {token}"))
    }

    /// Appends query parameters, percent-encoded.
    pub fn query(mut self, params: &[(&str, &str)]) -> Request {
        if params.is_empty() {
            return self;
        }
        let url = &mut self.inner.url;
        let (base, fragment) = match url.find('#') {
            Some(at) => (url[..at].to_string(), url[at..].to_string()),
            None => (url.clone(), String::new()),
        };
        let separator = if base.contains('?') {
            if base.ends_with('?') || base.ends_with('&') {
                ""
            } else {
                "&"
            }
        } else {
            "?"
        };
        *url = format!("{base}{separator}{}{fragment}", form_encode(params));
        self
    }

    /// Sets the body.
    pub fn body(mut self, body: Vec<u8>, content_type: &str) -> Request {
        self.inner.body = body;
        self.header("content-type", content_type)
    }

    /// Sets a JSON body.
    pub fn json<T: Serialize + ?Sized>(self, value: &T) -> Result<Request> {
        let body = serde_json::to_vec(value).map_err(|e| PluginError::internal(format!("JSON: {e}")))?;
        Ok(self.body(body, "application/json"))
    }

    /// Sets an `application/x-www-form-urlencoded` body.
    pub fn form(self, params: &[(&str, &str)]) -> Request {
        self.body(
            form_encode(params).into_bytes(),
            "application/x-www-form-urlencoded",
        )
    }

    /// Limits the whole request (all hops) to `ms` milliseconds.
    pub fn timeout_ms(mut self, ms: u32) -> Request {
        self.inner.timeout_ms = Some(ms);
        self
    }

    /// Returns redirect responses instead of following them.
    pub fn no_redirects(mut self) -> Request {
        self.inner.follow_redirects = false;
        self
    }

    /// Sends the request; any status is a response.
    pub fn send(self) -> core::result::Result<Response, HttpError> {
        wit::send(&self.inner).map(|r| Response {
            status: r.status,
            headers: r.headers,
            body: r.body,
            url: r.url,
        })
    }

    /// Sends the request, maps error statuses to [`PluginError`]s
    /// ([`Response::error_for_status`]) and parses the body as JSON.
    pub fn send_json<T: DeserializeOwned>(self) -> Result<T> {
        self.send()?.error_for_status()?.json()
    }
}

/// A complete response.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    /// Status code.
    pub status: u16,
    /// Headers, names in lowercase.
    pub headers: Vec<Header>,
    /// Body.
    pub body: Vec<u8>,
    /// The URL that answered, after redirects.
    pub url: String,
}

impl Response {
    /// The first header named `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.as_str())
    }

    /// Whether the status is 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The body as text (invalid UTF-8 replaced).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The body parsed as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_slice(&self.body)?)
    }

    /// Keeps 2xx (and 3xx) responses; maps 401/403 to
    /// [`PluginError::AuthRequired`], 404 to [`PluginError::NotFound`], 429
    /// to [`PluginError::RateLimited`] and anything else to
    /// [`PluginError::Service`].
    pub fn error_for_status(self) -> Result<Response> {
        match self.status {
            200..=399 => Ok(self),
            401 | 403 => Err(PluginError::AuthRequired),
            404 => Err(PluginError::NotFound),
            429 => Err(PluginError::RateLimited),
            status => Err(PluginError::Service(format!(
                "the service answered HTTP {status}"
            ))),
        }
    }
}

/// `GET url` and parse the JSON answer.
pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    Request::get(url).send_json()
}

/// Percent-encodes a URL component (everything but `A-Z a-z 0-9 - . _ ~`).
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `a=1&b=2`, percent-encoded.
pub fn form_encode(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_queries() {
        assert_eq!(encode("a b&c/ü"), "a%20b%26c%2F%C3%BC");
        let r = Request::get("https://x.example/s").query(&[("q", "rock & roll"), ("n", "1")]);
        assert_eq!(r.inner.url, "https://x.example/s?q=rock%20%26%20roll&n=1");
        let r = Request::get("https://x.example/s?a=1#frag").query(&[("b", "2")]);
        assert_eq!(r.inner.url, "https://x.example/s?a=1&b=2#frag");
    }

    #[test]
    fn maps_statuses() {
        let response = |status| Response {
            status,
            headers: vec![Header::new("Content-Type", "text/plain")],
            body: Vec::new(),
            url: String::new(),
        };
        assert!(response(204).error_for_status().is_ok());
        assert_eq!(response(401).error_for_status(), Err(PluginError::AuthRequired));
        assert_eq!(response(404).error_for_status(), Err(PluginError::NotFound));
        assert_eq!(response(429).error_for_status(), Err(PluginError::RateLimited));
        assert!(matches!(
            response(500).error_for_status(),
            Err(PluginError::Service(_))
        ));
        assert_eq!(response(200).header("content-type"), Some("text/plain"));
    }
}

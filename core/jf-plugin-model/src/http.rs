//! The `http` import's request rules, shared by every host.
//!
//! `jf-plugin-host` sends requests with ureq (and adds address checks, DNS
//! pinning and cookies); Jellyfesh Web sends them from a plugin worker. Both
//! check a request the same way first ([`check_request`]) and apply the same
//! limits, as `plugins/wit/imports.wit` states them.

use std::time::Duration;

use url::Url;

use crate::types::Header;

/// Largest response body.
pub const MAX_RESPONSE: u64 = 16 * 1024 * 1024;
/// Most response bytes one plugin call may receive in total.
pub const MAX_PER_CALL: u64 = 64 * 1024 * 1024;
/// Largest request body.
pub const MAX_REQUEST_BODY: usize = 16 * 1024 * 1024;
/// Longest a request may take.
pub const MAX_TIMEOUT: Duration = Duration::from_secs(30);
/// Most redirect hops followed.
pub const MAX_REDIRECTS: usize = 5;
/// Most request headers.
pub const MAX_HEADERS: usize = 64;
/// Largest request headers, names and values together.
pub const MAX_HEADER_BYTES: usize = 64 * 1024;

/// Whether the host sets this (lowercase) header itself, so a plugin may not.
pub fn host_sets_header(lower: &str) -> bool {
    matches!(
        lower,
        "host"
            | "content-length"
            | "transfer-encoding"
            | "connection"
            | "keep-alive"
            | "upgrade"
            | "te"
            | "trailer"
            | "expect"
    ) || lower.starts_with("proxy-")
}

/// Whether `name` is an HTTP token (RFC 9110).
pub fn valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Whether `value` is a valid field value: no control characters except tab.
pub fn valid_header_value(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b == b'\t' || (0x20..0x7f).contains(&b) || b >= 0x80)
}

/// Checks the headers of a request: count, size, names, values, and none of
/// the headers the host sets. The text says what is wrong.
pub fn check_headers(headers: &[Header]) -> Result<(), String> {
    if headers.len() > MAX_HEADERS {
        return Err(format!("more than {MAX_HEADERS} headers"));
    }
    let total: usize = headers.iter().map(|h| h.name.len() + h.value.len()).sum();
    if total > MAX_HEADER_BYTES {
        return Err("the headers are larger than 64 KiB".into());
    }
    for header in headers {
        if !valid_header_name(&header.name) {
            return Err(format!("bad header name {:?}", header.name));
        }
        let lower = header.name.to_ascii_lowercase();
        if host_sets_header(&lower) {
            return Err(format!("the host sets the {lower} header"));
        }
        if !valid_header_value(&header.value) {
            return Err(format!("bad value for header {lower}"));
        }
    }
    Ok(())
}

/// Checks a request's URL, body and method: an absolute URL without user
/// information (the fragment is dropped), a body within the limit, no body
/// on `HEAD`. Returns the URL to send. The text says what is wrong.
pub fn check_request(url: &str, headers: &[Header], body_len: usize, is_head: bool) -> Result<Url, String> {
    if body_len > MAX_REQUEST_BODY {
        return Err("the request body is larger than 16 MiB".into());
    }
    check_headers(headers)?;
    if is_head && body_len > 0 {
        return Err("a HEAD request cannot have a body".into());
    }
    let mut parsed = Url::parse(url).map_err(|e| format!("bad URL {url:?}: {e}"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("URLs with user information are not allowed".into());
    }
    parsed.set_fragment(None);
    Ok(parsed)
}

/// The request timeout: what the plugin asked for, at most [`MAX_TIMEOUT`].
pub fn timeout(requested_ms: Option<u32>) -> Duration {
    requested_ms.map_or(MAX_TIMEOUT, |ms| {
        Duration::from_millis(u64::from(ms)).min(MAX_TIMEOUT)
    })
}

/// A token bucket for `max_rps`: bursts of `rate`, then `rate` a second.
/// Times are milliseconds from any fixed start, so hosts with and without
/// `Instant` can use it.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    rate: f64,
    tokens: f64,
    last_ms: Option<f64>,
}

impl TokenBucket {
    /// A full bucket for `rate` requests a second (at least 1).
    pub fn new(rate: u32) -> TokenBucket {
        let rate = f64::from(rate.max(1));
        TokenBucket {
            rate,
            tokens: rate,
            last_ms: None,
        }
    }

    /// Takes a token at `now_ms`, or says how many milliseconds to wait for one.
    pub fn take(&mut self, now_ms: f64) -> Option<f64> {
        if let Some(last) = self.last_ms {
            let elapsed = ((now_ms - last) / 1000.0).max(0.0);
            self.tokens = (self.tokens + elapsed * self.rate).min(self.rate);
        }
        self.last_ms = Some(now_ms);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            None
        } else {
            Some((1.0 - self.tokens) / self.rate * 1000.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(name: &str, value: &str) -> Header {
        Header {
            name: name.into(),
            value: value.into(),
        }
    }

    #[test]
    fn refuses_what_the_host_sets_and_bad_headers() {
        assert!(check_headers(&[header("Authorization", "MediaBrowser Token=\"x\"")]).is_ok());
        for bad in [
            header("Host", "x"),
            header("content-length", "3"),
            header("Proxy-Authorization", "x"),
            header("bad name", "x"),
            header("x-ok", "line\nbreak"),
        ] {
            assert!(check_headers(std::slice::from_ref(&bad)).is_err(), "{bad:?}");
        }
        let many: Vec<Header> = (0..=MAX_HEADERS)
            .map(|i| header(&format!("x-{i}"), "1"))
            .collect();
        assert!(check_headers(&many).is_err());
    }

    #[test]
    fn checks_urls_bodies_and_methods() {
        let url = check_request("https://music.example.com/rest/ping#frag", &[], 0, false).unwrap();
        assert_eq!(url.as_str(), "https://music.example.com/rest/ping");
        assert!(check_request("https://user:pw@music.example.com/", &[], 0, false).is_err());
        assert!(check_request("not a url", &[], 0, false).is_err());
        assert!(check_request("https://music.example.com/", &[], 1, true).is_err());
        assert!(check_request("https://music.example.com/", &[], MAX_REQUEST_BODY + 1, false).is_err());
        assert_eq!(timeout(Some(100)), Duration::from_millis(100));
        assert_eq!(timeout(Some(100_000)), MAX_TIMEOUT);
        assert_eq!(timeout(None), MAX_TIMEOUT);
    }

    #[test]
    fn token_bucket_allows_bursts_then_paces() {
        let mut bucket = TokenBucket::new(4);
        for _ in 0..4 {
            assert_eq!(bucket.take(0.0), None);
        }
        let wait = bucket.take(0.0).unwrap();
        assert!((wait - 250.0).abs() < 1e-6, "{wait}");
        assert_eq!(bucket.take(250.0), None);
    }
}

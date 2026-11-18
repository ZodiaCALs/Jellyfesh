//! A plugin's cookie jar in the browser, for the requests that go through a relay
//! (`docs/web-relay.md`).
//!
//! The native host keeps one jar per plugin (`cookie_store`) and sends its cookies with every
//! request. A page cannot use the browser's own jar for another site, and it never sees
//! `Set-Cookie`; through the relay the service's `Set-Cookie` headers reach the plugin's worker,
//! so the worker keeps the plugin's cookies itself, by the rules of RFC 6265 §5 as the native
//! jar applies them: host-only and domain cookies, path matching, `Secure` only over https,
//! `Max-Age` before `Expires`, an expired cookie deleting its namesake. Cookies without an
//! expiry last for the visit; the page keeps the others encrypted with the plugin's secrets, and
//! signing out deletes them.

use serde::{Deserialize, Serialize};
use url::Url;

/// Most cookies kept per domain, and in all.
const MAX_PER_DOMAIN: usize = 50;
const MAX_COOKIES: usize = 300;
/// Longest name and value together.
const MAX_COOKIE: usize = 4096;

/// One cookie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    /// The host it was set by (`host_only`), or the domain it covers, lowercase.
    pub domain: String,
    pub host_only: bool,
    pub path: String,
    pub secure: bool,
    /// When it expires, in ms since the epoch; `None` lasts for the visit.
    pub expires_ms: Option<u64>,
}

/// The cookies of one plugin.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Jar {
    cookies: Vec<Cookie>,
}

/// RFC 6265 §5.1.1 delimiters.
fn delimiter(b: u8) -> bool {
    b == 0x09
        || (0x20..=0x2f).contains(&b)
        || (0x3b..=0x40).contains(&b)
        || (0x5b..=0x60).contains(&b)
        || (0x7b..=0x7e).contains(&b)
}

/// Leading digits (at least `min`, at most `max`) followed by nothing or a non-digit.
fn digits(token: &str, min: usize, max: usize) -> Option<u32> {
    let count = token.bytes().take_while(u8::is_ascii_digit).count();
    (min..=max)
        .contains(&count)
        .then(|| token[..count].parse().ok())?
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (i64::from(month) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// An `Expires` date by RFC 6265 §5.1.1, in ms since the epoch.
pub fn parse_date(text: &str) -> Option<u64> {
    let mut time: Option<(u32, u32, u32)> = None;
    let mut day: Option<u32> = None;
    let mut month: Option<u32> = None;
    let mut year: Option<u32> = None;
    for token in text.as_bytes().split(|b| delimiter(*b)).filter(|t| !t.is_empty()) {
        let Ok(token) = std::str::from_utf8(token) else {
            continue;
        };
        if time.is_none() {
            let mut fields = token.splitn(3, ':');
            if let (Some(h), Some(m), Some(s)) = (fields.next(), fields.next(), fields.next())
                && let (Some(h), Some(m), Some(s)) = (
                    digits(h, 1, 2).filter(|_| h.len() <= 2),
                    digits(m, 1, 2).filter(|_| m.len() <= 2),
                    digits(s, 1, 2),
                )
            {
                time = Some((h, m, s));
                continue;
            }
        }
        if day.is_none()
            && let Some(d) = digits(token, 1, 2)
        {
            day = Some(d);
            continue;
        }
        if month.is_none()
            && let Some(head) = token.get(..3)
        {
            const MONTHS: [&str; 12] = [
                "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
            ];
            if let Some(index) = MONTHS.iter().position(|m| m.eq_ignore_ascii_case(head)) {
                month = Some(index as u32 + 1);
                continue;
            }
        }
        if year.is_none()
            && let Some(y) = digits(token, 2, 4)
        {
            year = Some(y);
        }
    }
    let (hour, minute, second) = time?;
    let (day, month, mut year) = (day?, month?, year?);
    if (70..=99).contains(&year) {
        year += 1900;
    } else if year <= 69 {
        year += 2000;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in_month = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ][month as usize - 1];
    if day < 1 || day > days_in_month || year < 1601 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let days = days_from_civil(i64::from(year), month, day);
    let seconds = days * 86_400 + i64::from(hour * 3600 + minute * 60 + second);
    u64::try_from(seconds).ok().map(|s| s * 1000)
}

/// The default path of a cookie set by `url` (RFC 6265 §5.1.4).
fn default_path(url: &Url) -> String {
    let path = url.path();
    match path.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(at) => path[..at].to_owned(),
    }
}

fn domain_matches(host: &str, domain: &str) -> bool {
    host == domain
        || (host.len() > domain.len()
            && host.ends_with(domain)
            && host.as_bytes()[host.len() - domain.len() - 1] == b'.'
            && host.parse::<std::net::IpAddr>().is_err())
}

fn path_matches(request: &str, cookie: &str) -> bool {
    request == cookie
        || (request.starts_with(cookie)
            && (cookie.ends_with('/') || request.as_bytes().get(cookie.len()) == Some(&b'/')))
}

impl Jar {
    /// A jar from what [`Jar::to_json`] wrote; an empty one when it cannot be read.
    pub fn from_json(text: &str) -> Jar {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{\"cookies\":[]}".into())
    }

    /// Only the cookies that outlive the visit and have not expired.
    pub fn persistent(&self, now_ms: u64) -> Jar {
        Jar {
            cookies: self
                .cookies
                .iter()
                .filter(|c| c.expires_ms.is_some_and(|at| at > now_ms))
                .cloned()
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    /// Stores one `Set-Cookie` header received from `url`; true when the jar changed.
    pub fn store(&mut self, url: &Url, header: &str, now_ms: u64) -> bool {
        let Some(host) = url
            .host_str()
            .map(|h| h.trim_end_matches('.').to_ascii_lowercase())
        else {
            return false;
        };
        let mut parts = header.split(';');
        let pair = parts.next().unwrap_or_default();
        let Some((name, value)) = pair.split_once('=') else {
            return false;
        };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() || name.len() + value.len() > MAX_COOKIE {
            return false;
        }
        let mut expires: Option<u64> = None;
        let mut max_age: Option<u64> = None;
        let mut domain: Option<String> = None;
        let mut path: Option<String> = None;
        let mut secure = false;
        for attribute in parts {
            let (key, value) = match attribute.split_once('=') {
                Some((key, value)) => (key.trim(), value.trim()),
                None => (attribute.trim(), ""),
            };
            match key.to_ascii_lowercase().as_str() {
                "expires" => {
                    if let Some(at) = parse_date(value) {
                        expires = Some(at);
                    }
                }
                "max-age" => {
                    let negative = value.starts_with('-');
                    let number = value.strip_prefix('-').unwrap_or(value);
                    if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) {
                        let seconds: u64 = number.parse().unwrap_or(u64::MAX);
                        max_age = Some(if negative || seconds == 0 {
                            0
                        } else {
                            now_ms.saturating_add(seconds.saturating_mul(1000))
                        });
                    }
                }
                "domain" if !value.is_empty() => {
                    domain = Some(
                        value
                            .trim_start_matches('.')
                            .trim_end_matches('.')
                            .to_ascii_lowercase(),
                    );
                }
                "path" => path = value.starts_with('/').then(|| value.to_owned()),
                "secure" => secure = true,
                _ => {}
            }
        }
        if secure && url.scheme() != "https" {
            return false;
        }
        let (domain, host_only) = match domain {
            Some(domain) if domain == host => (domain, false),
            Some(domain) => {
                // A domain cookie must cover the host that set it, and more than a top-level domain.
                if !domain.contains('.') || !domain_matches(&host, &domain) {
                    return false;
                }
                (domain, false)
            }
            None => (host, true),
        };
        let cookie = Cookie {
            name: name.to_owned(),
            value: value.to_owned(),
            domain,
            host_only,
            path: path.unwrap_or_else(|| default_path(url)),
            secure,
            expires_ms: max_age.or(expires),
        };
        let before = self.cookies.len();
        let existing = self
            .cookies
            .iter()
            .position(|c| c.name == cookie.name && c.domain == cookie.domain && c.path == cookie.path);
        if cookie.expires_ms.is_some_and(|at| at <= now_ms) {
            // An expired cookie deletes its namesake.
            if let Some(index) = existing {
                self.cookies.remove(index);
            }
            return self.cookies.len() != before;
        }
        match existing {
            Some(index) if self.cookies[index] == cookie => return false,
            Some(index) => self.cookies[index] = cookie,
            None => {
                let domain = cookie.domain.clone();
                self.cookies.push(cookie);
                if self.cookies.iter().filter(|c| c.domain == domain).count() > MAX_PER_DOMAIN
                    && let Some(oldest) = self.cookies.iter().position(|c| c.domain == domain)
                {
                    self.cookies.remove(oldest);
                }
                if self.cookies.len() > MAX_COOKIES {
                    self.cookies.remove(0);
                }
            }
        }
        true
    }

    /// The `Cookie` header for a request to `url`, or `None`; expired cookies are dropped.
    pub fn header(&mut self, url: &Url, now_ms: u64) -> Option<String> {
        self.cookies.retain(|c| c.expires_ms.is_none_or(|at| at > now_ms));
        let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
        let path = if url.path().is_empty() { "/" } else { url.path() };
        let https = url.scheme() == "https";
        let mut matching: Vec<&Cookie> = self
            .cookies
            .iter()
            .filter(|c| {
                (if c.host_only {
                    host == c.domain
                } else {
                    domain_matches(&host, &c.domain)
                }) && path_matches(path, &c.path)
                    && (https || !c.secure)
            })
            .collect();
        if matching.is_empty() {
            return None;
        }
        // Longer paths first; otherwise in the order they were set.
        matching.sort_by_key(|c| std::cmp::Reverse(c.path.len()));
        Some(
            matching
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_000_000_000;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn reads_cookie_dates() {
        // 2026-12-01T08:00:00Z
        assert_eq!(
            parse_date("Tue, 01 Dec 2026 08:00:00 GMT"),
            Some(1_796_112_000_000)
        );
        assert_eq!(
            parse_date("Tuesday, 01-Dec-26 08:00:00 GMT"),
            Some(1_796_112_000_000)
        );
        assert_eq!(parse_date("Tue Dec  1 08:00:00 2026"), Some(1_796_112_000_000));
        assert_eq!(parse_date("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
        assert_eq!(
            parse_date("Sun, 29 Feb 2032 23:59:59 GMT"),
            Some(1_961_711_999_000)
        );
        // A token that is not ASCII is skipped, not cut in the middle of a character.
        assert_eq!(
            parse_date("a\u{65e5}, 01 Dec 2026 08:00:00 GMT"),
            Some(1_796_112_000_000)
        );
        for bad in [
            "",
            "Tue, 31 Nov 2026 08:00:00 GMT",
            "Tue, 01 Dec 2026",
            "01 Foo 2026 08:00:00",
            "Tue, 01 Dec 2026 24:00:00 GMT",
        ] {
            assert_eq!(parse_date(bad), None, "{bad}");
        }
    }

    #[test]
    fn keeps_and_sends_cookies_like_the_native_jar() {
        let mut jar = Jar::default();
        let login = url("https://music.163.com/weapi/login/qrcode/client/login?csrf_token=");
        assert!(jar.store(
            &login,
            "MUSIC_U=abc; Max-Age=1296000; Expires=Tue, 01 Dec 2026 08:00:00 GMT; Path=/; HTTPOnly",
            NOW
        ));
        assert!(jar.store(&login, "__csrf=0123abcd; Max-Age=1296010; Path=/", NOW));
        assert!(jar.store(&login, "NMTID=visit; Path=/; Domain=.163.com", NOW));
        assert!(
            !jar.store(&login, "__csrf=0123abcd; Max-Age=1296010; Path=/", NOW),
            "the same again changes nothing"
        );
        assert_eq!(
            jar.header(&url("https://music.163.com/weapi/song/enhance/player/url"), NOW)
                .as_deref(),
            Some("MUSIC_U=abc; __csrf=0123abcd; NMTID=visit")
        );
        // A domain cookie covers subdomains; host-only ones do not.
        assert_eq!(
            jar.header(&url("https://interface.music.163.com/x"), NOW)
                .as_deref(),
            Some("NMTID=visit")
        );
        assert_eq!(jar.header(&url("https://music.126.net/x"), NOW), None);
        // Max-Age won over Expires; a cookie past its time is gone.
        assert_eq!(
            jar.header(&url("https://music.163.com/"), NOW + 1_296_005_000)
                .as_deref(),
            Some("__csrf=0123abcd; NMTID=visit")
        );
    }

    #[test]
    fn follows_paths_secure_and_deletion() {
        let mut jar = Jar::default();
        let api = url("https://u.y.qq.com/cgi-bin/musics.fcg");
        assert!(jar.store(&api, "a=1", NOW));
        assert!(jar.store(&api, "b=2; Path=/cgi-bin/deeper", NOW));
        assert!(jar.store(&api, "s=3; Secure", NOW));
        assert_eq!(
            jar.header(&url("https://u.y.qq.com/cgi-bin/other"), NOW)
                .as_deref(),
            Some("a=1; s=3")
        );
        assert_eq!(
            jar.header(&url("https://u.y.qq.com/cgi-bin/deeper/x"), NOW)
                .as_deref(),
            Some("b=2; a=1; s=3"),
            "longer paths first"
        );
        assert_eq!(
            jar.header(&url("https://u.y.qq.com/cgi-bin/deeperx"), NOW)
                .as_deref(),
            Some("a=1; s=3")
        );
        assert_eq!(
            jar.header(&url("https://u.y.qq.com/"), NOW),
            None,
            "the default path is /cgi-bin"
        );
        // Secure cookies only over https, and only set over https.
        let plain = url("http://u.y.qq.com/cgi-bin/x");
        assert_eq!(jar.header(&plain, NOW).as_deref(), Some("a=1"));
        assert!(!jar.store(&plain, "t=4; Secure", NOW));
        // An expired cookie deletes its namesake.
        assert!(jar.store(&api, "a=gone; Max-Age=0", NOW));
        assert_eq!(
            jar.header(&url("https://u.y.qq.com/cgi-bin/other"), NOW)
                .as_deref(),
            Some("s=3")
        );
    }

    #[test]
    fn refuses_cookies_for_other_sites() {
        let mut jar = Jar::default();
        let api = url("https://music.163.com/api");
        assert!(!jar.store(&api, "x=1; Domain=evil.example", NOW));
        assert!(!jar.store(&api, "x=1; Domain=com", NOW), "a top-level domain");
        assert!(
            !jar.store(&api, "x=1; Domain=sic.music.163.com", NOW),
            "a domain the host is not in"
        );
        assert!(!jar.store(&api, "novalue", NOW));
        assert!(!jar.store(&api, "=1", NOW));
        let ip = url("http://192.168.1.20:4533/rest");
        assert!(jar.store(&ip, "y=2; Domain=192.168.1.20", NOW));
        assert_eq!(jar.header(&ip, NOW).as_deref(), Some("y=2"));
        assert_eq!(jar.header(&url("http://192.168.1.21:4533/rest"), NOW), None);
    }

    #[test]
    fn keeps_only_lasting_cookies() {
        let mut jar = Jar::default();
        let api = url("https://music.163.com/");
        jar.store(&api, "visit=1", NOW);
        jar.store(&api, "kept=2; Max-Age=60", NOW);
        let kept = jar.persistent(NOW);
        assert_eq!(Jar::from_json(&kept.to_json()), kept);
        let mut kept = kept;
        assert_eq!(kept.header(&api, NOW).as_deref(), Some("kept=2"));
        assert_eq!(Jar::from_json("not json"), Jar::default());
    }
}

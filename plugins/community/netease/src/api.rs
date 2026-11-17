//! Talking to NetEase Cloud Music: the stored session, weapi requests and
//! their errors, media URLs, and the plugin's small in-memory caches.
//!
//! Every request is a weapi `POST https://music.163.com/weapi/<path>` with
//! the parameters encoded by [`crate::weapi`], the session's `csrf_token`
//! in the query and the parameters, and the headers the web player sends.
//! The session cookie (`MUSIC_U`) lives in the host's cookie jar, which the
//! host keeps encrypted and adds to requests by itself; the plugin keeps only
//! the account id, its name and the `__csrf` value, in the system keystore.

use std::cell::RefCell;
use std::collections::HashMap;

use jf_community_common::{host_in, upgrade_to_https, url_host};
use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::settings::Settings;
use crate::weapi;

/// The web player's origin, where every API request goes.
pub const WEB_ORIGIN: &str = "https://music.163.com";
/// Streams and pictures come from subdomains of this domain
/// (`m701.music.126.net`, `p1.music.126.net`, …).
pub const MEDIA_DOMAIN: &str = "music.126.net";
/// The web endpoints are made for browsers, so the plugin sends a desktop
/// browser's User-Agent, as the web player would.
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
/// The keystore entry with the session.
const SESSION: &str = "session";
/// How long a fetched list is reused while the person pages through it.
const LIST_TTL_MS: u64 = 2 * 60 * 1000;
/// Most picture URLs remembered.
const MAX_COVERS: usize = 20_000;
/// Wait at most this long for one answer.
const TIMEOUT_MS: u32 = 15_000;

/// The signed-in account, as kept in the keystore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The account's user id (for its playlists and liked songs).
    pub user_id: u64,
    /// Its display name.
    pub nickname: String,
    /// The `__csrf` cookie's value, which weapi requests repeat as
    /// `csrf_token`.
    pub csrf: String,
}

#[derive(Debug, Default)]
struct State {
    settings: Settings,
    /// The session read from the keystore: outer `None` means not read yet.
    session: Option<Option<Session>>,
    /// Item id → picture URL, from listings, so `artwork` needs no request.
    covers: HashMap<String, String>,
    /// Lists of song ids (a playlist's, the liked songs) kept for paging.
    lists: Vec<(String, u64, Vec<u64>)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The current settings.
pub fn settings() -> Settings {
    STATE.with(|s| s.borrow().settings.clone())
}

/// New settings.
pub fn set_settings(settings: Settings) {
    STATE.with(|s| s.borrow_mut().settings = settings);
}

/// The mock service's origin, in a test build pointed at one.
#[cfg(feature = "mock-service")]
pub fn mock_origin() -> Option<String> {
    STATE.with(|s| s.borrow().settings.mock_origin.clone())
}

/// The URL the plugin requests (or hands out) for a NetEase URL: the URL
/// itself, or in a test build pointed at the mock service, the same host and
/// path under the mock's origin (`http://127.0.0.1:…/music.163.com/weapi/…`).
pub fn physical(url: &str) -> String {
    #[cfg(feature = "mock-service")]
    if let Some(origin) = mock_origin()
        && let Some(rest) = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
    {
        return format!("{origin}/{rest}");
    }
    url.to_string()
}

/// The stored session, if someone is signed in.
pub fn session() -> Result<Option<Session>> {
    if let Some(cached) = STATE.with(|s| s.borrow().session.clone()) {
        return Ok(cached);
    }
    let found = kv::secret::get(SESSION)?.and_then(|text| serde_json::from_str::<Session>(&text).ok());
    STATE.with(|s| s.borrow_mut().session = Some(found.clone()));
    Ok(found)
}

/// The session, or `auth-required`.
pub fn require_session() -> Result<Session> {
    session()?.ok_or(PluginError::AuthRequired)
}

/// Keeps a new session in the keystore.
pub fn save_session(session: &Session) -> Result<()> {
    let text = serde_json::to_string(session).map_err(|e| PluginError::internal(e.to_string()))?;
    kv::secret::set(SESSION, &text)?;
    STATE.with(|s| s.borrow_mut().session = Some(Some(session.clone())));
    Ok(())
}

/// Forgets the session and everything cached (sign-out; the host deletes
/// the keystore entry and the cookies itself).
pub fn forget() {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let settings = state.settings.clone();
        *state = State {
            settings,
            ..State::default()
        };
    });
}

/// An answer: the parsed body and the response it came in (for its
/// `set-cookie` headers).
#[derive(Debug, Clone)]
pub struct Answer {
    pub body: Value,
    pub response: http::Response,
}

impl Answer {
    /// The answer's `code` (200 when it has none).
    pub fn code(&self) -> i64 {
        code_of(&self.body)
    }
}

/// Sends a weapi request with the stored session's `csrf_token`.
pub fn send(path: &str, params: Value) -> Result<Answer> {
    let csrf = session()?.map(|s| s.csrf).unwrap_or_default();
    send_with(path, params, &csrf)
}

/// Sends a weapi request with this `csrf_token` (empty when signed out).
pub fn send_with(path: &str, params: Value, csrf: &str) -> Result<Answer> {
    let mut params = match params {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    params.insert("csrf_token".into(), Value::String(csrf.to_string()));
    let encoded = weapi::encode(&Value::Object(params).to_string())?;
    let url = physical(&format!(
        "{WEB_ORIGIN}/weapi{path}?csrf_token={}",
        http::encode(csrf)
    ));
    let response = http::Request::post(url)
        .header("referer", "https://music.163.com/")
        .header("origin", WEB_ORIGIN)
        .header("user-agent", USER_AGENT)
        .header("accept", "*/*")
        .form(&[
            ("params", encoded.params.as_str()),
            ("encSecKey", encoded.enc_sec_key.as_str()),
        ])
        .timeout_ms(TIMEOUT_MS)
        .send()?;
    match serde_json::from_slice::<Value>(&response.body) {
        Ok(body) if body.is_object() => Ok(Answer { body, response }),
        _ => Err(status_error(response.status)),
    }
}

/// Sends a weapi request and turns error codes into errors.
pub fn call(path: &str, params: Value) -> Result<Value> {
    let answer = send(path, params)?;
    check(&answer.body)?;
    Ok(answer.body)
}

/// Like [`call`], then reads the answer as `T`.
pub fn call_as<T: serde::de::DeserializeOwned>(path: &str, params: Value) -> Result<T> {
    Ok(serde_json::from_value(call(path, params)?)?)
}

/// An answer's `code`; 200 when it has none.
pub fn code_of(body: &Value) -> i64 {
    body.get("code").and_then(Value::as_i64).unwrap_or(200)
}

/// An answer's own message, if it has one.
pub fn message_of(body: &Value) -> Option<String> {
    ["message", "msg"]
        .iter()
        .filter_map(|key| body.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|m| !m.is_empty())
        .map(str::to_string)
}

/// Turns an answer's error code into an error.
pub fn check(body: &Value) -> Result<()> {
    let code = code_of(body);
    match code {
        200 => Ok(()),
        // "Needs a sign-in": signed out, or the session expired.
        301 => Err(PluginError::AuthRequired),
        404 => Err(PluginError::NotFound),
        // "Too many requests".
        405 | 406 => Err(PluginError::RateLimited),
        -460 => Err(PluginError::service(
            "NetEase Cloud Music turned the request away as automated (code -460). Try again later.",
        )),
        -462 | 8821 => Err(PluginError::service(format!(
            "NetEase Cloud Music asked for a security check that Jellyfesh cannot show (code {code}). \
             Try again later."
        ))),
        _ => Err(PluginError::service(match message_of(body) {
            Some(message) => format!("NetEase Cloud Music: {message} (code {code})"),
            None => format!("NetEase Cloud Music reported error {code}."),
        })),
    }
}

/// An answer that is not NetEase JSON.
fn status_error(status: u16) -> PluginError {
    match status {
        401 | 403 => PluginError::service(format!(
            "NetEase Cloud Music refused the request (HTTP {status}). It may block this network or this \
             kind of client."
        )),
        404 => PluginError::service(
            "NetEase Cloud Music no longer answers at this address (HTTP 404); the plugin may need an update.",
        ),
        429 => PluginError::RateLimited,
        500..=599 => PluginError::service(format!(
            "NetEase Cloud Music had an internal error (HTTP {status}). Try again later."
        )),
        _ => PluginError::service(format!(
            "NetEase Cloud Music sent an answer the plugin cannot read (HTTP {status})."
        )),
    }
}

/// The value of the cookie `name` among a response's `set-cookie` headers.
pub fn set_cookie(response: &http::Response, name: &str) -> Option<String> {
    response
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("set-cookie"))
        .filter_map(|h| h.value.split(';').next())
        .filter_map(|pair| pair.split_once('='))
        .find(|(n, _)| n.trim() == name)
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// A stream or picture URL from an answer, as the host may fetch it: https,
/// on a subdomain of [`MEDIA_DOMAIN`]. `None` for anything else.
pub fn media_url(url: &str) -> Option<String> {
    let https = upgrade_to_https(url.trim());
    let host = url_host(&https)?;
    (host_in(&host, MEDIA_DOMAIN) && host != MEDIA_DOMAIN).then_some(https)
}

/// A picture at about `size` pixels: the service scales on request
/// (`?param=<w>y<h>`).
pub fn image(pic_url: &str, size: u32) -> Option<ImageRef> {
    let base = media_url(pic_url)?;
    let base = base.split(['?', '#']).next().unwrap_or(&base).to_string();
    let size = size.clamp(64, 1200);
    let mut image = ImageRef::new(physical(&format!("{base}?param={size}y{size}")));
    image.width = Some(size);
    image.height = Some(size);
    Some(image)
}

/// Remembers the picture of an item from a listing.
pub fn remember_cover(item_id: &str, pic_url: Option<&str>) {
    if let Some(url) = pic_url.filter(|u| !u.trim().is_empty()) {
        STATE.with(|s| {
            let mut state = s.borrow_mut();
            if state.covers.len() >= MAX_COVERS {
                state.covers.clear();
            }
            state.covers.insert(item_id.to_string(), url.to_string());
        });
    }
}

/// The picture of an item listed earlier, if remembered.
pub fn cover_of(item_id: &str) -> Option<String> {
    STATE.with(|s| s.borrow().covers.get(item_id).cloned())
}

/// A list of song ids fetched in the last two minutes.
pub fn cached_ids(key: &str) -> Option<Vec<u64>> {
    let now = clock::monotonic_ms();
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state
            .lists
            .retain(|(_, at, _)| now.saturating_sub(*at) < LIST_TTL_MS);
        state
            .lists
            .iter()
            .find(|(k, _, _)| k == key)
            .map(|(_, _, ids)| ids.clone())
    })
}

/// Keeps a list of song ids for paging (the last four lists).
pub fn keep_ids(key: &str, ids: &[u64]) {
    let now = clock::monotonic_ms();
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.lists.retain(|(k, _, _)| k != key);
        if state.lists.len() >= 4 {
            state.lists.remove(0);
        }
        state.lists.push((key.to_string(), now, ids.to_vec()));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_codes() {
        assert!(check(&json!({ "code": 200 })).is_ok());
        assert!(check(&json!({})).is_ok());
        assert_eq!(check(&json!({ "code": 301 })), Err(PluginError::AuthRequired));
        assert_eq!(check(&json!({ "code": 405 })), Err(PluginError::RateLimited));
        assert_eq!(check(&json!({ "code": 404 })), Err(PluginError::NotFound));
        assert_eq!(
            check(&json!({ "code": 50, "message": " busy " })),
            Err(PluginError::Service("NetEase Cloud Music: busy (code 50)".into()))
        );
        assert!(
            matches!(check(&json!({ "code": -460 })), Err(PluginError::Service(m)) if m.contains("-460"))
        );
    }

    #[test]
    fn media_urls_stay_on_the_media_domain() {
        assert_eq!(
            media_url("http://m701.music.126.net/a/b.mp3?x=1").as_deref(),
            Some("https://m701.music.126.net/a/b.mp3?x=1")
        );
        assert_eq!(media_url("https://music.126.net/a.mp3"), None);
        assert_eq!(media_url("https://evil.example/m701.music.126.net/a.mp3"), None);
        assert_eq!(media_url("https://m701.music.126.net.evil.example/a.mp3"), None);
        assert_eq!(media_url(""), None);
        let pic = image("http://p1.music.126.net/abc==/1.jpg?old=1", 5000).unwrap();
        assert_eq!(pic.url, "https://p1.music.126.net/abc==/1.jpg?param=1200y1200");
        assert_eq!(pic.width, Some(1200));
    }

    #[test]
    fn reads_set_cookie() {
        let response = http::Response {
            status: 200,
            headers: vec![
                Header::new("set-cookie", "MUSIC_U=abc; Max-Age=100; Path=/"),
                Header::new("Set-Cookie", "__csrf=0123abcd; Max-Age=100; Path=/"),
            ],
            body: Vec::new(),
            url: String::new(),
        };
        assert_eq!(set_cookie(&response, "__csrf").as_deref(), Some("0123abcd"));
        assert_eq!(set_cookie(&response, "MUSIC_U").as_deref(), Some("abc"));
        assert_eq!(set_cookie(&response, "missing"), None);
    }
}

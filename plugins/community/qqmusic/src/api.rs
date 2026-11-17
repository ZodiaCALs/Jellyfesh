//! Talking to QQ Music: the stored session, signed requests to the web
//! player's API and their errors, media URLs, and small in-memory caches.
//!
//! Every call is a module and a method on the web player's single endpoint:
//! `POST https://u.y.qq.com/cgi-bin/musics.fcg?_=<ms>&sign=<zzb…>` with the
//! JSON body `{"comm": {…}, "req_1": {"module": …, "method": …, "param": …}}`,
//! signed over the exact body ([`crate::sign`]). The session (the account's
//! `musicid` and `musickey`) goes in `comm` and in the cookies the web player
//! would send.

use std::cell::RefCell;
use std::collections::HashMap;

use jf_community_common::{host_in, upgrade_to_https, url_host};
use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::settings::Settings;
use crate::sign;

/// The web player's API endpoint.
pub const API_URL: &str = "https://u.y.qq.com/cgi-bin/musics.fcg";
/// The web player's origin.
pub const WEB_ORIGIN: &str = "https://y.qq.com";
/// Album and singer pictures.
pub const PICTURE_HOST: &str = "y.gtimg.cn";
/// Playlist covers.
pub const COVER_HOST: &str = "qpic.y.qq.com";
/// Streams come from subdomains of this domain (`ws.stream.qqmusic.qq.com`, …).
pub const STREAM_DOMAIN: &str = "stream.qqmusic.qq.com";
/// The web endpoints are made for browsers, so the plugin sends a desktop
/// browser's User-Agent, as the web player would.
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
/// The client version the web player sends in `comm`.
const WEB_CV: u32 = 4_747_474;
/// The keystore entry with the session.
const SESSION: &str = "session";
/// kv: the player id (`guid`) the stream service wants, made once.
const GUID: &str = "guid";
/// Wait at most this long for one answer.
const TIMEOUT_MS: u32 = 15_000;
/// Refresh the session key when less than this part of its life is left.
const REFRESH_WHEN_LEFT: u64 = 4;
/// Most remembered covers and refusals.
const MAX_REMEMBERED: usize = 20_000;

/// The signed-in account, as kept in the keystore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The account's number (`musicid`).
    pub musicid: u64,
    /// The session key (`musickey`).
    pub musickey: String,
    /// What renews the key, when the service gave it.
    #[serde(default)]
    pub refresh_key: String,
    #[serde(default)]
    pub refresh_token: String,
    /// The account's display name.
    pub nickname: String,
    /// When the key was made, ms since the epoch.
    pub created_ms: u64,
    /// How long the key lasts, in seconds (0: not said).
    pub expires_in_s: u64,
    /// `tmeLoginType`: 1 for WeChat, the only sign-in this plugin offers.
    pub login_type: u32,
}

impl Session {
    /// Reads the session out of a sign-in answer's `data`.
    pub fn from_login(data: &Value, nickname: &str, login_type: u32, now_ms: u64) -> Option<Session> {
        let text = |key: &str| {
            data.get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let musicid = data
            .get("musicid")
            .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
            .or_else(|| data.get("str_musicid").and_then(|v| v.as_str()?.parse().ok()))
            .filter(|id| *id > 0)?;
        let musickey = text("musickey");
        if musickey.is_empty() {
            return None;
        }
        Some(Session {
            musicid,
            musickey,
            refresh_key: text("refresh_key"),
            refresh_token: text("refresh_token"),
            nickname: nickname.to_string(),
            created_ms: now_ms,
            expires_in_s: data.get("keyExpiresIn").and_then(Value::as_u64).unwrap_or(0),
            login_type,
        })
    }

    /// Whether the key should be renewed before use.
    pub fn stale(&self, now_ms: u64) -> bool {
        if self.expires_in_s == 0 || self.refresh_key.is_empty() {
            return false;
        }
        let life = self.expires_in_s * 1000;
        now_ms >= self.created_ms + life - life / REFRESH_WHEN_LEFT
    }
}

#[derive(Debug, Default)]
struct State {
    settings: Settings,
    /// The session read from the keystore: outer `None` means not read yet.
    session: Option<Option<Session>>,
    /// Whether this instance already tried to renew the key.
    refreshed: bool,
    /// Item id → picture URL, from listings.
    covers: HashMap<String, String>,
    /// Song mid → why the service would not stream it to this account,
    /// learnt when asked for its stream, so later listings say so.
    refusals: HashMap<String, Unplayable>,
    /// The account's playlists: (id, title, cover, count, is the liked songs).
    playlists: Option<Vec<OwnPlaylist>>,
}

/// One of the account's own playlists.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnPlaylist {
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
    pub count: Option<u32>,
    pub liked: bool,
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

/// The URL the plugin requests (or hands out) for a QQ Music URL: the URL
/// itself, or in a test build pointed at the mock service, the same host and
/// path under the mock's origin (`http://127.0.0.1:…/u.y.qq.com/cgi-bin/…`).
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

/// The session (renewed first when its key is near the end of its life), or
/// `auth-required`.
pub fn require_session() -> Result<Session> {
    let session = session()?.ok_or(PluginError::AuthRequired)?;
    if session.stale(clock::now_utc_ms()) && !STATE.with(|s| s.borrow().refreshed) {
        return Ok(refresh(&session).unwrap_or(session));
    }
    Ok(session)
}

/// Keeps a new session in the keystore.
pub fn save_session(session: &Session) -> Result<()> {
    let text = serde_json::to_string(session).map_err(|e| PluginError::internal(e.to_string()))?;
    kv::secret::set(SESSION, &text)?;
    STATE.with(|s| s.borrow_mut().session = Some(Some(session.clone())));
    Ok(())
}

/// Forgets the session and everything cached about the account.
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

/// Renews the session key once per instance; `None` when that fails.
fn refresh(session: &Session) -> Option<Session> {
    STATE.with(|s| s.borrow_mut().refreshed = true);
    if session.refresh_key.is_empty() {
        return None;
    }
    let param = json!({
        "refresh_key": session.refresh_key,
        "refresh_token": session.refresh_token,
        "musickey": session.musickey,
        "musicid": session.musicid,
    });
    let data = call_as(Some(session), "music.login.LoginServer", "Login", param).ok()?;
    let renewed = Session::from_login(&data, &session.nickname, session.login_type, clock::now_utc_ms())?;
    let renewed = Session {
        refresh_key: if renewed.refresh_key.is_empty() {
            session.refresh_key.clone()
        } else {
            renewed.refresh_key
        },
        refresh_token: if renewed.refresh_token.is_empty() {
            session.refresh_token.clone()
        } else {
            renewed.refresh_token
        },
        ..renewed
    };
    save_session(&renewed).ok()?;
    jf_plugin_sdk::info!("renewed the QQ Music session key");
    Some(renewed)
}

/// The request's `comm` block: what the web player sends, with the session.
pub fn comm(session: Option<&Session>) -> Value {
    let key = session.map(|s| s.musickey.as_str()).unwrap_or_default();
    let g_tk = sign::g_tk(key);
    let mut comm = json!({
        "cv": WEB_CV,
        "ct": 24,
        "format": "json",
        "inCharset": "utf-8",
        "outCharset": "utf-8",
        "notice": 0,
        "platform": "yqq.json",
        "needNewCode": 1,
        "uin": session.map_or(0, |s| s.musicid),
        "g_tk_new_20200303": g_tk,
        "g_tk": g_tk,
    });
    if let Some(session) = session {
        comm["authst"] = json!(session.musickey);
        comm["tmeLoginType"] = json!(session.login_type);
        comm["tmeAppID"] = json!("qqmusic");
    }
    comm
}

/// Sends one call (`module.method` with `param`) as `session` and returns
/// its `data`.
pub fn call_as(session: Option<&Session>, module: &str, method: &str, param: Value) -> Result<Value> {
    call_with_comm(comm(session), session, module, method, param)
}

/// Like [`call_as`] with a `comm` block of the caller's.
pub fn call_with_comm(
    comm: Value,
    session: Option<&Session>,
    module: &str,
    method: &str,
    param: Value,
) -> Result<Value> {
    let body = json!({
        "comm": comm,
        "req_1": { "module": module, "method": method, "param": param },
    })
    .to_string();
    let answer = send(&body, session)?;
    let part = answer.get("req_1").cloned().unwrap_or(Value::Null);
    check(&part, module, method)?;
    Ok(part.get("data").cloned().unwrap_or(Value::Null))
}

/// Sends one call with the stored session, if any. When the service says the
/// session is over and its key can be renewed, renews it once and tries again.
pub fn call(module: &str, method: &str, param: Value) -> Result<Value> {
    let session = session()?.map(|session| require_session().unwrap_or(session));
    match call_as(session.as_ref(), module, method, param.clone()) {
        Err(PluginError::AuthRequired) if session.is_some() && !STATE.with(|s| s.borrow().refreshed) => {
            match session.as_ref().and_then(refresh) {
                Some(renewed) => call_as(Some(&renewed), module, method, param),
                None => Err(PluginError::AuthRequired),
            }
        }
        other => other,
    }
}

/// Like [`call`] for a call that needs the account.
pub fn call_signed_in(module: &str, method: &str, param: Value) -> Result<Value> {
    require_session()?;
    call(module, method, param)
}

/// Posts a signed body and returns the whole answer.
fn send(body: &str, session: Option<&Session>) -> Result<Value> {
    let digest: [u8; 16] = crypto::digest(crypto::HashAlgorithm::Md5, body.as_bytes())?
        .try_into()
        .map_err(|_| PluginError::internal("MD5 is 16 bytes"))?;
    let url = physical(&format!(
        "{API_URL}?_={}&sign={}",
        clock::now_utc_ms(),
        sign::sign(&digest)
    ));
    let mut request = http::Request::post(url)
        .header("referer", "https://y.qq.com/")
        .header("origin", WEB_ORIGIN)
        .header("user-agent", USER_AGENT)
        .header("accept", "application/json")
        .body(body.as_bytes().to_vec(), "application/x-www-form-urlencoded")
        .timeout_ms(TIMEOUT_MS);
    if let Some(session) = session {
        request = request.header(
            "cookie",
            &format!(
                "uin={id}; qqmusic_key={key}; qm_keyst={key}; tmeLoginType={kind}",
                id = session.musicid,
                key = session.musickey,
                kind = session.login_type
            ),
        );
    }
    let response = request.send()?;
    let answer = match serde_json::from_slice::<Value>(&response.body) {
        Ok(answer) if answer.is_object() => answer,
        _ => return Err(status_error(response.status)),
    };
    match answer.get("code").and_then(Value::as_i64).unwrap_or(0) {
        0 => Ok(answer),
        code => {
            jf_plugin_sdk::warn!("QQ Music refused a request with code {code}");
            Err(PluginError::service(format!(
                "QQ Music refused the request (code {code}). The plugin may need an update."
            )))
        }
    }
}

/// Turns one call's code into an error.
pub fn check(part: &Value, module: &str, method: &str) -> Result<()> {
    if part.is_null() {
        return Err(PluginError::service(format!(
            "QQ Music did not answer {module}.{method}."
        )));
    }
    match part.get("code").and_then(Value::as_i64).unwrap_or(0) {
        0 => Ok(()),
        // "Not signed in" (or the key expired).
        1000 | 1001 => Err(PluginError::AuthRequired),
        code => {
            let message = ["msg", "message", "errmsg"]
                .iter()
                .find_map(|k| part.get(*k).and_then(Value::as_str))
                .map(str::trim)
                .filter(|m| !m.is_empty());
            Err(PluginError::service(match message {
                Some(message) => format!("QQ Music: {message} (code {code})"),
                None => format!("QQ Music reported error {code} for {module}.{method}."),
            }))
        }
    }
}

/// An answer that is not QQ Music JSON.
fn status_error(status: u16) -> PluginError {
    match status {
        401 | 403 => PluginError::service(format!(
            "QQ Music refused the request (HTTP {status}). It may block this network or this kind of client."
        )),
        429 => PluginError::RateLimited,
        500..=599 => PluginError::service(format!(
            "QQ Music had an internal error (HTTP {status}). Try again later."
        )),
        _ => PluginError::service(format!(
            "QQ Music sent an answer the plugin cannot read (HTTP {status})."
        )),
    }
}

/// The player id the stream service wants: ten random digits, made once and
/// kept.
pub fn guid() -> Result<String> {
    if let Some(bytes) = kv::get(GUID)?
        && let Ok(text) = String::from_utf8(bytes)
        && text.len() == 10
    {
        return Ok(text);
    }
    let random = crypto::random_bytes(10)?;
    let mut guid: String = random.iter().map(|b| char::from(b'0' + b % 10)).collect();
    if guid.starts_with('0') {
        guid.replace_range(0..1, "1");
    }
    kv::set(GUID, guid.as_bytes())?;
    Ok(guid)
}

/// A stream URL as the host may fetch it: https, on a subdomain of
/// [`STREAM_DOMAIN`].
pub fn stream_base(sip: &str) -> Option<String> {
    let https = upgrade_to_https(sip.trim());
    let host = url_host(&https)?;
    (host_in(&host, STREAM_DOMAIN) && host != STREAM_DOMAIN).then(|| {
        if https.ends_with('/') {
            https
        } else {
            format!("{https}/")
        }
    })
}

/// A picture URL from an answer, as the host may fetch it: https, on the
/// picture or cover host.
pub fn picture_url(url: &str) -> Option<String> {
    let https = upgrade_to_https(url.trim());
    let host = url_host(&https)?;
    (host == PICTURE_HOST || host == COVER_HOST).then_some(https)
}

/// The sizes the picture service makes.
const SIZES: [u32; 5] = [90, 150, 300, 500, 800];

fn picture_size(size: u32) -> u32 {
    SIZES.iter().copied().find(|s| *s >= size).unwrap_or(800)
}

/// An album's cover at about `size` pixels, from its mid.
pub fn album_picture(album_mid: &str, size: u32) -> Option<ImageRef> {
    mid_picture("T002", album_mid, size)
}

/// A singer's photo at about `size` pixels, from their mid.
pub fn singer_picture(singer_mid: &str, size: u32) -> Option<ImageRef> {
    mid_picture("T001", singer_mid, size)
}

fn mid_picture(kind: &str, mid: &str, size: u32) -> Option<ImageRef> {
    if mid.is_empty() || !mid.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let size = picture_size(size);
    let mut image = ImageRef::new(physical(&format!(
        "https://{PICTURE_HOST}/music/photo_new/{kind}R{size}x{size}M000{mid}.jpg?max_age=2592000"
    )));
    image.width = Some(size);
    image.height = Some(size);
    Some(image)
}

/// A picture from an answer (a playlist cover), at its own size.
pub fn picture(url: &str) -> Option<ImageRef> {
    picture_url(url).map(|url| ImageRef::new(physical(&url)))
}

/// Remembers the picture of an item from a listing.
pub fn remember_cover(item_id: &str, url: Option<&str>) {
    if let Some(url) = url.filter(|u| !u.trim().is_empty()) {
        STATE.with(|s| {
            let mut state = s.borrow_mut();
            if state.covers.len() >= MAX_REMEMBERED {
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

/// Remembers why the service would not stream a song to this account.
pub fn remember_refusal(song_mid: &str, why: Unplayable) {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        if state.refusals.len() >= MAX_REMEMBERED {
            state.refusals.clear();
        }
        state.refusals.insert(song_mid.to_string(), why);
    });
}

/// Why the service would not stream a song, if it said so earlier.
pub fn refusal_of(song_mid: &str) -> Option<Unplayable> {
    STATE.with(|s| s.borrow().refusals.get(song_mid).cloned())
}

/// The account's playlists, if fetched in this instance.
pub fn cached_playlists() -> Option<Vec<OwnPlaylist>> {
    STATE.with(|s| s.borrow().playlists.clone())
}

/// Keeps the account's playlists.
pub fn keep_playlists(lists: &[OwnPlaylist]) {
    STATE.with(|s| s.borrow_mut().playlists = Some(lists.to_vec()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(expires_in_s: u64, refresh_key: &str) -> Session {
        Session {
            musicid: 1,
            musickey: "k".into(),
            refresh_key: refresh_key.into(),
            refresh_token: String::new(),
            nickname: "n".into(),
            created_ms: 1_000_000,
            expires_in_s,
            login_type: 1,
        }
    }

    #[test]
    fn reads_a_login_answer() {
        let data = json!({
            "musicid": 7000001, "musickey": "W_X_mock", "refresh_key": "rk", "refresh_token": "rt",
            "keyExpiresIn": 259200, "encryptUin": "x"
        });
        let s = Session::from_login(&data, "Mock", 1, 5).unwrap();
        assert_eq!(
            (s.musicid, s.musickey.as_str(), s.expires_in_s),
            (7_000_001, "W_X_mock", 259_200)
        );
        let by_text = json!({ "str_musicid": "42", "musickey": "k" });
        assert_eq!(Session::from_login(&by_text, "n", 1, 0).unwrap().musicid, 42);
        assert!(Session::from_login(&json!({ "musicid": 1 }), "n", 1, 0).is_none());
        assert!(Session::from_login(&json!({ "musickey": "k" }), "n", 1, 0).is_none());
    }

    #[test]
    fn renews_late_in_the_key_life() {
        let s = session(100, "rk");
        assert!(!s.stale(1_000_000 + 70_000));
        assert!(s.stale(1_000_000 + 76_000));
        assert!(!session(0, "rk").stale(u64::MAX / 2), "no lifetime said");
        assert!(
            !session(100, "").stale(1_000_000 + 99_000),
            "nothing to renew with"
        );
    }

    #[test]
    fn maps_codes() {
        assert!(check(&json!({ "code": 0, "data": {} }), "m", "f").is_ok());
        assert_eq!(
            check(&json!({ "code": 1000 }), "m", "f"),
            Err(PluginError::AuthRequired)
        );
        assert_eq!(
            check(&json!({ "code": 24, "msg": "busy" }), "m", "f"),
            Err(PluginError::Service("QQ Music: busy (code 24)".into()))
        );
        assert!(matches!(
            check(&Value::Null, "m", "f"),
            Err(PluginError::Service(_))
        ));
    }

    #[test]
    fn media_hosts() {
        assert_eq!(
            stream_base("http://ws.stream.qqmusic.qq.com/").as_deref(),
            Some("https://ws.stream.qqmusic.qq.com/")
        );
        assert_eq!(
            stream_base("https://isure.stream.qqmusic.qq.com").as_deref(),
            Some("https://isure.stream.qqmusic.qq.com/")
        );
        assert_eq!(stream_base("http://aqqmusic.tc.qq.com/"), None);
        assert_eq!(stream_base("https://stream.qqmusic.qq.com/"), None);
        assert_eq!(
            picture_url("http://qpic.y.qq.com/music_cover/x/300").as_deref(),
            Some("https://qpic.y.qq.com/music_cover/x/300")
        );
        assert_eq!(picture_url("https://evil.example/a.jpg"), None);
        let art = album_picture("0000MockAlbum1", 400).unwrap();
        assert_eq!(
            art.url,
            "https://y.gtimg.cn/music/photo_new/T002R500x500M0000000MockAlbum1.jpg?max_age=2592000"
        );
        assert_eq!(album_picture("../x", 300), None);
        assert_eq!(singer_picture("abc", 2000).unwrap().width, Some(800));
    }
}

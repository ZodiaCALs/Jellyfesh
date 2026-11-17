//! A mock of the QQ Music web API (u.y.qq.com) and of WeChat's web QR
//! sign-in, for the integration tests.
//!
//! It answers the calls the plugin makes the way this plugin's author
//! understands the services to answer them (they could not be reached from
//! where the plugin was written; see the README for what to check first). It
//! is strict where the protocol is: every API request must carry a `sign`
//! that matches its body (checked with the mock's own implementation of the
//! scheme), the web player's referer and a `comm` block whose `g_tk` matches
//! its key; signed-in calls want a live session key; anything else is
//! recorded as a violation. The library it serves (`fixtures/library.json`)
//! is invented.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use jf_community_testsupport::{Request, Response, TestServer, split_host};
use md5::{Digest, Md5};
use serde_json::{Value, json};

/// QQ Music's app id on WeChat's web sign-in.
pub const WECHAT_APP_ID: &str = "wx48db31d50e334801";
/// What the mock streams: not music.
pub const AUDIO: &[u8] = b"mock audio bytes, not music";
/// What the mock serves for pictures.
pub const PICTURE: &[u8] = b"\x89PNG\r\n\x1a\nmock picture";

/// One API call as the mock received it.
#[derive(Debug, Clone)]
pub struct Call {
    pub module: String,
    pub method: String,
    pub param: Value,
    pub comm: Value,
    pub cookies: String,
}

/// What the mock remembers and how it behaves.
#[derive(Debug, Default)]
pub struct State {
    /// Every API call.
    pub calls: Vec<Call>,
    /// Everything the plugin did that the protocol does not allow.
    pub violations: Vec<String>,
    /// Media paths fetched (`host/path`).
    pub served: Vec<String>,
    /// What WeChat's long poll answers, in order (the last one repeats).
    pub wechat_script: Vec<u32>,
    /// `keyExpiresIn` given with new session keys.
    pub key_lifetime_s: u64,
    /// Whether renewing a key works.
    pub refresh_works: bool,
    polls: HashMap<String, usize>,
    pages: u32,
    codes: Vec<String>,
    /// Live session keys.
    keys: Vec<String>,
    issued: u32,
}

/// The running mock.
pub struct Mock {
    pub server: TestServer,
    pub state: Arc<Mutex<State>>,
}

impl Mock {
    /// A mock whose WeChat sign-in goes waiting → scanned → confirmed.
    pub fn start() -> Mock {
        Mock::with(|state| state.wechat_script = vec![408, 404, 405])
    }

    /// A mock set up by `setup`.
    pub fn with(setup: impl FnOnce(&mut State)) -> Mock {
        let mut state = State {
            key_lifetime_s: 259_200,
            refresh_works: true,
            ..State::default()
        };
        setup(&mut state);
        let state = Arc::new(Mutex::new(state));
        let library: Value = serde_json::from_str(include_str!("../fixtures/library.json")).unwrap();
        let shared = state.clone();
        let server = TestServer::start(move |request| handle(&library, &shared, request));
        Mock { server, state }
    }

    /// Every API call so far.
    pub fn calls(&self) -> Vec<Call> {
        self.state.lock().unwrap().calls.clone()
    }

    /// How many calls went to `module.method`.
    pub fn count(&self, module: &str, method: &str) -> usize {
        self.calls()
            .iter()
            .filter(|c| c.module == module && c.method == method)
            .count()
    }

    /// Protocol violations so far.
    pub fn violations(&self) -> Vec<String> {
        self.state.lock().unwrap().violations.clone()
    }

    /// Forgets the calls so far.
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.calls.clear();
        state.served.clear();
    }

    /// Ends every session key, as when keys expire.
    pub fn expire_keys(&self) {
        self.state.lock().unwrap().keys.clear();
    }
}

/// The signature, computed the way the description of the web player's
/// scheme reads, independently of the plugin's code: three bytes at a time
/// into four symbols, the last byte into two.
pub fn expected_sign(body: &[u8]) -> String {
    const MIX: [u8; 16] = [
        212, 45, 80, 68, 195, 163, 163, 203, 157, 220, 254, 91, 204, 79, 104, 6,
    ];
    const SYMBOLS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let hex: Vec<char> = Md5::digest(body)
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<String>()
        .chars()
        .collect();
    let pick = |positions: &[usize]| positions.iter().map(|&i| hex[i]).collect::<String>();
    let mixed: Vec<u8> = (0..16)
        .map(|i| {
            let byte = u8::from_str_radix(&hex[2 * i..2 * i + 2].iter().collect::<String>(), 16).unwrap();
            byte ^ MIX[i]
        })
        .collect();
    let mut middle = String::new();
    for group in 0..5 {
        let (a, b, c) = (mixed[group * 3], mixed[group * 3 + 1], mixed[group * 3 + 2]);
        for index in [
            a >> 2,
            ((a & 3) << 4) | (b >> 4),
            ((b & 15) << 2) | (c >> 6),
            c & 63,
        ] {
            middle.push(SYMBOLS[usize::from(index)] as char);
        }
    }
    let last = mixed[15];
    middle.push(SYMBOLS[usize::from(last >> 2)] as char);
    middle.push(SYMBOLS[usize::from((last & 3) << 4)] as char);
    let middle: String = middle.chars().filter(|c| !matches!(c, '/' | '+')).collect();
    format!(
        "zzb{}",
        (pick(&[21, 4, 9, 26, 16, 20, 27, 30]) + &middle + &pick(&[18, 11, 3, 2, 1, 7, 6, 25]))
            .to_lowercase()
    )
}

/// `g_tk` of a key.
fn g_tk(key: &str) -> u64 {
    let mut hash: u64 = 5381;
    for unit in key.encode_utf16() {
        hash = (hash + ((hash << 5) & 0xffff_ffff) + u64::from(unit)) & 0xffff_ffff;
    }
    hash & 0x7fff_ffff
}

fn handle(library: &Value, state: &Mutex<State>, request: &Request) -> Response {
    let Some((host, path)) = split_host(request.path()) else {
        return Response::text(404, "not found");
    };
    match host {
        "u.y.qq.com" => api(library, state, request, path),
        "open.weixin.qq.com" => wechat_page(state, request, path),
        "lp.open.weixin.qq.com" => wechat_poll(state, request, path),
        "y.gtimg.cn" | "qpic.y.qq.com" => media(state, request, host, path, PICTURE, "image/jpeg"),
        _ if host.ends_with(".stream.qqmusic.qq.com") => {
            media(state, request, host, path, AUDIO, "audio/mpeg")
        }
        _ => {
            state.lock().unwrap().violations.push(format!(
                "a request to {host}, which the plugin should never contact"
            ));
            Response::text(404, "not found")
        }
    }
}

fn media(
    state: &Mutex<State>,
    request: &Request,
    host: &str,
    path: &str,
    body: &[u8],
    kind: &str,
) -> Response {
    let mut state = state.lock().unwrap();
    if request.method != "GET" {
        state.violations.push(format!("{} {host}{path}", request.method));
    }
    state.served.push(format!("{host}{path}"));
    Response::new(200, body.to_vec()).header("content-type", kind)
}

fn wechat_page(state: &Mutex<State>, request: &Request, path: &str) -> Response {
    let mut state = state.lock().unwrap();
    if path != "/connect/qrconnect" {
        state.violations.push(format!("open.weixin.qq.com{path}"));
        return Response::text(404, "not found");
    }
    let redirect = request.query("redirect_uri").unwrap_or_default();
    if request.query("appid").as_deref() != Some(WECHAT_APP_ID)
        || request.query("response_type").as_deref() != Some("code")
        || request.query("scope").as_deref() != Some("snsapi_login")
        || !redirect.starts_with("https://y.qq.com/")
    {
        state
            .violations
            .push(format!("WeChat sign-in page asked as {}", request.target));
    }
    state.pages += 1;
    let html = format!(
        "<!DOCTYPE html><html><head><title>mock WeChat sign-in</title></head><body>\
         <div class=\"wrp_code\"><img class=\"qrcode lightBorder\" src=\"/connect/qrcode/091MockUuid{:04}\" /></div>\
         </body></html>",
        state.pages
    );
    Response::new(200, html).header("content-type", "text/html; charset=utf-8")
}

fn wechat_poll(state: &Mutex<State>, request: &Request, path: &str) -> Response {
    let mut state = state.lock().unwrap();
    if path != "/connect/l/qrconnect" {
        state.violations.push(format!("lp.open.weixin.qq.com{path}"));
        return Response::text(404, "not found");
    }
    let uuid = request.query("uuid").unwrap_or_default();
    if !uuid.starts_with("091MockUuid") {
        return Response::new(200, "window.wx_errcode=402;window.wx_code='';");
    }
    let last = state.wechat_script.len().saturating_sub(1);
    let polls = state.polls.entry(uuid).or_insert(0);
    let index = (*polls).min(last);
    *polls += 1;
    let status = state.wechat_script.get(index).copied().unwrap_or(402);
    let code = if status == 405 {
        let code = format!("081MockWxCode{:04}", state.codes.len() + 1);
        state.codes.push(code.clone());
        code
    } else {
        String::new()
    };
    Response::new(
        200,
        format!("window.wx_errcode={status};window.wx_code='{code}';"),
    )
    .header("content-type", "application/javascript")
}

fn api(library: &Value, state: &Mutex<State>, request: &Request, path: &str) -> Response {
    let mut state = state.lock().unwrap();
    let mut violations = Vec::new();
    if path != "/cgi-bin/musics.fcg" || request.method != "POST" {
        violations.push(format!("{} {path}: not a POST to musics.fcg", request.method));
    }
    let sign = request.query("sign").unwrap_or_default();
    if sign != expected_sign(&request.body) {
        violations.push(format!("{path}: sign {sign:?} does not match the body"));
    }
    if !request
        .query("_")
        .is_some_and(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()))
    {
        violations.push(format!("{path}: no _ time stamp"));
    }
    if request.header("referer") != Some("https://y.qq.com/") {
        violations.push(format!("{path}: referer {:?}", request.header("referer")));
    }
    if !request
        .header("user-agent")
        .is_some_and(|ua| ua.starts_with("Mozilla/5.0"))
    {
        violations.push(format!("{path}: not a browser user agent"));
    }
    let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
        violations.push(format!("{path}: the body is not JSON"));
        state.violations.extend(violations);
        return Response::json(&json!({ "code": 500001 }));
    };
    let comm = body["comm"].clone();
    let key = comm["authst"].as_str().unwrap_or_default().to_string();
    if comm["g_tk"].as_u64() != Some(g_tk(&key)) || comm["g_tk_new_20200303"] != comm["g_tk"] {
        violations.push(format!("{path}: g_tk {} for key {key:?}", comm["g_tk"]));
    }
    if comm["ct"] != 24 || comm["format"] != "json" {
        violations.push(format!("{path}: comm {comm}"));
    }
    let cookies = request.header("cookie").unwrap_or_default().to_string();
    let signed_in = !key.is_empty()
        && state.keys.contains(&key)
        && comm["uin"] == library["account"]["musicid"]
        && cookies.contains(&format!("qm_keyst={key}"));
    let mut answer = json!({ "code": 0, "ts": 1_700_000_000_000_u64 });
    let parts: Vec<(String, Value)> = body
        .as_object()
        .map(|o| {
            o.iter()
                .filter(|(k, _)| k.starts_with("req_"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    if parts.is_empty() {
        violations.push(format!("{path}: no req_ part"));
    }
    state.violations.extend(violations);
    for (name, part) in parts {
        let module = part["module"].as_str().unwrap_or_default().to_string();
        let method = part["method"].as_str().unwrap_or_default().to_string();
        let param = part["param"].clone();
        state.calls.push(Call {
            module: module.clone(),
            method: method.clone(),
            param: param.clone(),
            comm: comm.clone(),
            cookies: cookies.clone(),
        });
        answer[name] = call(library, &mut state, &module, &method, &param, &comm, signed_in);
    }
    Response::json(&answer)
}

fn data(data: Value) -> Value {
    json!({ "code": 0, "data": data })
}

fn needs_session() -> Value {
    json!({ "code": 1000, "msg": "mock: not signed in" })
}

/// A song as QQ Music sends it.
fn song(library: &Value, mid: &str) -> Option<Value> {
    let mut song = library["songs"]
        .as_array()?
        .iter()
        .find(|s| s["mid"] == mid)?
        .clone();
    let object = song.as_object_mut()?;
    object.remove("stream");
    object.remove("result");
    Some(song)
}

fn raw_song<'a>(library: &'a Value, mid: &str) -> Option<&'a Value> {
    library["songs"].as_array()?.iter().find(|s| s["mid"] == mid)
}

fn mids(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|l| l.iter().filter_map(|m| m.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn window<T: Clone>(list: &[T], begin: usize, count: usize) -> Vec<T> {
    list.iter().skip(begin).take(count).cloned().collect()
}

fn base64(text: &str) -> String {
    jf_community_common::base64::encode(text.as_bytes())
}

fn new_key(state: &mut State) -> (String, String) {
    state.issued += 1;
    let key = format!("W_X_mock_key_{:04}", state.issued);
    state.keys.push(key.clone());
    (key, format!("mock_refresh_{:04}", state.issued))
}

fn call(
    library: &Value,
    state: &mut State,
    module: &str,
    method: &str,
    param: &Value,
    comm: &Value,
    signed_in: bool,
) -> Value {
    let account = &library["account"];
    match (module, method) {
        ("music.login.LoginServer", "Login") => {
            if comm["tmeLoginType"] != 1 {
                state
                    .violations
                    .push(format!("Login with tmeLoginType {}", comm["tmeLoginType"]));
            }
            if let Some(code) = param["code"].as_str() {
                if param["strAppid"] != WECHAT_APP_ID {
                    state
                        .violations
                        .push(format!("Login with strAppid {}", param["strAppid"]));
                }
                if !state.codes.iter().any(|c| c == code) {
                    return json!({ "code": 1000, "msg": "mock: unknown WeChat code" });
                }
                state.codes.retain(|c| c != code);
            } else if let Some(refresh) = param["refresh_key"].as_str() {
                if !state.refresh_works || !refresh.starts_with("mock_refresh_") {
                    return json!({ "code": 1000, "msg": "mock: cannot renew" });
                }
                state.violations.extend(
                    (param["musicid"] != account["musicid"])
                        .then(|| format!("renewal for musicid {}", param["musicid"])),
                );
            } else {
                state
                    .violations
                    .push(format!("Login without code or refresh key: {param}"));
                return json!({ "code": 1000 });
            }
            let (key, refresh) = new_key(state);
            data(json!({
                "musicid": account["musicid"], "str_musicid": account["musicid"].to_string(),
                "musickey": key, "refresh_key": refresh, "refresh_token": "mock_refresh_token",
                "keyExpiresIn": state.key_lifetime_s, "encryptUin": "mock-encrypted-uin", "loginType": 1
            }))
        }
        ("music.UserInfo.userInfoServer", "GetLoginUserInfo") if signed_in => {
            data(json!({ "info": { "nick": account["nick"], "uin": account["musicid"] } }))
        }
        ("music.UserInfo.userInfoServer", "GetLoginUserInfo") => needs_session(),
        ("music.search.SearchCgiService", "DoSearchForQQMusicDesktop") => search(library, param),
        ("music.pf_song_detail_svr", "get_song_detail_yqq") => {
            match song(library, param["song_mid"].as_str().unwrap_or_default()) {
                Some(track) => data(json!({ "track_info": track, "info": [], "extras": {} })),
                None => json!({ "code": 104400, "msg": "mock: no such song" }),
            }
        }
        ("vkey.GetVkeyServer", "CgiGetVkey") if !signed_in => needs_session(),
        ("vkey.GetVkeyServer", "CgiGetVkey") => vkey(library, state, param),
        ("music.musichallSong.PlayLyricInfo", "GetPlayLyricInfo") => {
            if param["qrc"] != 0 {
                state.violations.push("asked for QRC lyrics".into());
            }
            let mid = param["songMID"].as_str().unwrap_or_default();
            match library["lyrics"].get(mid) {
                Some(found) => data(json!({
                    "songID": 0, "crypt": found["crypt"].as_i64().unwrap_or(0),
                    "lyric": base64(found["lyric"].as_str().unwrap_or_default()),
                    "trans": base64(found["trans"].as_str().unwrap_or_default()),
                    "roma": ""
                })),
                None => data(json!({ "songID": 0, "crypt": 0, "lyric": "", "trans": "", "roma": "" })),
            }
        }
        ("music.musichallAlbum.AlbumSongList", "GetAlbumSongList") => {
            let album = library["albums"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["albumMID"] == param["albumMid"]);
            match album {
                Some(album) => {
                    let all: Vec<Value> = mids(&album["songs"])
                        .iter()
                        .filter_map(|mid| song(library, mid))
                        .map(|s| json!({ "songInfo": s, "listenCount": 0 }))
                        .collect();
                    let begin = param["begin"].as_u64().unwrap_or(0) as usize;
                    let num = param["num"].as_u64().unwrap_or(30) as usize;
                    data(
                        json!({ "albumMid": album["albumMID"], "totalNum": all.len(), "songList": window(&all, begin, num) }),
                    )
                }
                None => json!({ "code": 104400, "msg": "mock: no such album" }),
            }
        }
        ("music.musichallAlbum.AlbumListServer", "GetAlbumList") => {
            let all: Vec<Value> = library["albums"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|a| a["singer_list"][0]["mid"] == param["singerMid"])
                .map(|a| {
                    json!({
                        "albumMid": a["albumMID"], "albumName": a["albumName"], "publishDate": a["publicTime"],
                        "singerName": a["singer_list"][0]["name"], "totalNum": mids(&a["songs"]).len()
                    })
                })
                .collect();
            let begin = param["begin"].as_u64().unwrap_or(0) as usize;
            let num = param["num"].as_u64().unwrap_or(30) as usize;
            data(
                json!({ "singerMid": param["singerMid"], "total": all.len(), "albumList": window(&all, begin, num) }),
            )
        }
        ("music.musicasset.PlaylistBaseRead", "GetPlaylistByUin") if !signed_in => needs_session(),
        ("music.musicasset.PlaylistBaseRead", "GetPlaylistByUin") => {
            if param["uin"].as_str() != Some(account["musicid"].to_string().as_str()) {
                state
                    .violations
                    .push(format!("playlists of uin {}", param["uin"]));
            }
            let lists: Vec<Value> = library["playlists"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p["own"] == true)
                .map(|p| {
                    json!({
                        "tid": p["tid"], "dirId": p["dirId"], "dirName": p["dirName"],
                        "songNum": mids(&p["songs"]).len(), "picUrl": p["picUrl"]
                    })
                })
                .collect();
            data(json!({ "v_playlist": lists, "totoal": lists.len() }))
        }
        ("music.srfDissInfo.aiDissInfo", "uniform_get_Dissinfo") => {
            let list = library["playlists"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["tid"] == param["disstid"]);
            match list {
                Some(list) if list["own"] == true && !signed_in => needs_session(),
                Some(list) => {
                    let all: Vec<Value> = mids(&list["songs"])
                        .iter()
                        .filter_map(|mid| song(library, mid))
                        .collect();
                    let begin = param["song_begin"].as_u64().unwrap_or(0) as usize;
                    let num = param["song_num"].as_u64().unwrap_or(30) as usize;
                    data(json!({
                        "code": 0,
                        "dirinfo": {
                            "id": list["tid"], "title": list["dirName"], "picurl": list["picUrl"],
                            "songnum": all.len(), "host_nick": list["owner"]
                        },
                        "songlist": window(&all, begin, num),
                        "total_song_num": all.len()
                    }))
                }
                None => json!({ "code": 4000, "msg": "mock: no such playlist" }),
            }
        }
        _ => {
            state
                .violations
                .push(format!("{module}.{method}: a call the mock does not know"));
            json!({ "code": 500001 })
        }
    }
}

fn search(library: &Value, param: &Value) -> Value {
    let query = param["query"].as_str().unwrap_or_default().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let matches = |text: String| {
        let text = text.to_lowercase();
        !words.is_empty() && words.iter().all(|w| text.contains(w))
    };
    let names = |list: &Value| {
        list.as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|s| s["name"].as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default()
    };
    let (key, found): (&str, Vec<Value>) = match param["search_type"].as_u64() {
        Some(0) => (
            "song",
            library["songs"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| {
                    matches(format!(
                        "{} {}",
                        s["name"].as_str().unwrap_or_default(),
                        names(&s["singer"])
                    ))
                })
                .filter_map(|s| song(library, s["mid"].as_str()?))
                .collect(),
        ),
        Some(1) => (
            "singer",
            library["singers"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| matches(s["singerName"].as_str().unwrap_or_default().to_string()))
                .cloned()
                .collect(),
        ),
        Some(2) => (
            "album",
            library["albums"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|a| {
                    matches(format!(
                        "{} {}",
                        a["albumName"].as_str().unwrap_or_default(),
                        names(&a["singer_list"])
                    ))
                })
                .map(|a| {
                    let mut a = a.clone();
                    a["song_count"] = json!(mids(&a["songs"]).len());
                    a.as_object_mut().unwrap().remove("songs");
                    a
                })
                .collect(),
        ),
        Some(3) => (
            "songlist",
            library["playlists"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| {
                    p["own"] != true && matches(p["dirName"].as_str().unwrap_or_default().to_string())
                })
                .map(|p| {
                    json!({
                        "dissid": p["tid"].to_string(), "dissname": p["dirName"], "imgurl": p["picUrl"],
                        "song_count": mids(&p["songs"]).len(), "creator": { "name": p["owner"] }
                    })
                })
                .collect(),
        ),
        _ => ("song", Vec::new()),
    };
    let page = param["page_num"].as_u64().unwrap_or(1).max(1) as usize;
    let size = param["num_per_page"].as_u64().unwrap_or(10) as usize;
    let list = window(&found, (page - 1) * size, size);
    let mut body = json!({ "song": { "list": [] }, "singer": { "list": [] }, "album": { "list": [] }, "songlist": { "list": [] } });
    body[key]["list"] = Value::Array(list);
    data(json!({ "body": body, "meta": { "sum": found.len(), "curpage": page, "perpage": size } }))
}

fn vkey(library: &Value, state: &mut State, param: &Value) -> Value {
    let uin = library["account"]["musicid"].to_string();
    if param["uin"].as_str() != Some(uin.as_str()) || param["loginflag"] != 1 {
        state.violations.push(format!("vkey asked as {param}"));
    }
    let guid = param["guid"].as_str().unwrap_or_default().to_string();
    if guid.len() != 10 || !guid.bytes().all(|b| b.is_ascii_digit()) {
        state.violations.push(format!("vkey guid {guid:?}"));
    }
    let files = mids(&param["filename"]);
    let mids = mids(&param["songmid"]);
    if mids.len() != files.len() || param["songtype"].as_array().map(Vec::len) != Some(files.len()) {
        state
            .violations
            .push("songmid, songtype and filename differ in length".into());
    }
    let mut preview = String::new();
    let infos: Vec<Value> = mids
        .iter()
        .zip(&files)
        .map(|(mid, file)| {
            let Some(song) = raw_song(library, mid) else {
                return json!({ "songmid": mid, "filename": file, "purl": "", "result": 104400 });
            };
            let media = song["file"]["media_mid"].as_str().unwrap_or_default();
            let size_key = match &file[..4] {
                "F000" => "size_flac",
                "M800" => "size_320mp3",
                "M500" => "size_128mp3",
                _ => "size_96aac",
            };
            let exists =
                song["file"][size_key].as_u64().unwrap_or(0) > 0 && file.contains(media) && !media.is_empty();
            let signed = |name: &str| {
                format!(
                    "{name}?guid={guid}&vkey=MOCKVKEY{}&uin=7000001&fromtag=120032",
                    &file[..4]
                )
            };
            let purl = match song["stream"].as_str().unwrap_or("none") {
                "full" if exists => signed(file),
                "preview" => {
                    preview = signed(&format!("RS02{media}.mp3"));
                    preview.clone()
                }
                _ => String::new(),
            };
            json!({
                "songmid": mid, "filename": file, "purl": purl,
                "result": if purl.is_empty() { song["result"].as_i64().unwrap_or(0) } else { 0 }
            })
        })
        .collect();
    data(json!({
        "midurlinfo": infos,
        "sip": ["http://aqqmusic.tc.qq.com/", "http://ws.stream.qqmusic.qq.com/"],
        "testfile2g": if preview.is_empty() { "RS020000MockMedia3.mp3?vkey=MOCKPREVIEW".to_string() } else { preview.clone() },
        "testfilewifi": "RS020000MockMedia3.mp3?vkey=MOCKPREVIEW"
    }))
}

#[test]
fn signs_like_the_reference() {
    // Not a plugin test: the mock's own signature against the values of the
    // independent Python computation.
    assert_eq!(expected_sign(b""), "zzb98ffe087addcnuyjec90xpfdilcqea80d149dc");
    assert_eq!(expected_sign(b"{}"), "zzb54b0bca3tbwb1hw2zskxzu8buwbpq83199399");
    assert_eq!(g_tk("W_X_mock"), 724_166_652);
}

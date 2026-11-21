//! A mock of NetEase Cloud Music's web API ("weapi"), for the integration
//! tests.
//!
//! It answers the endpoints the plugin uses the way this plugin's author
//! understands music.163.com to answer them (the live service could not be
//! reached from where the plugin was written; see the README for what to
//! check first). It is strict where the protocol is: every request must be a
//! weapi POST from the web player's origin whose `encSecKey` opens its
//! `params` (the plugin's test build uses a prime stand-in for the public
//! modulus, so the mock can undo the RSA step), signed-in endpoints want the
//! session cookie and the matching `csrf_token`, and anything else is
//! recorded as a violation. The library it serves (`fixtures/library.json`)
//! is invented.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockModeDecrypt, KeyIvInit};
use jf_community_common::base64;
use jf_community_testsupport::{Request, Response, TestServer, split_host};
use rsa::BoxedUint;
use serde_json::{Value, json};

/// The web client's fixed key of the inner AES layer.
const PRESET_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";
/// The IV of both layers.
const IV: &[u8; 16] = b"0102030405060708";
/// The prime the plugin's `mock-service` build uses as its public modulus…
const PRIME: &str = concat!(
    "c401a027e41a7f60d4068435b624201f496d66a178d5fb4aed4b8175a4cd17e9023ce7f036a428aeaa8903d06f",
    "bdc81ac0d529a952eba1f434727425c03e397b77209a562f0a47f371efacdea8d84efaa25619cf1116ec57f06cbd",
    "99be32ba687c61a3866e07267c6065d6659c9b696a6415a42497bac3fb2da6cffe9db0e3ad",
);
/// …and the exponent that undoes 65537 modulo it (65537⁻¹ mod (p − 1)).
const UNDO: &str = concat!(
    "8fd13d89679685b62f7ca48fa098bb3519b47b24a9bc2089a6b9cb40ddee5ac7ee8b7007d29ffaf5c64d888ded1f",
    "8c6f0ce49b901136b1fbb46bf28b2c96bba8bb0d221170ac9384ae2a1566beef106c58a6e2830be00d04ad75ac5c",
    "ddb0ed13f387d4ebb359edc43f1b1a082eb3ddd261a37b701a20c0e25169df9dcc68d975",
);

/// What the mock streams: not music.
pub const AUDIO: &[u8] = b"mock audio bytes, not music";
/// What the mock serves for pictures.
pub const PICTURE: &[u8] = b"\x89PNG\r\n\x1a\nmock picture";

/// One request as the mock decoded it.
#[derive(Debug, Clone)]
pub struct Decoded {
    /// The endpoint, without `/weapi`.
    pub endpoint: String,
    /// The JSON parameters.
    pub params: Value,
    /// The random key from `encSecKey`.
    pub key: String,
    /// `csrf_token` from the query.
    pub query_csrf: String,
    /// The `cookie` header.
    pub cookies: String,
}

/// What the mock remembers and how it behaves.
#[derive(Debug, Default)]
pub struct State {
    /// Every weapi request, decoded.
    pub decoded: Vec<Decoded>,
    /// Everything the plugin did that the protocol does not allow.
    pub violations: Vec<String>,
    /// Media paths fetched (`host/path`).
    pub served: Vec<String>,
    /// The codes the QR poll answers, in order (the last one repeats).
    pub qr_script: Vec<i64>,
    /// The `url/v1` endpoint fails, so the plugin must use the older one.
    pub v1_fails: bool,
    /// The `song/lyric/v1` endpoint fails, so the plugin must use the older one.
    pub lyric_v1_fails: bool,
    /// Search answers in the scrambled form used outside mainland China.
    pub scrambled_search: bool,
    qr_polls: HashMap<String, usize>,
    keys_issued: u32,
    /// Live sessions: (MUSIC_U, csrf).
    sessions: Vec<(String, String)>,
}

/// The running mock.
pub struct Mock {
    pub server: TestServer,
    pub state: Arc<Mutex<State>>,
}

impl Mock {
    /// A mock whose QR sign-in goes waiting → scanned → confirmed.
    pub fn start() -> Mock {
        Mock::with(|state| state.qr_script = vec![801, 802, 803])
    }

    /// A mock set up by `setup`.
    pub fn with(setup: impl FnOnce(&mut State)) -> Mock {
        let mut state = State::default();
        setup(&mut state);
        let state = Arc::new(Mutex::new(state));
        let library: Value = serde_json::from_str(include_str!("../fixtures/library.json")).unwrap();
        let shared = state.clone();
        let server = TestServer::start(move |request| handle(&library, &shared, request));
        Mock { server, state }
    }

    /// Every weapi request so far, decoded.
    pub fn decoded(&self) -> Vec<Decoded> {
        self.state.lock().unwrap().decoded.clone()
    }

    /// How many requests went to `endpoint`.
    pub fn calls(&self, endpoint: &str) -> usize {
        self.decoded().iter().filter(|d| d.endpoint == endpoint).count()
    }

    /// Protocol violations so far.
    pub fn violations(&self) -> Vec<String> {
        self.state.lock().unwrap().violations.clone()
    }

    /// Forgets the requests so far.
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.decoded.clear();
        state.served.clear();
    }

    /// Ends every session, as when a session expires.
    pub fn expire_sessions(&self) {
        self.state.lock().unwrap().sessions.clear();
    }
}

fn handle(library: &Value, state: &Mutex<State>, request: &Request) -> Response {
    let Some((host, path)) = split_host(request.path()) else {
        return Response::text(404, "not found");
    };
    if host == "music.163.com" {
        return weapi(library, state, request, path);
    }
    if host.ends_with(".music.126.net") {
        return media(state, request, host, path);
    }
    state.lock().unwrap().violations.push(format!(
        "a request to {host}, which the plugin should never contact"
    ));
    Response::text(404, "not found")
}

/// Opens a weapi request: the key from `encSecKey`, then both AES layers.
pub fn decode(params: &str, enc_sec_key: &str) -> Result<(Value, String), String> {
    if enc_sec_key.len() != 256
        || !enc_sec_key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!(
            "encSecKey is not 256 lowercase hex digits: {enc_sec_key:?}"
        ));
    }
    let number = from_hex(enc_sec_key).ok_or("encSecKey is not a number")?;
    let plain = undo_rsa(&number);
    if plain.len() > 16 {
        return Err("encSecKey does not hold a 16-byte key".into());
    }
    let mut key = vec![0u8; 16 - plain.len()];
    key.extend(plain);
    key.reverse();
    if !key.iter().all(u8::is_ascii_alphanumeric) {
        return Err(format!("the key is not 16 letters and digits: {key:?}"));
    }
    let outer = base64::decode(params).ok_or("params is not base64")?;
    let inner_text = aes_decrypt(&key, &outer)?;
    let inner_text = String::from_utf8(inner_text).map_err(|_| "the outer layer is not base64 text")?;
    let inner = base64::decode(&inner_text).ok_or("the outer layer is not base64")?;
    let json = aes_decrypt(PRESET_KEY, &inner)?;
    let value: Value = serde_json::from_slice(&json).map_err(|e| format!("not JSON: {e}"))?;
    Ok((value, String::from_utf8(key).unwrap()))
}

/// The size of the numbers here: the test prime's 1024 bits.
const BITS: u32 = 1024;

/// A number of at most 1024 bits from hex digits, as rsa's `BoxedUint`
/// (crypto-bigint arithmetic, independent of the plugin's own `rsa_raw`).
fn from_hex(hex: &str) -> Option<BoxedUint> {
    BoxedUint::from_str_radix_with_precision_vartime(hex, 16, BITS).ok()
}

/// `value^UNDO mod PRIME` as big-endian bytes without leading zeros: undoes
/// the plugin's test build raising a key to 65537 modulo the prime.
fn undo_rsa(value: &BoxedUint) -> Vec<u8> {
    let prime = from_hex(PRIME).expect("the test prime");
    let reduced = value.rem(&prime.to_nz().expect("the prime is not zero"));
    let prime = prime.to_odd().expect("the prime is odd");
    let undo = from_hex(UNDO).expect("the exponent");
    reduced
        .pow_mod(&undo, &prime)
        .to_be_bytes_trimmed_vartime()
        .into_vec()
}

fn aes_decrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    cbc::Decryptor::<aes::Aes128>::new_from_slices(key, IV)
        .map_err(|e| e.to_string())?
        .decrypt_padded_vec::<Pkcs7>(data)
        .map_err(|e| format!("AES: {e}"))
}

fn cookie(request: &Request, name: &str) -> Option<String> {
    request
        .header("cookie")?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.to_string())
}

fn ok(mut body: Value) -> Response {
    if body.get("code").is_none() {
        body["code"] = json!(200);
    }
    Response::json(&body)
}

fn weapi(library: &Value, state: &Mutex<State>, request: &Request, path: &str) -> Response {
    let mut violations = Vec::new();
    if request.method != "POST" {
        violations.push(format!("{} {path}: weapi takes POST", request.method));
    }
    if !request
        .header("referer")
        .is_some_and(|r| r.starts_with("https://music.163.com"))
    {
        violations.push(format!("{path}: no music.163.com referer"));
    }
    if !request
        .header("user-agent")
        .is_some_and(|ua| ua.starts_with("Mozilla/5.0"))
    {
        violations.push(format!("{path}: not a browser user agent"));
    }
    if !request
        .header("content-type")
        .is_some_and(|t| t.starts_with("application/x-www-form-urlencoded"))
    {
        violations.push(format!("{path}: not a form"));
    }
    let form = request.form_pairs();
    let keys: Vec<&str> = form.iter().map(|(k, _)| k.as_str()).collect();
    if keys != ["params", "encSecKey"] {
        violations.push(format!("{path}: form fields {keys:?}"));
    }
    let field = |name: &str| {
        form.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let Some(endpoint) = path.strip_prefix("/weapi") else {
        violations.push(format!("{path}: not a weapi path"));
        state.lock().unwrap().violations.extend(violations);
        return Response::text(404, "not found");
    };
    let (params, key) = match decode(&field("params"), &field("encSecKey")) {
        Ok(decoded) => decoded,
        Err(why) => {
            violations.push(format!("{endpoint}: {why}"));
            state.lock().unwrap().violations.extend(violations);
            return Response::text(400, "bad request");
        }
    };
    let query_csrf = request.query("csrf_token").unwrap_or_default();
    let param_csrf = params["csrf_token"].as_str().unwrap_or_default().to_string();
    if params.get("csrf_token").is_none() || param_csrf != query_csrf {
        violations.push(format!(
            "{endpoint}: csrf_token {param_csrf:?} in the parameters, {query_csrf:?} in the query"
        ));
    }
    let mut state = state.lock().unwrap();
    state.violations.extend(violations);
    state.decoded.push(Decoded {
        endpoint: endpoint.to_string(),
        params: params.clone(),
        key,
        query_csrf,
        cookies: request.header("cookie").unwrap_or_default().to_string(),
    });
    let session = cookie(request, "MUSIC_U").and_then(|music_u| {
        state
            .sessions
            .iter()
            .find(|(m, _)| *m == music_u)
            .map(|(_, csrf)| csrf.clone())
    });
    let signed_in = session.as_deref().is_some_and(|csrf| csrf == param_csrf);
    let needs_session = json!({ "code": 301, "message": "mock: needs a sign-in" });

    match endpoint {
        "/login/qrcode/unikey" => {
            state.keys_issued += 1;
            ok(json!({ "unikey": format!("mock-unikey-{}", state.keys_issued) }))
        }
        "/login/qrcode/client/login" => {
            let key = params["key"].as_str().unwrap_or_default().to_string();
            if !key.starts_with("mock-unikey-") {
                return ok(json!({ "code": 800, "message": "mock: no such QR code" }));
            }
            let last = state.qr_script.len().saturating_sub(1);
            let polls = state.qr_polls.entry(key).or_insert(0);
            let index = (*polls).min(last);
            *polls += 1;
            let code = state.qr_script.get(index).copied().unwrap_or(800);
            match code {
                803 => {
                    let n = state.sessions.len() + 1;
                    let music_u = format!("mock-music-u-{n}-{}", "0123456789abcdef".repeat(4));
                    let csrf = format!("mockcsrf{n:04}");
                    state.sessions.push((music_u.clone(), csrf.clone()));
                    Response::json(&json!({ "code": 803, "message": "mock: signed in" }))
                        .header(
                            "set-cookie",
                            &format!("MUSIC_U={music_u}; Max-Age=1296000; Path=/; HttpOnly"),
                        )
                        .header("set-cookie", &format!("__csrf={csrf}; Max-Age=1296010; Path=/"))
                }
                802 => ok(json!({
                    "code": 802, "message": "mock: scanned",
                    "nickname": library["account"]["nickname"],
                    "avatarUrl": "http://p1.music.126.net/mock/avatar.jpg"
                })),
                801 => ok(json!({ "code": 801, "message": "mock: waiting for a scan" })),
                other => ok(json!({ "code": other, "message": "mock: QR code expired" })),
            }
        }
        "/nuser/account/get" => {
            if signed_in {
                let account = &library["account"];
                ok(json!({
                    "account": { "id": account["userId"], "userName": "mock" },
                    "profile": { "userId": account["userId"], "nickname": account["nickname"] }
                }))
            } else {
                ok(json!({ "account": null, "profile": null }))
            }
        }
        "/logout" => {
            if let Some(music_u) = cookie(request, "MUSIC_U") {
                state.sessions.retain(|(m, _)| *m != music_u);
            }
            ok(json!({})).header("set-cookie", "MUSIC_U=; Max-Age=0; Path=/")
        }
        "/cloudsearch/get/web" => {
            if state.scrambled_search {
                return ok(json!({ "result": "c2NyYW1ibGVkIChtb2NrKQ==", "abroad": true }));
            }
            ok(search(library, &params))
        }
        "/v3/song/detail" => ok(song_details(library, &params)),
        "/v6/playlist/detail" => match find(library, "playlists", params["id"].as_u64())
            .or_else(|| find(library, "charts", params["id"].as_u64()))
        {
            Some(list) => {
                let ids = ids_of(&list["trackIds"]);
                let first: Vec<Value> = ids
                    .iter()
                    .take(2)
                    .filter_map(|id| song(library, *id, false))
                    .collect();
                let privileges: Vec<Value> = ids
                    .iter()
                    .take(2)
                    .filter_map(|id| privilege(library, *id))
                    .collect();
                let mut playlist = list.clone();
                playlist["trackIds"] =
                    Value::Array(ids.iter().map(|id| json!({ "id": id, "v": 1 })).collect());
                playlist["tracks"] = Value::Array(first);
                playlist["trackCount"] = json!(ids.len());
                ok(json!({ "playlist": playlist, "privileges": privileges }))
            }
            None => ok(json!({ "code": 404, "message": "mock: no such playlist" })),
        },
        "/v3/discovery/recommend/songs" | "/v1/discovery/recommend/resource" if !signed_in => {
            ok(needs_session)
        }
        "/v3/discovery/recommend/songs" => {
            // Song 1003 comes with its privilege, the others without.
            let songs: Vec<Value> = ids_of(&library["daily"])
                .iter()
                .filter_map(|id| song(library, *id, *id == 1003))
                .collect();
            ok(json!({ "data": { "dailySongs": songs, "recommendReasons": [] } }))
        }
        "/v1/discovery/recommend/resource" => {
            ok(json!({ "recommend": library["recommended"], "featureFirst": false }))
        }
        "/personalized/playlist" => {
            let limit = params["limit"].as_u64().unwrap_or(30) as usize;
            let lists: Vec<Value> = library["personalized"]
                .as_array()
                .unwrap()
                .iter()
                .take(limit)
                .cloned()
                .collect();
            ok(json!({ "hasTaste": false, "category": 0, "result": lists }))
        }
        "/toplist" => {
            let lists: Vec<Value> = library["charts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| {
                    let mut l = l.clone();
                    l["trackCount"] = json!(ids_of(&l["trackIds"]).len());
                    l.as_object_mut().unwrap().remove("trackIds");
                    l
                })
                .collect();
            ok(json!({ "list": lists }))
        }
        "/user/playlist" if !signed_in => ok(needs_session),
        "/user/playlist" => {
            let lists: Vec<Value> = library["playlists"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| {
                    let mut l = l.clone();
                    l["trackCount"] = json!(ids_of(&l["trackIds"]).len());
                    l.as_object_mut().unwrap().remove("trackIds");
                    l
                })
                .collect();
            ok(json!({ "playlist": lists, "more": false }))
        }
        "/song/like/get" if !signed_in => ok(needs_session),
        "/song/like/get" => ok(json!({ "ids": library["liked"], "checkPoint": 0 })),
        "/song/enhance/player/url/v1" if state.v1_fails => {
            ok(json!({ "code": -2, "message": "mock: this endpoint is unavailable" }))
        }
        "/song/enhance/player/url/v1" | "/song/enhance/player/url" if !signed_in => ok(needs_session),
        "/song/enhance/player/url/v1" => {
            let bit_rate = match params["level"].as_str().unwrap_or_default() {
                "standard" => 128_000,
                "higher" => 192_000,
                "exhigh" => 320_000,
                "lossless" => 999_000,
                _ => 128_000,
            };
            ok(urls(library, &params, bit_rate))
        }
        "/song/enhance/player/url" => {
            let bit_rate = params["br"].as_u64().unwrap_or(128_000);
            ok(urls(library, &params, bit_rate))
        }
        "/song/lyric/v1" if state.lyric_v1_fails => {
            ok(json!({ "code": -2, "message": "mock: this endpoint is unavailable" }))
        }
        "/song/lyric" | "/song/lyric/v1" => {
            let id = params["id"].as_u64().unwrap_or(0).to_string();
            match library["lyrics"].get(&id) {
                Some(found) if found.get("nolyric").is_some() => ok(json!({ "nolyric": true })),
                Some(found) => {
                    let mut body = json!({ "lrc": { "version": 3, "lyric": found["lrc"] } });
                    if let Some(translation) = found.get("tlyric") {
                        body["tlyric"] = json!({ "version": 1, "lyric": translation });
                    }
                    // Word timing only from the newer endpoint, when asked for
                    // (`yv`; `ytv` for its translation).
                    if endpoint == "/song/lyric/v1" {
                        for (key, flag) in [("yrc", "yv"), ("ytlrc", "ytv")] {
                            if let Some(text) = found.get(key).filter(|_| params.get(flag).is_some()) {
                                body[key] = json!({ "version": 2, "lyric": text });
                            }
                        }
                    }
                    ok(body)
                }
                None => ok(json!({ "uncollected": true })),
            }
        }
        _ => {
            if let Some(id) = endpoint.strip_prefix("/v1/album/") {
                return match find(library, "albums", id.parse().ok()) {
                    Some(album) => {
                        // Songs without privileges, so the plugin has to ask.
                        let songs: Vec<Value> = ids_of(&album["songs"])
                            .iter()
                            .filter_map(|id| song(library, *id, false))
                            .collect();
                        let mut album = album.clone();
                        album.as_object_mut().unwrap().remove("songs");
                        ok(json!({ "album": album, "songs": songs }))
                    }
                    None => ok(json!({ "code": 404, "message": "mock: no such album" })),
                };
            }
            if let Some(id) = endpoint.strip_prefix("/artist/albums/") {
                let id: u64 = id.parse().unwrap_or(0);
                let albums: Vec<Value> = library["albums"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|a| a["artist"]["id"].as_u64() == Some(id))
                    .map(|a| {
                        let mut a = a.clone();
                        a.as_object_mut().unwrap().remove("songs");
                        a
                    })
                    .collect();
                let (page, more) = window(&albums, &params);
                return ok(
                    json!({ "artist": find(library, "artists", Some(id)), "hotAlbums": page, "more": more }),
                );
            }
            if let Some(thread) = endpoint.strip_prefix("/v1/resource/comments/") {
                return ok(comments(thread, &params));
            }
            if let Some(id) = endpoint.strip_prefix("/v1/artist/") {
                return match find(library, "artists", id.parse().ok()) {
                    Some(artist) => ok(json!({ "artist": artist, "hotSongs": [] })),
                    None => ok(json!({ "code": 404, "message": "mock: no such artist" })),
                };
            }
            state
                .violations
                .push(format!("{endpoint}: an endpoint the mock does not know"));
            ok(json!({ "code": 404, "message": "mock: unknown endpoint" }))
        }
    }
}

fn media(state: &Mutex<State>, request: &Request, host: &str, path: &str) -> Response {
    let mut state = state.lock().unwrap();
    if request.method != "GET" {
        state.violations.push(format!("{} {host}{path}", request.method));
    }
    state.served.push(format!("{host}{path}"));
    if path.ends_with(".mp3") {
        Response::new(200, AUDIO).header("content-type", "audio/mpeg")
    } else if path.ends_with(".flac") {
        Response::new(200, AUDIO).header("content-type", "audio/flac")
    } else if path.ends_with(".jpg") {
        Response::new(200, PICTURE).header("content-type", "image/jpeg")
    } else {
        Response::text(404, "not found")
    }
}

/// Invented comments: 75 on song 1001 (two of them hot), none elsewhere.
fn comments(thread: &str, params: &Value) -> Value {
    if params["rid"].as_str() != Some(thread) {
        return json!({ "code": 400, "message": "mock: rid is not the thread" });
    }
    let total = if thread == "R_SO_4_1001" { 75 } else { 0 };
    let comment = |n: u64| {
        json!({
            "commentId": 880_000 + n, "content": format!("Invented comment {n}"),
            "time": 1_700_000_000_000_u64 - n * 60_000, "likedCount": n % 7,
            "user": { "userId": 70_000 + n, "nickname": format!("Mock Fan {n}"),
                      "avatarUrl": format!("http://p1.music.126.net/mock/avatar-{n}.jpg") },
            "beReplied": [], "showFloorComment": { "replyCount": n % 3 }
        })
    };
    let all: Vec<Value> = (1..=total).map(comment).collect();
    let (page, more) = window(&all, params);
    let mut body = json!({ "comments": page, "total": total, "more": more, "topComments": [] });
    if params["offset"].as_u64().unwrap_or(0) == 0 && total > 0 {
        body["hotComments"] = json!([
            { "commentId": 990_001, "content": "An invented hot comment", "time": 1_690_000_000_000_u64, "likedCount": 1200,
              "user": { "userId": 1, "nickname": "Mock Star Fan", "avatarUrl": "http://p2.music.126.net/mock/avatar-hot.jpg" } },
            { "commentId": 990_002, "content": "Another invented hot comment", "likedCount": 800,
              "user": { "userId": 2, "nickname": "Mock Second Fan" } }
        ]);
    }
    body
}

fn find<'a>(library: &'a Value, list: &str, id: Option<u64>) -> Option<&'a Value> {
    let id = id?;
    library[list]
        .as_array()?
        .iter()
        .find(|entry| entry["id"].as_u64() == Some(id))
}

fn ids_of(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .map(|list| list.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default()
}

/// A song as NetEase sends it, with or without its privilege inside.
fn song(library: &Value, id: u64, with_privilege: bool) -> Option<Value> {
    let mut song = find(library, "songs", Some(id))?.clone();
    let object = song.as_object_mut()?;
    object.remove("stream");
    if !with_privilege {
        object.remove("privilege");
    }
    Some(song)
}

fn privilege(library: &Value, id: u64) -> Option<Value> {
    Some(find(library, "songs", Some(id))?["privilege"].clone())
}

/// `limit` entries from `offset`, and whether more follow.
fn window(list: &[Value], params: &Value) -> (Vec<Value>, bool) {
    let offset = params["offset"].as_u64().unwrap_or(0) as usize;
    let limit = params["limit"].as_u64().unwrap_or(30) as usize;
    let page: Vec<Value> = list.iter().skip(offset).take(limit).cloned().collect();
    (page, offset + limit < list.len())
}

fn search(library: &Value, params: &Value) -> Value {
    let query = params["s"].as_str().unwrap_or_default().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let matches = |text: String| {
        let text = text.to_lowercase();
        !words.is_empty() && words.iter().all(|w| text.contains(w))
    };
    let names = |list: &Value| -> String {
        list.as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|a| a["name"].as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default()
    };
    let (key, count_key, found): (&str, &str, Vec<Value>) = match params["type"].as_u64() {
        Some(10) => (
            "albums",
            "albumCount",
            library["albums"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|a| {
                    matches(format!(
                        "{} {}",
                        a["name"].as_str().unwrap_or_default(),
                        names(&a["artists"])
                    ))
                })
                .map(|a| {
                    let mut a = a.clone();
                    a.as_object_mut().unwrap().remove("songs");
                    a
                })
                .collect(),
        ),
        Some(100) => (
            "artists",
            "artistCount",
            library["artists"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|a| matches(a["name"].as_str().unwrap_or_default().to_string()))
                .cloned()
                .collect(),
        ),
        Some(1000) => (
            "playlists",
            "playlistCount",
            library["playlists"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| matches(p["name"].as_str().unwrap_or_default().to_string()))
                .map(|p| {
                    let mut p = p.clone();
                    p["trackCount"] = json!(ids_of(&p["trackIds"]).len());
                    p.as_object_mut().unwrap().remove("trackIds");
                    p
                })
                .collect(),
        ),
        _ => (
            "songs",
            "songCount",
            library["songs"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| {
                    matches(format!(
                        "{} {}",
                        s["name"].as_str().unwrap_or_default(),
                        names(&s["ar"])
                    ))
                })
                .filter_map(|s| song(library, s["id"].as_u64()?, true))
                .collect(),
        ),
    };
    let total = found.len();
    let (page, _) = window(&found, params);
    json!({ "result": { key: page, count_key: total } })
}

fn song_details(library: &Value, params: &Value) -> Value {
    let wanted: Vec<Value> = serde_json::from_str(params["c"].as_str().unwrap_or("[]")).unwrap_or_default();
    let ids: Vec<u64> = wanted.iter().filter_map(|w| w["id"].as_u64()).collect();
    // In another order than asked, as nothing promises the asked order.
    let songs: Vec<Value> = ids
        .iter()
        .rev()
        .filter_map(|id| song(library, *id, false))
        .collect();
    let privileges: Vec<Value> = ids.iter().filter_map(|id| privilege(library, *id)).collect();
    json!({ "songs": songs, "privileges": privileges })
}

fn urls(library: &Value, params: &Value, asked: u64) -> Value {
    let ids: Vec<u64> = serde_json::from_str(params["ids"].as_str().unwrap_or("[]")).unwrap_or_default();
    let data: Vec<Value> = ids
        .iter()
        .filter_map(|id| find(library, "songs", Some(*id)))
        .map(|song| {
            let id = song["id"].as_u64().unwrap();
            let stream = &song["stream"];
            let fee = song["fee"].clone();
            let max = stream["maxbr"].as_u64().unwrap_or(0);
            let bit_rate = asked.min(max);
            let kind = if bit_rate >= 999_000 && stream["type"] == "flac" {
                "flac"
            } else {
                "mp3"
            };
            match stream["kind"].as_str().unwrap_or("none") {
                "full" => json!({
                    "id": id, "code": 200, "br": bit_rate, "type": kind, "expi": 1200, "fee": fee,
                    "url": format!("http://m701.music.126.net/mock/{id}-{bit_rate}.{kind}?vuutv=mock"),
                    "time": song["dt"], "freeTrialInfo": null
                }),
                "trial" => json!({
                    "id": id, "code": 200, "br": 128_000, "type": "mp3", "expi": 1200, "fee": fee,
                    "url": format!("http://m801.music.126.net/mock/{id}-preview.mp3"),
                    "time": 30_000, "freeTrialInfo": { "start": 0, "end": 30, "fragmentType": 1 }
                }),
                "short" => json!({
                    "id": id, "code": 200, "br": 128_000, "type": "mp3", "expi": 1200, "fee": fee,
                    "url": format!("http://m701.music.126.net/mock/{id}-clip.mp3"),
                    "time": 30_000, "freeTrialInfo": null
                }),
                "elsewhere" => json!({
                    "id": id, "code": 200, "br": 128_000, "type": "mp3", "expi": 1200, "fee": fee,
                    "url": format!("http://cdn.elsewhere.example/mock/{id}.mp3"),
                    "time": song["dt"], "freeTrialInfo": null
                }),
                _ => json!({
                    "id": id, "code": stream["code"].as_i64().unwrap_or(200), "br": 0, "url": null,
                    "fee": fee, "freeTrialInfo": null
                }),
            }
        })
        .collect();
    json!({ "data": data })
}

#[test]
fn decodes_what_python_encoded() {
    // Not a plugin test: the mock's own decoder against a key encrypted by an
    // independent computation (Python's pow) under the test prime.
    let enc = concat!(
        "70d9cbc6a9b65b0a21405666623176f6d3afb3db6dc8fe8adab6efb330325bd3f1b8e0bedd4f87a9170db91a66db",
        "ea7eafb6f1fa2008bbd12689d78bc269f791abf778033f7a5b928c2dc147978fae4b49d99055920fbfa56ab2752f",
        "5cefe80e6f0e3cfd7dcc04fc945bb8666b1a69ebdc750231b067edfe18c49f9e5d22545a",
    );
    let mut key = undo_rsa(&from_hex(enc).unwrap());
    key.reverse();
    assert_eq!(key, b"abcdefghijklmnop");
}

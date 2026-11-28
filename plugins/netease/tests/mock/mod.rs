//! A mock of NetEase Cloud Music's open platform, written from the research
//! for ADR-0018 (what the official client ncm-cli 0.1.7 sends and reads; the
//! platform's own documentation was not reachable). Every name and id is
//! invented test data; the key pair is made fresh for each test.
//!
//! Every request must be a signed GET: the mock rebuilds the canonical string
//! from the decoded query (non-empty parameters but `sign`, sorted by name,
//! `k=v` joined with `&`) and verifies `sign` (base64, RSASSA-PKCS1-v1_5 with
//! SHA-256) with the public key. Where the research has no shape (the
//! playlist lists, the profile, the refresh answer, the code for a bad
//! signature), the mock's choice is marked. Search follows what independent
//! public clients of the open platform send and read (ADR-0027), not
//! NetEase's documentation.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use jf_plugin_testkit::{Request, Response, TestServer};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use serde_json::{Value, json};

pub const APP_ID: &str = "mock-app-1001";
/// An App ID whose creator is someone else: the profile answers 301.
pub const OTHER_APP_ID: &str = "mock-app-2002";

/// Songs of the mock (encrypted ids are 32 hex digits).
pub const FULL: &str = "0123456789abcdef0123456789abc001";
pub const VIP: &str = "0123456789abcdef0123456789abc002";
pub const PREVIEW: &str = "0123456789abcdef0123456789abc003";
pub const FREE_LISTEN: &str = "0123456789abcdef0123456789abc004";
pub const HIDDEN: &str = "0123456789abcdef0123456789abc005";
pub const ALBUM_ONLY: &str = "0123456789abcdef0123456789abc006";
pub const NO_URL: &str = "0123456789abcdef0123456789abc007";
pub const CAPPED: &str = "0123456789abcdef0123456789abc008";

/// What a search for "tide" finds: 25 songs, two albums, an artist and a
/// playlist.
pub const SEARCH_SONGS: u64 = 25;
pub const SEARCH_ALBUM: &str = "a1b00000000000000000000000000001";
pub const SEARCH_ARTIST: &str = "a7000000000000000000000000000001";

#[derive(Debug, Default)]
pub struct State {
    /// QR polls answered so far, by QR key.
    pub polls: BTreeMap<String, usize>,
    /// Statuses to answer, in order, for every QR key.
    pub statuses: Vec<i64>,
    pub access: Vec<String>,
    pub refresh: Vec<String>,
    pub issued: u32,
    /// Requests whose signature did not verify.
    pub bad_signatures: usize,
    /// The data of the liked-songs playlist's answer, instead of the usual.
    pub star: Option<Value>,
    /// The data of the answers with the liked songs, instead of the songs.
    pub liked_songs: Option<Value>,
}

pub struct Mock {
    pub server: TestServer,
    pub state: Arc<Mutex<State>>,
    /// The person's key as their console would show it: base64 DER (PKCS #8).
    pub key_base64: String,
    /// The same key as PKCS #1 PEM.
    pub key_pkcs1_pem: String,
}

pub fn start(statuses: &[i64]) -> Mock {
    let private = rsa::PrivateKey::new(&mut getrandom::SysRng, 2048).expect("a test key pair");
    let public = private.to_public_key();
    let der = private.to_pkcs8_der().unwrap();
    let key_base64 = base64::engine::general_purpose::STANDARD.encode(der.as_bytes());
    let key_pkcs1_pem = private.to_pkcs1_pem(LineEnding::LF).unwrap().to_string();
    let state = Arc::new(Mutex::new(State {
        statuses: statuses.to_vec(),
        ..State::default()
    }));
    let shared = state.clone();
    let server = TestServer::start(move |request| answer(&shared, &public, request));
    Mock {
        server,
        state,
        key_base64,
        key_pkcs1_pem,
    }
}

fn code(code: i64, message: &str) -> Response {
    Response::json(&json!({ "code": code, "message": message }))
}

fn data(data: Value) -> Response {
    Response::json(&json!({ "code": 200, "message": "", "data": data }))
}

/// The canonical string of a request, from its decoded query.
pub fn canonical(query: &[(String, String)]) -> String {
    let mut pairs: Vec<&(String, String)> = query
        .iter()
        .filter(|(k, v)| k != "sign" && !v.is_empty())
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn song(id: &str, name: &str, flags: Value) -> Value {
    // `al.picUrl` is the mock's choice: the research saw no cover in lists.
    let mut song = json!({
        "encryptedId": id, "originalId": 1_900_000 + id[29..].parse::<u64>().unwrap_or(0), "name": name,
        "ar": [{ "name": "Mei Example" }], "al": { "name": "Harbour Lights", "picUrl": "http://p2.music.126.net/mock/album.jpg" },
        "dt": 215_000,
        "playFlag": false, "visible": true, "songFee": 0, "freeTrailFlag": false
    });
    for (k, v) in flags.as_object().cloned().unwrap_or_default() {
        song[k] = v;
    }
    song
}

pub fn playlist_songs() -> Vec<Value> {
    vec![
        song(FULL, "Tidal Clock", json!({ "playFlag": true })),
        song(VIP, "Members Only", json!({ "songFee": 1 })),
        song(
            PREVIEW,
            "Thirty Seconds",
            json!({ "songFee": 1, "freeTrailFlag": true }),
        ),
        song(
            FREE_LISTEN,
            "On the House",
            json!({ "songFee": 1, "resConsumable": true, "userConsumable": true }),
        ),
        song(HIDDEN, "Elsewhere", json!({ "visible": false })),
        song(ALBUM_ONLY, "Boxed Set", json!({ "songFee": 4 })),
    ]
}

/// A song a search finds, with the fields public clients read from search
/// records (the cover under `album` is the mock's choice). The second is a
/// VIP song.
fn search_song(i: u64) -> Value {
    let mut song = json!({
        "id": format!("5eac{i:028}"), "originalId": 1_000 + i, "name": format!("Tide Song {i}"), "duration": 200_000,
        "artists": [{ "id": SEARCH_ARTIST, "originalId": 31, "name": "Mei Example" }],
        "album": { "id": SEARCH_ALBUM, "originalId": 41, "name": "Tide Tables", "picUrl": "http://p1.music.126.net/mock/album.jpg" },
        "playFlag": true, "visible": true
    });
    if i == 1 {
        song["playFlag"] = json!(false);
        song["songFee"] = json!(1);
    }
    song
}

/// `search/{kind}/get`: `keyword`, `limit` and `offset` are all required;
/// without one NetEase answers HTTP 400 with an empty body (as public
/// clients report). The answer is `recordCount` and a page of `records`.
fn search(path: &str, biz: &Value) -> Response {
    let (Some(keyword), Some(limit), Some(offset)) = (
        biz["keyword"].as_str(),
        biz["limit"].as_u64(),
        biz["offset"].as_u64(),
    ) else {
        return Response::new(400, "");
    };
    let artists = json!([{ "id": SEARCH_ARTIST, "name": "Mei Example" }]);
    let records: Vec<Value> = match (keyword, path) {
        // An answer in a shape the plugin does not know.
        ("garbled", _) => {
            return data(json!({ "result": { "songs": [{ "id": FULL, "name": "Tidal Clock" }] } }));
        }
        ("slow down", _) => return code(-447, "操作太频繁 (mock)"),
        ("tide", "/search/song/get/v3") => (0..SEARCH_SONGS).map(search_song).collect(),
        ("tide", "/search/album/get/v2") => vec![
            json!({ "id": SEARCH_ALBUM, "originalId": 41, "name": "Tide Tables", "artists": artists,
                    "coverImgUrl": "http://p1.music.126.net/mock/album.jpg" }),
            json!({ "id": "a1b00000000000000000000000000002", "originalId": 42, "name": "Low Tide Live",
                    "artists": artists, "picUrl": "http://p1.music.126.net/mock/live.jpg" }),
        ],
        ("tide", "/search/artists/get/v2") => vec![
            json!({ "id": SEARCH_ARTIST, "originalId": 31, "name": "Mei Example", "picUrl": "http://p1.music.126.net/mock/artist.jpg" }),
        ],
        ("tide", "/search/playlist/get/v2") => vec![json!({
            "id": "pl-created-0", "originalId": 7_000, "name": "created list 0", "trackCount": 6,
            "coverImgUrl": "http://p1.music.126.net/mock/cover.jpg", "creatorNickName": "Mock Listener"
        })],
        _ => Vec::new(),
    };
    let found = records.len();
    let page: Vec<Value> = records
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();
    data(json!({ "recordCount": found, "records": page }))
}

fn answer(state: &Mutex<State>, public: &rsa::PublicKey, request: &Request) -> Response {
    if request.method != "GET" {
        return Response::text(405, "GET only (mock)");
    }
    if let Some(path) = request.path().strip_prefix("/audio/") {
        return Response::new(200, format!("ID3 mock audio {path}")).header("content-type", "audio/mpeg");
    }
    let query = request.query_pairs();
    let param = |k: &str| {
        query
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let mut s = state.lock().unwrap();
    // The signature over everything else, with the person's public key.
    let signature = base64::engine::general_purpose::STANDARD
        .decode(param("sign"))
        .unwrap_or_default();
    if public
        .pkcs1v15_verify::<sha2::Sha256>(canonical(&query).as_bytes(), &signature)
        .is_err()
    {
        s.bad_signatures += 1;
        // The platform's code for a bad signature is not known: the mock's own.
        return code(-1, "invalid sign (mock)");
    }
    let device: Value = serde_json::from_str(&param("device")).unwrap_or_default();
    if param("signType") != "RSA_SHA256"
        || param("timestamp").parse::<u64>().is_err()
        || device["deviceType"] != "openapi"
        || device["clientIp"] != "127.0.0.1"
    {
        return code(-2, "bad protocol fields (mock)");
    }
    let app = param("appId");
    if app != APP_ID && app != OTHER_APP_ID {
        return code(-3, "unknown appId (mock)");
    }
    let biz: Value = serde_json::from_str(&param("bizContent")).unwrap_or_default();
    let token = param("accessToken");
    let path = request
        .path()
        .strip_prefix("/openapi/music/basic")
        .unwrap_or_default();
    let signed_in = s.access.contains(&token);
    match path {
        "/user/oauth2/qrcodekey/get/v2" => {
            if biz != json!({ "type": 2, "expiredKey": "300" }) {
                return code(-4, "bad QR request (mock)");
            }
            data(
                json!({ "qrCodeUrl": "https://music.163.com/login?codekey=mock-qr-1", "uniKey": "mock-qr-1" }),
            )
        }
        "/oauth2/device/login/qrcode/get" => {
            if biz["clientId"] != app || biz["key"] != "mock-qr-1" {
                return code(-5, "bad poll (mock)");
            }
            let n = {
                let polls = s.polls.entry("mock-qr-1".into()).or_insert(0);
                *polls += 1;
                *polls
            };
            let status = s.statuses.get(n - 1).copied().unwrap_or(800);
            if status != 803 {
                return data(json!({ "status": status }));
            }
            s.issued += 1;
            let (at, rt) = (format!("at-{}", s.issued), format!("rt-{}", s.issued));
            s.access.push(at.clone());
            s.refresh.push(rt.clone());
            data(
                json!({ "status": 803, "accessToken": { "accessToken": at, "refreshToken": rt, "expireTime": 1_800_000_000_000_i64 } }),
            )
        }
        "/user/oauth2/token/refresh/v2" => {
            let refresh = biz["refreshToken"].as_str().unwrap_or_default().to_owned();
            if biz["clientId"] != app || biz["clientSecret"] != "" || !s.refresh.contains(&refresh) {
                // The refusal's code is the mock's.
                return code(-6, "refresh refused (mock)");
            }
            s.refresh.retain(|r| *r != refresh);
            s.issued += 1;
            let (at, rt) = (format!("at-{}", s.issued), format!("rt-{}", s.issued));
            s.access.push(at.clone());
            s.refresh.push(rt.clone());
            data(
                json!({ "accessToken": { "accessToken": at, "refreshToken": rt, "expireTime": 1_800_000_000_000_i64 } }),
            )
        }
        _ if token == "at-expired" => code(1406, "accessToken过期，请重新授权登录"),
        _ if !signed_in => code(-7, "not signed in (mock)"),
        "/user/profile/get/v2" if app == OTHER_APP_ID => code(301, "user mismatch (mock)"),
        // The profile's fields are the mock's choice.
        "/user/profile/get/v2" => data(json!({ "nickname": "Mock Listener", "userId": 42 })),
        "/playlist/created/get/v2" | "/playlist/subed/get/v2" => {
            let (limit, offset) = (
                biz["limit"].as_u64().unwrap_or(0),
                biz["offset"].as_u64().unwrap_or(0),
            );
            let kind = path.split('/').nth(2).unwrap_or_default().to_owned();
            let total = if kind == "created" { 3 } else { 1 };
            let records: Vec<Value> = (offset..total.min(offset + limit))
                .map(|i| json!({ "id": format!("pl-{kind}-{i}"), "originalId": 7_000 + i, "name": format!("{kind} list {i}"),
                                 "coverImgUrl": "http://p1.music.126.net/mock/cover.jpg", "trackCount": 6,
                                 "creatorNickName": "Mock Listener" }))
                .collect();
            // `records` and `recordCount`, as the official client reads them.
            data(json!({ "records": records, "recordCount": total }))
        }
        // One playlist: the liked songs (the official client reads `data` as
        // a playlist, with these field names).
        "/playlist/star/get/v2" if s.star.is_some() => data(s.star.clone().unwrap_or_default()),
        "/playlist/star/get/v2" => data(json!({
            "id": "pl-liked", "name": "Mock Listener's liked songs", "describe": "",
            "coverImgUrl": "http://p1.music.126.net/mock/liked.jpg", "creatorNickName": "Mock Listener",
            "playCount": 0, "subscribedCount": 0, "trackCount": 6, "tags": [], "createTime": 1_600_000_000_000_i64, "subed": false
        })),
        "/playlist/detail/get/v2" => {
            if biz["playlistId"] != "pl-created-0" {
                return code(404, "no such playlist (mock)");
            }
            data(json!({
                "id": "pl-created-0", "name": "created list 0", "describe": "Invented songs for a slow tide.",
                "coverImgUrl": "http://p1.music.126.net/mock/cover.jpg", "creatorNickName": "Mock Listener",
                "playCount": 1234, "subscribedCount": 5, "trackCount": 6, "tags": ["Chill", "华语"],
                "createTime": 1_700_000_000_000_i64, "subed": false
            }))
        }
        "/playlist/song/list/get/v3" => {
            if biz["playlistId"] != "pl-created-0" && biz["playlistId"] != "pl-liked" {
                return code(404, "no such playlist (mock)");
            }
            if let Some(liked) = s.liked_songs.clone().filter(|_| biz["playlistId"] == "pl-liked") {
                return data(liked);
            }
            let (limit, offset) = (
                biz["limit"].as_u64().unwrap_or(0) as usize,
                biz["offset"].as_u64().unwrap_or(0) as usize,
            );
            let songs: Vec<Value> = playlist_songs().into_iter().skip(offset).take(limit).collect();
            data(json!({ "records": songs }))
        }
        "/search/song/get/v3"
        | "/search/album/get/v2"
        | "/search/artists/get/v2"
        | "/search/playlist/get/v2" => search(path, &biz),
        // An album's songs as public clients ask for them; the answer's
        // shape is the mock's choice (as the playlists' songs).
        "/album/song/list/get/v2" => {
            if biz["albumId"] != SEARCH_ALBUM {
                return code(404, "no such album (mock)");
            }
            let (limit, offset) = (
                biz["limit"].as_u64().unwrap_or(0) as usize,
                biz["offset"].as_u64().unwrap_or(0) as usize,
            );
            let songs: Vec<Value> = playlist_songs().into_iter().skip(offset).take(limit).collect();
            data(json!({ "records": songs, "recordCount": 6 }))
        }
        // An artist's popular songs: the parameter and the answer (a bare
        // list) are the mock's choice.
        "/artists/hotsong/list/get/v2" => {
            if biz["artistId"] != SEARCH_ARTIST {
                return code(404, "no such artist (mock)");
            }
            data(json!(playlist_songs()))
        }
        "/song/detail/get/v2" => {
            let id = biz["songId"].as_str().unwrap_or_default();
            if biz["withUrl"] != true {
                return code(-8, "withUrl missing (mock)");
            }
            let bitrate = biz["bitrate"].as_u64().unwrap_or(0);
            let url = format!(
                "{}/audio/{id}-{bitrate}.mp3",
                request
                    .header("host")
                    .map(|h| format!("http://{h}"))
                    .unwrap_or_default()
            );
            let mut detail = json!({ "name": "Tidal Clock", "artistName": "Mei Example", "albumName": "Harbour Lights",
                                     "duration": 215_000, "br": bitrate * 1000, "level": "standard", "playUrl": url,
                                     "freeTrail": null, "freeTrialPrivilege": { "resConsumable": false, "userConsumable": false } });
            match id {
                FULL => {}
                VIP => detail["songFee"] = json!(1),
                PREVIEW => detail["freeTrail"] = json!({ "start": 0, "end": 30 }),
                FREE_LISTEN => {
                    detail["freeTrail"] = json!({ "start": 0, "end": 30 });
                    detail["freeTrialPrivilege"] = json!({ "resConsumable": true, "userConsumable": true });
                }
                NO_URL => detail["playUrl"] = Value::Null,
                CAPPED => return code(-461, "今日播放次数已达上限，明天再来听歌吧~"),
                _ => return code(404, "no such song (mock)"),
            }
            data(detail)
        }
        _ => code(404, "unknown path (mock)"),
    }
}

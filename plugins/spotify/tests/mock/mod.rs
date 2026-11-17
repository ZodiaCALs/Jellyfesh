//! A mock of the Spotify accounts service and Web API, written from the
//! Web API reference (its OpenAPI schema, 2026) and the authorisation guide
//! (Authorization Code with PKCE). Every name and id is invented test data.
//!
//! It checks what Spotify checks for PKCE (the verifier against the
//! challenge, the redirect URI, the Client ID), keeps a small player for the
//! commands, and can play a free account (403 on commands) or a throttled
//! one (429 with `Retry-After`). The 403 body's `reason` follows Spotify's
//! player error reasons, which the published schema does not list.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use jf_plugin_testkit::{Request, Response, TestServer};
use serde_json::{Value, json};

/// The Client ID of the person's own (invented) app.
pub const CLIENT_ID: &str = "0123456789abcdef0123456789abcdef";
pub const ALBUM: &str = "6akEvsycLGftJxYudPjmqK";
pub const PLAYLIST: &str = "37i9dQZF1DXmock01";
pub const ARTIST: &str = "0OdUWJ0sBjDrqHygGUXeCF";

/// What the mock knows and what the player does.
#[derive(Debug, Default)]
pub struct State {
    /// The code challenge of the authorisation in progress.
    pub challenge: Option<String>,
    pub redirect_uri: Option<String>,
    pub access: Vec<String>,
    pub refresh: Vec<String>,
    pub issued: u32,
    /// A free account: player commands answer 403.
    pub free: bool,
    /// Every request answers 429 with this many seconds.
    pub throttle: Option<u32>,
    pub playing: bool,
    pub device: Option<String>,
    pub context: Option<String>,
    pub uris: Vec<String>,
    pub index: usize,
    pub position_ms: u64,
    pub volume: u32,
    pub shuffle: bool,
    pub repeat: String,
}

pub struct Mock {
    pub server: TestServer,
    pub state: Arc<Mutex<State>>,
}

pub fn start() -> Mock {
    let state = Arc::new(Mutex::new(State {
        repeat: "off".into(),
        volume: 50,
        ..State::default()
    }));
    let shared = state.clone();
    let server = TestServer::start(move |request| answer(&shared, request));
    Mock { server, state }
}

fn api_error(status: u16, message: &str, reason: Option<&str>) -> Response {
    let mut error = json!({ "status": status, "message": message });
    if let Some(reason) = reason {
        error["reason"] = json!(reason);
    }
    Response::new(status, json!({ "error": error }).to_string()).header("content-type", "application/json")
}

fn token_error(error: &str, description: &str) -> Response {
    Response::new(
        400,
        json!({ "error": error, "error_description": description }).to_string(),
    )
    .header("content-type", "application/json")
}

fn image(id: &str) -> Value {
    json!([
        { "url": format!("https://i.scdn.co/image/{id}-640"), "width": 640, "height": 640 },
        { "url": format!("https://i.scdn.co/image/{id}-300"), "width": 300, "height": 300 },
        { "url": format!("https://i.scdn.co/image/{id}-64"), "width": 64, "height": 64 }
    ])
}

fn album(id: &str, name: &str) -> Value {
    json!({ "id": id, "name": name, "type": "album", "total_tracks": 4, "release_date": "2021-05-07",
            "artists": [{ "id": ARTIST, "name": "Ada Example" }], "images": image(id) })
}

pub fn track_id(n: usize) -> String {
    format!("4uLU6hMCjMI75M1A2tK{n:03}")
}

fn track(n: usize) -> Value {
    json!({ "id": track_id(n), "name": format!("Harbour Song {n}"), "type": "track", "duration_ms": 200_000,
            "track_number": n % 4 + 1, "disc_number": 1, "explicit": false, "is_local": false,
            "external_ids": { "isrc": format!("QZMCK26{n:05}") },
            "artists": [{ "id": ARTIST, "name": "Ada Example" }], "album": album(ALBUM, "Night Ferry") })
}

/// A Web API page of `total` items made by `item`.
fn paging(
    path: &str,
    total: usize,
    query: &BTreeMap<String, String>,
    item: impl Fn(usize) -> Value,
) -> Value {
    let limit: usize = query.get("limit").and_then(|l| l.parse().ok()).unwrap_or(20);
    let offset: usize = query.get("offset").and_then(|o| o.parse().ok()).unwrap_or(0);
    let items: Vec<Value> = (offset..total.min(offset + limit)).map(item).collect();
    let next = (offset + limit < total)
        .then(|| format!("https://api.spotify.com/v1{path}?offset={}", offset + limit));
    json!({ "href": path, "items": items, "limit": limit, "offset": offset, "next": next, "total": total })
}

fn answer(state: &Mutex<State>, request: &Request) -> Response {
    let mut s = state.lock().unwrap();
    if let Some(seconds) = s.throttle {
        return Response::new(429, "").header("retry-after", &seconds.to_string());
    }
    let path = request.path().to_owned();
    if path == "/api/token" {
        return token(&mut s, request);
    }
    let Some(rest) = path.strip_prefix("/v1") else {
        return Response::text(404, "not found");
    };
    let token = request
        .header("authorization")
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or_default();
    if !s.access.iter().any(|t| t == token) {
        return api_error(401, "The access token expired", None);
    }
    let query: BTreeMap<String, String> = request.query_pairs().into_iter().collect();
    let method = request.method.as_str();
    if rest.starts_with("/me/player") && method != "GET" {
        return player_command(&mut s, method, rest, &query, request.json());
    }
    let ok = |value: Value| Response::json(&value);
    match (method, rest) {
        ("GET", "/me") => ok(json!({ "display_name": "Mock Listener", "id": "mockuser" })),
        ("GET", "/search") => {
            let types = query.get("type").cloned().unwrap_or_default();
            let mut body = json!({});
            for kind in types.split(',') {
                let page = match kind {
                    "track" => paging("/search", 12, &query, track),
                    "artist" => paging(
                        "/search",
                        1,
                        &query,
                        |_| json!({ "id": ARTIST, "name": "Ada Example", "images": image("artist") }),
                    ),
                    "album" => paging("/search", 1, &query, |_| album(ALBUM, "Night Ferry")),
                    "playlist" => {
                        // Spotify sometimes sends null entries here.
                        let mut page = paging("/search", 2, &query, |i| {
                            json!({ "id": PLAYLIST, "name": format!("Harbour Mix {i}"),
                                    "owner": { "display_name": "Mock Curator" }, "images": image("pl"), "items": { "total": 30 } })
                        });
                        if let Some(items) = page["items"].as_array_mut() {
                            items.insert(0, Value::Null);
                        }
                        page
                    }
                    _ => continue,
                };
                body[format!("{kind}s")] = page;
            }
            ok(body)
        }
        ("GET", "/me/playlists") => ok(paging(rest, 3, &query, |i| {
            json!({ "id": format!("{PLAYLIST}{i}"), "name": format!("Playlist {i}"), "owner": { "display_name": "Mock Listener" },
                    "images": image("pl"), "items": { "total": 30 }, "tracks": { "total": 30 } })
        })),
        ("GET", "/me/albums") => ok(paging(
            rest,
            2,
            &query,
            |i| json!({ "added_at": "2026-01-01T00:00:00Z", "album": album(&format!("{ALBUM}{i}"), &format!("Saved {i}")) }),
        )),
        ("GET", "/me/tracks") => ok(paging(
            rest,
            60,
            &query,
            |i| json!({ "added_at": "2026-01-01T00:00:00Z", "track": track(i) }),
        )),
        ("GET", path) if path == format!("/playlists/{PLAYLIST}/items") => {
            ok(paging(path, 5, &query, |i| match i {
                // A local file and a removed track are skipped.
                3 => {
                    json!({ "is_local": true, "item": { "id": null, "name": "Local file", "type": "track", "is_local": true } })
                }
                4 => json!({ "item": null, "track": null }),
                // Older playlists answered with `track`; newer with `item`.
                0 => json!({ "track": track(i) }),
                _ => json!({ "item": track(i) }),
            }))
        }
        ("GET", path) if path == format!("/albums/{ALBUM}/tracks") => ok(paging(path, 4, &query, |i| {
            let mut t = track(i);
            t.as_object_mut().unwrap().remove("album");
            t
        })),
        ("GET", path) if path == format!("/artists/{ARTIST}/albums") => {
            if query.get("include_groups").map(String::as_str) != Some("album,single") {
                return api_error(400, "include_groups missing (mock)", None);
            }
            ok(paging(path, 15, &query, |i| {
                album(&format!("{ALBUM}{i:02}"), &format!("Album {i}"))
            }))
        }
        ("GET", path) if path == format!("/albums/{ALBUM}") => ok(album(ALBUM, "Night Ferry")),
        ("GET", path) if path == format!("/playlists/{PLAYLIST}") => ok(
            json!({ "id": PLAYLIST, "name": "Harbour Mix", "owner": { "display_name": "Mock Curator" }, "images": image("pl") }),
        ),
        ("GET", "/me/player/devices") => ok(json!({ "devices": [
            { "id": "dev-phone", "is_active": false, "is_private_session": false, "is_restricted": false,
              "name": "Pocket Phone", "type": "Smartphone", "volume_percent": 80, "supports_volume": true },
            { "id": "dev-kitchen", "is_active": s.device.as_deref() == Some("dev-kitchen"), "is_private_session": false,
              "is_restricted": false, "name": "Kitchen Speaker", "type": "Speaker", "volume_percent": s.volume, "supports_volume": true },
            { "id": "dev-tv", "is_active": false, "is_private_session": false, "is_restricted": true,
              "name": "Locked TV", "type": "TV", "volume_percent": null, "supports_volume": false }
        ] })),
        ("GET", "/me/player") => {
            let Some(device) = s.device.clone() else {
                return Response::no_content();
            };
            let item = s.uris.get(s.index).map(|uri| {
                let n: usize = uri.rsplit('K').next().and_then(|n| n.parse().ok()).unwrap_or(0);
                track(n)
            });
            ok(json!({
                "device": { "id": device, "is_active": true, "is_restricted": false, "name": "Kitchen Speaker",
                            "type": "Speaker", "volume_percent": s.volume, "supports_volume": true },
                "repeat_state": s.repeat, "shuffle_state": s.shuffle,
                "context": s.context.as_ref().map(|uri| json!({ "type": uri.split(':').nth(1), "uri": uri, "href": "" })),
                "timestamp": 1_780_000_000_000_i64, "progress_ms": s.position_ms, "is_playing": s.playing,
                "item": item, "currently_playing_type": "track", "actions": {}
            }))
        }
        _ => api_error(404, "Not found (mock)", None),
    }
}

fn token(s: &mut State, request: &Request) -> Response {
    let form: BTreeMap<String, String> = request.form_pairs().into_iter().collect();
    if form.get("client_id").map(String::as_str) != Some(CLIENT_ID) {
        return token_error("invalid_client", "Invalid client");
    }
    if form.contains_key("client_secret") {
        return token_error("invalid_request", "PKCE clients send no secret (mock)");
    }
    match form.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            let verifier = form.get("code_verifier").cloned().unwrap_or_default();
            if form.get("code").map(String::as_str) != Some("good-code")
                || s.challenge.as_deref() != Some(super_challenge(&verifier).as_str())
                || s.redirect_uri.as_ref() != form.get("redirect_uri")
            {
                return token_error("invalid_grant", "Invalid authorization code");
            }
        }
        Some("refresh_token") => {
            let refresh = form.get("refresh_token").cloned().unwrap_or_default();
            if !s.refresh.contains(&refresh) {
                return token_error("invalid_grant", "Refresh token revoked");
            }
            // Rotation: the old refresh token stops working.
            s.refresh.retain(|r| *r != refresh);
        }
        _ => return token_error("unsupported_grant_type", "grant_type"),
    }
    s.issued += 1;
    let (access, refresh) = (format!("at-{}", s.issued), format!("rt-{}", s.issued));
    s.access.push(access.clone());
    s.refresh.push(refresh.clone());
    Response::json(
        &json!({ "access_token": access, "token_type": "Bearer", "scope": "user-read-private",
                            "expires_in": 3600, "refresh_token": refresh }),
    )
}

/// base64url(SHA-256(verifier)) without padding (RFC 7636), done here without
/// the plugin's code so the check is independent.
pub fn super_challenge(verifier: &str) -> String {
    use base64::Engine as _;
    use sha2::Digest as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()))
}

fn player_command(
    s: &mut State,
    method: &str,
    path: &str,
    query: &BTreeMap<String, String>,
    body: Option<Value>,
) -> Response {
    if s.free {
        return api_error(
            403,
            "Player command failed: Premium required",
            Some("PREMIUM_REQUIRED"),
        );
    }
    let number = |key: &str| query.get(key).and_then(|v| v.parse::<u64>().ok());
    match (method, path) {
        ("PUT", "/me/player") => {
            let body = body.unwrap_or_default();
            let ids = body["device_ids"].as_array().cloned().unwrap_or_default();
            if ids.len() != 1 {
                return api_error(400, "Only one device id (mock)", None);
            }
            s.device = ids[0].as_str().map(str::to_owned);
            if body["play"] == true {
                s.playing = true;
            }
        }
        ("PUT", "/me/player/play") => {
            if let Some(device) = query.get("device_id") {
                s.device = Some(device.clone());
            }
            if s.device.is_none() {
                return api_error(
                    404,
                    "Player command failed: No active device found",
                    Some("NO_ACTIVE_DEVICE"),
                );
            }
            if let Some(body) = body.filter(|b| b.is_object() && !b.as_object().unwrap().is_empty()) {
                let position = body["offset"]["position"].as_u64().unwrap_or(0) as usize;
                if let Some(context) = body["context_uri"].as_str() {
                    s.context = Some(context.to_owned());
                    // The mock's albums and playlists list tracks 0, 1, 2, …
                    s.uris = (0..12)
                        .map(|n| format!("spotify:track:{}", track_id(n)))
                        .collect();
                } else {
                    s.context = None;
                    s.uris = body["uris"]
                        .as_array()
                        .map(|u| u.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                        .unwrap_or_default();
                }
                s.index = position;
                s.position_ms = 0;
            }
            s.playing = true;
        }
        ("PUT", "/me/player/pause") => s.playing = false,
        ("POST", "/me/player/next") => s.index += 1,
        ("POST", "/me/player/previous") => s.index = s.index.saturating_sub(1),
        ("PUT", "/me/player/seek") => s.position_ms = number("position_ms").unwrap_or(0),
        ("PUT", "/me/player/volume") => s.volume = number("volume_percent").unwrap_or(0) as u32,
        ("PUT", "/me/player/shuffle") => s.shuffle = query.get("state").map(String::as_str) == Some("true"),
        ("PUT", "/me/player/repeat") => s.repeat = query.get("state").cloned().unwrap_or_default(),
        _ => return api_error(404, "Not found (mock)", None),
    }
    Response::no_content()
}

//! A mock (Open)Subsonic server.
//!
//! It answers like the API in the OpenSubsonic documentation
//! (opensubsonic.netlify.app, which could not be reached from here; read
//! from its sources, github.com/opensubsonic/open-subsonic-api at commit
//! c1fd43f, 2026-11-02, checked again at e0b7989, 2026-11-09):
//! `/rest/<method>.view` with `v`, `c` and `f=json`, `subsonic-response`
//! envelopes with the documented error codes, token sign-in (`u`,
//! `t = md5(password + salt)`, `s`), API keys (`apiKey`, never with `u`),
//! form POST, and the methods the plugin uses. The library and lyrics it
//! serves (`fixtures/`) are invented; nothing here comes from a real server
//! or account.

use std::collections::HashMap;

use jf_plugin_testkit::{Request, Response, TestServer};
use md5::{Digest, Md5};
use serde_json::{Map, Value, json};

/// Accounts and their passwords.
pub const USERS: &[(&str, &str)] = &[
    ("alice", "correct horse battery staple"),
    // A password inside the user name, which the host refuses to store.
    ("jukebox-demo", "demo"),
];
/// The API key the mock accepts (for alice).
pub const API_KEY: &str = "7EF3C618-96A7-4121-A89B-C63629298729-mock";
/// Bytes the mock streams.
pub const AUDIO: &[u8] = b"mock audio bytes";
/// What the mock answers for cover art.
pub const PNG: &[u8] = b"\x89PNG\r\n\x1a\nmock image";

/// What kind of server the mock pretends to be.
#[derive(Debug, Clone)]
pub struct Mock {
    /// `getOpenSubsonicExtensions` exists (an OpenSubsonic server).
    pub open_subsonic: bool,
    /// Subsonic API version in every answer.
    pub version: &'static str,
    /// Offers `apiKeyAuthentication`.
    pub api_keys: bool,
    /// Accepts token sign-in (`u`, `t`, `s`); with false it answers 41.
    pub token_auth: bool,
    /// Offers `formPost`.
    pub form_post: bool,
    /// Offers `songLyrics` (`getLyricsBySongId`).
    pub song_lyrics: bool,
    /// The account's `streamRole`.
    pub stream_role: bool,
    /// Unknown methods answer an HTTP 404 page instead of a Subsonic error.
    pub unknown_404: bool,
}

impl Mock {
    /// Like Navidrome 0.64: OpenSubsonic with form POST and structured
    /// lyrics, token sign-in, no API keys yet.
    pub fn navidrome_like() -> Mock {
        Mock {
            open_subsonic: true,
            version: "1.16.1",
            api_keys: false,
            token_auth: true,
            form_post: true,
            song_lyrics: true,
            stream_role: true,
            unknown_404: false,
        }
    }

    /// An OpenSubsonic server with API keys that no longer takes tokens (as
    /// the extension recommends), and without form POST.
    pub fn with_api_keys() -> Mock {
        Mock {
            api_keys: true,
            token_auth: false,
            form_post: false,
            ..Mock::navidrome_like()
        }
    }

    /// A plain Subsonic server on API 1.12.0: no OpenSubsonic, no tokens.
    pub fn old() -> Mock {
        Mock {
            open_subsonic: false,
            version: "1.12.0",
            api_keys: false,
            token_auth: false,
            form_post: false,
            song_lyrics: false,
            stream_role: true,
            unknown_404: false,
        }
    }

    /// Starts the server.
    pub fn start(self) -> TestServer {
        let library: Value = serde_json::from_str(include_str!("../fixtures/library.json")).unwrap();
        let lyrics: Value = serde_json::from_str(include_str!("../fixtures/lyrics.json")).unwrap();
        TestServer::start(move |request| self.handle(&library, &lyrics, request))
    }

    fn extensions(&self) -> Value {
        let mut list = vec![json!({ "name": "transcodeOffset", "versions": [1] })];
        if self.api_keys {
            list.push(json!({ "name": "apiKeyAuthentication", "versions": [1] }));
        }
        if self.form_post {
            list.push(json!({ "name": "formPost", "versions": [1] }));
        }
        if self.song_lyrics {
            list.push(json!({ "name": "songLyrics", "versions": [1] }));
        }
        Value::Array(list)
    }

    fn envelope(&self, status: &str, extra: Value) -> Value {
        let mut body = Map::new();
        body.insert("status".into(), status.into());
        body.insert("version".into(), self.version.into());
        if self.open_subsonic {
            body.insert("type".into(), "mocksonic".into());
            body.insert("serverVersion".into(), "0.1.0 (test)".into());
            body.insert("openSubsonic".into(), true.into());
        }
        if let Value::Object(extra) = extra {
            body.extend(extra);
        }
        json!({ "subsonic-response": body })
    }

    fn ok(&self, extra: Value) -> Response {
        Response::json(&self.envelope("ok", extra))
    }

    fn fail(&self, code: i64, message: &str) -> Response {
        Response::json(&self.envelope("failed", json!({ "error": { "code": code, "message": message } })))
    }

    fn authenticate(&self, request: &Request) -> Result<String, Response> {
        let p = |key: &str| request.param(key);
        if p("p").is_some() {
            // The plugin must never send a password.
            return Err(self.fail(42, "Password authentication is not allowed"));
        }
        match (p("apiKey"), p("u"), p("t"), p("s")) {
            (Some(_), Some(_), _, _) => {
                Err(self.fail(43, "Multiple conflicting authentication mechanisms provided"))
            }
            (Some(key), None, _, _) => {
                if !self.api_keys {
                    Err(self.fail(42, "Provided authentication mechanism not supported"))
                } else if key == API_KEY {
                    Ok("alice".into())
                } else {
                    Err(self.fail(44, "Invalid API key"))
                }
            }
            (None, Some(user), Some(token), Some(salt)) => {
                if !self.token_auth {
                    return Err(Response::json(&self.envelope(
                        "failed",
                        json!({ "error": {
                            "code": 41,
                            "message": "Token authentication not supported for LDAP users.",
                            "helpUrl": "https://music.example/settings/api-keys"
                        }}),
                    )));
                }
                let known = USERS.iter().find(|(u, _)| *u == user);
                match known {
                    Some((_, password))
                        if salt.len() >= 6 && token == md5_hex(&format!("{password}{salt}")) =>
                    {
                        Ok(user)
                    }
                    _ => Err(self.fail(40, "Wrong username or password")),
                }
            }
            _ => Err(self.fail(10, "Required parameter is missing")),
        }
    }

    fn handle(&self, library: &Value, lyrics: &Value, request: &Request) -> Response {
        let Some(method) = request.path().strip_prefix("/rest/") else {
            return Response::text(404, "<html><body>Not found</body></html>");
        };
        let method = method.trim_end_matches(".view");
        let p = |key: &str| request.param(key);
        if p("c").as_deref() != Some("Jellyfesh") || p("v").is_none() {
            return self.fail(10, "Required parameter is missing");
        }
        if p("f").as_deref() != Some("json") {
            return Response::new(200, "<subsonic-response status=\"ok\"/>")
                .header("content-type", "text/xml");
        }
        if request.method == "POST" && !self.form_post {
            return Response::text(405, "Method not allowed");
        }
        if method == "getOpenSubsonicExtensions" {
            return if self.open_subsonic {
                self.ok(json!({ "openSubsonicExtensions": self.extensions() }))
            } else if self.unknown_404 {
                Response::text(404, "<html><body>Not found</body></html>")
            } else {
                self.fail(0, "Unknown method getOpenSubsonicExtensions")
            };
        }
        let user = match self.authenticate(request) {
            Ok(user) => user,
            Err(response) => return response,
        };
        let lib = Library(library);
        let id = p("id").unwrap_or_default();
        match method {
            "ping" => self.ok(json!({})),
            "tokenInfo" if self.api_keys => self.ok(json!({ "tokenInfo": { "username": user } })),
            "getUser" => self.ok(json!({ "user": {
                "username": p("username").unwrap_or_default(),
                "scrobblingEnabled": true,
                "adminRole": false,
                "streamRole": self.stream_role,
                "downloadRole": true,
                "folder": [1]
            }})),
            "getArtists" => {
                // Grouped by first letter, groups and artists in name order.
                let mut artists = lib.list("artists").clone();
                artists.sort_by_key(|a| a["name"].as_str().unwrap_or_default().to_uppercase());
                let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
                for artist in &artists {
                    let letter = artist["name"]
                        .as_str()
                        .and_then(|n| n.chars().next())
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_default();
                    match groups.iter_mut().find(|(l, _)| *l == letter) {
                        Some((_, list)) => list.push(artist.clone()),
                        None => groups.push((letter, vec![artist.clone()])),
                    }
                }
                let index: Vec<Value> = groups
                    .into_iter()
                    .map(|(name, artist)| json!({ "name": name, "artist": artist }))
                    .collect();
                self.ok(json!({ "artists": { "ignoredArticles": "The An A", "index": index } }))
            }
            "getArtist" => match lib.find("artists", &id) {
                Some(artist) => {
                    let mut artist = artist.clone();
                    let albums: Vec<Value> = lib
                        .list("albums")
                        .iter()
                        .filter(|a| a["artistId"] == artist["id"])
                        .map(without_songs)
                        .collect();
                    artist["album"] = Value::Array(albums);
                    self.ok(json!({ "artist": artist }))
                }
                None => self.fail(70, "Artist not found"),
            },
            "getAlbum" => match lib.find("albums", &id) {
                Some(album) => self.ok(json!({ "album": album })),
                None => self.fail(70, "Album not found"),
            },
            "getSong" => match lib.song(&id) {
                Some(song) => self.ok(json!({ "song": song })),
                None => self.fail(70, "Song not found"),
            },
            "getArtistInfo2" => match lib.find("artists", &id) {
                Some(_) => self.ok(
                    json!({ "artistInfo2": lib.0["artistInfo2"].get(&id).cloned().unwrap_or(json!({})) }),
                ),
                None => self.fail(70, "Artist not found"),
            },
            "getAlbumInfo2" if self.version != "1.12.0" => match lib.find("albums", &id) {
                Some(_) => {
                    self.ok(json!({ "albumInfo": lib.0["albumInfo"].get(&id).cloned().unwrap_or(json!({})) }))
                }
                None => self.fail(70, "Album not found"),
            },
            "getAlbumList2" => {
                let mut albums: Vec<Value> = lib.list("albums").iter().map(without_songs).collect();
                match p("type").as_deref() {
                    Some("alphabeticalByName") => {
                        albums.sort_by_key(|a| a["name"].as_str().map(str::to_string))
                    }
                    Some("newest") => {
                        albums.sort_by_key(|a| std::cmp::Reverse(a["created"].as_str().map(str::to_string)))
                    }
                    Some(
                        "recent" | "frequent" | "random" | "alphabeticalByArtist" | "starred" | "highest",
                    ) => {}
                    _ => return self.fail(10, "Required parameter is missing: type"),
                }
                let (offset, size) = (number(p("offset"), 0), number(p("size"), 10).min(500));
                let page: Vec<Value> = albums.into_iter().skip(offset).take(size).collect();
                self.ok(json!({ "albumList2": { "album": page } }))
            }
            "getPlaylists" => {
                let lists: Vec<Value> = lib
                    .list("playlists")
                    .iter()
                    .map(|p| {
                        let mut p = p.clone();
                        p.as_object_mut().unwrap().remove("entry");
                        p
                    })
                    .collect();
                self.ok(json!({ "playlists": { "playlist": lists } }))
            }
            "getPlaylist" => match lib.find("playlists", &id) {
                Some(playlist) => {
                    let mut playlist = playlist.clone();
                    let entries: Vec<Value> = playlist["entry"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|id| lib.song(id.as_str().unwrap()))
                        .collect();
                    playlist["entry"] = Value::Array(entries);
                    self.ok(json!({ "playlist": playlist }))
                }
                None => self.fail(70, "Playlist not found"),
            },
            "getStarred2" => {
                let starred = &library["starred"];
                let pick = |kind: &str, find: &dyn Fn(&str) -> Option<Value>| -> Vec<Value> {
                    starred[kind]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|id| find(id.as_str().unwrap()))
                        .collect()
                };
                self.ok(json!({ "starred2": {
                    "artist": pick("artist", &|id| lib.find("artists", id).cloned()),
                    "album": pick("album", &|id| lib.find("albums", id).map(without_songs)),
                    "song": pick("song", &|id| lib.song(id)),
                }}))
            }
            "search3" => {
                let query = p("query").unwrap_or_default().to_lowercase();
                let matches =
                    |text: &Value| text.as_str().unwrap_or_default().to_lowercase().contains(&query);
                let slice = |items: Vec<Value>, count: &str, offset: &str| -> Vec<Value> {
                    items
                        .into_iter()
                        .skip(number(p(offset), 0))
                        .take(number(p(count), 20))
                        .collect()
                };
                let artists = lib
                    .list("artists")
                    .iter()
                    .filter(|a| matches(&a["name"]))
                    .cloned()
                    .collect();
                let albums = lib
                    .list("albums")
                    .iter()
                    .filter(|a| matches(&a["name"]))
                    .map(without_songs)
                    .collect();
                let songs = lib.songs().into_iter().filter(|s| matches(&s["title"])).collect();
                self.ok(json!({ "searchResult3": {
                    "artist": slice(artists, "artistCount", "artistOffset"),
                    "album": slice(albums, "albumCount", "albumOffset"),
                    "song": slice(songs, "songCount", "songOffset"),
                }}))
            }
            "getLyricsBySongId" if self.song_lyrics => match lib.song(&id) {
                Some(_) => self
                    .ok(json!({ "lyricsList": lyrics["lyricsList"].get(&id).cloned().unwrap_or(json!({})) })),
                None => self.fail(70, "Song not found"),
            },
            "getLyrics" => {
                let key = format!(
                    "{}|{}",
                    p("artist").unwrap_or_default(),
                    p("title").unwrap_or_default()
                );
                self.ok(json!({ "lyrics": lyrics["lyrics"].get(&key).cloned().unwrap_or(json!({})) }))
            }
            "getSimilarSongs" => {
                if lib.song(&id).is_none()
                    && lib.find("albums", &id).is_none()
                    && lib.find("artists", &id).is_none()
                {
                    return self.fail(70, "Not found");
                }
                let songs: Vec<Value> = library["similar"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|id| lib.song(id.as_str().unwrap()))
                    .take(number(p("count"), 50))
                    .collect();
                self.ok(json!({ "similarSongs": { "song": songs } }))
            }
            "stream" => match lib.song(&id) {
                Some(song) => {
                    let format = p("format").unwrap_or_else(|| "raw".into());
                    let kind = if format == "raw" {
                        song["contentType"]
                            .as_str()
                            .unwrap_or("application/octet-stream")
                            .to_string()
                    } else {
                        format!("audio/{format}")
                    };
                    Response::new(200, AUDIO).header("content-type", &kind)
                }
                None => self.fail(70, "Song not found"),
            },
            "getCoverArt" => Response::new(200, PNG).header("content-type", "image/png"),
            _ if self.unknown_404 => Response::text(404, "<html><body>Not found</body></html>"),
            _ => self.fail(0, &format!("Unknown method {method}")),
        }
    }
}

/// The library fixture.
struct Library<'a>(&'a Value);

impl Library<'_> {
    fn list(&self, kind: &str) -> &Vec<Value> {
        self.0[kind].as_array().unwrap()
    }

    fn find(&self, kind: &str, id: &str) -> Option<&Value> {
        self.list(kind).iter().find(|v| v["id"] == id)
    }

    fn songs(&self) -> Vec<Value> {
        self.list("albums")
            .iter()
            .flat_map(|a| a["song"].as_array().unwrap().iter().cloned())
            .collect()
    }

    fn song(&self, id: &str) -> Option<Value> {
        self.songs().into_iter().find(|s| s["id"] == id)
    }
}

fn without_songs(album: &Value) -> Value {
    let mut album = album.clone();
    album.as_object_mut().unwrap().remove("song");
    album
}

fn number(value: Option<String>, default: usize) -> usize {
    value.and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Lowercase hex MD5, as the token is computed.
pub fn md5_hex(text: &str) -> String {
    Md5::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Query and form parameters of every request, for checks on what was sent.
pub fn all_params(server: &TestServer) -> Vec<(String, HashMap<String, String>)> {
    server
        .requests()
        .into_iter()
        .map(|r| (r.path().to_string(), r.params().into_iter().collect()))
        .collect()
}

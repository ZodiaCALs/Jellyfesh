//! A mock Jellyfin server.
//!
//! It answers like the Jellyfin API as the server's own sources define it
//! (github.com/jellyfin/jellyfin at tag v12.4, 2026-10-26: the controllers in
//! `Jellyfin.Api/Controllers`, the DTOs in `MediaBrowser.Model`, the
//! `MediaBrowser` header parser in `AuthorizationContext.GetParts`, Quick
//! Connect in `QuickConnectManager`, error statuses in `ExceptionMiddleware`;
//! api.jellyfin.org could not be reached from here). PascalCase JSON,
//! `QueryResult` pages, `userId` taken from the token, enum query values
//! checked (an unknown one is a 400, as model binding makes it). The library
//! (`fixtures/library.json`) is invented; nothing here comes from a real
//! server or account.

#![allow(dead_code, reason = "each test uses a different part of the mock")]

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use jf_plugin_testkit::{Request, Response, TestServer};
use serde_json::{Map, Value, json};

/// Accounts: (name, password, user id, may play media).
pub const USERS: &[(&str, &str, &str, bool)] = &[
    (
        "alice",
        "correct horse battery staple",
        "e6000000000000000000000000000001",
        true,
    ),
    // No password, and no permission to play (PlayAccess None).
    ("bob", "", "e6000000000000000000000000000002", false),
];
/// The server's id.
pub const SERVER_ID: &str = "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f";
/// What the mock serves for an image.
pub const PNG: &[u8] = b"\x89PNG\r\n\x1a\nmock image";

/// The `ItemFields` values the server knows (12.4).
const ITEM_FIELDS: &[&str] = &[
    "AirTime",
    "CanDelete",
    "CanDownload",
    "ChannelInfo",
    "Chapters",
    "Trickplay",
    "ChildCount",
    "CumulativeRunTimeTicks",
    "CustomRating",
    "DateCreated",
    "DateLastMediaAdded",
    "DisplayPreferencesId",
    "Etag",
    "ExternalUrls",
    "Genres",
    "ItemCounts",
    "MediaSourceCount",
    "MediaSources",
    "OriginalTitle",
    "Overview",
    "ParentId",
    "Path",
    "People",
    "PlayAccess",
    "ProductionLocations",
    "ProviderIds",
    "PrimaryImageAspectRatio",
    "RecursiveItemCount",
    "Settings",
    "SeriesStudio",
    "SortName",
    "SpecialEpisodeNumbers",
    "Studios",
    "Taglines",
    "Tags",
    "RemoteTrailers",
    "MediaStreams",
    "SeasonUserData",
    "DateLastRefreshed",
    "DateLastSaved",
    "RefreshState",
    "ChannelImage",
    "EnableMediaSourceDisplay",
    "Width",
    "Height",
    "ExtraIds",
    "LocalTrailerCount",
    "IsHD",
    "SpecialFeatureCount",
];
/// `ItemSortBy` values the mock sorts by (all exist in 12.4).
const SORT_BY: &[&str] = &[
    "SortName",
    "Name",
    "DateCreated",
    "ProductionYear",
    "ParentIndexNumber",
    "IndexNumber",
    "Album",
    "AlbumArtist",
    "Artist",
    "Random",
];
/// `BaseItemKind` values the mock knows.
const ITEM_KINDS: &[&str] = &[
    "Audio",
    "MusicAlbum",
    "MusicArtist",
    "Playlist",
    "Folder",
    "CollectionFolder",
    "Movie",
    "Episode",
];
/// `ImageType` values.
const IMAGE_TYPES: &[&str] = &[
    "Primary", "Art", "Backdrop", "Banner", "Logo", "Thumb", "Disc", "Box",
];

/// What kind of server the mock pretends to be.
#[derive(Debug, Clone)]
pub struct Mock {
    /// `Version` of `/System/Info/Public`.
    pub version: &'static str,
    /// `ProductName`.
    pub product: &'static str,
    /// Quick Connect turned on.
    pub quick_connect: bool,
    /// A second music library is visible.
    pub two_libraries: bool,
}

impl Default for Mock {
    fn default() -> Mock {
        Mock {
            version: "12.4.0",
            product: "Jellyfin Server",
            quick_connect: true,
            two_libraries: false,
        }
    }
}

/// A session the mock issued.
#[derive(Debug, Clone)]
pub struct Session {
    pub user: String,
    pub device_id: String,
    pub device: String,
    pub client: String,
    pub version: String,
    pub revoked: bool,
}

/// A Quick Connect request.
#[derive(Debug, Clone)]
pub struct QuickRequest {
    pub secret: String,
    pub code: String,
    pub device_id: String,
    pub device: String,
    /// The user who approved it.
    pub approved_by: Option<String>,
}

/// The mock's state, shared with the test.
#[derive(Debug, Default)]
pub struct State {
    /// Token → session.
    pub sessions: BTreeMap<String, Session>,
    pub quick: Vec<QuickRequest>,
    /// Tokens of reported logouts.
    pub logouts: Vec<String>,
    counter: u64,
}

impl State {
    fn next(&mut self) -> u64 {
        self.counter += 1;
        self.counter
    }

    /// The token for `user` on `device_id`: reissued for the same device,
    /// as `SessionManager.GetAuthorizationToken` does.
    fn issue(&mut self, user: &str, parts: &HashMap<String, String>) -> String {
        let device_id = parts.get("DeviceId").cloned().unwrap_or_default();
        if let Some((token, _)) = self
            .sessions
            .iter()
            .find(|(_, s)| s.user == user && s.device_id == device_id && !s.revoked)
        {
            return token.clone();
        }
        let token = format!("{:032x}", 0x7a11_0000_0000u64 + self.next());
        self.sessions.insert(
            token.clone(),
            Session {
                user: user.to_string(),
                device_id,
                device: parts.get("Device").cloned().unwrap_or_default(),
                client: parts.get("Client").cloned().unwrap_or_default(),
                version: parts.get("Version").cloned().unwrap_or_default(),
                revoked: false,
            },
        );
        token
    }
}

/// A running mock: the server and its shared state.
#[derive(Debug)]
pub struct Running {
    pub server: TestServer,
    pub state: Arc<Mutex<State>>,
}

impl std::ops::Deref for Running {
    type Target = TestServer;
    fn deref(&self) -> &TestServer {
        &self.server
    }
}

impl Running {
    /// Approves a Quick Connect code (spaces ignored) as `user`, as the person
    /// would in Jellyfin.
    pub fn approve(&self, code: &str, user: &str) {
        let code: String = code.chars().filter(char::is_ascii_digit).collect();
        let mut state = self.state.lock().unwrap();
        let request = state
            .quick
            .iter_mut()
            .find(|r| r.code == code)
            .unwrap_or_else(|| panic!("no Quick Connect request with code {code}"));
        request.approved_by = Some(user.to_string());
    }

    /// Forgets every Quick Connect request (they expired).
    pub fn expire_codes(&self) {
        self.state.lock().unwrap().quick.clear();
    }

    /// Revokes every token (an administrator signed the device out).
    pub fn revoke_all(&self) {
        for session in self.state.lock().unwrap().sessions.values_mut() {
            session.revoked = true;
        }
    }

    /// The live sessions.
    pub fn sessions(&self) -> Vec<(String, Session)> {
        self.state
            .lock()
            .unwrap()
            .sessions
            .iter()
            .filter(|(_, s)| !s.revoked)
            .map(|(t, s)| (t.clone(), s.clone()))
            .collect()
    }

    /// Tokens whose logout was reported.
    pub fn logouts(&self) -> Vec<String> {
        self.state.lock().unwrap().logouts.clone()
    }
}

impl Mock {
    /// Starts the server.
    pub fn start(self) -> Running {
        let library: Value = serde_json::from_str(include_str!("../fixtures/library.json")).unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        let server = TestServer::start(move |request| self.handle(&library, &shared, request));
        Running { server, state }
    }

    fn handle(&self, library: &Value, state: &Mutex<State>, request: &Request) -> Response {
        let path = request.path().to_string();
        let method = request.method.as_str();
        let parts = request.header("authorization").and_then(auth_parts);
        // Public endpoints.
        match (method, path.as_str()) {
            ("GET", "/System/Info/Public") => {
                return json(json!({
                    "LocalAddress": "http://127.0.0.1:8096",
                    "ServerName": "Mock Jellyfin",
                    "Version": self.version,
                    "ProductName": self.product,
                    "OperatingSystem": "",
                    "Id": SERVER_ID,
                    "StartupWizardCompleted": true
                }));
            }
            ("GET", "/QuickConnect/Enabled") => return json(json!(self.quick_connect)),
            _ => {}
        }
        if let Some(rest) = path.strip_prefix("/Items/")
            && let Some(id) = rest.strip_suffix("/Images/Primary")
            && method == "GET"
        {
            // Images need no sign-in (ImageController has no [Authorize]).
            return match Library(library).find(id) {
                Some(item) if item["ImageTags"]["Primary"].is_string() => {
                    Response::new(200, PNG).header("content-type", "image/png")
                }
                _ => not_found(),
            };
        }
        // Everything else needs a client that names itself.
        let Some(parts) = parts else {
            return Response::text(401, "");
        };
        if ["Client", "Device", "DeviceId", "Version"]
            .iter()
            .any(|key| parts.get(*key).is_none_or(|v| v.is_empty()))
        {
            // QuickConnectManager.TryConnect throws ArgumentException → 400.
            return Response::text(400, "Error processing request.");
        }
        match (method, path.as_str()) {
            ("POST", "/Users/AuthenticateByName") => {
                let body = request.json().unwrap_or(Value::Null);
                let name = body["Username"].as_str().unwrap_or_default();
                let password = body["Pw"].as_str().unwrap_or_default();
                return match USERS.iter().find(|(u, p, _, _)| *u == name && *p == password) {
                    Some((user, ..)) => {
                        let token = state.lock().unwrap().issue(user, &parts);
                        json(auth_result(user, &token, &parts))
                    }
                    None => Response::text(401, "Error processing request."),
                };
            }
            ("POST", "/QuickConnect/Initiate") => {
                if !self.quick_connect {
                    return Response::text(401, "Quick connect is disabled");
                }
                let mut state = state.lock().unwrap();
                let n = state.next();
                let request = QuickRequest {
                    secret: format!("{:064X}", 0x005e_c2e7_0000_u64 + n),
                    code: format!("{}", 482_913 + n * 7),
                    device_id: parts["DeviceId"].clone(),
                    device: parts["Device"].clone(),
                    approved_by: None,
                };
                let answer = quick_result(&request, &parts);
                state.quick.push(request);
                return json(answer);
            }
            ("GET", "/QuickConnect/Connect") => {
                if !self.quick_connect {
                    return Response::text(401, "Quick connect is disabled");
                }
                let secret = request.query("secret").unwrap_or_default();
                let state = state.lock().unwrap();
                return match state.quick.iter().find(|r| r.secret == secret) {
                    Some(found) => json(quick_result(found, &parts)),
                    None => Response::text(404, "Unknown secret"),
                };
            }
            ("POST", "/Users/AuthenticateWithQuickConnect") => {
                let body = request.json().unwrap_or(Value::Null);
                let secret = body["Secret"].as_str().unwrap_or_default().to_string();
                let mut state = state.lock().unwrap();
                let Some(found) = state.quick.iter().find(|r| r.secret == secret).cloned() else {
                    return not_found();
                };
                let Some(user) = found.approved_by.clone() else {
                    // Not in the authorized secrets yet: ResourceNotFoundException.
                    return not_found();
                };
                let mut device = parts.clone();
                device.insert("DeviceId".into(), found.device_id.clone());
                device.insert("Device".into(), found.device.clone());
                let token = state.issue(&user, &device);
                return json(auth_result(&user, &token, &device));
            }
            _ => {}
        }
        // Signed-in endpoints.
        let token = parts.get("Token").cloned().unwrap_or_default();
        let user = {
            let state = state.lock().unwrap();
            match state.sessions.get(&token) {
                Some(session) if !session.revoked && session.device_id == parts["DeviceId"] => {
                    session.user.clone()
                }
                _ => return Response::text(401, ""),
            }
        };
        let may_play = USERS.iter().any(|(u, _, _, play)| *u == user && *play);
        let lib = Library(library);
        let visible = |item: &Value| {
            self.two_libraries || item["_library"].as_str() != Some("f1000000000000000000000000000002")
        };
        let shape = |item: &Value, fields: &[String]| shape(item, fields, may_play);
        match (method, path.as_str()) {
            ("POST", "/Sessions/Logout") => {
                let mut state = state.lock().unwrap();
                if let Some(session) = state.sessions.get_mut(&token) {
                    session.revoked = true;
                }
                state.logouts.push(token);
                Response::no_content()
            }
            ("GET", "/Users/Me") => {
                let (name, _, id, _) = USERS.iter().find(|(u, ..)| *u == user).unwrap();
                json(user_dto(name, id))
            }
            ("GET", "/UserViews") => {
                let views: Vec<Value> = lib
                    .list("views")
                    .iter()
                    .filter(|v| self.two_libraries || v["_second"].is_null())
                    .map(public)
                    .collect();
                json(query_result(views, 0))
            }
            ("GET", "/Items") => match self.items(&lib, request, &visible) {
                Ok((list, start, fields)) => {
                    let total = list.len();
                    let page = page(list, request);
                    let shaped: Vec<Value> = page.iter().map(|i| shape(i, &fields)).collect();
                    json(query_result_total(shaped, start, total))
                }
                Err(response) => response,
            },
            ("GET", "/Artists" | "/Artists/AlbumArtists") => {
                if let Err(response) = check_enums(request) {
                    return response;
                }
                let fields = list_param(request, "fields");
                let mut artists: Vec<Value> = lib.list("artists").to_vec();
                let parent = request.query("parentId");
                artists.retain(|artist| {
                    // An artist is in a library through its albums there.
                    lib.list("albums").iter().any(|album| {
                        visible(album)
                            && parent.as_deref().is_none_or(|p| album["_library"] == p)
                            && album["AlbumArtists"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|a| a["Id"] == artist["Id"])
                    }) || (path == "/Artists"
                        && parent.is_none()
                        && lib.songs().iter().filter(|s| visible(s)).any(|s| {
                            s["ArtistItems"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|a| a["Id"] == artist["Id"])
                        }))
                });
                if let Some(term) = request.query("searchTerm") {
                    artists.retain(|a| contains(&a["Name"], &term));
                }
                if request.query("isFavorite").as_deref() == Some("true") {
                    let favourites = &library["favourites"]["artists"];
                    artists.retain(|a| favourites.as_array().unwrap().contains(&a["Id"]));
                }
                sort(&mut artists, request);
                let start = number(request.query("startIndex"), 0);
                let total = artists.len();
                let page = page(artists, request);
                json(query_result_total(
                    page.iter().map(|a| shape(a, &fields)).collect(),
                    start,
                    total,
                ))
            }
            _ => self.by_id(&lib, request, &path, &user, may_play, &visible),
        }
    }

    /// `/Items`: the filters the server applies, in `ItemsController.GetItems`.
    #[allow(clippy::type_complexity)]
    fn items(
        &self,
        lib: &Library<'_>,
        request: &Request,
        visible: &dyn Fn(&Value) -> bool,
    ) -> Result<(Vec<Value>, usize, Vec<String>), Response> {
        check_enums(request)?;
        let fields = list_param(request, "fields");
        let kinds = list_param(request, "includeItemTypes");
        let mut all: Vec<Value> = Vec::new();
        for kind in ["albums", "songs", "playlists"] {
            all.extend(lib.list(kind).iter().filter(|i| visible(i)).cloned());
        }
        if kinds.is_empty() && request.query("ids").is_none() && request.query("parentId").is_none() {
            // The mock only answers the queries a music client makes.
            return Err(Response::text(
                400,
                "the mock needs includeItemTypes, ids or parentId",
            ));
        }
        if !kinds.is_empty() {
            all.retain(|i| kinds.iter().any(|k| i["Type"] == k.as_str()));
        }
        let ids = list_param(request, "ids");
        if !ids.is_empty() {
            all.retain(|i| {
                ids.iter()
                    .any(|id| i["Id"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(id)))
            });
        }
        if let Some(parent) = request.query("parentId") {
            let recursive = request.query("recursive").as_deref() == Some("true") || !kinds.is_empty();
            match lib.find(&parent) {
                Some(album) if album["Type"] == "MusicAlbum" => {
                    all.retain(|i| i["AlbumId"] == album["Id"]);
                }
                Some(view) if view["CollectionType"] == "music" && recursive => {
                    all.retain(|i| i["_library"] == view["Id"]);
                }
                _ => return Err(not_found()),
            }
        }
        if let Some(term) = request.query("searchTerm") {
            all.retain(|i| contains(&i["Name"], &term));
        }
        if request.query("isFavorite").as_deref() == Some("true") {
            let favourites = &lib.0["favourites"];
            all.retain(|i| {
                let list = match i["Type"].as_str() {
                    Some("Audio") => &favourites["songs"],
                    Some("MusicAlbum") => &favourites["albums"],
                    _ => return false,
                };
                list.as_array().unwrap().contains(&i["Id"])
            });
        }
        let artists = list_param(request, "artistIds");
        if !artists.is_empty() {
            all.retain(|i| {
                let credited = |key: &str| {
                    i[key].as_array().is_some_and(|list| {
                        list.iter()
                            .any(|a| artists.iter().any(|id| a["Id"] == id.as_str()))
                    })
                };
                credited("AlbumArtists") || credited("ArtistItems")
            });
        }
        let media = list_param(request, "mediaTypes");
        if !media.is_empty() {
            all.retain(|i| media.iter().any(|m| i["MediaType"] == m.as_str()));
        }
        sort(&mut all, request);
        Ok((all, number(request.query("startIndex"), 0), fields))
    }

    fn by_id(
        &self,
        lib: &Library<'_>,
        request: &Request,
        path: &str,
        user: &str,
        may_play: bool,
        visible: &dyn Fn(&Value) -> bool,
    ) -> Response {
        let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        let fields = list_param(request, "fields");
        match segments.as_slice() {
            ["Playlists", id, "Items"] => match lib.find(id) {
                Some(playlist) if playlist["Type"] == "Playlist" => {
                    let entries: Vec<Value> = playlist["_entries"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|id| lib.find(id.as_str().unwrap()).cloned())
                        .collect();
                    let start = number(request.query("startIndex"), 0);
                    let total = entries.len();
                    let page = page(entries, request);
                    json(query_result_total(
                        page.iter().map(|i| shape(i, &fields, may_play)).collect(),
                        start,
                        total,
                    ))
                }
                _ => not_found(),
            },
            ["Items", id, "InstantMix"] => match lib.find(id) {
                Some(_) => {
                    let limit = number(request.query("limit"), 200);
                    let mix: Vec<Value> = lib.0["instantMix"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|id| lib.find(id.as_str().unwrap()).cloned())
                        .take(limit)
                        .map(|i| shape(&i, &fields, may_play))
                        .collect();
                    json(query_result(mix, 0))
                }
                None => not_found(),
            },
            ["Audio", id, "Lyrics"] => match (lib.find(id), lib.0["lyrics"].get(*id)) {
                (Some(_), Some(lyrics)) => json(lyrics.clone()),
                _ => not_found(),
            },
            ["Audio", id, "universal"] => {
                let Some(song) = lib.find(id).filter(|s| s["Type"] == "Audio" && visible(s)) else {
                    return not_found();
                };
                if !may_play {
                    return Response::text(403, "Error processing request.");
                }
                let Some(source) = song["MediaSources"].as_array().and_then(|s| s.first()) else {
                    return not_found();
                };
                let _ = user;
                universal(source, request)
            }
            _ => Response::text(404, ""),
        }
    }
}

/// `/Audio/{id}/universal`: direct play when a `container` entry matches the
/// file and its bit rate fits `maxStreamingBitrate`, else a progressive
/// conversion (`UniversalAudioController` with a `http` transcoding profile).
fn universal(source: &Value, request: &Request) -> Response {
    let container = source["Container"].as_str().unwrap_or_default();
    let codec = source["MediaStreams"][0]["Codec"].as_str().unwrap_or_default();
    let bitrate = source["Bitrate"].as_u64().unwrap_or(0);
    let max = request
        .query("maxStreamingBitrate")
        .and_then(|m| m.parse::<u64>().ok())
        .unwrap_or(8_000_000);
    let direct = request
        .query("container")
        .unwrap_or_default()
        .split(',')
        .any(|entry| {
            let mut parts = entry.split('|');
            let name = parts.next().unwrap_or_default();
            let codecs: Vec<&str> = parts.collect();
            name.eq_ignore_ascii_case(container) && (codecs.is_empty() || codecs.contains(&codec))
        })
        && bitrate <= max;
    if direct {
        return Response::new(200, format!("original {container}"))
            .header("content-type", &format!("audio/{container}"))
            .header("accept-ranges", "bytes");
    }
    if request.query("transcodingProtocol").as_deref() == Some("hls") {
        return Response::text(200, "#EXTM3U").header("content-type", "application/x-mpegURL");
    }
    let target = request
        .query("transcodingContainer")
        .unwrap_or_else(|| "mp3".into());
    let codec = request.query("audioCodec").unwrap_or_else(|| "mp3".into());
    let rate = request.query("audioBitRate").unwrap_or_else(|| max.to_string());
    Response::new(200, format!("converted {codec} {rate}"))
        .header("content-type", &format!("audio/{target}"))
        .header("accept-ranges", "none")
}

/// The library fixture.
pub struct Library<'a>(pub &'a Value);

impl Library<'_> {
    fn list(&self, kind: &str) -> &Vec<Value> {
        self.0[kind].as_array().unwrap()
    }

    fn songs(&self) -> Vec<Value> {
        self.list("songs").clone()
    }

    fn find(&self, id: &str) -> Option<&Value> {
        ["views", "artists", "albums", "songs", "playlists"]
            .into_iter()
            .flat_map(|kind| self.list(kind).iter())
            .find(|i| i["Id"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(id)))
    }
}

/// An item as the server sends it: the mock's `_` keys removed, and the
/// fields that only come when asked for (`fields=`) only then.
fn shape(item: &Value, fields: &[String], may_play: bool) -> Value {
    let mut out = public(item);
    let object = out.as_object_mut().unwrap();
    let asked = |name: &str| fields.iter().any(|f| f == name);
    if !asked("MediaSources") {
        object.remove("MediaSources");
    }
    if !asked("ChildCount") {
        object.remove("ChildCount");
    }
    if asked("PlayAccess") {
        object.insert("PlayAccess".into(), if may_play { "Full" } else { "None" }.into());
    }
    if !asked("SortName") {
        object.remove("SortName");
    }
    if !asked("DateCreated") {
        object.remove("DateCreated");
    }
    object.insert("ServerId".into(), SERVER_ID.into());
    out
}

fn public(item: &Value) -> Value {
    let mut out = Map::new();
    for (key, value) in item.as_object().unwrap() {
        if !key.starts_with('_') {
            out.insert(key.clone(), value.clone());
        }
    }
    Value::Object(out)
}

fn query_result(items: Vec<Value>, start: usize) -> Value {
    let total = items.len();
    query_result_total(items, start, total)
}

fn query_result_total(items: Vec<Value>, start: usize, total: usize) -> Value {
    json!({ "Items": items, "TotalRecordCount": total, "StartIndex": start })
}

fn json(value: Value) -> Response {
    Response::json(&value)
}

fn not_found() -> Response {
    Response::text(404, "Error processing request.")
}

fn contains(text: &Value, term: &str) -> bool {
    text.as_str()
        .unwrap_or_default()
        .to_lowercase()
        .contains(&term.to_lowercase())
}

fn number(value: Option<String>, default: usize) -> usize {
    value.and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// A comma-separated query parameter.
fn list_param(request: &Request, key: &str) -> Vec<String> {
    request
        .query(key)
        .map(|v| {
            v.split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Enum-typed query values must be names the server knows; model binding
/// turns anything else into a 400.
fn check_enums(request: &Request) -> Result<(), Response> {
    let checks: [(&str, &[&str]); 5] = [
        ("fields", ITEM_FIELDS),
        ("sortBy", SORT_BY),
        ("includeItemTypes", ITEM_KINDS),
        ("enableImageTypes", IMAGE_TYPES),
        ("sortOrder", &["Ascending", "Descending"]),
    ];
    for (key, allowed) in checks {
        for value in list_param(request, key) {
            if !allowed.contains(&value.as_str()) {
                return Err(Response::text(
                    400,
                    &format!("The value '{value}' is not valid for {key}."),
                ));
            }
        }
    }
    for key in ["startIndex", "limit"] {
        if let Some(value) = request.query(key)
            && value.parse::<u32>().is_err()
        {
            return Err(Response::text(
                400,
                &format!("The value '{value}' is not valid for {key}."),
            ));
        }
    }
    Ok(())
}

/// Sorts by `sortBy` and `sortOrder` (one order per key, the first one for
/// keys without their own).
fn sort(items: &mut [Value], request: &Request) {
    let keys = list_param(request, "sortBy");
    let orders = list_param(request, "sortOrder");
    if keys.is_empty() {
        return;
    }
    let value = |item: &Value, key: &str| -> (i64, String) {
        match key {
            "SortName" | "Name" => (
                0,
                item["SortName"]
                    .as_str()
                    .or_else(|| item["Name"].as_str())
                    .unwrap_or_default()
                    .to_lowercase(),
            ),
            "DateCreated" => (0, item["DateCreated"].as_str().unwrap_or_default().to_string()),
            other => (item[other].as_i64().unwrap_or(0), String::new()),
        }
    };
    items.sort_by(|a, b| {
        for (i, key) in keys.iter().enumerate() {
            let order = orders
                .get(i)
                .or(orders.first())
                .map(String::as_str)
                .unwrap_or("Ascending");
            let mut ordering = value(a, key).cmp(&value(b, key));
            if order == "Descending" {
                ordering = ordering.reverse();
            }
            if ordering.is_ne() {
                return ordering;
            }
        }
        std::cmp::Ordering::Equal
    });
}

/// The first `limit` items from `startIndex`.
fn page(items: Vec<Value>, request: &Request) -> Vec<Value> {
    let start = number(request.query("startIndex"), 0);
    let limit = number(request.query("limit"), usize::MAX);
    items.into_iter().skip(start).take(limit).collect()
}

fn user_dto(name: &str, id: &str) -> Value {
    let may_play = USERS.iter().any(|(u, _, _, play)| *u == name && *play);
    json!({
        "Name": name,
        "ServerId": SERVER_ID,
        "Id": id,
        "HasPassword": true,
        "HasConfiguredPassword": true,
        "EnableAutoLogin": false,
        "Policy": {
            "IsAdministrator": false,
            "IsDisabled": false,
            "EnableMediaPlayback": may_play,
            "EnableAudioPlaybackTranscoding": true,
            "EnableRemoteAccess": true
        }
    })
}

fn auth_result(user: &str, token: &str, parts: &HashMap<String, String>) -> Value {
    let (name, _, id, _) = USERS.iter().find(|(u, ..)| *u == user).unwrap();
    json!({
        "User": user_dto(name, id),
        "SessionInfo": {
            "Id": format!("s{token}"),
            "UserId": id,
            "UserName": name,
            "Client": parts.get("Client"),
            "DeviceName": parts.get("Device"),
            "DeviceId": parts.get("DeviceId"),
            "ApplicationVersion": parts.get("Version"),
            "IsActive": true
        },
        "AccessToken": token,
        "ServerId": SERVER_ID
    })
}

fn quick_result(request: &QuickRequest, parts: &HashMap<String, String>) -> Value {
    json!({
        "Authenticated": request.approved_by.is_some(),
        "Secret": request.secret,
        "Code": request.code,
        "DeviceId": request.device_id,
        "DeviceName": request.device,
        "AppName": parts.get("Client"),
        "AppVersion": parts.get("Version"),
        "DateAdded": "2026-11-09T12:00:00.0000000Z"
    })
}

/// `Authorization: MediaBrowser k="v", …` parsed like
/// `AuthorizationContext.GetParts`: commas outside quotes separate pairs,
/// values are trimmed of quotes and URL-decoded.
pub fn auth_parts(header: &str) -> Option<HashMap<String, String>> {
    let (scheme, rest) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("MediaBrowser") {
        return None;
    }
    let chars: Vec<char> = rest.chars().collect();
    let text = |from: usize, to: usize| chars[from..to].iter().collect::<String>();
    let mut result = HashMap::new();
    let mut escaped = false;
    let mut start = 0;
    let mut key = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '"' || c == ',' {
            escaped = (!escaped) == (c == '"');
            if c == ',' && !escaped {
                if start < i {
                    result.insert(
                        std::mem::take(&mut key),
                        url_decode(text(start, i).trim_matches('"')),
                    );
                }
                start = i + 1;
            }
        } else if !escaped && c == '=' {
            key = text(start, i).trim().to_string();
            start = i + 1;
        }
    }
    if start < chars.len() {
        result.insert(key, url_decode(text(start, chars.len()).trim_matches('"')));
    }
    Some(result)
}

/// `WebUtility.UrlDecode`: `+` is a space, `%xx` a byte.
fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(byte) => {
                        out.push(byte);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

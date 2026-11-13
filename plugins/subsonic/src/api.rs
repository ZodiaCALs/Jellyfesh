//! Talking to the server: what it supports, the stored sign-in, requests
//! and their errors, and the plugin's in-memory caches.
//!
//! Every request carries `v`, `c` and `f=json` and, unless it is the public
//! `getOpenSubsonicExtensions`, the sign-in: `apiKey=…` or `u`, `t` and `s`
//! (a salt and `md5(password + salt)` made once at sign-in). Arguments go in
//! a form POST when the server offers the `formPost` extension, so they stay
//! out of web server logs; otherwise in the query string.

use std::cell::RefCell;
use std::collections::HashMap;

use jf_plugin_sdk::bindings::jellyfesh::plugin::kv as wit_kv;
use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Extension, TokenInfo, User};
use crate::settings::Settings;

/// The client name sent as `c`.
pub const CLIENT: &str = "Jellyfesh";
/// The protocol version sent as `v`: the oldest that has token sign-in
/// (1.13.0); every call the plugin makes exists since then or earlier, and
/// newer servers accept older clients.
pub const API_VERSION: &str = "1.13.0";
/// The public call that says what the server supports.
const DISCOVERY: &str = "getOpenSubsonicExtensions";
/// The keystore entry with the sign-in.
const SECRET: &str = "login";
/// kv: what the server supports, for `meta.capabilities`.
const INFO_KEY: &str = "server-info";
/// kv: whether the account may stream.
const ROLES_KEY: &str = "roles";
/// How long the account's roles are trusted before asking again.
const ROLES_TTL_MS: u64 = 24 * 60 * 60 * 1000;
/// How long a fetched list (artists, an album, a playlist) is reused while
/// the person pages through it.
const LIST_TTL_MS: u64 = 2 * 60 * 1000;
/// Most cover ids remembered.
const MAX_COVERS: usize = 50_000;

/// A sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Creds {
    /// OpenSubsonic API key.
    ApiKey(String),
    /// User name, salt and `md5(password + salt)`.
    Token {
        user: String,
        salt: String,
        token: String,
    },
}

impl Creds {
    fn params(&self) -> Vec<(&'static str, String)> {
        match self {
            Creds::ApiKey(key) => vec![("apiKey", key.clone())],
            Creds::Token { user, salt, token } => {
                vec![("u", user.clone()), ("t", token.clone()), ("s", salt.clone())]
            }
        }
    }
}

/// What is stored in the keystore: the sign-in and the server it is for.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Stored {
    ApiKey {
        server: String,
        key: String,
    },
    Token {
        server: String,
        user: String,
        salt: String,
        token: String,
    },
}

/// What the server says about itself.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ServerInfo {
    /// Subsonic API version, like `1.16.1`.
    pub api_version: Option<String>,
    /// `Navidrome`, `gonic`, … (OpenSubsonic servers only).
    pub server_type: Option<String>,
    pub server_version: Option<String>,
    pub open_subsonic: bool,
    pub extensions: Vec<Extension>,
}

impl ServerInfo {
    /// Whether the server offers an OpenSubsonic extension.
    pub fn has(&self, name: &str) -> bool {
        self.extensions.iter().any(|e| e.name == name)
    }

    /// Whether the API version is at least `major.minor`; none if unknown.
    pub fn api_at_least(&self, major: u32, minor: u32) -> Option<bool> {
        let version = self.api_version.as_deref()?;
        let mut parts = version.split('.').map(|p| p.trim().parse::<u32>().ok());
        let found_major = parts.next().flatten()?;
        let found_minor = parts.next().flatten().unwrap_or(0);
        Some((found_major, found_minor) >= (major, minor))
    }

    /// `Navidrome 0.64.2`, for messages.
    pub fn describe(&self) -> String {
        match (&self.server_type, &self.server_version) {
            (Some(kind), Some(version)) => format!("{kind} {version}"),
            (Some(kind), None) => kind.clone(),
            _ => "The server".into(),
        }
    }

    fn from_envelope(envelope: &Value) -> ServerInfo {
        let text = |key: &str| envelope.get(key).and_then(Value::as_str).map(str::to_string);
        let extensions = envelope
            .get("openSubsonicExtensions")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|e| serde_json::from_value::<Extension>(e.clone()).ok())
                    .filter(|e| !e.name.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        ServerInfo {
            api_version: text("version"),
            server_type: text("type"),
            server_version: text("serverVersion"),
            open_subsonic: envelope
                .get("openSubsonic")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            extensions,
        }
    }
}

/// A Subsonic error answer.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: i64,
    pub message: String,
    pub help_url: Option<String>,
    /// The whole `subsonic-response`, which still names the API version.
    pub envelope: Value,
}

/// Why a request failed: no answer (or not a Subsonic one), or an error
/// answer from the server.
#[derive(Debug, Clone)]
pub enum Failure {
    Plugin(PluginError),
    Api(ApiError),
}

impl From<PluginError> for Failure {
    fn from(error: PluginError) -> Failure {
        Failure::Plugin(error)
    }
}

impl From<Failure> for PluginError {
    fn from(failure: Failure) -> PluginError {
        match failure {
            Failure::Plugin(error) => error,
            Failure::Api(error) => error.into(),
        }
    }
}

impl From<ApiError> for PluginError {
    fn from(error: ApiError) -> PluginError {
        let said = |fallback: &str| {
            if error.message.trim().is_empty() {
                fallback.to_string()
            } else {
                error.message.trim().to_string()
            }
        };
        match error.code {
            40 | 44 => PluginError::AuthRequired,
            41 | 42 => {
                jf_plugin_sdk::warn!(
                    "the server no longer accepts this kind of sign-in (code {}): {}",
                    error.code,
                    error.message
                );
                PluginError::AuthRequired
            }
            43 => PluginError::internal("conflicting authentication parameters"),
            10 => PluginError::internal(format!(
                "the server says a parameter is missing: {}",
                error.message
            )),
            20 => PluginError::service(format!(
                "The server needs a newer Subsonic client than Jellyfesh: {}",
                said("incompatible protocol version")
            )),
            30 => PluginError::service(format!(
                "The server speaks an older Subsonic API than Jellyfesh needs ({API_VERSION}); update the server."
            )),
            50 => PluginError::service("Your account on this server is not allowed to do that."),
            70 => PluginError::NotFound,
            _ => PluginError::service(said(&format!("The server reported error {}", error.code))),
        }
    }
}

#[derive(Debug, Default)]
struct State {
    settings: Settings,
    info: Option<ServerInfo>,
    /// The sign-in read from the keystore: outer none means not read yet.
    creds: Option<Option<Creds>>,
    /// Item id → cover art id, from listings, so `artwork` needs no request.
    covers: HashMap<String, String>,
    lists: Vec<(String, u64, Vec<Item>)>,
    can_stream: Option<bool>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The current settings.
pub fn settings() -> Settings {
    STATE.with(|s| s.borrow().settings.clone())
}

/// New settings; a different server forgets everything learnt about the old one.
pub fn set_settings(settings: Settings) {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        if state.settings.base != settings.base {
            *state = State::default();
        }
        state.settings = settings;
    });
}

/// Forgets the sign-in and everything cached (logout).
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

/// The server's base address.
pub fn base() -> Result<String> {
    settings().base.ok_or_else(|| {
        PluginError::invalid_input("Enter your server's address in this source's settings first.")
    })
}

/// Which server a stored sign-in belongs to: a short hash of the address,
/// so the sign-in is never sent to another server after the address changes.
fn binding(base: &str) -> Result<String> {
    let digest = crypto::sha256(base.to_ascii_lowercase().as_bytes())?;
    Ok(crypto::hex(&digest[..8]))
}

/// The stored sign-in for the configured server, if any.
pub fn creds() -> Result<Option<Creds>> {
    if let Some(cached) = STATE.with(|s| s.borrow().creds.clone()) {
        return Ok(cached);
    }
    let server = binding(&base()?)?;
    let found = match kv::secret::get(SECRET)? {
        None => None,
        Some(text) => match serde_json::from_str::<Stored>(&text) {
            Ok(Stored::ApiKey { server: s, key }) if s == server => Some(Creds::ApiKey(key)),
            Ok(Stored::Token {
                server: s,
                user,
                salt,
                token,
            }) if s == server => Some(Creds::Token { user, salt, token }),
            Ok(_) => {
                jf_plugin_sdk::warn!("the stored sign-in is for another server address; sign in again");
                None
            }
            Err(_) => None,
        },
    };
    STATE.with(|s| s.borrow_mut().creds = Some(found.clone()));
    Ok(found)
}

/// Stores a sign-in in the keystore. The raw error lets the login tell a
/// refused value (it contained the password) from other failures.
pub fn save_creds(creds: &Creds) -> core::result::Result<(), wit_kv::KvError> {
    let server = binding(&base().map_err(|e| wit_kv::KvError::Unavailable(format!("{e:?}")))?)
        .map_err(|e| wit_kv::KvError::Unavailable(format!("{e:?}")))?;
    let stored = match creds.clone() {
        Creds::ApiKey(key) => Stored::ApiKey { server, key },
        Creds::Token { user, salt, token } => Stored::Token {
            server,
            user,
            salt,
            token,
        },
    };
    let text = serde_json::to_string(&stored).map_err(|e| wit_kv::KvError::Unavailable(e.to_string()))?;
    wit_kv::set_secret(SECRET, &text)?;
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.creds = Some(Some(creds.clone()));
        state.can_stream = None;
    });
    let _ = kv::delete(ROLES_KEY);
    Ok(())
}

/// How a request signs in.
#[derive(Debug, Clone, Copy)]
pub enum Auth<'a> {
    /// Not at all (`getOpenSubsonicExtensions` is public).
    None,
    /// With the stored sign-in; `auth-required` without one.
    Session,
    /// With these credentials (checking them at sign-in).
    With(&'a Creds),
}

/// Calls `endpoint` and returns its `subsonic-response` object.
pub fn call(endpoint: &str, params: &[(&str, &str)]) -> Result<Value> {
    Ok(request(endpoint, params, Auth::Session)?)
}

/// Calls `endpoint` and parses the field `key` of its answer.
pub fn call_field<T: serde::de::DeserializeOwned + Default>(
    endpoint: &str,
    params: &[(&str, &str)],
    key: &str,
) -> Result<T> {
    let mut envelope = call(endpoint, params)?;
    match envelope.get_mut(key).map(Value::take) {
        None | Some(Value::Null) => Ok(T::default()),
        Some(value) => Ok(serde_json::from_value(value)?),
    }
}

/// Calls `endpoint`, keeping Subsonic errors apart for callers that act on
/// their codes.
pub fn request(
    endpoint: &str,
    params: &[(&str, &str)],
    auth: Auth<'_>,
) -> core::result::Result<Value, Failure> {
    // Form POST when the server offers it (finding out may ask the server).
    let post = info().is_ok_and(|i| i.has("formPost"));
    send(endpoint, params, auth, post)
}

/// Sends one request, with GET or form POST as told. Discovery uses it
/// directly, so finding out what the server supports never recurses.
fn send(
    endpoint: &str,
    params: &[(&str, &str)],
    auth: Auth<'_>,
    post: bool,
) -> core::result::Result<Value, Failure> {
    let base = base()?;
    let creds = match auth {
        Auth::None => None,
        Auth::With(creds) => Some(creds.clone()),
        Auth::Session => Some(creds()?.ok_or(PluginError::AuthRequired)?),
    };
    let mut owned: Vec<(&str, String)> = vec![
        ("v", API_VERSION.into()),
        ("c", CLIENT.into()),
        ("f", "json".into()),
    ];
    if let Some(creds) = &creds {
        owned.extend(creds.params());
    }
    let mut pairs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
    pairs.extend_from_slice(params);
    let url = format!("{base}/rest/{endpoint}.view");
    let request = if post {
        http::Request::post(url).form(&pairs)
    } else {
        http::Request::get(url).query(&pairs)
    };
    let response = request
        .header("accept", "application/json")
        .send()
        .map_err(PluginError::from)?;
    parse_envelope(&response)
}

/// A URL for the engine or the host to fetch (`stream`, `getCoverArt`). It
/// has to carry the sign-in in its query: Subsonic has no other way.
pub fn media_url(endpoint: &str, params: &[(&str, &str)]) -> Result<String> {
    let base = base()?;
    let creds = creds()?.ok_or(PluginError::AuthRequired)?;
    let mut owned: Vec<(&str, String)> = params.iter().map(|(k, v)| (*k, v.to_string())).collect();
    owned.push(("v", API_VERSION.into()));
    owned.push(("c", CLIENT.into()));
    owned.push(("f", "json".into()));
    owned.extend(creds.params());
    let pairs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
    Ok(format!(
        "{base}/rest/{endpoint}.view?{}",
        http::form_encode(&pairs)
    ))
}

fn parse_envelope(response: &http::Response) -> core::result::Result<Value, Failure> {
    let envelope = serde_json::from_slice::<Value>(&response.body)
        .ok()
        .and_then(|mut v| v.get_mut("subsonic-response").map(Value::take));
    let Some(envelope) = envelope else {
        return Err(Failure::Plugin(not_subsonic(response.status)));
    };
    if envelope.get("status").and_then(Value::as_str) == Some("ok") {
        return Ok(envelope);
    }
    let error = envelope.get("error").cloned().unwrap_or(Value::Null);
    Err(Failure::Api(ApiError {
        code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
        message: error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        help_url: error.get("helpUrl").and_then(Value::as_str).map(str::to_string),
        envelope,
    }))
}

/// An answer that is not Subsonic JSON.
fn not_subsonic(status: u16) -> PluginError {
    PluginError::service(match status {
        401 | 403 => format!(
            "The address answered HTTP {status} before reaching a Subsonic server; \
             a proxy in front of it may want its own sign-in."
        ),
        404 => "No Subsonic server answers at this address (HTTP 404). Check the server address: \
                use the one you open the server at, without a page path."
            .to_string(),
        500..=599 => format!("The server had an internal error (HTTP {status}). Try again later."),
        _ => format!(
            "The address did not answer like a Subsonic server (HTTP {status}). Check the server address."
        ),
    })
}

/// What the server supports, asked once per instance (and after a change of
/// server) and kept in kv for `meta.capabilities`.
pub fn info() -> Result<ServerInfo> {
    if let Some(info) = known_info() {
        return Ok(info);
    }
    let info = discover()?;
    STATE.with(|s| s.borrow_mut().info = Some(info.clone()));
    if let Ok(server) = binding(&base()?) {
        let _ = kv::set_json(INFO_KEY, &serde_json::json!({ "server": server, "info": info }));
    }
    Ok(info)
}

fn known_info() -> Option<ServerInfo> {
    STATE.with(|s| s.borrow().info.clone())
}

/// `getOpenSubsonicExtensions`: public on OpenSubsonic servers, an error
/// (that still names the API version) on older ones. A server that answers
/// an unknown method with something other than a Subsonic answer (an HTTP
/// 404 page, say) is asked `ping` instead, which every Subsonic server
/// answers, with an error without a sign-in that still names its version.
pub fn discover() -> Result<ServerInfo> {
    match send(DISCOVERY, &[], Auth::None, false) {
        Ok(envelope) => Ok(ServerInfo::from_envelope(&envelope)),
        Err(Failure::Api(error)) => {
            // Some servers want a sign-in even for this call.
            if matches!(error.code, 10 | 40)
                && let Ok(Some(creds)) = creds()
                && let Ok(envelope) = send(DISCOVERY, &[], Auth::With(&creds), false)
            {
                return Ok(ServerInfo::from_envelope(&envelope));
            }
            Ok(without_extensions(&error.envelope))
        }
        Err(Failure::Plugin(PluginError::Service(not_subsonic))) => {
            match send("ping", &[], Auth::None, false) {
                Ok(envelope) => Ok(without_extensions(&envelope)),
                Err(Failure::Api(error)) => Ok(without_extensions(&error.envelope)),
                Err(Failure::Plugin(_)) => Err(PluginError::Service(not_subsonic)),
            }
        }
        Err(Failure::Plugin(error)) => Err(error),
    }
}

/// What an answer that is not to `getOpenSubsonicExtensions` says about the
/// server: its API version, and no extensions.
fn without_extensions(envelope: &Value) -> ServerInfo {
    let mut info = ServerInfo::from_envelope(envelope);
    info.extensions.clear();
    info
}

/// What the server supported when last asked, without a request (for
/// `meta.capabilities`, which must be quick).
pub fn remembered_info() -> Option<ServerInfo> {
    if let Some(info) = known_info() {
        return Some(info);
    }
    let server = binding(&settings().base?).ok()?;
    let stored: Value = kv::get_json(INFO_KEY).ok()??;
    if stored.get("server").and_then(Value::as_str) != Some(server.as_str()) {
        return None;
    }
    serde_json::from_value(stored.get("info")?.clone()).ok()
}

/// Whether the signed-in account may stream (`getUser`'s `streamRole`).
/// Asked once a day; when the server cannot say, playing is allowed and
/// the server decides.
pub fn can_stream() -> bool {
    if let Some(known) = STATE.with(|s| s.borrow().can_stream) {
        return known;
    }
    let allowed = stream_role().unwrap_or(true);
    STATE.with(|s| s.borrow_mut().can_stream = Some(allowed));
    allowed
}

fn stream_role() -> Option<bool> {
    let server = binding(&base().ok()?).ok()?;
    let now = clock::now_utc_ms();
    if let Ok(Some(stored)) = kv::get_json::<Value>(ROLES_KEY)
        && stored.get("server").and_then(Value::as_str) == Some(server.as_str())
        && stored
            .get("at")
            .and_then(Value::as_u64)
            .is_some_and(|at| now.saturating_sub(at) < ROLES_TTL_MS)
    {
        return stored.get("stream").and_then(Value::as_bool);
    }
    let user = match creds().ok()?? {
        Creds::Token { user, .. } => user,
        Creds::ApiKey(_) => {
            call_field::<TokenInfo>("tokenInfo", &[], "tokenInfo")
                .ok()?
                .username
        }
    };
    if user.is_empty() {
        return None;
    }
    let found: User = call_field("getUser", &[("username", &user)], "user").ok()?;
    let stream = found.stream_role.unwrap_or(true);
    let _ = kv::set_json(
        ROLES_KEY,
        &serde_json::json!({ "server": server, "at": now, "stream": stream }),
    );
    Some(stream)
}

/// Remembers the cover art id of an item from a listing.
pub fn remember_cover(item_id: &str, cover: &Option<String>) {
    if let Some(cover) = cover {
        STATE.with(|s| {
            let mut state = s.borrow_mut();
            if state.covers.len() >= MAX_COVERS {
                state.covers.clear();
            }
            state.covers.insert(item_id.to_string(), cover.clone());
        });
    }
}

/// The cover art id of an item listed earlier, if remembered.
pub fn cover_of(item_id: &str) -> Option<String> {
    STATE.with(|s| s.borrow().covers.get(item_id).cloned())
}

/// A list fetched in the last two minutes.
pub fn cached_list(key: &str) -> Option<Vec<Item>> {
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
            .map(|(_, _, items)| items.clone())
    })
}

/// Keeps a fetched list for paging (the last four).
pub fn keep_list(key: &str, items: &[Item]) {
    let now = clock::monotonic_ms();
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.lists.retain(|(k, _, _)| k != key);
        if state.lists.len() >= 4 {
            state.lists.remove(0);
        }
        state.lists.push((key.to_string(), now, items.to_vec()));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_server_info() {
        let info = ServerInfo::from_envelope(&json!({
            "status": "ok", "version": "1.16.1", "type": "navidrome", "serverVersion": "0.64.2",
            "openSubsonic": true,
            "openSubsonicExtensions": [
                { "name": "formPost", "versions": [1] },
                { "name": "songLyrics", "versions": [1, 2] }
            ]
        }));
        assert!(info.open_subsonic && info.has("formPost") && !info.has("apiKeyAuthentication"));
        assert_eq!(info.api_at_least(1, 13), Some(true));
        assert_eq!(info.describe(), "navidrome 0.64.2");
        let old = ServerInfo::from_envelope(&json!({ "status": "failed", "version": "1.12.0" }));
        assert_eq!(old.api_at_least(1, 13), Some(false));
        assert_eq!(ServerInfo::default().api_at_least(1, 13), None);
    }

    #[test]
    fn maps_error_codes() {
        let error = |code: i64| {
            PluginError::from(ApiError {
                code,
                message: "said".into(),
                help_url: None,
                envelope: Value::Null,
            })
        };
        assert_eq!(error(40), PluginError::AuthRequired);
        assert_eq!(error(44), PluginError::AuthRequired);
        assert_eq!(error(70), PluginError::NotFound);
        assert!(matches!(error(50), PluginError::Service(_)));
        assert_eq!(error(0), PluginError::Service("said".into()));
    }
}

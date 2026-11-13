//! Talking to the server: the authorisation header, the stored session,
//! requests and their errors, and small in-memory caches.
//!
//! Every request carries `Authorization: MediaBrowser Client="Jellyfesh",
//! Device=…, DeviceId=…, Version=…[, Token=…]` (Jellyfin 12 removed the
//! legacy `X-Emby-*` headers and `api_key`; this header works since 10.x).
//! Values are percent-encoded, as the server URL-decodes them. The device id
//! is random, made once and kept in kv, so the server sees one device across
//! sign-ins. The access token is the only credential kept, in the keystore,
//! tied to the server address. Requests leave out `userId`: since 10.9 the
//! server takes the signed-in user from the token.

use std::cell::RefCell;
use std::collections::HashMap;

use jf_plugin_sdk::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::model::PublicInfo;
use crate::settings::Settings;

/// `Client=`.
pub const CLIENT: &str = "Jellyfesh";
/// `Version=`: the plugin's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The oldest Jellyfin this plugin speaks to: 10.9 added `/UserViews`,
/// `userId` taken from the token, and lyrics.
pub const OLDEST: (u32, u32) = (10, 9);
/// Keystore: the session (`{"server": …, "token": …}`).
const SESSION: &str = "session";
/// Keystore, during a Quick Connect sign-in: the request's secret.
pub const QUICK_CONNECT: &str = "quick-connect";
/// kv: the random device id.
const DEVICE_KEY: &str = "device-id";
/// How long the list of libraries is reused.
const VIEWS_TTL_MS: u64 = 2 * 60 * 1000;
/// Most image tags remembered.
const MAX_IMAGES: usize = 50_000;

/// What the keystore holds for a session or a Quick Connect request.
#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    /// [`binding`] of the server address.
    server: String,
    /// The access token, or the Quick Connect secret.
    value: String,
}

#[derive(Debug, Default)]
struct State {
    settings: Settings,
    /// The token from the keystore: outer none means not read yet.
    token: Option<Option<String>>,
    device_id: Option<String>,
    /// Music libraries: (id, name), with when they were fetched.
    views: Option<(u64, Vec<(String, String)>)>,
    /// Item id → (id of the item whose Primary image to show, its tag).
    images: HashMap<String, (String, String)>,
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
            let device_id = state.device_id.take();
            *state = State {
                device_id,
                ..State::default()
            };
        }
        state.settings = settings;
    });
}

/// Forgets the session and the caches (logout).
pub fn forget() {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.token = Some(None);
        state.views = None;
        state.images.clear();
    });
}

/// The server's base address.
pub fn base() -> Result<String> {
    settings().base.ok_or_else(|| {
        PluginError::invalid_input("Enter your server's address in this source's settings first.")
    })
}

/// Which server a stored credential belongs to: a short hash of the address,
/// so a token is never sent to another server after the address changes.
fn binding(base: &str) -> Result<String> {
    let digest = crypto::sha256(base.to_ascii_lowercase().as_bytes())?;
    Ok(crypto::hex(&digest[..8]))
}

/// The device id, made on first use.
pub fn device_id() -> Result<String> {
    if let Some(id) = STATE.with(|s| s.borrow().device_id.clone()) {
        return Ok(id);
    }
    let id = match kv::get(DEVICE_KEY)?.and_then(|bytes| String::from_utf8(bytes).ok()) {
        Some(id) if !id.is_empty() => id,
        _ => {
            let id = crypto::random_hex(16)?;
            kv::set(DEVICE_KEY, id.as_bytes())?;
            id
        }
    };
    STATE.with(|s| s.borrow_mut().device_id = Some(id.clone()));
    Ok(id)
}

/// The `Authorization` header value, with the token if given.
pub fn authorization(token: Option<&str>) -> Result<String> {
    let settings = settings();
    let mut text = format!(
        "MediaBrowser Client=\"{}\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
        http::encode(CLIENT),
        http::encode(&settings.device_name),
        http::encode(&device_id()?),
        http::encode(VERSION)
    );
    if let Some(token) = token {
        text.push_str(&format!(", Token=\"{}\"", http::encode(token)));
    }
    Ok(text)
}

fn read_stored(name: &str) -> Result<Option<String>> {
    let server = binding(&base()?)?;
    Ok(match kv::secret::get(name)? {
        Some(text) => match serde_json::from_str::<Stored>(&text) {
            Ok(stored) if stored.server == server => Some(stored.value),
            Ok(_) => {
                jf_plugin_sdk::warn!("the stored {name} is for another server address");
                None
            }
            Err(_) => None,
        },
        None => None,
    })
}

/// Stores `value` under `name` in the keystore, tied to the server. The raw
/// error lets a login tell a refused value (it contained the password) from
/// other failures.
pub fn store(name: &str, value: &str) -> core::result::Result<(), kv::KvError> {
    let unavailable = |e: PluginError| kv::KvError::Unavailable(format!("{e:?}"));
    let server = binding(&base().map_err(unavailable)?).map_err(unavailable)?;
    let text = serde_json::to_string(&Stored {
        server,
        value: value.to_string(),
    })
    .map_err(|e| kv::KvError::Unavailable(e.to_string()))?;
    jf_plugin_sdk::bindings::jellyfesh::plugin::kv::set_secret(name, &text)
}

/// Reads a value stored with [`store`] for the configured server.
pub fn stored(name: &str) -> Result<Option<String>> {
    read_stored(name)
}

/// Keeps the access token of a new session.
pub fn save_token(token: &str) -> core::result::Result<(), kv::KvError> {
    store(SESSION, token)?;
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.token = Some(Some(token.to_string()));
        state.views = None;
    });
    Ok(())
}

/// The access token for the configured server, if signed in.
pub fn token() -> Result<Option<String>> {
    if let Some(cached) = STATE.with(|s| s.borrow().token.clone()) {
        return Ok(cached);
    }
    let found = read_stored(SESSION)?;
    STATE.with(|s| s.borrow_mut().token = Some(found.clone()));
    Ok(found)
}

/// The access token, or `auth-required`.
pub fn session() -> Result<String> {
    token()?.ok_or(PluginError::AuthRequired)
}

/// How a request signs in.
#[derive(Debug, Clone, Copy)]
pub enum Auth<'a> {
    /// No token (public calls and sign-in itself still identify the client).
    Client,
    /// The stored session; `auth-required` without one.
    Session,
    /// This token.
    Token(&'a str),
}

/// Sends a request to `path` (with its query) and returns the response,
/// whatever its status.
pub fn send(request: http::Request, auth: Auth<'_>) -> Result<http::Response> {
    let token = match auth {
        Auth::Client => None,
        Auth::Session => Some(session()?),
        Auth::Token(token) => Some(token.to_string()),
    };
    Ok(request
        .header("authorization", &authorization(token.as_deref())?)
        .header("accept", "application/json")
        .send()?)
}

/// A URL on the server.
pub fn url(path: &str) -> Result<String> {
    Ok(format!("{}{path}", base()?))
}

/// `GET path?params` with the session, parsed as JSON.
pub fn get<T: DeserializeOwned>(path: &str, params: &[(&str, &str)]) -> Result<T> {
    let response = send(http::Request::get(url(path)?).query(params), Auth::Session)?;
    json(&check(response)?)
}

/// Maps error statuses to plugin errors.
pub fn check(response: http::Response) -> Result<http::Response> {
    match response.status {
        200..=299 => Ok(response),
        401 => {
            // The token was revoked or expired.
            STATE.with(|s| s.borrow_mut().token = None);
            Err(PluginError::AuthRequired)
        }
        403 => Err(PluginError::service(
            "The server does not allow your account to do that.",
        )),
        404 => Err(PluginError::NotFound),
        429 => Err(PluginError::RateLimited),
        status => Err(PluginError::service(status_text(status))),
    }
}

/// Words for an unexpected status.
pub fn status_text(status: u16) -> String {
    match status {
        400 => "The server could not use Jellyfesh's request (HTTP 400).".to_string(),
        503 => "The server is starting or busy (HTTP 503). Try again in a moment.".to_string(),
        502 | 504 => {
            format!("A proxy in front of the server could not reach it (HTTP {status}). Try again later.")
        }
        500..=599 => format!("The server had an internal error (HTTP {status}). Try again later."),
        _ => format!("The server answered HTTP {status}."),
    }
}

/// The body as JSON, or a clear error when it is not (a web page from a
/// proxy, for example).
pub fn json<T: DeserializeOwned>(response: &http::Response) -> Result<T> {
    serde_json::from_slice(&response.body).map_err(|_| {
        PluginError::service(
            "The server did not answer like Jellyfin. Check the server address: use the one you open Jellyfin at.",
        )
    })
}

/// `/System/Info/Public`: what the server says about itself, without a
/// sign-in.
pub fn public_info() -> Result<PublicInfo> {
    let response = send(http::Request::get(url("/System/Info/Public")?), Auth::Client)?;
    match response.status {
        200..=299 => json(&response),
        404 => Err(PluginError::service(
            "No Jellyfin server answers at this address (HTTP 404). Check the server address: use the one \
             you open Jellyfin at, without a page path.",
        )),
        401 | 403 => Err(PluginError::service(format!(
            "The address answered HTTP {} before reaching Jellyfin; a proxy in front of it may want its own sign-in.",
            response.status
        ))),
        status => Err(PluginError::service(status_text(status))),
    }
}

/// Why this plugin cannot use the server, if it cannot: not Jellyfin, or
/// older than 10.9.
pub fn server_problem(info: &PublicInfo) -> Option<String> {
    let product = info.product_name.as_deref().unwrap_or_default();
    if !product.to_ascii_lowercase().contains("jellyfin") {
        return Some(if product.is_empty() {
            "The server at this address does not say it is Jellyfin.".to_string()
        } else {
            format!("The server at this address is {product}, not Jellyfin.")
        });
    }
    let version = info.version.as_deref().unwrap_or_default();
    match parse_version(version) {
        Some(found) if found < OLDEST => Some(format!(
            "This server runs Jellyfin {version}; Jellyfesh needs Jellyfin {}.{} or newer. Update the server.",
            OLDEST.0, OLDEST.1
        )),
        _ => None,
    }
}

/// `10.11.9` → (10, 11).
pub fn parse_version(text: &str) -> Option<(u32, u32)> {
    let mut parts = text.trim().split('.').map(|p| p.parse::<u32>().ok());
    let major = parts.next().flatten()?;
    let minor = parts.next().flatten().unwrap_or(0);
    Some((major, minor))
}

/// The music libraries the account sees: (id, name). Kept two minutes.
pub fn music_libraries() -> Result<Vec<(String, String)>> {
    let now = clock::monotonic_ms();
    if let Some((at, views)) = STATE.with(|s| s.borrow().views.clone())
        && now.saturating_sub(at) < VIEWS_TTL_MS
    {
        return Ok(views);
    }
    let found: crate::model::ItemList = get("/UserViews", &[])?;
    let views: Vec<(String, String)> = found
        .items
        .into_iter()
        .filter(|v| v.collection_type.as_deref() == Some("music") && !v.id.is_empty())
        .map(|v| {
            let name = v.name.unwrap_or_else(|| "Music".into());
            (v.id, name)
        })
        .collect();
    STATE.with(|s| s.borrow_mut().views = Some((now, views.clone())));
    Ok(views)
}

/// Remembers which image shows an item (from a listing).
pub fn remember_image(item_id: &str, image: &(String, String)) {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        if state.images.len() >= MAX_IMAGES {
            state.images.clear();
        }
        state.images.insert(item_id.to_string(), image.clone());
    });
}

/// The image remembered for an item, if any.
pub fn image_of(item_id: &str) -> Option<(String, String)> {
    STATE.with(|s| s.borrow().images.get(item_id).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_the_server() {
        let info = |product: &str, version: &str| PublicInfo {
            product_name: Some(product.into()),
            version: Some(version.into()),
            ..PublicInfo::default()
        };
        assert_eq!(server_problem(&info("Jellyfin Server", "12.4.0")), None);
        assert_eq!(server_problem(&info("Jellyfin Server", "10.9.11")), None);
        let old = server_problem(&info("Jellyfin Server", "10.8.13")).unwrap();
        assert!(old.contains("10.8.13") && old.contains("10.9"), "{old}");
        assert!(
            server_problem(&info("Emby Server", "4.9.1"))
                .unwrap()
                .contains("Emby Server")
        );
        assert_eq!(parse_version("10.11.9"), Some((10, 11)));
        assert_eq!(parse_version("12"), Some((12, 0)));
        assert_eq!(parse_version("x"), None);
    }
}

//! The Spotify Web API and the accounts service: the person's session
//! (tokens from Authorization Code with PKCE, no client secret), requests
//! with refresh and back-off, and Spotify's errors in Jellyfesh's words.
//!
//! * Refresh tokens rotate: the newest one replaces the stored one, and
//!   `invalid_grant` (an expired or revoked token; Spotify expires them after
//!   six months) ends the session: sign in again.
//! * HTTP 429: the `Retry-After` seconds (Spotify computes its limit over a
//!   rolling 30 s window) are kept in `kv`, and nothing is sent before then.
//! * Player commands need Spotify Premium: HTTP 403 says so in Jellyfesh's
//!   words.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The accounts service (authorisation and tokens).
pub const ACCOUNTS: &str = "https://accounts.spotify.com";
/// The Web API.
pub const API: &str = "https://api.spotify.com/v1";
/// What the plugin asks the person to allow.
pub const SCOPES: &str = "user-read-playback-state user-modify-playback-state user-read-currently-playing \
                          user-library-read playlist-read-private playlist-read-collaborative user-read-private \
                          user-read-recently-played user-top-read";
/// The person's own Client ID, when they typed one at sign-in.
pub const CLIENT_SECRET_NAME: &str = "client-id";
/// The Client ID of the Spotify app a release build signs in with, from the
/// `JF_SPOTIFY_CLIENT_ID` secret when the plugin was built (a public value:
/// it is in every sign-in URL). Builds without it ask each person for their
/// own.
const BUILT_IN_CLIENT_ID: Option<&str> = option_env!("JF_SPOTIFY_CLIENT_ID");
/// Shown when Spotify refuses an account the built-in app has no seat for.
pub const NO_SEAT: &str = "Jellyfesh's Spotify app has no place for this account yet: Spotify lets such an \
                           app sign in only a few people its owner adds. Ask whoever provides your Jellyfesh \
                           build to add you, or enter the Client ID of your own Spotify app in this source's \
                           settings (Your Own Client ID) and sign in again.";
/// The tokens.
pub const SESSION: &str = "session";
/// Until when (ms) Spotify asked not to call it.
const PAUSE: &str = "pause-until-ms";
/// Shown when a player command needs Premium.
pub const PREMIUM: &str = "Spotify Premium is needed to control playback.";
/// Shown when no device is active for a command.
pub const NO_DEVICE: &str =
    "No Spotify device is active. Open Spotify on one of your devices, or choose one in Jellyfesh.";

thread_local! {
    static BASES: RefCell<(String, String)> = RefCell::new((ACCOUNTS.to_owned(), API.to_owned()));
    /// The `client_id` setting: the person's own Client ID, if they set one.
    static OWN_CLIENT_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Takes the `client_id` setting.
pub fn set_own_client_id(id: Option<&str>) {
    let id = id.map(str::trim).filter(|id| !id.is_empty()).map(str::to_owned);
    OWN_CLIENT_ID.with(|own| *own.borrow_mut() = id);
}

/// The built-in Client ID, when this build has a plausible one.
pub fn built_in_client_id() -> Option<&'static str> {
    BUILT_IN_CLIENT_ID
        .map(str::trim)
        .filter(|id| crate::login::plausible_client_id(id))
}

/// Which Client ID signs in: the person's own (the setting first, then one
/// they typed at an earlier sign-in), else the built-in one.
pub fn choose_client_id(
    setting: Option<&str>,
    typed: Option<&str>,
    built_in: Option<&str>,
) -> Option<String> {
    [setting, typed, built_in]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|id| !id.is_empty())
        .map(str::to_owned)
}

/// Uses `server` for both services (tests), or Spotify's own when `None`.
pub fn set_server(server: Option<&str>) {
    let bases = match server {
        Some(base) => {
            let base = base.trim_end_matches('/');
            (base.to_owned(), format!("{base}/v1"))
        }
        None => (ACCOUNTS.to_owned(), API.to_owned()),
    };
    BASES.with(|b| *b.borrow_mut() = bases);
}

pub fn accounts() -> String {
    BASES.with(|b| b.borrow().0.clone())
}

pub fn api() -> String {
    BASES.with(|b| b.borrow().1.clone())
}

/// The tokens of a signed-in person.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_ms: u64,
    /// The Client ID the tokens belong to (renewals must use it).
    #[serde(default)]
    pub client_id: Option<String>,
}

/// A token answer of the accounts service.
#[derive(Debug, Deserialize)]
pub struct TokenAnswer {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

/// An error answer of the accounts service.
#[derive(Debug, Default, Deserialize)]
pub struct TokenError {
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub error_description: Option<String>,
}

impl Session {
    /// The session a token answer starts or continues: a rotated refresh
    /// token replaces the old one, which stays when none comes.
    pub fn from_answer(answer: TokenAnswer, previous_refresh: Option<&str>, now_ms: u64) -> Result<Session> {
        let refresh_token = answer
            .refresh_token
            .filter(|t| !t.is_empty())
            .or_else(|| previous_refresh.map(str::to_owned))
            .ok_or_else(|| PluginError::service("Spotify sent no refresh token."))?;
        Ok(Session {
            access_token: answer.access_token,
            refresh_token,
            expires_at_ms: now_ms + answer.expires_in.unwrap_or(3600).min(86_400) * 1000,
            client_id: None,
        })
    }

    pub fn store(&self) -> Result<()> {
        let text = serde_json::to_string(self).map_err(|e| PluginError::internal(e.to_string()))?;
        kv::secret::set(SESSION, &text)
    }
}

pub fn client_id() -> Result<Option<String>> {
    let typed = kv::secret::get(CLIENT_SECRET_NAME)?;
    let setting = OWN_CLIENT_ID.with(|own| own.borrow().clone());
    Ok(choose_client_id(
        setting.as_deref(),
        typed.as_deref(),
        built_in_client_id(),
    ))
}

fn session() -> Result<Session> {
    let text = kv::secret::get(SESSION)?.ok_or(PluginError::AuthRequired)?;
    serde_json::from_str(&text).map_err(|_| PluginError::AuthRequired)
}

/// Exchanges the stored refresh token for new tokens and stores them.
fn refresh(old: &Session) -> Result<Session> {
    let client_id = match &old.client_id {
        Some(id) => id.clone(),
        None => client_id()?.ok_or(PluginError::AuthRequired)?,
    };
    let response = http::Request::post(format!("{}/api/token", accounts()))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &old.refresh_token),
            ("client_id", &client_id),
        ])
        .send()?;
    if response.status == 429 {
        pause_from(&response);
        return Err(PluginError::RateLimited);
    }
    if !response.is_success() {
        let error: TokenError = response.json().unwrap_or_default();
        if response.status == 400 && matches!(error.error.as_str(), "invalid_grant" | "invalid_client") {
            // Expired (six months), revoked, or the app is gone: sign in again.
            kv::secret::delete(SESSION)?;
            return Err(PluginError::AuthRequired);
        }
        return Err(PluginError::service(format!(
            "Spotify could not renew the sign-in (HTTP {}).",
            response.status
        )));
    }
    let mut session = Session::from_answer(response.json()?, Some(&old.refresh_token), clock::now_utc_ms())?;
    session.client_id = Some(client_id);
    session.store()?;
    Ok(session)
}

/// Fails with `RateLimited` while Spotify's last `Retry-After` lasts.
fn check_pause() -> Result<()> {
    let until = kv::get_json::<u64>(PAUSE)?.unwrap_or(0);
    if until > clock::now_utc_ms() {
        return Err(PluginError::RateLimited);
    }
    Ok(())
}

/// Remembers how long to wait after a 429: `Retry-After` seconds, else 30 s,
/// or five minutes for an exhausted quota; at most a day.
pub fn pause_from(response: &http::Response) {
    let quota = response.text().contains("QUOTA_EXCEEDED");
    let seconds = response
        .header("retry-after")
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(if quota { 300 } else { 30 })
        .clamp(1, 86_400);
    let _ = kv::set_json(PAUSE, &(clock::now_utc_ms() + seconds * 1000));
}

/// A Web API request: `method` on `path` (with its query) and an optional
/// JSON body, with the access token, renewed when it expired. Statuses other
/// than 401 and 429 are the caller's to read.
pub fn send(method: http::Method, path: &str, body: Option<&Value>) -> Result<http::Response> {
    check_pause()?;
    let mut current = session()?;
    if current.expires_at_ms <= clock::now_utc_ms() + 60_000 {
        current = refresh(&current)?;
    }
    let build = |token: &str| -> Result<http::Request> {
        let request = http::Request::new(method, format!("{}{path}", api())).bearer(token);
        Ok(match body {
            Some(body) => request.json(body)?,
            None => request,
        })
    };
    let mut response = build(&current.access_token)?.send()?;
    if response.status == 401 {
        current = refresh(&current)?;
        response = build(&current.access_token)?.send()?;
        if response.status == 401 {
            return Err(PluginError::AuthRequired);
        }
    }
    if response.status == 429 {
        pause_from(&response);
        return Err(PluginError::RateLimited);
    }
    Ok(response)
}

/// `{"error": {"status": …, "message": …, "reason": …}}`; the reason is not
/// in the published schema, so it is read when present.
#[derive(Debug, Default, Deserialize)]
struct ApiError {
    #[serde(default)]
    error: ApiErrorBody,
}

#[derive(Debug, Default, Deserialize)]
struct ApiErrorBody {
    #[serde(default)]
    message: String,
    #[serde(default)]
    reason: Option<String>,
}

/// What failed, for a response that is not a success. `player` marks
/// commands to the person's devices (Premium only).
pub fn error(response: &http::Response, player: bool) -> PluginError {
    let parsed: ApiError = response.json().unwrap_or_default();
    let message = parsed.error.message.trim().to_owned();
    let reason = parsed.error.reason.unwrap_or_default();
    match response.status {
        403 if player
            && (reason == "PREMIUM_REQUIRED"
                || message.is_empty()
                || message.to_ascii_lowercase().contains("premium")) =>
        {
            PluginError::service(PREMIUM)
        }
        404 if player => {
            if reason == "NO_ACTIVE_DEVICE"
                || message.to_ascii_lowercase().contains("device")
                || message.is_empty()
            {
                PluginError::service(NO_DEVICE)
            } else {
                PluginError::service(format!("Spotify: {message}"))
            }
        }
        404 => PluginError::NotFound,
        status if !message.is_empty() => PluginError::service(format!("Spotify: {message} (HTTP {status})")),
        status => PluginError::service(format!("Spotify answered HTTP {status}")),
    }
}

/// Sends a request and parses a successful JSON answer.
pub fn get_json<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T> {
    let response = send(http::Method::Get, path, None)?;
    if !response.is_success() {
        return Err(error(&response, false));
    }
    response.json()
}

/// Sends a player command (204 or 200 is success).
pub fn command(method: http::Method, path: &str, body: Option<&Value>) -> Result<()> {
    let response = send(method, path, body)?;
    if response.is_success() {
        Ok(())
    } else {
        Err(error(&response, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, body: &str) -> http::Response {
        http::Response {
            status,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
            url: String::new(),
        }
    }

    #[test]
    fn premium_and_device_errors_are_explained() {
        let premium = r#"{"error":{"status":403,"message":"Player command failed: Premium required","reason":"PREMIUM_REQUIRED"}}"#;
        assert_eq!(
            error(&response(403, premium), true),
            PluginError::service(PREMIUM)
        );
        assert_eq!(error(&response(403, ""), true), PluginError::service(PREMIUM));
        let restricted = r#"{"error":{"status":403,"message":"Player command failed: Restriction violated","reason":"UNKNOWN"}}"#;
        assert_eq!(
            error(&response(403, restricted), true),
            PluginError::service("Spotify: Player command failed: Restriction violated (HTTP 403)")
        );
        let no_device = r#"{"error":{"status":404,"message":"Player command failed: No active device found","reason":"NO_ACTIVE_DEVICE"}}"#;
        assert_eq!(
            error(&response(404, no_device), true),
            PluginError::service(NO_DEVICE)
        );
        assert_eq!(error(&response(404, ""), false), PluginError::NotFound);
        assert_eq!(
            error(&response(500, "oops"), false),
            PluginError::service("Spotify answered HTTP 500")
        );
    }

    #[test]
    fn rotated_refresh_tokens_replace_old_ones() {
        let answer = |refresh: Option<&str>| TokenAnswer {
            access_token: "at".into(),
            refresh_token: refresh.map(str::to_owned),
            expires_in: Some(3600),
        };
        let s = Session::from_answer(answer(Some("new")), Some("old"), 1_000).unwrap();
        assert_eq!((s.refresh_token.as_str(), s.expires_at_ms), ("new", 3_601_000));
        let s = Session::from_answer(answer(None), Some("old"), 0).unwrap();
        assert_eq!(s.refresh_token, "old");
        assert!(Session::from_answer(answer(None), None, 0).is_err());
    }
}

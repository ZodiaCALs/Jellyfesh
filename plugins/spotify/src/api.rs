//! The Spotify Web API and the accounts service: the person's session
//! (tokens from Authorization Code with PKCE, no client secret), requests
//! with refresh and back-off, and Spotify's errors in Jellyfesh's words.
//!
//! * Refresh tokens rotate: the newest one replaces the stored one, and
//!   `invalid_grant` (an expired or revoked token; Spotify expires them after
//!   six months) ends the session with "Sign in to Spotify again."
//! * The session keeps the scopes Spotify granted (the token answer's
//!   `scope`), so a list that needs a scope an older sign-in lacks is left
//!   out instead of failing.
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
                          user-read-recently-played user-top-read user-follow-read";
/// The scope of the followed artists (`/me/following`), asked for since
/// plugin version 0.0.8; older sign-ins lack it.
pub const FOLLOW_SCOPE: &str = "user-follow-read";
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
/// Shown when Spotify ends the sign-in: the refresh token expired (after six
/// months) or was revoked, or the app is gone.
pub const SIGN_IN_AGAIN: &str = "Sign in to Spotify again.";
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
    /// The scopes Spotify granted, space-separated; none for sessions from
    /// before plugin version 0.0.8, which did not keep them.
    #[serde(default)]
    pub scope: Option<String>,
    /// The person's Spotify user id (`/me`), to tell their own playlists
    /// from followed ones; kept since plugin version 0.0.8.
    #[serde(default)]
    pub user_id: Option<String>,
}

/// A token answer of the accounts service.
#[derive(Debug, Deserialize)]
pub struct TokenAnswer {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    /// The scopes granted, space-separated.
    #[serde(default)]
    pub scope: Option<String>,
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
    /// The session a token answer starts or continues (`previous`): a
    /// rotated refresh token replaces the old one, which stays when none
    /// comes, and so do the granted scopes.
    pub fn from_answer(answer: TokenAnswer, previous: Option<&Session>, now_ms: u64) -> Result<Session> {
        let refresh_token = answer
            .refresh_token
            .filter(|t| !t.is_empty())
            .or_else(|| previous.map(|p| p.refresh_token.clone()))
            .ok_or_else(|| PluginError::service("Spotify sent no refresh token."))?;
        let scope = answer
            .scope
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .or_else(|| previous.and_then(|p| p.scope.clone()));
        Ok(Session {
            access_token: answer.access_token,
            refresh_token,
            expires_at_ms: now_ms + answer.expires_in.unwrap_or(3600).min(86_400) * 1000,
            client_id: None,
            scope,
            user_id: previous.and_then(|p| p.user_id.clone()),
        })
    }

    /// Whether Spotify granted `scope`: none when the session does not say
    /// (signed in before plugin version 0.0.8).
    pub fn granted(&self, scope: &str) -> Option<bool> {
        self.scope
            .as_deref()
            .map(|granted| granted.split_ascii_whitespace().any(|s| s == scope))
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

/// The signed-in person's Spotify user id: kept with the session since
/// plugin version 0.0.8, asked of `/me` once for older sessions.
pub fn user_id() -> Result<String> {
    if let Some(id) = session()?.user_id.filter(|id| !id.is_empty()) {
        return Ok(id);
    }
    #[derive(Deserialize)]
    struct Me {
        #[serde(default)]
        id: String,
    }
    let me: Me = get_json("/me")?;
    if me.id.is_empty() {
        return Err(PluginError::service("Spotify did not say who is signed in."));
    }
    // Read again: the request may have renewed and stored the session.
    let mut current = session()?;
    current.user_id = Some(me.id.clone());
    current.store()?;
    Ok(me.id)
}

/// The stored session, or `auth-required`.
pub fn session() -> Result<Session> {
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
            // Expired (six months), revoked, or the app is gone: the session
            // ends, and the next call answers `auth-required`.
            kv::secret::delete(SESSION)?;
            return Err(PluginError::service(SIGN_IN_AGAIN));
        }
        return Err(PluginError::service(format!(
            "Spotify could not renew the sign-in (HTTP {}).",
            response.status
        )));
    }
    let mut session = Session::from_answer(response.json()?, Some(old), clock::now_utc_ms())?;
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

/// Whether a 403 refuses the whole account rather than one request: an
/// account the app has no seat for (Development Mode), or an app whose owner
/// has no Premium (Spotify's rule since February 2026). Their messages are
/// not in the published schema, so this reads them loosely.
pub fn refuses_account(response: &http::Response) -> bool {
    let parsed: ApiError = response.json().unwrap_or_default();
    let message = parsed.error.message.to_ascii_lowercase();
    response.status == 403
        && ["registered", "developer.spotify.com", "dashboard", "premium"]
            .iter()
            .any(|word| message.contains(word))
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

    fn answer(refresh: Option<&str>, scope: Option<&str>) -> TokenAnswer {
        TokenAnswer {
            access_token: "at".into(),
            refresh_token: refresh.map(str::to_owned),
            expires_in: Some(3600),
            scope: scope.map(str::to_owned),
        }
    }

    #[test]
    fn rotated_refresh_tokens_replace_old_ones() {
        let old = Session::from_answer(answer(Some("old"), Some("user-library-read")), None, 0).unwrap();
        let s = Session::from_answer(answer(Some("new"), None), Some(&old), 1_000).unwrap();
        assert_eq!((s.refresh_token.as_str(), s.expires_at_ms), ("new", 3_601_000));
        let s = Session::from_answer(answer(None, None), Some(&old), 0).unwrap();
        assert_eq!(s.refresh_token, "old");
        assert!(Session::from_answer(answer(None, None), None, 0).is_err());
    }

    #[test]
    fn granted_scopes_are_kept_across_renewals() {
        let first = Session::from_answer(
            answer(Some("r1"), Some("user-library-read  playlist-read-private")),
            None,
            0,
        )
        .unwrap();
        assert_eq!(first.granted("playlist-read-private"), Some(true));
        assert_eq!(first.granted("user-follow-read"), Some(false));
        assert_eq!(first.granted("playlist-read"), Some(false), "whole names only");
        // A renewal without `scope` keeps the granted ones; one with it
        // replaces them.
        let renewed = Session::from_answer(answer(None, None), Some(&first), 0).unwrap();
        assert_eq!(renewed.granted("user-library-read"), Some(true));
        let renewed = Session::from_answer(answer(None, Some("user-follow-read")), Some(&first), 0).unwrap();
        assert_eq!(renewed.granted("user-follow-read"), Some(true));
        assert_eq!(renewed.granted("user-library-read"), Some(false));
        // A session stored before 0.0.8 does not know.
        let old: Session =
            serde_json::from_str(r#"{"access_token":"a","refresh_token":"r","expires_at_ms":1}"#).unwrap();
        assert_eq!(old.granted("user-library-read"), None);
    }

    #[test]
    fn account_refusals_are_told_from_other_403s() {
        let no_seat = r#"{"error":{"status":403,"message":"Check settings on developer.spotify.com/dashboard, the user may not be registered."}}"#;
        assert!(refuses_account(&response(403, no_seat)));
        let owner = r#"{"error":{"status":403,"message":"Active premium subscription required for the owner of the app."}}"#;
        assert!(refuses_account(&response(403, owner)));
        assert!(!refuses_account(&response(
            403,
            r#"{"error":{"status":403,"message":"Forbidden"}}"#
        )));
        assert!(!refuses_account(&response(403, "")));
        assert!(!refuses_account(&response(404, no_seat)));
    }
}

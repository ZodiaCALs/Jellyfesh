//! Signing in (ADR-0015): the plugin keeps only the access token the
//! server issues, in the keystore, tied to the server address.
//!
//! * **Quick Connect** (the `qr` kind): `POST /QuickConnect/Initiate` gives a
//!   six-digit code, shown with the WIT `show-code` step, and a secret kept
//!   in the keystore while the flow runs. `poll` asks
//!   `/QuickConnect/Connect?secret=` until the person approves the code in
//!   Jellyfin, then `POST /Users/AuthenticateWithQuickConnect` gives the
//!   token. Codes expire after ten minutes. No password is ever typed.
//! * **User name and password**: `POST /Users/AuthenticateByName` once, in
//!   the submit call; the password is not kept.

use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::{self, Auth};
use crate::model::{AuthResult, QuickConnect};

/// How long a Quick Connect code works (the server's ten minutes).
const CODE_LIFETIME_MS: u64 = 10 * 60 * 1000;
/// How often the host asks whether the code was approved.
const POLL_MS: u32 = 2000;

/// `auth.begin`.
pub fn begin(flow: Flow) -> Result<FlowStep> {
    let base = match api::base() {
        Ok(base) => base,
        Err(_) => {
            return Ok(FlowStep::failed(
                "Enter your server's address in this source's settings first.",
            ));
        }
    };
    let info = match api::public_info() {
        Ok(info) => info,
        Err(error) => return Ok(FlowStep::failed(unreachable(&base, &error))),
    };
    if let Some(problem) = api::server_problem(&info) {
        return Ok(FlowStep::failed(problem));
    }
    // Made before any password is typed, so it can never contain one.
    api::device_id()?;
    match flow.kind {
        AuthKind::PasswordExchange => {
            let mut user = FieldSpec::text("username", "User name");
            user.placeholder = Some("Your user name on the server".into());
            let mut password = FieldSpec::secret("password", "Password");
            password.required = false;
            password.help = Some(
                "Jellyfesh keeps only the access token Jellyfin gives it, not your password. Leave the field \
                 empty if your account has no password."
                    .into(),
            );
            if base.to_ascii_lowercase().starts_with("http://") && !is_local(&base) {
                password.help = Some(format!(
                    "{} This address uses http, so your password travels unencrypted.",
                    password.help.unwrap_or_default()
                ));
            }
            Ok(FlowStep::new(Step::AskFields(vec![user, password])))
        }
        AuthKind::Qr => start_quick_connect(&base),
        _ => Ok(FlowStep::failed("This way of signing in is not supported.")),
    }
}

/// Whether the address is on this device or the local network (where http
/// is common and the warning would only be noise).
fn is_local(base: &str) -> bool {
    let host = base
        .split("://")
        .nth(1)
        .unwrap_or_default()
        .split(['/', ':'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".local")
        || host.ends_with(".lan")
        || host.ends_with(".home.arpa")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("127.")
        || (host.starts_with("172.")
            && host
                .split('.')
                .nth(1)
                .and_then(|b| b.parse::<u8>().ok())
                .is_some_and(|b| (16..32).contains(&b)))
}

/// `auth.submit` (user name and password).
pub fn submit(flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
    let field = |name: &str| {
        fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.clone())
            .unwrap_or_default()
    };
    if flow.kind != AuthKind::PasswordExchange {
        return Ok(FlowStep::failed("Nothing to submit. Start again."));
    }
    let base = api::base()?;
    let user = field("username").trim().to_string();
    if user.is_empty() {
        return Ok(FlowStep::failed("Enter your user name."));
    }
    let request = http::Request::post(api::url("/Users/AuthenticateByName")?)
        .json(&json!({ "Username": user, "Pw": field("password") }))?;
    let response = match api::send(request, Auth::Client) {
        Ok(response) => response,
        Err(error) => return Ok(FlowStep::failed(unreachable(&base, &error))),
    };
    match response.status {
        200..=299 => {}
        401 => return Ok(FlowStep::failed("Wrong user name or password.")),
        403 => {
            return Ok(FlowStep::failed(
                "The server refused this sign-in: the account may be disabled, not allowed on this device, \
                 or at its limit of sessions.",
            ));
        }
        status => return Ok(FlowStep::failed(api::status_text(status))),
    }
    let result: AuthResult = match api::json(&response) {
        Ok(result) => result,
        Err(error) => return Ok(FlowStep::failed(crate::error_text(&error))),
    };
    finish(&result, true)
}

/// Keeps the token of a new session and ends the flow.
fn finish(result: &AuthResult, with_password: bool) -> Result<FlowStep> {
    if result.access_token.is_empty() {
        return Ok(FlowStep::failed(
            "The server signed you in but sent no access token.",
        ));
    }
    match api::save_token(&result.access_token) {
        Ok(()) => {
            let account = if result.user.name.is_empty() {
                "Jellyfin".to_string()
            } else {
                result.user.name.clone()
            };
            Ok(FlowStep::done(account))
        }
        Err(kv::KvError::Forbidden(_)) if with_password => Ok(FlowStep::failed(
            "Jellyfesh could not keep this sign-in: by chance the access token contains your password. Sign in \
             with Quick Connect instead, or change your password.",
        )),
        Err(error) => Ok(FlowStep::failed(format!(
            "Jellyfesh could not store the sign-in in your system's keystore: {}",
            kv_text(&error)
        ))),
    }
}

/// What a Quick Connect flow keeps in `flow.state` (no secrets).
#[derive(Debug, Serialize, Deserialize)]
struct Pending {
    /// The code as the server sent it.
    code: String,
    /// When the code stops working, in ms since the epoch.
    until: u64,
}

fn start_quick_connect(base: &str) -> Result<FlowStep> {
    let enabled = match api::send(
        http::Request::get(api::url("/QuickConnect/Enabled")?),
        Auth::Client,
    ) {
        Ok(response) if response.is_success() => api::json::<bool>(&response).unwrap_or(false),
        Ok(response) => {
            return Ok(FlowStep::failed(api::status_text(response.status)));
        }
        Err(error) => return Ok(FlowStep::failed(unreachable(base, &error))),
    };
    if !enabled {
        return Ok(quick_connect_off());
    }
    let response = match api::send(
        http::Request::post(api::url("/QuickConnect/Initiate")?),
        Auth::Client,
    ) {
        Ok(response) => response,
        Err(error) => return Ok(FlowStep::failed(unreachable(base, &error))),
    };
    if response.status == 401 {
        return Ok(quick_connect_off());
    }
    if !response.is_success() {
        return Ok(FlowStep::failed(api::status_text(response.status)));
    }
    let request: QuickConnect = match api::json(&response) {
        Ok(request) => request,
        Err(error) => return Ok(FlowStep::failed(crate::error_text(&error))),
    };
    if request.secret.is_empty() || request.code.is_empty() {
        return Ok(FlowStep::failed("The server sent no Quick Connect code."));
    }
    if let Err(error) = api::store(api::QUICK_CONNECT, &request.secret) {
        return Ok(FlowStep::failed(format!(
            "Jellyfesh could not keep the Quick Connect request in your system's keystore: {}",
            kv_text(&error)
        )));
    }
    let pending = Pending {
        code: request.code,
        until: clock::now_utc_ms() + CODE_LIFETIME_MS,
    };
    Ok(show_code(&pending))
}

fn quick_connect_off() -> FlowStep {
    FlowStep::failed(
        "Quick Connect is turned off on this server. An administrator can turn it on in Jellyfin's \
         dashboard (General, Quick Connect), or sign in with your user name and password.",
    )
}

fn show_code(pending: &Pending) -> FlowStep {
    FlowStep::new(Step::ShowCode(CodeDisplay {
        code: spaced(&pending.code),
        hint: "In Jellyfin, open your profile, choose Quick Connect and enter this code.".into(),
        expires_at_ms: Some(pending.until),
    }))
    .state(serde_json::to_string(pending).unwrap_or_default())
    .poll_after_ms(POLL_MS)
}

/// `482913` → `482 913`, easier to read and type.
fn spaced(code: &str) -> String {
    if code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()) {
        format!("{} {}", &code[..3], &code[3..])
    } else {
        code.to_string()
    }
}

/// `auth.poll`: has the code been approved?
pub fn poll(flow: Flow) -> Result<FlowStep> {
    if flow.kind != AuthKind::Qr {
        return Ok(FlowStep::failed("Nothing to wait for. Start again."));
    }
    let Ok(pending) = serde_json::from_str::<Pending>(&flow.state) else {
        return Ok(FlowStep::failed("The sign-in was interrupted. Start again."));
    };
    if clock::now_utc_ms() >= pending.until {
        let _ = kv::secret::delete(api::QUICK_CONNECT);
        return Ok(expired());
    }
    let Some(secret) = api::stored(api::QUICK_CONNECT)? else {
        return Ok(FlowStep::failed("The sign-in was interrupted. Start again."));
    };
    let base = api::base()?;
    let state = match api::send(
        http::Request::get(api::url("/QuickConnect/Connect")?).query(&[("secret", &secret)]),
        Auth::Client,
    ) {
        Ok(response) => response,
        // A dropped connection while waiting: keep waiting.
        Err(PluginError::Network(_)) => return Ok(show_code(&pending)),
        Err(error) => return Ok(FlowStep::failed(unreachable(&base, &error))),
    };
    match state.status {
        200..=299 => {}
        404 => {
            let _ = kv::secret::delete(api::QUICK_CONNECT);
            return Ok(expired());
        }
        401 => {
            let _ = kv::secret::delete(api::QUICK_CONNECT);
            return Ok(quick_connect_off());
        }
        status => return Ok(FlowStep::failed(api::status_text(status))),
    }
    let request: QuickConnect = api::json(&state).unwrap_or_default();
    if !request.authenticated {
        return Ok(show_code(&pending));
    }
    let response = match api::send(
        http::Request::post(api::url("/Users/AuthenticateWithQuickConnect")?)
            .json(&json!({ "Secret": secret }))?,
        Auth::Client,
    ) {
        Ok(response) => response,
        Err(error) => return Ok(FlowStep::failed(unreachable(&base, &error))),
    };
    let _ = kv::secret::delete(api::QUICK_CONNECT);
    if !response.is_success() {
        return Ok(match response.status {
            404 => expired(),
            status => FlowStep::failed(api::status_text(status)),
        });
    }
    match api::json::<AuthResult>(&response) {
        Ok(result) => finish(&result, false),
        Err(error) => Ok(FlowStep::failed(crate::error_text(&error))),
    }
}

fn expired() -> FlowStep {
    FlowStep::failed("The Quick Connect code expired. Start again for a new code.")
}

/// "The server cannot be reached" in words.
pub fn unreachable(base: &str, error: &PluginError) -> String {
    match error {
        PluginError::Network(detail) => format!("Cannot reach the server at {base}: {detail}"),
        other => crate::error_text(other),
    }
}

fn kv_text(error: &kv::KvError) -> String {
    match error {
        kv::KvError::InvalidKey(m) | kv::KvError::Forbidden(m) | kv::KvError::Unavailable(m) => m.clone(),
        kv::KvError::QuotaExceeded => "storage is full".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_spaced() {
        assert_eq!(spaced("482913"), "482 913");
        assert_eq!(spaced("48291"), "48291");
        assert_eq!(spaced("ABCDEF"), "ABCDEF");
    }

    #[test]
    fn local_addresses() {
        for local in [
            "http://192.168.1.20:8096",
            "http://localhost:8096",
            "http://jellyfin.local",
            "http://10.0.0.5",
            "http://172.20.1.1:8096/jf",
            "http://nas.home.arpa",
        ] {
            assert!(is_local(local), "{local}");
        }
        for remote in [
            "http://jellyfin.example.com",
            "http://172.40.1.1",
            "http://8.8.8.8",
        ] {
            assert!(!is_local(remote), "{remote}");
        }
    }
}

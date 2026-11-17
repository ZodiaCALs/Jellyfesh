//! Signing in with a QR code, as on the music.163.com web player.
//!
//! 1. `weapi/login/qrcode/unikey` (`type: 1`) gives a one-time key;
//! 2. the QR code is `https://music.163.com/login?codekey=<key>`, which the
//!    NetEase Cloud Music app scans;
//! 3. `weapi/login/qrcode/client/login` (`key`, `type: 1`) says how it is
//!    going: 801 waiting for a scan, 802 scanned and waiting for the person
//!    to confirm, 803 signed in (the answer sets the session cookies), 800
//!    expired;
//! 4. on 803 the plugin reads the `__csrf` cookie from the answer, asks
//!    `weapi/nuser/account/get` who signed in, and keeps the account id, its
//!    name and the csrf value in the keystore. The session cookie itself stays
//!    in the host's encrypted cookie jar.
//!
//! The key travels in the flow state; it is shown to everyone in the QR code
//! anyway, so it is not a secret.

use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::{self, Answer, Session};
use crate::model::{Account, Profile};

/// How often the host asks how the sign-in is going.
const POLL_MS: u32 = 2000;

/// What a QR flow keeps in `flow.state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pending {
    /// The one-time key in the QR code.
    key: String,
}

/// `auth.begin`.
pub fn begin(flow: Flow) -> Result<FlowStep> {
    if flow.kind != AuthKind::Qr {
        return Ok(FlowStep::failed("This way of signing in is not supported."));
    }
    let answer = match api::send_with("/login/qrcode/unikey", json!({ "type": 1 }), "") {
        Ok(answer) => answer,
        Err(error) => return Ok(FlowStep::failed(unreachable(&error))),
    };
    let key = answer
        .body
        .get("unikey")
        .and_then(|k| k.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if answer.code() != 200 || key.is_empty() {
        return Ok(FlowStep::failed(refused(&answer)));
    }
    Ok(show_qr(&Pending { key }))
}

fn show_qr(pending: &Pending) -> FlowStep {
    FlowStep::new(Step::ShowQr(QrCode {
        data: format!("https://music.163.com/login?codekey={}", pending.key),
        hint: Some(
            "Scan with the NetEase Cloud Music app on your phone (the scan button on its home screen), \
             then confirm the sign-in there."
                .into(),
        ),
        expires_at_ms: None,
    }))
    .state(serde_json::to_string(pending).unwrap_or_default())
    .poll_after_ms(POLL_MS)
}

/// `auth.poll`.
pub fn poll(flow: Flow) -> Result<FlowStep> {
    let Ok(pending) = serde_json::from_str::<Pending>(&flow.state) else {
        return Ok(FlowStep::failed("The sign-in was interrupted. Start again."));
    };
    let answer = match api::send_with(
        "/login/qrcode/client/login",
        json!({ "key": pending.key, "type": 1 }),
        "",
    ) {
        Ok(answer) => answer,
        // A dropped connection while waiting: keep waiting.
        Err(PluginError::Network(_)) => return Ok(show_qr(&pending)),
        Err(error) => return Ok(FlowStep::failed(unreachable(&error))),
    };
    match answer.code() {
        801 => Ok(show_qr(&pending)),
        802 => {
            let who = answer
                .body
                .get("nickname")
                .and_then(|n| n.as_str())
                .map(str::trim)
                .filter(|n| !n.is_empty());
            let message = match who {
                Some(name) => format!("Scanned by {name}. Confirm the sign-in on your phone."),
                None => "Scanned. Confirm the sign-in on your phone.".to_string(),
            };
            Ok(FlowStep::new(Step::Waiting(message))
                .state(serde_json::to_string(&pending).unwrap_or_default())
                .poll_after_ms(POLL_MS))
        }
        803 => finish(&answer),
        800 => Ok(FlowStep::failed(
            "The QR code expired. Start again for a new one.",
        )),
        _ => Ok(FlowStep::failed(refused(&answer))),
    }
}

/// Signed in: keep the session.
fn finish(answer: &Answer) -> Result<FlowStep> {
    let csrf = api::set_cookie(&answer.response, "__csrf").unwrap_or_default();
    if csrf.is_empty() {
        jf_plugin_sdk::warn!("the sign-in answer set no __csrf cookie; continuing without one");
    }
    let account = match api::send_with("/nuser/account/get", json!({}), &csrf) {
        Ok(account) if account.code() == 200 => account,
        Ok(account) => {
            return Ok(FlowStep::failed(format!(
                "Signed in, but NetEase Cloud Music did not say which account: {}",
                refused(&account)
            )));
        }
        Err(error) => {
            return Ok(FlowStep::failed(format!(
                "Signed in, but NetEase Cloud Music did not say which account: {}",
                unreachable(&error)
            )));
        }
    };
    let profile: Option<Profile> = account
        .body
        .get("profile")
        .filter(|p| !p.is_null())
        .and_then(|p| serde_json::from_value(p.clone()).ok());
    let account_id = account
        .body
        .get("account")
        .filter(|a| !a.is_null())
        .and_then(|a| serde_json::from_value::<Account>(a.clone()).ok())
        .map(|a| a.id)
        .unwrap_or(0);
    let user_id = profile
        .as_ref()
        .map(|p| p.user_id)
        .filter(|id| *id > 0)
        .unwrap_or(account_id);
    if user_id == 0 {
        return Ok(FlowStep::failed(
            "NetEase Cloud Music did not confirm the sign-in. Start again.",
        ));
    }
    let nickname = profile
        .and_then(|p| p.nickname)
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("NetEase user {user_id}"));
    let session = Session {
        user_id,
        nickname: nickname.clone(),
        csrf,
    };
    match api::save_session(&session) {
        Ok(()) => Ok(FlowStep::done(nickname)),
        Err(error) => Ok(FlowStep::failed(format!(
            "Jellyfesh could not keep the sign-in in your system's keystore: {}",
            crate::error_text(&error)
        ))),
    }
}

/// What NetEase said when it would not go on.
fn refused(answer: &Answer) -> String {
    match api::message_of(&answer.body) {
        Some(message) => format!(
            "NetEase Cloud Music stopped the sign-in: {message} (code {}).",
            answer.code()
        ),
        None => format!(
            "NetEase Cloud Music stopped the sign-in (code {}).",
            answer.code()
        ),
    }
}

/// "Cannot reach NetEase" in words.
fn unreachable(error: &PluginError) -> String {
    match error {
        PluginError::Network(detail) => format!("Cannot reach NetEase Cloud Music: {detail}"),
        other => crate::error_text(other),
    }
}

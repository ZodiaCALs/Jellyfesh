//! Signing in (`qr`): the person's own developer credentials first, then a
//! QR code for the NetEase Cloud Music app.
//!
//! 1. Without stored credentials, a form asks for the App ID and the RSA
//!    private key of the person's individual-developer registration. Both go
//!    to the keystore; the key is checked by signing a test string (the host
//!    signs, from the keystore) and never read back.
//! 2. A QR key (`qrcodekey/get/v2`, five minutes) shown as a QR code.
//! 3. Polls every 3 s: 801 waiting for a scan, 802 scanned (confirm in the
//!    app), 803 authorised with tokens, 800 expired. After 803 the profile is
//!    read; code 301 there means the account is not the App ID's creator.

use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::api::{self, APP_ID, Answer, PRIVATE_KEY, Session};

/// How long a QR code works, as the client asks for it.
const QR_SECONDS: u64 = 300;
/// The client's poll interval.
const POLL_MS: u32 = 3000;

/// Why NetEase answers 301 after a login, in the client's words and ours.
pub const NOT_THE_CREATOR: &str = "NetEase Cloud Music did not accept this account for this App ID: the account \
     that signs in must be the one that registered the App ID as a developer (请检查当前授权用户是否与开放平台appId创建者一致). \
     Sign in with that account, or use the App ID and key of your own registration.";

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    /// The QR key being shown, and what to show again while it waits.
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    url: String,
    #[serde(default)]
    expires_at_ms: u64,
}

fn state(flow: &Flow) -> State {
    serde_json::from_str(&flow.state).unwrap_or_default()
}

fn ask(problem: Option<&str>) -> FlowStep {
    let mut key_help = "The RSA private key from the same console, with or without its BEGIN and END lines. It \
                        goes to your system's keystore; Jellyfesh signs requests with it there and never shows it \
                        again."
        .to_owned();
    if let Some(problem) = problem {
        key_help = format!("{problem} {key_help}");
    }
    FlowStep::new(Step::AskFields(vec![
        FieldSpec {
            help: Some(
                "From your own individual developer registration on NetEase Cloud Music's open platform \
                 (developer.music.163.com). Each person registers their own; Jellyfesh has none to share."
                    .into(),
            ),
            ..FieldSpec::text("app-id", "App ID")
        },
        FieldSpec {
            help: Some(key_help),
            ..FieldSpec::secret("private-key", "Private Key")
        },
    ]))
    .state("{}")
}

fn show(state: &State) -> Result<FlowStep> {
    Ok(FlowStep::new(Step::ShowQr(QrCode {
        data: state.url.clone(),
        hint: Some("Scan with the NetEase Cloud Music app, signed in to your developer account.".into()),
        expires_at_ms: Some(state.expires_at_ms),
    }))
    .state(serde_json::to_string(state).map_err(|e| PluginError::internal(e.to_string()))?)
    .poll_after_ms(POLL_MS))
}

/// Asks NetEase for a QR key and shows it.
fn new_code() -> Result<FlowStep> {
    let data = match api::get(
        api::QR_KEY,
        &json!({ "type": 2, "expiredKey": QR_SECONDS.to_string() }),
        None,
    )? {
        Answer::Ok(data) => data,
        Answer::Expired => {
            return Ok(FlowStep::failed(
                "NetEase Cloud Music refused the request. Try again.",
            ));
        }
        Answer::Code(code, message) => {
            return Ok(FlowStep::failed(format!(
                "NetEase Cloud Music did not give a sign-in code: {}",
                if message.is_empty() {
                    format!("code {code}")
                } else {
                    message
                }
            )));
        }
    };
    let text = |k: &str| data.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let (url, key) = (text("qrCodeUrl"), text("uniKey"));
    if url.is_empty() || key.is_empty() {
        return Ok(FlowStep::failed("NetEase Cloud Music sent no sign-in code."));
    }
    show(&State {
        key: Some(key),
        url,
        expires_at_ms: clock::now_utc_ms() + QR_SECONDS * 1000,
    })
}

pub fn begin(flow: Flow) -> Result<FlowStep> {
    if flow.kind != AuthKind::Qr {
        return Err(PluginError::Unsupported);
    }
    if kv::secret::get(APP_ID)?.is_none() || kv::secret::get(PRIVATE_KEY)?.is_none() {
        return Ok(ask(None));
    }
    new_code()
}

/// An error as a sentence for a failed sign-in.
fn plain(error: PluginError) -> String {
    match error {
        PluginError::Service(message) => message,
        PluginError::RateLimited => "NetEase Cloud Music asks to slow down. Try again in a minute.".into(),
        other => format!("NetEase Cloud Music sign-in failed: {other:?}"),
    }
}

/// Overwrites a string's bytes before it is dropped (best effort: the
/// private key should not linger in the plugin's memory).
fn wipe(text: String) {
    let mut bytes = text.into_bytes();
    bytes.fill(0);
    std::hint::black_box(&bytes);
}

pub fn submit(_flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
    let mut app_id = String::new();
    let mut key = String::new();
    for field in fields {
        match field.name.as_str() {
            "app-id" => app_id = field.value.trim().to_owned(),
            "private-key" => key = field.value,
            _ => wipe(field.value),
        }
    }
    if app_id.is_empty() || app_id.len() > 128 || !app_id.bytes().all(|b| b.is_ascii_graphic()) {
        wipe(key);
        return Ok(ask(Some(
            "Enter the App ID from your developer console, without spaces.",
        )));
    }
    if key.trim().is_empty() || key.len() > 16 * 1024 {
        wipe(key);
        return Ok(ask(Some("Paste the whole private key.")));
    }
    let stored = kv::secret::set(PRIVATE_KEY, key.trim());
    wipe(key);
    stored?;
    kv::secret::set(APP_ID, &app_id)?;
    // The host must be able to sign with it.
    if let Err(error) = crypto::rsa_sign_pkcs1_sha256(PRIVATE_KEY, b"jellyfesh key check") {
        kv::secret::delete(PRIVATE_KEY)?;
        kv::secret::delete(APP_ID)?;
        let why = match error {
            crypto::CryptoError::InvalidKey(why) | crypto::CryptoError::InvalidInput(why) => why,
            other => format!("{other:?}"),
        };
        return Ok(ask(Some(&format!(
            "Jellyfesh cannot use that private key: {why}."
        ))));
    }
    new_code()
}

pub fn poll(flow: Flow) -> Result<FlowStep> {
    let state = state(&flow);
    let Some(key) = state.key.clone() else {
        return Ok(FlowStep::failed("The sign-in lost its code. Start again."));
    };
    let app_id = kv::secret::get(APP_ID)?.ok_or(PluginError::AuthRequired)?;
    let data = match api::get(api::QR_POLL, &json!({ "key": key, "clientId": app_id }), None)? {
        Answer::Ok(data) => data,
        Answer::Expired => {
            return Ok(FlowStep::failed(
                "NetEase Cloud Music refused the request. Start again.",
            ));
        }
        Answer::Code(code, message) => return Ok(FlowStep::failed(plain(api::code_error(code, &message)))),
    };
    match data.get("status").and_then(Value::as_i64) {
        Some(801) => show(&state),
        Some(802) => Ok(FlowStep::new(Step::Waiting(
            "Scanned. Confirm the sign-in in the NetEase Cloud Music app.".into(),
        ))
        .state(flow.state.clone())
        .poll_after_ms(POLL_MS)),
        Some(803) => {
            let Some(session) = Session::from_data(&data, None) else {
                return Ok(FlowStep::failed(
                    "NetEase Cloud Music signed in without a token. Start again.",
                ));
            };
            session.store()?;
            // Only the App ID's creator may sign in with it (code 301).
            match api::get(api::PROFILE, &json!({}), Some(&session.access_token))? {
                Answer::Ok(profile) => {
                    let name = ["nickname", "nickName", "userName"]
                        .iter()
                        .find_map(|k| profile.get(*k).and_then(Value::as_str))
                        .filter(|n| !n.trim().is_empty())
                        .unwrap_or("NetEase Cloud Music account");
                    Ok(FlowStep::done(name))
                }
                Answer::Code(301, _) => {
                    kv::secret::delete(api::SESSION)?;
                    Ok(FlowStep::failed(NOT_THE_CREATOR))
                }
                Answer::Code(code, message) => {
                    kv::secret::delete(api::SESSION)?;
                    Ok(FlowStep::failed(plain(api::code_error(code, &message))))
                }
                Answer::Expired => {
                    kv::secret::delete(api::SESSION)?;
                    Ok(FlowStep::failed(
                        "NetEase Cloud Music refused the new sign-in. Start again.",
                    ))
                }
            }
        }
        Some(800) => Ok(FlowStep::failed("The QR code expired. Start again.")),
        other => Ok(FlowStep::failed(format!(
            "NetEase Cloud Music answered an unknown sign-in status ({}).",
            other.map_or("none".to_owned(), |s| s.to_string())
        ))),
    }
}

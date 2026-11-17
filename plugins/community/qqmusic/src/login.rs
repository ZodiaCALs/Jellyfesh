//! Signing in with a WeChat QR code, as on the y.qq.com web player.
//!
//! 1. `GET https://open.weixin.qq.com/connect/qrconnect` with QQ Music's
//!    web app id (`wx48db31d50e334801`), its web page as `redirect_uri`,
//!    `response_type=code` and `scope=snsapi_login`: WeChat's web sign-in
//!    page, which names a one-time `uuid` (its QR picture is
//!    `/connect/qrcode/<uuid>`);
//! 2. the QR code is `https://open.weixin.qq.com/connect/confirm?uuid=<uuid>`,
//!    which the person scans with WeChat;
//! 3. `GET https://lp.open.weixin.qq.com/connect/l/qrconnect?uuid=<uuid>`
//!    waits for news and answers `window.wx_errcode=…;window.wx_code='…';`:
//!    408 still waiting, 404 scanned, 405 confirmed (with the code), 403
//!    cancelled, 402 expired;
//! 4. QQ Music turns the code into a session: `music.login.LoginServer`
//!    `Login` with `code` and `strAppid` (`tmeLoginType` 1, WeChat) answers
//!    the account's `musicid`, `musickey` and what renews the key. They go to
//!    the keystore.
//!
//! The uuid travels in the flow state; it is in the QR code anyway.
//!
//! QQ's own QR sign-in (ptlogin) is not offered: it hands out the QR code
//! only as a picture, and Jellyfesh draws QR codes itself from their text;
//! reading the picture would take a PNG and a QR decoder in the plugin.

use jf_plugin_sdk::http::HttpError;
use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::api::{self, Session};

/// QQ Music's app id on WeChat's web sign-in (as on y.qq.com's sign-in page).
pub const WECHAT_APP_ID: &str = "wx48db31d50e334801";
/// Where WeChat would send the browser back to (the web player's page).
const REDIRECT_URI: &str = "https://y.qq.com/portal/wx_redirect.html?login_type=2&surl=https://y.qq.com/";
/// `tmeLoginType` for WeChat.
const WECHAT: u32 = 1;
/// How long a QR code is shown before the plugin gives up on it.
const QR_LIFETIME_MS: u64 = 5 * 60 * 1000;
/// How long one long poll may wait (the sign-in step's own limit is 30 s).
const LONG_POLL_MS: u32 = 20_000;
/// Pause between polls (the long poll does the waiting).
const POLL_MS: u32 = 1000;

/// What a flow keeps in `flow.state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pending {
    uuid: String,
    /// The last status WeChat reported, sent back so it answers changes only.
    last: Option<u32>,
    /// When the QR code was shown, ms since the epoch.
    started: u64,
}

/// The uuid in WeChat's sign-in page.
pub fn uuid_from_page(html: &str) -> Option<String> {
    let token = |rest: &str| -> String {
        rest.chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect()
    };
    if let Some(at) = html.find("/connect/qrcode/") {
        let found = token(&html[at + "/connect/qrcode/".len()..]);
        if found.len() >= 8 {
            return Some(found);
        }
    }
    let mut rest = html;
    while let Some(at) = rest.find("uuid") {
        rest = &rest[at + 4..];
        let value = rest.trim_start_matches(['"', '\'', ' ', '=', ':']);
        let found = token(value);
        if found.len() >= 8 {
            return Some(found);
        }
    }
    None
}

/// The status and code in a long-poll answer.
pub fn poll_answer(body: &str) -> Option<(u32, String)> {
    let at = body.find("wx_errcode=")? + "wx_errcode=".len();
    let status: u32 = body[at..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()?;
    let code = body
        .find("wx_code='")
        .map(|at| {
            body[at + "wx_code='".len()..]
                .chars()
                .take_while(|c| *c != '\'')
                .collect::<String>()
        })
        .unwrap_or_default();
    Some((status, code))
}

/// `auth.begin`.
pub fn begin(flow: Flow) -> Result<FlowStep> {
    if flow.kind != AuthKind::Qr {
        return Ok(FlowStep::failed("This way of signing in is not supported."));
    }
    let url = format!(
        "https://open.weixin.qq.com/connect/qrconnect?appid={WECHAT_APP_ID}&redirect_uri={}\
         &response_type=code&scope=snsapi_login&state=STATE",
        http::encode(REDIRECT_URI)
    );
    let page = match http::Request::get(api::physical(&url))
        .header("referer", "https://y.qq.com/")
        .header("user-agent", api::USER_AGENT)
        .timeout_ms(15_000)
        .send()
    {
        Ok(page) if page.is_success() => page.text(),
        Ok(page) => {
            return Ok(FlowStep::failed(format!(
                "WeChat's sign-in page did not open (HTTP {}).",
                page.status
            )));
        }
        Err(error) => return Ok(FlowStep::failed(unreachable("WeChat", &error.into()))),
    };
    let Some(uuid) = uuid_from_page(&page) else {
        return Ok(FlowStep::failed(
            "WeChat's sign-in page has changed; this plugin cannot find its QR code.",
        ));
    };
    Ok(show_qr(&Pending {
        uuid,
        last: None,
        started: clock::now_utc_ms(),
    }))
}

fn state(pending: &Pending) -> String {
    serde_json::to_string(pending).unwrap_or_default()
}

fn show_qr(pending: &Pending) -> FlowStep {
    FlowStep::new(Step::ShowQr(QrCode {
        data: format!("https://open.weixin.qq.com/connect/confirm?uuid={}", pending.uuid),
        hint: Some(
            "Scan with WeChat on your phone, then confirm the sign-in to QQ Music there. \
             (Your WeChat account must be linked to QQ Music.)"
                .into(),
        ),
        expires_at_ms: Some(pending.started + QR_LIFETIME_MS),
    }))
    .state(state(pending))
    .poll_after_ms(POLL_MS)
}

/// `auth.poll`.
pub fn poll(flow: Flow) -> Result<FlowStep> {
    let Ok(mut pending) = serde_json::from_str::<Pending>(&flow.state) else {
        return Ok(FlowStep::failed("The sign-in was interrupted. Start again."));
    };
    if clock::now_utc_ms() >= pending.started + QR_LIFETIME_MS {
        return Ok(expired());
    }
    let mut url = format!(
        "https://lp.open.weixin.qq.com/connect/l/qrconnect?uuid={}&_={}",
        http::encode(&pending.uuid),
        clock::now_utc_ms()
    );
    if let Some(last) = pending.last {
        url.push_str(&format!("&last={last}"));
    }
    let body = match http::Request::get(api::physical(&url))
        .header("referer", "https://open.weixin.qq.com/")
        .header("user-agent", api::USER_AGENT)
        .timeout_ms(LONG_POLL_MS)
        .send()
    {
        Ok(response) => response.text(),
        // Nothing happened while the long poll waited, or the connection
        // dropped: keep waiting.
        Err(HttpError::Timeout | HttpError::Connection(_)) => return Ok(waiting_step(&pending)),
        Err(error) => return Ok(FlowStep::failed(unreachable("WeChat", &error.into()))),
    };
    let Some((status, code)) = poll_answer(&body) else {
        return Ok(waiting_step(&pending));
    };
    match status {
        408 => Ok(waiting_step(&pending)),
        404 => {
            pending.last = Some(404);
            Ok(FlowStep::new(Step::Waiting(
                "Scanned. Confirm the sign-in in WeChat on your phone.".into(),
            ))
            .state(state(&pending))
            .poll_after_ms(POLL_MS))
        }
        405 if !code.is_empty() => finish(&code),
        403 => Ok(FlowStep::failed("The sign-in was cancelled in WeChat.")),
        402 | 500 => Ok(expired()),
        other => Ok(FlowStep::failed(format!(
            "WeChat stopped the sign-in (status {other})."
        ))),
    }
}

/// The QR code again, or the "scanned" message once WeChat reported a scan.
fn waiting_step(pending: &Pending) -> FlowStep {
    if pending.last == Some(404) {
        FlowStep::new(Step::Waiting(
            "Scanned. Confirm the sign-in in WeChat on your phone.".into(),
        ))
        .state(state(pending))
        .poll_after_ms(POLL_MS)
    } else {
        show_qr(pending)
    }
}

fn expired() -> FlowStep {
    FlowStep::failed("The QR code expired. Start again for a new one.")
}

/// Turns WeChat's code into a QQ Music session.
fn finish(code: &str) -> Result<FlowStep> {
    let mut comm = api::comm(None);
    comm["tmeAppID"] = json!("qqmusic");
    comm["tmeLoginType"] = json!(WECHAT);
    let data = match api::call_with_comm(
        comm,
        None,
        "music.login.LoginServer",
        "Login",
        json!({ "code": code, "strAppid": WECHAT_APP_ID }),
    ) {
        Ok(data) => data,
        Err(PluginError::AuthRequired) => {
            return Ok(FlowStep::failed(
                "QQ Music did not accept this WeChat account. Link it to QQ Music in the QQ Music app first.",
            ));
        }
        Err(error) => return Ok(FlowStep::failed(unreachable("QQ Music", &error))),
    };
    let Some(mut session) = Session::from_login(&data, "", WECHAT, clock::now_utc_ms()) else {
        return Ok(FlowStep::failed(
            "QQ Music did not confirm the sign-in. Start again.",
        ));
    };
    session.nickname = nickname(&data, &session);
    match api::save_session(&session) {
        Ok(()) => Ok(FlowStep::done(session.nickname.clone())),
        Err(error) => Ok(FlowStep::failed(format!(
            "Jellyfesh could not keep the sign-in in your system's keystore: {}",
            crate::error_text(&error)
        ))),
    }
}

/// The account's display name: from the sign-in answer, else from the
/// account's details, else its number.
fn nickname(login: &Value, session: &Session) -> String {
    let from = |value: &Value| {
        ["nick", "nickname", "nickName"]
            .iter()
            .find_map(|k| value.get(*k).and_then(Value::as_str))
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
    };
    if let Some(name) = from(login) {
        return name;
    }
    let details = api::call_as(
        Some(session),
        "music.UserInfo.userInfoServer",
        "GetLoginUserInfo",
        json!({}),
    );
    match details {
        Ok(data) => from(&data)
            .or_else(|| data.get("info").and_then(from))
            .unwrap_or_else(|| format!("QQ Music user {}", session.musicid)),
        Err(_) => format!("QQ Music user {}", session.musicid),
    }
}

/// "Cannot reach …" in words.
fn unreachable(service: &str, error: &PluginError) -> String {
    match error {
        PluginError::Network(detail) => format!("Cannot reach {service}: {detail}"),
        other => crate::error_text(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_uuid() {
        let page = "<div><img class=\"qrcode lightBorder\" src=\"/connect/qrcode/091MockUuid_a-1\" /></div>";
        assert_eq!(uuid_from_page(page).as_deref(), Some("091MockUuid_a-1"));
        let script = "<script>var x = 1; window.uuid = \"071MockUuid2\";</script>";
        assert_eq!(uuid_from_page(script).as_deref(), Some("071MockUuid2"));
        assert_eq!(uuid_from_page("<html>nothing</html>"), None);
        assert_eq!(uuid_from_page("/connect/qrcode/short"), None);
    }

    #[test]
    fn reads_long_poll_answers() {
        assert_eq!(
            poll_answer("window.wx_errcode=408;window.wx_code='';"),
            Some((408, String::new()))
        );
        assert_eq!(
            poll_answer("window.wx_errcode=405;window.wx_code='081MockCode';"),
            Some((405, "081MockCode".into()))
        );
        assert_eq!(poll_answer("window.wx_errcode=;"), None);
        assert_eq!(poll_answer("<html>"), None);
    }
}

//! NetEase Cloud Music's open platform as its official client (ncm-cli
//! 0.1.7) speaks it, per the research for ADR-0018 (the platform's own
//! documentation was not reachable):
//!
//! * every protocol field travels in the query string: `appId`,
//!   `signType=RSA_SHA256`, `timestamp` (ms), `device` (JSON), `bizContent`
//!   (JSON of the business parameters), `accessToken` when signed in, and
//!   `sign`;
//! * `sign` is base64 of an RSASSA-PKCS1-v1_5 SHA-256 signature, with the
//!   person's own private key, over the other non-empty parameters sorted by
//!   name and joined as `k=v` with `&`, without URL encoding;
//! * answers are `{code, message, data}`: 200 is success, 1406 an expired
//!   access token, -461 the daily play limit, -444/-446/-447 "too fast".
//!
//! The key never enters the plugin's memory after it is stored: the host
//! signs with it from the keystore (`crypto.rsa-sign-pkcs1-sha256`). The
//! plugin calls no IP lookup service and reports no plays.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The open platform's API.
pub const BASE: &str = "https://openncm.music.163.com";
/// The person's App ID.
pub const APP_ID: &str = "app-id";
/// The person's RSA private key (PEM, or base64 as the console shows it).
pub const PRIVATE_KEY: &str = "private-key";
/// The access and refresh tokens of the signed-in account.
pub const SESSION: &str = "session";
/// A random id for this installation, sent in `device`.
const DEVICE_ID: &str = "device-id";

pub const QR_KEY: &str = "/openapi/music/basic/user/oauth2/qrcodekey/get/v2";
pub const QR_POLL: &str = "/openapi/music/basic/oauth2/device/login/qrcode/get";
pub const PROFILE: &str = "/openapi/music/basic/user/profile/get/v2";
const REFRESH: &str = "/openapi/music/basic/user/oauth2/token/refresh/v2";

/// Shown for the daily play limit when NetEase gives no text.
const DAILY_LIMIT: &str = "NetEase Cloud Music's daily play limit is reached; try again tomorrow.";

thread_local! {
    static BASE_URL: RefCell<String> = RefCell::new(BASE.to_owned());
}

/// Uses `server` instead of the open platform (tests), or the platform for `None`.
pub fn set_server(server: Option<&str>) {
    let base = server.map_or(BASE, |s| s.trim_end_matches('/')).to_owned();
    BASE_URL.with(|b| *b.borrow_mut() = base);
}

fn base() -> String {
    BASE_URL.with(|b| b.borrow().clone())
}

/// Base64 with padding (RFC 4648, section 4).
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> shift) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The string the signature covers: non-empty parameters but `sign`, sorted
/// by name (the names are ASCII, so byte order is the client's
/// `localeCompare` order), as `k=v` joined with `&`, not URL-encoded.
pub fn canonical(params: &[(&str, String)]) -> String {
    let mut sorted: Vec<&(&str, String)> = params
        .iter()
        .filter(|(k, v)| *k != "sign" && !v.is_empty())
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    sorted
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// `device`: what the platform asks a client to say about itself. Jellyfesh
/// names itself, sends a random installation id, and a fixed loopback
/// address instead of looking up the person's public IP.
fn device() -> Result<String> {
    let id = match kv::get(DEVICE_ID)? {
        Some(id) if !id.is_empty() => String::from_utf8_lossy(&id).into_owned(),
        _ => {
            let id = crypto::random_hex(16)?;
            kv::set(DEVICE_ID, id.as_bytes())?;
            id
        }
    };
    Ok(json!({
        "deviceType": "openapi",
        "os": "jellyfesh",
        "appVer": env!("CARGO_PKG_VERSION"),
        "channel": "jellyfesh",
        "model": "jellyfesh",
        "brand": "jellyfesh",
        "osVer": "",
        "clientIp": "127.0.0.1",
        "deviceId": id,
    })
    .to_string())
}

/// The account's tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
}

impl Session {
    /// The tokens in a login or refresh answer: `data.accessToken` is an
    /// object `{accessToken, refreshToken, expireTime}` after the QR login;
    /// the refresh answer's shape is not documented, so a plain string with
    /// `data.refreshToken` beside it is read too.
    pub fn from_data(data: &Value, previous_refresh: Option<&str>) -> Option<Session> {
        let text = |v: Option<&Value>| {
            v.and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let (access, refresh) = match data.get("accessToken") {
            Some(object @ Value::Object(_)) => {
                (text(object.get("accessToken")), text(object.get("refreshToken")))
            }
            other => (text(other), text(data.get("refreshToken"))),
        };
        Some(Session {
            access_token: access?,
            refresh_token: refresh
                .or_else(|| previous_refresh.map(str::to_owned))
                .unwrap_or_default(),
        })
    }

    pub fn load() -> Result<Option<Session>> {
        Ok(kv::secret::get(SESSION)?.and_then(|text| serde_json::from_str(&text).ok()))
    }

    pub fn store(&self) -> Result<()> {
        kv::secret::set(
            SESSION,
            &serde_json::to_string(self).map_err(|e| PluginError::internal(e.to_string()))?,
        )
    }
}

/// An answer of the platform: `code` and `data`, or what went wrong.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Ok(Value),
    /// 1406: the access token expired.
    Expired,
    /// Any other code, with NetEase's message.
    Code(i64, String),
}

/// Reads an answer body (HTTP errors are mapped before).
pub fn answer(status: u16, body: &[u8]) -> Result<Answer> {
    match status {
        200..=299 => {}
        429 => return Err(PluginError::RateLimited),
        status => {
            return Err(PluginError::service(format!(
                "NetEase Cloud Music answered HTTP {status}"
            )));
        }
    }
    let value: Value = serde_json::from_slice(body)?;
    let code = value.get("code").and_then(Value::as_i64).unwrap_or(-1);
    let message = ["message", "msg"]
        .iter()
        .find_map(|k| value.get(*k).and_then(Value::as_str))
        .unwrap_or_default()
        .trim()
        .to_owned();
    Ok(match code {
        200 => Answer::Ok(value.get("data").cloned().unwrap_or(Value::Null)),
        1406 => Answer::Expired,
        code => Answer::Code(code, message),
    })
}

/// A code other than success as a Jellyfesh error.
pub fn code_error(code: i64, message: &str) -> PluginError {
    match code {
        -461 => PluginError::service(if message.is_empty() { DAILY_LIMIT } else { message }),
        -444 | -446 | -447 => PluginError::RateLimited,
        _ if message.is_empty() => PluginError::service(format!("NetEase Cloud Music answered code {code}")),
        _ => PluginError::service(message),
    }
}

fn sign_error(error: crypto::CryptoError) -> PluginError {
    PluginError::service(format!(
        "Jellyfesh cannot sign with your private key ({error:?}). Sign out and sign in again with the \
         key from your NetEase open-platform console."
    ))
}

/// Signs `data` with the stored key (the host does it from the keystore).
pub fn sign(data: &[u8]) -> Result<Vec<u8>> {
    crypto::rsa_sign_pkcs1_sha256(PRIVATE_KEY, data).map_err(sign_error)
}

/// The signed URL of a request to `path` with `biz` as business parameters.
pub fn signed_url(path: &str, biz: &Value, token: Option<&str>) -> Result<String> {
    let app_id = kv::secret::get(APP_ID)?.ok_or(PluginError::AuthRequired)?;
    let mut params: Vec<(&str, String)> = vec![
        ("appId", app_id),
        ("bizContent", biz.to_string()),
        ("device", device()?),
        ("signType", "RSA_SHA256".to_owned()),
        ("timestamp", clock::now_utc_ms().to_string()),
    ];
    if let Some(token) = token {
        params.push(("accessToken", token.to_owned()));
    }
    let signature = sign(canonical(&params).as_bytes())?;
    params.push(("sign", base64(&signature)));
    let pairs: Vec<(&str, &str)> = params
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| (*k, v.as_str()))
        .collect();
    Ok(format!("{}{path}?{}", base(), http::form_encode(&pairs)))
}

/// One signed GET.
pub fn get(path: &str, biz: &Value, token: Option<&str>) -> Result<Answer> {
    let response = http::Request::get(signed_url(path, biz, token)?)
        .header("referer", "https://music.163.com/")
        .send()?;
    answer(response.status, &response.body)
}

/// Exchanges the refresh token for new tokens; `AuthRequired` (and the
/// session forgotten) when NetEase refuses.
fn refresh(session: &Session) -> Result<Session> {
    if session.refresh_token.is_empty() {
        kv::secret::delete(SESSION)?;
        return Err(PluginError::AuthRequired);
    }
    let app_id = kv::secret::get(APP_ID)?.ok_or(PluginError::AuthRequired)?;
    let biz = json!({ "clientId": app_id, "clientSecret": "", "refreshToken": session.refresh_token });
    match get(REFRESH, &biz, None)? {
        Answer::Ok(data) => {
            let fresh = Session::from_data(&data, Some(&session.refresh_token)).ok_or_else(|| {
                PluginError::service("NetEase Cloud Music renewed the sign-in without a token.")
            })?;
            fresh.store()?;
            Ok(fresh)
        }
        Answer::Code(code @ (-461 | -444 | -446 | -447), message) => Err(code_error(code, &message)),
        _ => {
            kv::secret::delete(SESSION)?;
            Err(PluginError::AuthRequired)
        }
    }
}

/// A signed GET as the signed-in account, renewing the token once on 1406.
pub fn call(path: &str, biz: &Value) -> Result<Value> {
    let session = Session::load()?.ok_or(PluginError::AuthRequired)?;
    match get(path, biz, Some(&session.access_token))? {
        Answer::Ok(data) => Ok(data),
        Answer::Code(code, message) => Err(code_error(code, &message)),
        Answer::Expired => {
            let fresh = refresh(&session)?;
            match get(path, biz, Some(&fresh.access_token))? {
                Answer::Ok(data) => Ok(data),
                Answer::Expired => Err(PluginError::AuthRequired),
                Answer::Code(code, message) => Err(code_error(code, &message)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        for (input, output) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), output, "{input}");
        }
        assert_eq!(base64(&[0xfb, 0xff]), "+/8=");
    }

    #[test]
    fn the_canonical_string_is_sorted_unencoded_and_skips_empty_values() {
        let params = [
            ("timestamp", "1700000000000".to_owned()),
            ("bizContent", r#"{"limit":20,"name":"a b&c"}"#.to_owned()),
            ("appId", "a1".to_owned()),
            ("accessToken", String::new()),
            ("signType", "RSA_SHA256".to_owned()),
            ("sign", "ignored".to_owned()),
            ("device", r#"{"os":"jellyfesh"}"#.to_owned()),
        ];
        assert_eq!(
            canonical(&params),
            r#"appId=a1&bizContent={"limit":20,"name":"a b&c"}&device={"os":"jellyfesh"}&signType=RSA_SHA256&timestamp=1700000000000"#
        );
    }

    #[test]
    fn answers_and_codes() {
        assert_eq!(
            answer(200, br#"{"code":200,"data":{"a":1}}"#).unwrap(),
            Answer::Ok(json!({ "a": 1 }))
        );
        assert_eq!(
            answer(200, br#"{"code":1406,"message":"expired"}"#).unwrap(),
            Answer::Expired
        );
        assert_eq!(
            answer(
                200,
                r#"{"code":-461,"message":"今日播放次数已达上限"}"#.as_bytes()
            )
            .unwrap(),
            Answer::Code(-461, "今日播放次数已达上限".into())
        );
        assert_eq!(answer(429, b"").unwrap_err(), PluginError::RateLimited);
        assert!(matches!(answer(502, b"").unwrap_err(), PluginError::Service(_)));
        assert_eq!(code_error(-461, ""), PluginError::service(DAILY_LIMIT));
        assert_eq!(code_error(-446, "too fast"), PluginError::RateLimited);
        assert_eq!(
            code_error(500, "请求总量超限"),
            PluginError::service("请求总量超限")
        );
    }

    #[test]
    fn sessions_come_from_either_token_shape() {
        let login = json!({ "status": 803, "accessToken": { "accessToken": "at", "refreshToken": "rt", "expireTime": 1 } });
        let s = Session::from_data(&login, None).unwrap();
        assert_eq!((s.access_token.as_str(), s.refresh_token.as_str()), ("at", "rt"));
        let flat = json!({ "accessToken": "at2" });
        assert_eq!(Session::from_data(&flat, Some("rt")).unwrap().refresh_token, "rt");
        assert!(Session::from_data(&json!({}), None).is_none());
    }
}

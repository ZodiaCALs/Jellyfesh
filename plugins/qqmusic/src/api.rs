//! QQ Music's open API as `qqmusic-skills` 0.0.3 documents it: every call is
//! `POST <base><path>` with `Authorization: Bearer <key>` and the JSON body
//! `{"params": {…}, "comm": {"skill_version": "0.0.3"}}`. Business parameters
//! must sit inside `params` (the server ignores them elsewhere). Answers carry
//! `ret` (gateway) and `sub_ret` (business) error codes, non-zero on errors,
//! with `msg`, and their fields flat beside them.

use jf_plugin_sdk::prelude::*;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// The API's gateway.
pub const BASE: &str = "https://a.y.qq.com";
/// The protocol version this plugin speaks, sent with every request. A plugin
/// release changes it, never the plugin itself.
pub const SKILL_VERSION: &str = "0.0.3";
/// The secret holding the person's `qmk-` key.
pub const KEY_SECRET: &str = "api-key";

/// How a call failed, before it becomes a [`PluginError`].
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// HTTP 401 or 403: the key is missing, wrong or revoked.
    Key,
    /// HTTP 429.
    RateLimited,
    /// The request did not get an answer.
    Network(String),
    /// QQ Music answered with an error; the text is its own where it gave one.
    Service(String),
}

impl From<Failure> for PluginError {
    fn from(failure: Failure) -> PluginError {
        match failure {
            Failure::Key => PluginError::AuthRequired,
            Failure::RateLimited => PluginError::RateLimited,
            Failure::Network(text) => PluginError::Network(text),
            Failure::Service(text) => PluginError::Service(text),
        }
    }
}

/// The request body for `params`.
pub fn body(params: Value) -> Value {
    json!({ "params": params, "comm": { "skill_version": SKILL_VERSION } })
}

/// Calls `path` with `params` as the person whose key is `key`.
pub fn call_with<T: DeserializeOwned>(
    base: &str,
    key: &str,
    path: &str,
    params: Value,
) -> Result<T, Failure> {
    let request = http::Request::post(format!("{}{path}", base.trim_end_matches('/')))
        .bearer(key)
        .json(&body(params))
        .map_err(|e| Failure::Service(format!("{e:?}")))?;
    let response = request.send().map_err(|e| match PluginError::from(e) {
        PluginError::RateLimited => Failure::RateLimited,
        PluginError::Network(text) => Failure::Network(text),
        other => Failure::Network(format!("{other:?}")),
    })?;
    let value = check(response.status, &response.body)?;
    serde_json::from_value(value)
        .map_err(|e| Failure::Service(format!("QQ Music answered something unexpected: {e}")))
}

/// Checks the HTTP status and the `ret`/`sub_ret` codes of an answer.
pub fn check(status: u16, body: &[u8]) -> Result<Value, Failure> {
    match status {
        200..=299 => {}
        401 | 403 => return Err(Failure::Key),
        429 => return Err(Failure::RateLimited),
        _ => {}
    }
    let value: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) if !(200..=299).contains(&status) => {
            return Err(Failure::Service(format!("QQ Music answered HTTP {status}")));
        }
        Err(e) => {
            return Err(Failure::Service(format!(
                "QQ Music answered something unexpected: {e}"
            )));
        }
    };
    let code = |name: &str| value.get(name).and_then(Value::as_i64).unwrap_or(0);
    let (ret, sub_ret) = (code("ret"), code("sub_ret"));
    if ret != 0 || sub_ret != 0 || !(200..=299).contains(&status) {
        let message = value
            .get("msg")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if (200..=299).contains(&status) {
                    format!("QQ Music answered error {ret}/{sub_ret}")
                } else {
                    format!("QQ Music answered HTTP {status}")
                }
            });
        return Err(Failure::Service(message));
    }
    Ok(value)
}

/// Whether `key` looks like a QQ Music key: `qmk-` and then letters, digits
/// and `-` or `_` only (no spaces, nothing pasted around it).
pub fn plausible_key(key: &str) -> bool {
    key.len() > "qmk-".len()
        && key.len() <= 256
        && key.starts_with("qmk-")
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_nest_params_and_carry_the_skill_version() {
        let b = body(json!({ "keyword": "晴天", "type": "0", "page": 2 }));
        assert_eq!(b["params"]["keyword"], "晴天");
        assert_eq!(b["params"]["page"], 2);
        assert_eq!(b["comm"]["skill_version"], "0.0.3");
        assert!(b.get("keyword").is_none(), "nothing at the top level");
    }

    #[test]
    fn answers_are_checked() {
        assert_eq!(
            check(200, br#"{"ret":0,"sub_ret":0,"songs":[]}"#).unwrap()["songs"],
            json!([])
        );
        assert_eq!(
            check(200, br#"{"traceId":"x","songs":null}"#).unwrap()["songs"],
            Value::Null
        );
        assert_eq!(
            check(200, br#"{"ret":40001,"sub_ret":0,"msg":"key invalid"}"#),
            Err(Failure::Service("key invalid".into()))
        );
        assert_eq!(
            check(200, br#"{"ret":0,"sub_ret":7,"msg":" "}"#),
            Err(Failure::Service("QQ Music answered error 0/7".into()))
        );
        assert_eq!(check(401, b"{}"), Err(Failure::Key));
        assert_eq!(check(403, b"denied"), Err(Failure::Key));
        assert_eq!(check(429, b""), Err(Failure::RateLimited));
        assert_eq!(
            check(502, b"<html>bad gateway</html>"),
            Err(Failure::Service("QQ Music answered HTTP 502".into()))
        );
        assert!(matches!(check(200, b"not json"), Err(Failure::Service(_))));
    }

    #[test]
    fn keys_look_like_qmk_keys() {
        assert!(plausible_key("qmk-1a2B3c_d-4"));
        for bad in ["", "qmk-", "1a2b3c", "qmk-1a2b 3c", "Bearer qmk-1", "qmk-ä"] {
            assert!(!plausible_key(bad), "{bad:?}");
        }
    }
}

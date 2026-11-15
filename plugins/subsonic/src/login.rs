//! Signing in (ADR-0015).
//!
//! * **API key** (OpenSubsonic `apiKeyAuthentication`): the key is checked
//!   with `ping`, the account name read with `tokenInfo`, and the key kept in
//!   the keystore. Offered only when the server lists the extension.
//! * **User name and password**: the password is turned into one salt and
//!   `md5(password + salt)` pair, checked with `ping`, and only the pair is
//!   kept in the keystore. It works like a password for this one server.
//!   When the server offers API keys, the plugin asks for one instead: the
//!   Subsonic API has no call that turns a password into a key.
//!
//! Either way the sign-in is tied to the server address it was made for.

use jf_plugin_sdk::bindings::jellyfesh::plugin::kv::KvError;
use jf_plugin_sdk::prelude::*;

use crate::api::{self, ApiError, Auth, Creds, Failure};
use crate::model::TokenInfo;

const API_KEYS: &str = "apiKeyAuthentication";

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
    let info = match api::discover() {
        Ok(info) => info,
        Err(error) => return Ok(FlowStep::failed(unreachable(&base, &error))),
    };
    Ok(match flow.kind {
        AuthKind::PasswordExchange => {
            if info.has(API_KEYS) {
                FlowStep::failed(format!(
                    "{} supports API keys, which are safer than signing in with your password. \
                     Create an API key in the server's settings, then sign in with \u{201c}Enter API Key\u{201d}.",
                    info.describe()
                ))
            } else if info.api_at_least(1, 13) == Some(false) {
                FlowStep::failed(format!(
                    "{} speaks Subsonic API {}; Jellyfesh needs 1.13.0 or newer to sign in without \
                     keeping your password. Update the server.",
                    info.describe(),
                    info.api_version.as_deref().unwrap_or("?")
                ))
            } else {
                let mut user = FieldSpec::text("username", "User Name");
                user.placeholder = Some("Your user name on the server".into());
                let mut password = FieldSpec::secret("password", "Password");
                password.help = Some(format!(
                    "Jellyfesh keeps a token made from your password, not the password. The token \
                     works like a password for this server only, so it stays in your system's keystore.{}",
                    http_warning(&base)
                ));
                FlowStep::new(Step::AskFields(vec![user, password]))
            }
        }
        AuthKind::ApiKey => {
            if info.has(API_KEYS) {
                let mut key = FieldSpec::secret("api-key", "API Key");
                key.help = Some(format!(
                    "Create one in your server's settings. Jellyfesh keeps it in your system's keystore.{}",
                    http_warning(&base)
                ));
                FlowStep::new(Step::AskFields(vec![key]))
            } else {
                FlowStep::failed(format!(
                    "{} does not offer API keys (the OpenSubsonic \u{201c}apiKeyAuthentication\u{201d} \
                     extension). Sign in with your user name and password instead.",
                    info.describe()
                ))
            }
        }
        _ => FlowStep::failed("This way of signing in is not supported."),
    })
}

/// A sentence for the sign-in form when the address uses http away from the
/// local network, where the sign-in would travel unencrypted (the
/// OpenSubsonic API asks clients to warn about it).
fn http_warning(base: &str) -> &'static str {
    if base.to_ascii_lowercase().starts_with("http://") && !is_local(base) {
        " This address uses http, so the sign-in travels unencrypted; use https if the server offers it."
    } else {
        ""
    }
}

/// Whether the address is on this device or the local network.
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

/// `auth.submit`.
pub fn submit(flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
    let field = |name: &str| {
        fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.clone())
            .unwrap_or_default()
    };
    let base = api::base()?;
    match flow.kind {
        AuthKind::PasswordExchange => password(&base, field("username").trim(), &field("password")),
        AuthKind::ApiKey => api_key(&base, field("api-key").trim()),
        _ => Ok(FlowStep::failed("This way of signing in is not supported.")),
    }
}

fn password(base: &str, user: &str, password: &str) -> Result<FlowStep> {
    if user.is_empty() {
        return Ok(FlowStep::failed("Enter your user name."));
    }
    // A fresh salt per attempt; a password made only of hex digits could, by
    // chance, appear inside the salt or token, which the host refuses to store.
    for _ in 0..8 {
        let salt = crypto::random_hex(8)?;
        let token = crypto::md5_hex(&format!("{password}{salt}"))?;
        if !password.is_empty() && (salt.contains(password) || token.contains(password)) {
            continue;
        }
        let creds = Creds::Token {
            user: user.to_string(),
            salt,
            token,
        };
        match api::request("ping", &[], Auth::With(&creds)) {
            Ok(_) => {}
            Err(failure) => return Ok(FlowStep::failed(refused(base, failure, false))),
        }
        return match api::save_creds(&creds) {
            Ok(()) => Ok(FlowStep::done(user)),
            Err(KvError::Forbidden(_)) if password.len() >= 4 && user.contains(password) => {
                Ok(FlowStep::failed(
                    "Your password is part of your user name, so Jellyfesh cannot keep this sign-in \
                     without keeping your password. Change the password on the server, or sign in \
                     with an API key.",
                ))
            }
            Err(KvError::Forbidden(_)) => continue,
            Err(error) => Ok(FlowStep::failed(format!(
                "Jellyfesh could not store the sign-in in your system's keystore: {}",
                kv_text(&error)
            ))),
        };
    }
    Ok(FlowStep::failed(
        "Jellyfesh could not store the sign-in. Try again.",
    ))
}

fn api_key(base: &str, key: &str) -> Result<FlowStep> {
    if key.is_empty() {
        return Ok(FlowStep::failed("Enter an API key."));
    }
    let creds = Creds::ApiKey(key.to_string());
    if let Err(failure) = api::request("ping", &[], Auth::With(&creds)) {
        return Ok(FlowStep::failed(refused(base, failure, true)));
    }
    let account = api::request("tokenInfo", &[], Auth::With(&creds))
        .ok()
        .and_then(|envelope| serde_json::from_value::<TokenInfo>(envelope.get("tokenInfo")?.clone()).ok())
        .map(|info| info.username)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "API Key".to_string());
    match api::save_creds(&creds) {
        Ok(()) => Ok(FlowStep::done(account)),
        Err(error) => Ok(FlowStep::failed(format!(
            "Jellyfesh could not store the API key in your system's keystore: {}",
            kv_text(&error)
        ))),
    }
}

/// What to tell the person when the server refused a sign-in.
fn refused(base: &str, failure: Failure, with_key: bool) -> String {
    match failure {
        Failure::Plugin(error) => unreachable(base, &error),
        Failure::Api(error) => {
            let help = help(&error);
            match error.code {
                40 if with_key => format!("The server did not accept this API key.{help}"),
                40 => "Wrong user name or password.".to_string(),
                44 => format!("The server did not accept this API key.{help}"),
                41 => format!(
                    "This server does not accept token sign-in (for example for LDAP accounts). \
                     Create an API key on the server and sign in with \u{201c}Enter API Key\u{201d}.{help}"
                ),
                42 => format!("This server does not accept this way of signing in.{help}"),
                30 => format!(
                    "The server speaks an older Subsonic API than Jellyfesh needs ({}); update the server.",
                    api::API_VERSION
                ),
                _ => crate::api_text(&PluginError::from(error)),
            }
        }
    }
}

fn help(error: &ApiError) -> String {
    match &error.help_url {
        Some(url) => format!(" More: {url}"),
        None => String::new(),
    }
}

/// "The server cannot be reached" in words.
pub fn unreachable(base: &str, error: &PluginError) -> String {
    match error {
        PluginError::Network(detail) => format!("Cannot reach the server at {base}: {detail}"),
        other => crate::api_text(other),
    }
}

fn kv_text(error: &KvError) -> String {
    match error {
        KvError::InvalidKey(m) | KvError::Forbidden(m) | KvError::Unavailable(m) => m.clone(),
        KvError::QuotaExceeded => "storage is full".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warns_about_http_away_from_home() {
        assert!(http_warning("http://music.example.com").contains("unencrypted"));
        assert_eq!(http_warning("https://music.example.com"), "");
        for local in [
            "http://192.168.1.20:4533",
            "http://localhost:4533",
            "http://nas.local",
            "http://10.0.0.5/music",
            "http://172.16.0.9",
            "http://127.0.0.1:4533",
        ] {
            assert_eq!(http_warning(local), "", "{local}");
        }
        assert!(!is_local("http://172.40.0.1"));
    }
}

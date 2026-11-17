//! Signing in: Authorization Code with PKCE in the system browser, with the
//! Client ID of the person's own Spotify app.
//!
//! 1. `begin`: with no Client ID stored yet, a form asks for it (and shows
//!    the exact Redirect URI to register); otherwise straight to step 2.
//! 2. `open-url`: Spotify's authorisation page with the host's redirect URI,
//!    S256 challenge and `state`; the host catches the redirect.
//! 3. `submit` with the code: exchanged for tokens (no client secret). The
//!    Client ID is stored only now, so a mistyped one is asked for again.
//!
//! The Client ID is kept with the tokens in the keystore: signing out
//! forgets both, and the next sign-in asks for a Client ID again.

use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};

use crate::api::{self, Session, TokenError};

/// The flow's progress, kept by the host in `flow.state`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    /// The Client ID the browser step uses (not a secret: it is in the URL).
    #[serde(default)]
    client_id: Option<String>,
}

fn state(flow: &Flow) -> State {
    serde_json::from_str(&flow.state).unwrap_or_default()
}

/// A Client ID as Spotify shows it: 32 letters and digits.
pub fn plausible_client_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn ask(redirect_uri: &str, problem: Option<&str>) -> FlowStep {
    let mut help = format!(
        "From your own app in the Spotify Developer Dashboard (developer.spotify.com/dashboard), in \
         Development Mode, with you added as a user. Its Redirect URI must be exactly {redirect_uri}. \
         Controlling playback needs Spotify Premium."
    );
    if let Some(problem) = problem {
        help = format!("{problem} {help}");
    }
    FlowStep::new(Step::AskFields(vec![FieldSpec {
        placeholder: Some("32 letters and digits".into()),
        help: Some(help),
        ..FieldSpec::text("client-id", "Client ID")
    }]))
    .state("{}")
}

fn authorize(client_id: &str, oauth: &OauthRequest) -> Result<FlowStep> {
    let query = http::form_encode(&[
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", &oauth.redirect_uri),
        ("code_challenge_method", &oauth.code_challenge_method),
        ("code_challenge", &oauth.code_challenge),
        ("state", &oauth.state),
        ("scope", api::SCOPES),
    ]);
    let state = serde_json::to_string(&State {
        client_id: Some(client_id.to_owned()),
    })
    .map_err(|e| PluginError::internal(e.to_string()))?;
    Ok(FlowStep::new(Step::OpenUrl(OpenUrl {
        url: format!("{}/authorize?{query}", api::accounts()),
        capture: RedirectCapture::Loopback,
    }))
    .state(state))
}

fn oauth(flow: &Flow) -> Result<&OauthRequest> {
    flow.oauth
        .as_ref()
        .ok_or_else(|| PluginError::invalid_input("the host prepared no OAuth request"))
}

pub fn begin(flow: Flow) -> Result<FlowStep> {
    if flow.kind != AuthKind::Oauth {
        return Err(PluginError::Unsupported);
    }
    let oauth = oauth(&flow)?;
    match api::client_id()? {
        Some(client_id) => authorize(&client_id, oauth),
        None => Ok(ask(&oauth.redirect_uri, None)),
    }
}

pub fn submit(flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
    let value = |name: &str| {
        fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.trim().to_owned())
            .unwrap_or_default()
    };
    let oauth = oauth(&flow)?;
    let Some(client_id) = state(&flow).client_id else {
        // The form.
        let client_id = value("client-id");
        if !plausible_client_id(&client_id) {
            return Ok(ask(
                &oauth.redirect_uri,
                Some("That is not a Client ID: copy the 32 letters and digits from your app's settings."),
            ));
        }
        return authorize(&client_id, oauth);
    };
    // The code the host caught on the redirect.
    let (code, verifier, redirect_uri) = (value("code"), value("code-verifier"), value("redirect-uri"));
    if code.is_empty() || verifier.is_empty() {
        return Ok(FlowStep::failed("Spotify sent no sign-in code. Start again."));
    }
    let response = http::Request::post(format!("{}/api/token", api::accounts()))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
            ("client_id", &client_id),
            ("code_verifier", &verifier),
        ])
        .send()?;
    if !response.is_success() {
        let error: TokenError = response.json().unwrap_or_default();
        let detail = error.error_description.unwrap_or(error.error);
        return Ok(FlowStep::failed(match response.status {
            400 | 401 if detail.to_ascii_lowercase().contains("client") => format!(
                "Spotify does not accept this Client ID ({detail}). Check it in your app's settings and \
                 sign in again."
            ),
            400 => format!("Spotify refused the sign-in: {detail}"),
            429 => "Spotify asks to slow down. Try again in a minute.".to_owned(),
            status => format!("Spotify answered HTTP {status}. Try again later."),
        }));
    }
    let session = Session::from_answer(response.json()?, None, clock::now_utc_ms())?;
    kv::secret::set(api::CLIENT_SECRET_NAME, &client_id)?;
    session.store()?;
    #[derive(Deserialize)]
    struct Me {
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default)]
        id: String,
    }
    let me: Me = api::get_json("/me")?;
    Ok(FlowStep::done(
        me.display_name.filter(|n| !n.trim().is_empty()).unwrap_or(me.id),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_ids_are_32_letters_and_digits() {
        assert!(plausible_client_id("0123456789abcdef0123456789ABCDEF"));
        for bad in [
            "",
            "0123456789abcdef",
            "0123456789abcdef0123456789abcdef0",
            "0123456789abcdef 123456789abcdef",
        ] {
            assert!(!plausible_client_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn the_form_names_the_exact_redirect_uri() {
        let step = ask("http://127.0.0.1:29817/callback", Some("Not that."));
        let Step::AskFields(fields) = step.step else {
            panic!("a form");
        };
        assert_eq!(fields[0].name, "client-id");
        assert!(!fields[0].secret);
        let help = fields[0].help.clone().unwrap();
        assert!(help.starts_with("Not that. "));
        assert!(help.contains("exactly http://127.0.0.1:29817/callback"));
        assert!(help.contains("Premium"));
    }
}

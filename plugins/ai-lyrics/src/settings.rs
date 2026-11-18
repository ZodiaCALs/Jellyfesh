//! The settings (`settings.schema.json`) and what they mean: the mode, the
//! on-device model, the online provider with its address and model, the
//! language and whether to make lyrics automatically.

use jf_plugin_sdk::prelude::*;
use serde_json::Value;

/// Where recognition happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    OnDevice,
    Online,
}

/// The online provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAi,
    Groq,
    Custom,
}

impl Provider {
    /// The provider's API, up to its version, for OpenAI and Groq.
    pub fn fixed_base(self) -> Option<&'static str> {
        match self {
            Provider::OpenAi => Some("https://api.openai.com/v1"),
            Provider::Groq => Some("https://api.groq.com/openai/v1"),
            Provider::Custom => None,
        }
    }

    /// The model used when the setting is empty: the provider's model that
    /// gives word times (OpenAI: only `whisper-1`; Groq: the Whisper models,
    /// the turbo one the faster).
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::OpenAi | Provider::Custom => "whisper-1",
            Provider::Groq => "whisper-large-v3-turbo",
        }
    }

    /// The provider's name for people.
    pub fn name(self) -> &'static str {
        match self {
            Provider::OpenAi => "OpenAI",
            Provider::Groq => "Groq",
            Provider::Custom => "your server",
        }
    }

    /// How long a part of the song sent in one request is. One request must
    /// finish within the 30 s the host allows: Groq transcribes eight
    /// minutes in a few seconds, OpenAI takes longer, and a server on a
    /// computer at home may be slower still.
    pub fn part_ms(self) -> u64 {
        match self {
            Provider::Groq => jf_plugin_sdk::audio::MAX_WAV_MS,
            Provider::OpenAi => 3 * 60 * 1000,
            Provider::Custom => 2 * 60 * 1000,
        }
    }
}

/// The settings, with their defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub mode: Mode,
    /// A speech model id of the app (`tiny`, `base`, `small`,
    /// `large-v3-turbo-q5_0`).
    pub model: String,
    pub provider: Provider,
    /// The Custom server's API address, as entered.
    pub server_url: String,
    /// The online model; empty: the provider's default.
    pub online_model: String,
    /// An ISO 639-1 code; none detects.
    pub language: Option<String>,
    pub automatic: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            mode: Mode::OnDevice,
            model: "base".into(),
            provider: Provider::OpenAi,
            server_url: String::new(),
            online_model: String::new(),
            language: None,
            automatic: false,
        }
    }
}

/// The app's models, for names when the app does not list one.
pub const MODELS: [(&str, &str); 4] = [
    ("tiny", "Tiny"),
    ("base", "Base"),
    ("small", "Small"),
    ("large-v3-turbo-q5_0", "Large Turbo"),
];

impl Settings {
    /// Reads the settings object (unknown or missing values: the defaults).
    pub fn parse(value: &Value) -> Settings {
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
        };
        let defaults = Settings::default();
        Settings {
            mode: match text("mode") {
                "online" => Mode::Online,
                _ => Mode::OnDevice,
            },
            model: match text("model") {
                "" => defaults.model,
                model => model.to_owned(),
            },
            provider: match text("provider") {
                "groq" => Provider::Groq,
                "custom" => Provider::Custom,
                _ => Provider::OpenAi,
            },
            server_url: text("server_url").to_owned(),
            online_model: text("online_model").to_owned(),
            language: match text("language") {
                "" | "auto" => None,
                code => Some(code.to_owned()),
            },
            automatic: value.get("automatic").and_then(Value::as_bool).unwrap_or(false),
        }
    }

    /// The online API's address up to its version (`https://api.openai.com/v1`),
    /// or `None` for Custom without a usable address.
    pub fn base(&self) -> Option<String> {
        match self.provider.fixed_base() {
            Some(base) => Some(base.to_owned()),
            None => {
                let url = self.server_url.trim().trim_end_matches('/');
                let usable =
                    (url.starts_with("http://") || url.starts_with("https://")) && host(url).is_some();
                usable.then(|| url.to_owned())
            }
        }
    }

    /// The online model.
    pub fn online_model(&self) -> String {
        match self.online_model.trim() {
            "" => self.provider.default_model().to_owned(),
            model => model.to_owned(),
        }
    }

    /// The model's name for people ("Base").
    pub fn model_name(&self, offered: &[(String, String)]) -> String {
        offered
            .iter()
            .find(|(id, _)| *id == self.model)
            .map(|(_, name)| name.clone())
            .or_else(|| {
                MODELS
                    .iter()
                    .find(|(id, _)| *id == self.model)
                    .map(|(_, name)| (*name).to_owned())
            })
            .unwrap_or_else(|| self.model.clone())
    }

    /// The language to recognise: the setting, else the language of the
    /// song's lyrics when it is known; `None` detects it.
    pub fn language_for(&self, song_language: Option<&str>) -> Option<String> {
        self.language
            .clone()
            .or_else(|| song_language.and_then(crate::text::primary_language))
    }
}

/// `host[:port]` of an http(s) URL.
pub fn host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?.to_ascii_lowercase();
    (!authority.is_empty()).then_some(authority)
}

/// `scheme://host[:port]` of an http(s) URL, the default port left out.
pub fn origin(url: &str) -> Option<String> {
    let scheme = url.split_once("://")?.0.to_ascii_lowercase();
    let host = host(url)?;
    let host = match (scheme.as_str(), host.rsplit_once(':')) {
        ("https", Some((name, "443"))) | ("http", Some((name, "80"))) => name.to_owned(),
        _ => host,
    };
    Some(format!("{scheme}://{host}"))
}

/// The plan for these settings, the app's speech models (id, name, ready)
/// and whether an API key for the provider is stored (ADR-0019).
pub fn plan(settings: &Settings, models: &[(String, String, bool)], key_stored: bool) -> MakerPlan {
    match settings.mode {
        Mode::OnDevice => {
            let offered: Vec<(String, String)> = models
                .iter()
                .map(|(id, name, _)| (id.clone(), name.clone()))
                .collect();
            let name = settings.model_name(&offered);
            let need = if models.is_empty() {
                MakerNeed::Unavailable(
                    "This app cannot recognise singing on the device. Choose Online in AI Lyrics' settings."
                        .into(),
                )
            } else {
                match models.iter().find(|(id, _, _)| *id == settings.model) {
                    None => MakerNeed::Unavailable(format!(
                        "This app does not offer the {name} model. Choose another one in AI Lyrics' settings."
                    )),
                    Some((_, _, true)) => MakerNeed::Nothing,
                    Some((id, _, false)) => MakerNeed::Model(id.clone()),
                }
            };
            MakerPlan {
                mode: MakerMode::OnDevice,
                engine: format!("Whisper {name} on this device"),
                upload_to: None,
                need,
                automatic: settings.automatic,
            }
        }
        Mode::Online => match settings.base() {
            None => MakerPlan {
                mode: MakerMode::Online,
                engine: format!("{} on your server", settings.online_model()),
                upload_to: None,
                need: MakerNeed::Unavailable(
                    "Enter your server's address (Server Address) in AI Lyrics' settings.".into(),
                ),
                automatic: settings.automatic,
            },
            Some(base) => MakerPlan {
                mode: MakerMode::Online,
                engine: format!(
                    "{} at {}",
                    settings.online_model(),
                    host(&base).unwrap_or_default()
                ),
                upload_to: origin(&base),
                // A server of one's own may take requests without a key.
                need: if key_stored || settings.provider == Provider::Custom {
                    MakerNeed::Nothing
                } else {
                    MakerNeed::SignIn
                },
                automatic: settings.automatic,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_and_values() {
        assert_eq!(Settings::parse(&json!({})), Settings::default());
        let s = Settings::parse(&json!({
            "mode": "online", "model": "small", "provider": "custom",
            "server_url": " http://192.168.1.20:8000/v1/ ", "online_model": " ",
            "language": "ja", "automatic": true
        }));
        assert_eq!(s.mode, Mode::Online);
        assert_eq!(s.base().as_deref(), Some("http://192.168.1.20:8000/v1"));
        assert_eq!(s.online_model(), "whisper-1");
        assert_eq!(s.language.as_deref(), Some("ja"));
        assert!(s.automatic);
        let groq = Settings::parse(&json!({ "provider": "groq", "language": "auto" }));
        assert_eq!(groq.base().as_deref(), Some("https://api.groq.com/openai/v1"));
        assert_eq!(groq.online_model(), "whisper-large-v3-turbo");
        assert_eq!(groq.language, None);
        let custom = Settings::parse(&json!({ "provider": "custom", "server_url": "not a url" }));
        assert_eq!(custom.base(), None);
    }

    #[test]
    fn origins_and_hosts() {
        assert_eq!(
            origin("https://api.openai.com/v1").as_deref(),
            Some("https://api.openai.com")
        );
        assert_eq!(
            origin("HTTP://Box.local:80/v1").as_deref(),
            Some("http://box.local")
        );
        assert_eq!(
            origin("http://10.0.0.2:8000/v1").as_deref(),
            Some("http://10.0.0.2:8000")
        );
        assert_eq!(
            host("https://api.groq.com/openai/v1").as_deref(),
            Some("api.groq.com")
        );
        assert_eq!(host("nope"), None);
    }

    #[test]
    fn languages_come_from_the_setting_or_the_lyrics() {
        let auto = Settings::default();
        assert_eq!(auto.language_for(Some("ja-JP")).as_deref(), Some("ja"));
        assert_eq!(auto.language_for(None), None);
        let english = Settings {
            language: Some("en".into()),
            ..Settings::default()
        };
        assert_eq!(english.language_for(Some("ja")).as_deref(), Some("en"));
    }
}

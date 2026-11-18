//! AI Lyrics: word-timed lyrics made with Whisper (ADR-0019), a Jellyfesh
//! Pro capability.
//!
//! * **On This Device** (the default): the app's recogniser (`speech`) with
//!   the model the person chose; nothing leaves the device.
//! * **Online**: the song's audio, after the person allows sending it, to
//!   OpenAI, Groq or a Custom server that speaks OpenAI's speech-to-text
//!   API, with the person's own API key ([`online`]).
//!
//! Plain lyrics the song has are timed with the recognised words
//! ([`align`]); without them, lines are made from the recognised words
//! ([`lines`]). The result is enhanced LRC with word times ([`elrc`]).
//!
//! The API key comes in through Sign In (`api-key`) and is kept in the
//! system keystore with the origin it was entered for, so a key never goes
//! to another provider when the settings change.

mod align;
mod elrc;
mod lines;
mod model;
mod online;
mod settings;
mod text;

#[cfg(test)]
mod golden;

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Serialize};

use crate::model::{Segment, Transcript, Word};
use crate::settings::{Mode, Provider, Settings};

struct AiLyrics;

thread_local! {
    static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default());
}

fn current() -> Settings {
    SETTINGS.with(|s| s.borrow().clone())
}

/// The keystore entry with the API key.
const KEY_SECRET: &str = "api-key";

/// The API key and the origin it is for.
#[derive(Debug, Serialize, Deserialize)]
struct StoredKey {
    origin: String,
    key: String,
}

/// The origin of the online provider the settings name.
fn provider_origin(settings: &Settings) -> Option<String> {
    settings.base().as_deref().and_then(settings::origin)
}

/// The stored API key, if it was entered for the provider the settings
/// name now.
fn stored_key(settings: &Settings) -> Result<Option<String>> {
    let Some(origin) = provider_origin(settings) else {
        return Ok(None);
    };
    Ok(kv::secret::get(KEY_SECRET)?
        .and_then(|text| serde_json::from_str::<StoredKey>(&text).ok())
        .filter(|stored| stored.origin == origin)
        .map(|stored| stored.key))
}

/// The app's speech models as (id, name, ready).
fn models() -> Vec<(String, String, bool)> {
    speech::models()
        .into_iter()
        .map(|m| (m.id, m.name, m.ready))
        .collect()
}

/// The plan's engine for the current settings.
fn engine(settings: &Settings) -> String {
    settings::plan(settings, &models(), true).engine
}

/// Where to get a key, for the sign-in form.
fn key_help(provider: Provider) -> &'static str {
    match provider {
        Provider::OpenAi => {
            "Your own key from platform.openai.com/api-keys. Jellyfesh keeps it in your system's keystore \
             and sends it only to api.openai.com. OpenAI charges your account for what it transcribes."
        }
        Provider::Groq => {
            "Your own key from console.groq.com/keys. Jellyfesh keeps it in your system's keystore and \
             sends it only to api.groq.com."
        }
        Provider::Custom => {
            "The key your server asks for, if it asks for one. Jellyfesh keeps it in your system's keystore \
             and sends it only to the Server Address in AI Lyrics' settings."
        }
    }
}

/// A recognised transcript in the plugin's terms.
fn from_speech(transcript: speech::Transcript) -> Transcript {
    Transcript {
        language: transcript.language,
        segments: transcript
            .segments
            .into_iter()
            .map(|s| Segment {
                text: s.text,
                start_ms: s.start_ms,
                end_ms: s.end_ms,
                words: s
                    .words
                    .into_iter()
                    .map(|w| Word {
                        text: w.text,
                        start_ms: w.start_ms,
                        end_ms: w.end_ms,
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Recognition in the app.
fn on_device(
    song: &Song,
    settings: &Settings,
    language: Option<String>,
    prompt: Option<String>,
) -> Result<Transcript> {
    let mut options = speech::Options::new(settings.model.clone());
    options.language = language;
    options.prompt = prompt;
    match speech::transcribe(song.track, &options) {
        Ok(transcript) => Ok(from_speech(transcript)),
        Err(speech::SpeechError::ModelMissing(_)) => Err(PluginError::service(format!(
            "The {} model is not downloaded yet. Download it in AI Lyrics' settings.",
            settings.model_name(&[])
        ))),
        Err(other) => Err(other.into()),
    }
}

/// Plain lyrics worth timing: some line with words.
fn plain_lyrics(song: &Song) -> Option<&str> {
    song.text.as_deref().filter(|t| {
        t.lines()
            .any(|l| !text::is_label(l) && !text::units(l).is_empty())
    })
}

impl Plugin for AiLyrics {
    fn capabilities() -> Vec<Capability> {
        // make_lyrics is a manifest capability that `meta` cannot report.
        vec![Capability::Auth]
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let parsed = Settings::parse(settings);
        SETTINGS.with(|s| *s.borrow_mut() = parsed);
        Ok(())
    }

    fn auth_begin(flow: Flow) -> Result<FlowStep> {
        if flow.kind != AuthKind::ApiKey {
            return Err(PluginError::Unsupported);
        }
        let settings = current();
        if settings.base().is_none() {
            return Ok(FlowStep::failed(
                "Enter your server's address (Server Address) in AI Lyrics' settings first.",
            ));
        }
        Ok(FlowStep::new(Step::AskFields(vec![FieldSpec {
            placeholder: Some(
                match settings.provider {
                    Provider::OpenAi => "sk-…",
                    Provider::Groq => "gsk_…",
                    Provider::Custom => "",
                }
                .to_owned(),
            )
            .filter(|p| !p.is_empty()),
            help: Some(key_help(settings.provider).into()),
            ..FieldSpec::secret("api-key", "API Key")
        }])))
    }

    fn auth_submit(_flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
        let settings = current();
        let key = fields
            .iter()
            .find(|f| f.name == "api-key")
            .map(|f| f.value.trim().to_owned())
            .unwrap_or_default();
        if key.is_empty() || key.len() > 512 || key.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Ok(FlowStep::failed(
                "That is not an API key: copy the whole key, without spaces.",
            ));
        }
        let (Some(base), Some(origin)) = (settings.base(), provider_origin(&settings)) else {
            return Ok(FlowStep::failed(
                "Enter your server's address (Server Address) in AI Lyrics' settings first.",
            ));
        };
        let host = settings::host(&base).unwrap_or_default();
        // A request that costs nothing and needs the key: the list of models.
        match http::Request::get(format!("{base}/models"))
            .bearer(&key)
            .timeout_ms(15_000)
            .send()
        {
            Ok(response) if response.is_success() => {}
            Ok(response) if matches!(response.status, 401 | 403) => {
                return Ok(FlowStep::failed(format!(
                    "{host} did not accept this API key. Check it, or make a new one, and try again."
                )));
            }
            // A server of one's own may not list its models.
            Ok(response) if settings.provider == Provider::Custom && matches!(response.status, 404 | 405) => {
            }
            Ok(response) => {
                return Ok(FlowStep::failed(format!(
                    "{host} answered HTTP {}. Try again later.",
                    response.status
                )));
            }
            Err(error) => {
                return Ok(FlowStep::failed(format!(
                    "Cannot reach {host}: {}",
                    match PluginError::from(error) {
                        PluginError::Network(text) | PluginError::Internal(text) => text,
                        other => format!("{other:?}"),
                    }
                )));
            }
        }
        let stored = serde_json::to_string(&StoredKey {
            origin,
            key: key.clone(),
        })?;
        kv::secret::set(KEY_SECRET, &stored)?;
        let tail: String = key
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        Ok(FlowStep::done(format!(
            "{} API key …{tail}",
            settings.provider.name().replace("your server", host.as_str())
        )))
    }

    fn logout() -> Result<()> {
        // The key stays valid at the provider; the host deletes the stored copy.
        Ok(())
    }

    fn maker_plan() -> Result<MakerPlan> {
        let settings = current();
        let key = settings.mode == Mode::Online && stored_key(&settings)?.is_some();
        Ok(settings::plan(&settings, &models(), key))
    }

    fn make_lyrics(song: Song) -> Result<Made> {
        let settings = current();
        let lyrics = plain_lyrics(&song);
        let language = settings.language_for(song.language.as_deref());
        let prompt = lyrics.and_then(text::prompt);
        let transcript = match settings.mode {
            Mode::OnDevice => on_device(&song, &settings, language.clone(), prompt)?,
            Mode::Online => {
                // Everything from storage before the audio: afterwards the
                // call may reach only the provider.
                let key = stored_key(&settings)?;
                if key.is_none() && settings.provider != Provider::Custom {
                    return Err(PluginError::AuthRequired);
                }
                online::transcribe(
                    song.track,
                    &settings,
                    key.as_deref(),
                    language.as_deref(),
                    prompt.as_deref(),
                )?
            }
        };
        let duration = u32::try_from(song.duration_ms).ok().filter(|d| *d > 0);
        let made = match lyrics {
            Some(lyrics) => match align::align(lyrics, &transcript, duration) {
                Ok(lines) => lines,
                Err(align::Failure::NoLyrics) => lines::lines(&transcript),
                Err(align::Failure::NothingHeard) => Vec::new(),
                Err(align::Failure::NoMatch) => {
                    return Err(PluginError::service(
                        "The words recognised in this song do not match its lyrics, so AI Lyrics could not \
                         time them. Check that the lyrics belong to this song, or try a larger model.",
                    ));
                }
            },
            None => lines::lines(&transcript),
        };
        if made.iter().all(|l| l.words.is_empty()) {
            return Err(PluginError::service(
                "No singing was recognised in this song, so AI Lyrics made no lyrics.",
            ));
        }
        let mut payload = LyricPayload::new(LyricFormat::Elrc, elrc::render(&made));
        payload.language = language.or(transcript.language);
        Ok(Made {
            lyrics: payload,
            engine: engine(&settings),
        })
    }
}

jf_plugin_sdk::export_plugin!(AiLyrics);

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_version_matches_the_crate() {
        let manifest = include_str!("../manifest.toml");
        let line = format!("version = \"{}\"", env!("CARGO_PKG_VERSION"));
        assert!(
            manifest.lines().any(|l| l == line),
            "manifest.toml must say {line}"
        );
    }
}

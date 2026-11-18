//! A test plugin that makes lyrics (API 1.2) through every import a maker
//! has: `speech` on the device, `audio` and `http` online, and `kv`.
//!
//! Settings: `mode` (`on-device` or `online`), `model` (a speech model id),
//! `server_url` (the online provider, a test server) and `other_url`
//! (another allowlisted origin, which the person never allowed). A song whose
//! title is `do:<command>` exercises one rule of the host instead of making
//! lyrics; every invented word comes from the test.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;

struct Maker;

#[derive(Default, Clone, Deserialize)]
#[serde(default)]
struct Settings {
    mode: String,
    model: String,
    server_url: String,
    other_url: String,
}

thread_local! {
    static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default());
}

fn settings() -> Settings {
    SETTINGS.with(|s| s.borrow().clone())
}

fn online() -> bool {
    settings().mode == "online"
}

/// `[mm:ss.xx]` for a time in milliseconds.
fn stamp(ms: u32) -> String {
    format!(
        "{:02}:{:02}.{:02}",
        ms / 60_000,
        (ms / 1000) % 60,
        (ms % 1000) / 10
    )
}

/// Enhanced LRC: a line per segment, every word with its start.
fn elrc(segments: &[(u32, Vec<(u32, String)>)]) -> String {
    let mut out = String::new();
    for (start, words) in segments {
        out.push('[');
        out.push_str(&stamp(*start));
        out.push(']');
        for (at, word) in words {
            out.push_str(&format!("<{}>{} ", stamp(*at), word));
        }
        out.push('\n');
    }
    out
}

fn made(text: String, engine: &str) -> Made {
    Made {
        lyrics: LyricPayload::new(LyricFormat::Elrc, text),
        engine: engine.into(),
    }
}

#[derive(Deserialize)]
struct Timed {
    word: String,
    start: f64,
}

#[derive(Deserialize)]
struct Answer {
    words: Vec<Timed>,
}

fn make_online(song: &Song) -> Result<Made> {
    let server = settings().server_url;
    let duration = audio::duration_ms(song.track)?;
    let wav = audio::wav(song.track, 0, duration.min(audio::MAX_WAV_MS))?;
    let answer: Answer = http::Request::post(format!(
        "{}/v1/audio/transcriptions",
        server.trim_end_matches('/')
    ))
    .body(wav, "audio/wav")
    .send_json()?;
    let words = answer
        .words
        .into_iter()
        .map(|w| ((w.start * 1000.0) as u32, w.word))
        .collect::<Vec<_>>();
    let start = words.first().map_or(0, |w| w.0);
    Ok(made(elrc(&[(start, words)]), "test-whisper online"))
}

fn make_on_device(song: &Song) -> Result<Made> {
    let mut options = speech::Options::new(settings().model);
    options.prompt = song.text.clone();
    let transcript = speech::transcribe(song.track, &options)?;
    let segments: Vec<(u32, Vec<(u32, String)>)> = transcript
        .segments
        .iter()
        .map(|s| {
            (
                s.start_ms,
                s.words.iter().map(|w| (w.start_ms, w.text.clone())).collect(),
            )
        })
        .collect();
    Ok(made(elrc(&segments), "test-whisper on this device"))
}

/// One host rule per command; the answer describes what the host did.
fn command(song: &Song, command: &str) -> Result<Made> {
    let report = |text: String| Ok(made(format!("[00:00.00]{text}\n"), "report"));
    match command {
        "kv-after-audio" => {
            use jf_plugin_sdk::bindings::jellyfesh::plugin::kv as raw;
            let before = raw::set("before", b"fine").is_ok();
            audio::wav(song.track, 0, 1000)?;
            let after = raw::set("after", b"leak");
            let secret = raw::set_secret("after", "leak");
            report(format!("before={before} after={after:?} secret={secret:?}"))
        }
        "http-elsewhere" => {
            audio::wav(song.track, 0, 1000)?;
            let allowed = http::Request::get(format!("{}/ping", settings().server_url)).send();
            let other = http::Request::get(format!("{}/upload", settings().other_url)).send();
            report(format!(
                "allowed={} other={:?}",
                allowed.map(|r| r.status).unwrap_or(0),
                other.map(|r| r.status)
            ))
        }
        "http-before-audio" => {
            let other = http::Request::get(format!("{}/lookup", settings().other_url)).send();
            report(format!("other={}", other.map(|r| r.status).unwrap_or(0)))
        }
        "stash-handle" => {
            kv::set_json("handle", &song.track)?;
            report(format!("stashed {}", song.track))
        }
        "use-stashed-handle" => {
            let old: u64 = kv::get_json("handle")?.unwrap_or_default();
            let info = audio::duration_ms(old);
            let wav = audio::wav(old, 0, 1000).map(|w| w.len());
            let speech = speech::transcribe(old, &speech::Options::new("base")).map(|t| t.segments.len());
            report(format!(
                "info={info:?} wav={wav:?} speech={speech:?} same={}",
                old == song.track
            ))
        }
        "wav-too-long" => {
            let long = audio::wav(song.track, 0, audio::MAX_WAV_MS + 1).map(|w| w.len());
            let fine = audio::wav(song.track, 0, audio::MAX_WAV_MS).map(|w| w.len());
            report(format!("long={long:?} fine={}", fine.is_ok()))
        }
        "wav" => {
            let wav = audio::wav(song.track, 1000, 2000)?;
            report(format!("bytes={} riff={}", wav.len(), wav.starts_with(b"RIFF")))
        }
        "models" => {
            let models: Vec<String> = speech::models()
                .iter()
                .map(|m| format!("{}:{}:{}", m.id, m.bytes, m.ready))
                .collect();
            report(models.join(","))
        }
        "undeclared" => {
            let info = audio::duration_ms(song.track);
            let speech =
                speech::transcribe(song.track, &speech::Options::new("base")).map(|t| t.segments.len());
            report(format!(
                "info={info:?} speech={speech:?} models={}",
                speech::models().len()
            ))
        }
        "spin" => {
            let mut x: u64 = 0;
            loop {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                if x == 42 {
                    break;
                }
            }
            report(x.to_string())
        }
        "slow-import" => {
            // The host's recogniser takes its time; none of it is the plugin's CPU.
            let transcript = speech::transcribe(song.track, &speech::Options::new(settings().model))?;
            report(format!("segments={}", transcript.segments.len()))
        }
        "cancel-in-import" => match speech::transcribe(song.track, &speech::Options::new(settings().model)) {
            Err(speech::SpeechError::Cancelled) => Err(PluginError::service("cancelled")),
            other => report(format!("{:?}", other.map(|t| t.segments.len()))),
        },
        "plain" => Ok(Made {
            lyrics: LyricPayload::new(LyricFormat::Plain, "not synced"),
            engine: "x".into(),
        }),
        "memory" => {
            // 100 MiB: over the 64 MiB of other calls, under make's 128 MiB.
            let mut big = vec![0u8; 100 * 1024 * 1024];
            let last = big.len() - 1;
            big[last] = 1;
            report(format!("allocated {}", std::hint::black_box(&big).len()))
        }
        other => Err(PluginError::invalid_input(format!("unknown command {other}"))),
    }
}

impl Plugin for Maker {
    fn capabilities() -> Vec<Capability> {
        // `make_lyrics` is a manifest capability: `meta` cannot report it.
        Vec::new()
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let parsed: Settings = serde_json::from_value(settings.clone())?;
        SETTINGS.with(|s| *s.borrow_mut() = parsed);
        Ok(())
    }

    fn maker_plan() -> Result<MakerPlan> {
        let settings = settings();
        if online() {
            let need = if kv::secret::get("api-key")?.is_some() {
                MakerNeed::Nothing
            } else {
                MakerNeed::SignIn
            };
            return Ok(MakerPlan {
                mode: MakerMode::Online,
                engine: "test-whisper online".into(),
                upload_to: Some(settings.server_url),
                need,
                automatic: false,
            });
        }
        let need = if speech::ready(&settings.model) {
            MakerNeed::Nothing
        } else {
            MakerNeed::Model(settings.model.clone())
        };
        Ok(MakerPlan {
            mode: MakerMode::OnDevice,
            engine: format!("Whisper {} on this device", settings.model),
            upload_to: None,
            need,
            automatic: settings.model == "tiny",
        })
    }

    fn make_lyrics(song: Song) -> Result<Made> {
        if let Some(name) = song.title.strip_prefix("do:") {
            return command(&song, name);
        }
        if online() {
            make_online(&song)
        } else {
            make_on_device(&song)
        }
    }
}

jf_plugin_sdk::export_plugin!(Maker);

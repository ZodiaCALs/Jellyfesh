//! Plugin API 1.2 (ADR-0019, ADR-0020): making lyrics with the song's audio
//! and on-device recognition (consent, scoped track handles, the 8-minute
//! cap, HTTP and `kv` after reading audio, `maker.make`'s own limits and
//! cancellation), Pro gating, appearance looks, Discover pages, details and
//! comments, settings schema profile 1.2 with translations, and 1.0 and 1.1
//! plugins in a 1.2 host.

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use jf_plugin_host::{
    AudioError, CallError, Capability, Density, FactKey, Highlight, HostError, Limits, MakeJob, MakeSong,
    MakeStage, MakerAccess, MakerMode, MakerNeed, Permission, Place, PluginError, ShelfStyle, SpeechError,
    SpeechModel, SpeechOptions, SpeechSegment, SpeechWord, Transcript,
};
use serde_json::json;

const MAKER: &str = "org.jellyfesh.test.maker";
const LOOK: &str = "org.jellyfesh.test.look";
const DISCOVER: &str = "org.jellyfesh.test.discover";
const COMPAT: &str = "org.jellyfesh.test.compat11";

const MAKER_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "mode": { "type": "string", "enum": ["on-device", "online"], "default": "on-device" },
        "model": { "type": "string", "default": "base" },
        "server_url": { "type": "string", "format": "uri" },
        "other_url": { "type": "string", "format": "uri" }
    }
}"#;

fn maker_manifest(permissions: &str) -> String {
    format!(
        "id = {MAKER:?}\nname = \"Test Maker\"\nversion = \"1.0.0\"\napi_version = \"1.2\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"make_lyrics\"]\nplayback_mode = \"stream\"\n\
         permissions = [{permissions}]\n\
         \n[network]\nallow = [\"${{settings.server_url}}\", \"${{settings.other_url}}\"]\nmax_rps = 20\n\
         \n[settings]\nschema = \"settings.schema.json\"\n"
    )
}

/// A host with the maker plugin (both permissions) and Pro on.
fn maker_env(limits: Limits) -> Env {
    let env = Env::with_limits(limits);
    env.install_with(
        "maker",
        &maker_manifest("\"audio\", \"speech\""),
        MAKER,
        MAKER_SCHEMA,
        &[],
    );
    env.host.set_pro(true);
    env
}

fn settings(env: &Env, value: serde_json::Value) {
    env.host.set_settings(MAKER, &value.to_string()).unwrap();
}

/// The song's audio and recognition as the core would serve them, with
/// invented words and silence for audio.
#[derive(Default)]
struct FakeAccess {
    /// How long recognition takes.
    delay: Duration,
    /// Recognition waits until the job is cancelled.
    wait_for_cancel: bool,
    stages: Mutex<Vec<MakeStage>>,
    wav_requests: Mutex<Vec<(u64, u64)>>,
    options: Mutex<Vec<SpeechOptions>>,
}

impl FakeAccess {
    fn stages(&self) -> Vec<MakeStage> {
        self.stages.lock().unwrap().clone()
    }
}

/// A 16 kHz mono 16-bit WAV of `ms` of silence (at most a second).
fn silence(ms: u64) -> Vec<u8> {
    let samples = (ms.min(1000) * 16) as u32;
    let data = samples * 2;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&16_000u32.to_le_bytes());
    wav.extend_from_slice(&32_000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    wav.resize(wav.len() + data as usize, 0);
    wav
}

impl MakerAccess for FakeAccess {
    fn duration_ms(&self) -> Result<u64, AudioError> {
        Ok(200_000)
    }

    fn wav(&self, start_ms: u64, length_ms: u64) -> Result<Vec<u8>, AudioError> {
        self.wav_requests.lock().unwrap().push((start_ms, length_ms));
        Ok(silence(length_ms))
    }

    fn transcribe(&self, options: &SpeechOptions, cancel: &AtomicBool) -> Result<Transcript, SpeechError> {
        self.options.lock().unwrap().push(options.clone());
        let started = Instant::now();
        while self.wait_for_cancel || started.elapsed() < self.delay {
            if cancel.load(Ordering::Relaxed) {
                return Err(SpeechError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let word = |text: &str, start_ms, end_ms| SpeechWord {
            text: text.into(),
            start_ms,
            end_ms,
            confidence: 0.9,
        };
        Ok(Transcript {
            language: Some("en".into()),
            segments: vec![
                SpeechSegment {
                    text: "invented words here".into(),
                    start_ms: 1000,
                    end_ms: 2500,
                    words: vec![
                        word("invented", 1000, 1400),
                        word("words", 1500, 1900),
                        word("here", 2000, 2500),
                    ],
                },
                SpeechSegment {
                    // The recogniser runs past the song's end; the host keeps it within.
                    text: "second line".into(),
                    start_ms: 3000,
                    end_ms: 900_000,
                    words: vec![word("second", 3000, 3400), word("line", 3500, 900_000)],
                },
            ],
        })
    }

    fn stage(&self, stage: MakeStage) {
        self.stages.lock().unwrap().push(stage);
    }
}

fn job(title: &str, access: &Arc<FakeAccess>, allowed: &[&str]) -> MakeJob {
    MakeJob {
        song: MakeSong {
            title: title.into(),
            artist: Some("Invented Artist".into()),
            album: None,
            duration_ms: 200_000,
            text: None,
            language: None,
        },
        access: access.clone(),
        allowed_origins: allowed.iter().map(|o| o.to_string()).collect(),
        upload_to: None,
        cancel: Arc::new(AtomicBool::new(false)),
    }
}

fn models(ready: bool) -> Vec<SpeechModel> {
    vec![
        SpeechModel {
            id: "tiny".into(),
            name: "Tiny".into(),
            bytes: 77_691_713,
            ready: false,
        },
        SpeechModel {
            id: "base".into(),
            name: "Base".into(),
            bytes: 147_951_465,
            ready,
        },
    ]
}

/// The text a `do:` command reports (the made lyrics' only line).
fn report(result: Result<jf_plugin_host::Made, CallError>) -> String {
    let made = result.unwrap_or_else(|e| panic!("the command failed: {e}"));
    assert_eq!(made.engine, "report");
    made.lyrics
        .text
        .trim()
        .trim_start_matches("[00:00.00]")
        .to_string()
}

#[test]
fn on_device_lyrics_come_from_the_apps_recogniser() {
    let env = maker_env(test_limits());
    let host = &env.host;
    let info = host.plugin(MAKER).unwrap();
    assert_eq!(info.permissions, vec![Permission::Audio, Permission::Speech]);
    assert_eq!(
        recv(host.capabilities(MAKER)).unwrap(),
        vec![Capability::MakeLyrics],
        "make_lyrics is listed although meta cannot report it"
    );

    // The model is not downloaded yet: the plan says so.
    host.set_speech_models(models(false));
    let plan = recv(host.maker_plan(MAKER)).unwrap();
    assert_eq!(plan.mode, MakerMode::OnDevice);
    assert_eq!(plan.need, MakerNeed::Model("base".into()));
    assert_eq!(plan.engine, "Whisper base on this device");
    assert_eq!(plan.upload_to, None);
    host.set_speech_models(models(true));
    assert_eq!(recv(host.maker_plan(MAKER)).unwrap().need, MakerNeed::Nothing);

    let access = Arc::new(FakeAccess::default());
    let made = recv(host.make_lyrics(MAKER, job("Invented Song", &access, &[]))).unwrap();
    assert_eq!(made.engine, "test-whisper on this device");
    assert_eq!(made.lyrics.format, jf_plugin_host::LyricFormat::Elrc);
    assert!(
        made.lyrics
            .text
            .starts_with("[00:01.00]<00:01.00>invented <00:01.50>words"),
        "{}",
        made.lyrics.text
    );
    assert!(made.lyrics.text.contains("<00:03.50>line"));
    assert_eq!(access.stages(), [MakeStage::Recognising, MakeStage::TimingWords]);
    assert_eq!(access.options.lock().unwrap()[0].model, "base");
    assert!(
        access.wav_requests.lock().unwrap().is_empty(),
        "the device mode reads no audio in the plugin"
    );

    // The models list reaches plugins with the speech permission.
    let listed = report(recv(host.make_lyrics(MAKER, job("do:models", &access, &[]))));
    assert_eq!(listed, "tiny:77691713:false,base:147951465:true");
}

#[test]
fn online_lyrics_send_audio_only_after_consent() {
    let server = TestServer::start(|request| match request.path() {
        "/v1/audio/transcriptions" if request.body.starts_with(b"RIFF") => Response::json(json!({
            "words": [{ "word": "invented", "start": 1.0 }, { "word": "online", "start": 1.5 }]
        })),
        _ => Response::new(400, "no audio"),
    });
    let env = maker_env(test_limits());
    settings(&env, json!({ "mode": "online", "server_url": server.url() }));
    let origin = server.url();

    let plan = recv(env.host.maker_plan(MAKER)).unwrap();
    assert_eq!(plan.mode, MakerMode::Online);
    assert_eq!(plan.upload_to.as_deref(), Some(origin.as_str()));
    assert_eq!(plan.need, MakerNeed::SignIn, "no API key yet");

    // Without consent the plugin gets no audio, and nothing is sent.
    let access = Arc::new(FakeAccess::default());
    let mut refused = job("Invented Song", &access, &[]);
    refused.upload_to = Some(origin.clone());
    match recv(env.host.make_lyrics(MAKER, refused)) {
        Err(CallError::Plugin(PluginError::Service(text))) => {
            assert!(
                text.contains("has not been allowed") && text.contains(&origin),
                "{text}"
            );
        }
        other => panic!("expected no consent, got {other:?}"),
    }
    assert_eq!(server.count("/v1"), 0);

    let access = Arc::new(FakeAccess::default());
    let made = recv(
        env.host
            .make_lyrics(MAKER, job("Invented Song", &access, &[&origin])),
    )
    .unwrap();
    assert_eq!(made.engine, "test-whisper online");
    assert!(
        made.lyrics.text.contains("<00:01.00>invented <00:01.50>online"),
        "{}",
        made.lyrics.text
    );
    assert_eq!(server.count("/v1/audio/transcriptions"), 1);
    assert_eq!(
        access.stages(),
        [
            MakeStage::ReadingAudio,
            MakeStage::Sending {
                origin: origin.clone()
            },
            MakeStage::TimingWords
        ]
    );
    assert_eq!(*access.wav_requests.lock().unwrap(), [(0, 200_000)]);
}

#[test]
fn after_reading_audio_requests_go_only_where_allowed_and_kv_takes_no_writes() {
    let allowed = TestServer::start(|_| Response::new(200, "ok"));
    let other = TestServer::start(|_| Response::new(200, "ok"));
    let env = maker_env(test_limits());
    settings(
        &env,
        json!({ "mode": "online", "server_url": allowed.url(), "other_url": other.url() }),
    );
    let access = Arc::new(FakeAccess::default());
    let consent = [allowed.url()];
    let consent: Vec<&str> = consent.iter().map(String::as_str).collect();

    let kv = report(recv(
        env.host
            .make_lyrics(MAKER, job("do:kv-after-audio", &access, &consent)),
    ));
    assert!(
        kv.starts_with("before=true after=Err(KvError::Forbidden("),
        "{kv}"
    );
    assert!(kv.contains("secret=Err(KvError::Forbidden("), "{kv}");

    let http = report(recv(
        env.host
            .make_lyrics(MAKER, job("do:http-elsewhere", &access, &consent)),
    ));
    assert!(
        http.starts_with("allowed=200 other=Err(HttpError::NotAllowed("),
        "{http}"
    );
    assert_eq!(other.count("/upload"), 0, "nothing reached the other origin");

    // Before reading audio the whole allowlist is open, as for any call.
    let before = report(recv(
        env.host
            .make_lyrics(MAKER, job("do:http-before-audio", &access, &consent)),
    ));
    assert_eq!(before, "other=200");
}

#[test]
fn track_handles_are_scoped_to_their_call_and_wav_is_capped() {
    let env = maker_env(test_limits());
    let access = Arc::new(FakeAccess::default());
    let consent = ["https://api.example.com"];
    let stashed = report(recv(
        env.host
            .make_lyrics(MAKER, job("do:stash-handle", &access, &consent)),
    ));
    assert!(stashed.starts_with("stashed "), "{stashed}");
    let used = report(recv(
        env.host
            .make_lyrics(MAKER, job("do:use-stashed-handle", &access, &consent)),
    ));
    assert!(used.contains("info=Err(AudioError::Unavailable("), "{used}");
    assert!(used.contains("wav=Err(AudioError::Unavailable("), "{used}");
    assert!(used.contains("speech=Err(SpeechError::Unavailable("), "{used}");
    assert!(
        used.ends_with("same=false"),
        "every call gets a new handle: {used}"
    );

    let capped = report(recv(
        env.host
            .make_lyrics(MAKER, job("do:wav-too-long", &access, &consent)),
    ));
    assert!(
        capped.starts_with("long=Err(AudioError::Unavailable("),
        "{capped}"
    );
    assert!(capped.ends_with("fine=true"), "{capped}");

    let wav = report(recv(
        env.host.make_lyrics(MAKER, job("do:wav", &access, &consent)),
    ));
    assert_eq!(wav, "bytes=32044 riff=true");
    assert!(access.wav_requests.lock().unwrap().contains(&(1000, 2000)));
}

#[test]
fn undeclared_imports_answer_not_declared() {
    let env = Env::new();
    env.install_with("maker", &maker_manifest(""), MAKER, MAKER_SCHEMA, &[]);
    env.host.set_pro(true);
    env.host.set_speech_models(models(true));
    assert!(env.host.plugin(MAKER).unwrap().permissions.is_empty());
    let access = Arc::new(FakeAccess::default());
    let answer = report(recv(
        env.host
            .make_lyrics(MAKER, job("do:undeclared", &access, &["https://a.example"])),
    ));
    assert_eq!(
        answer,
        "info=Err(AudioError::NotDeclared) speech=Err(SpeechError::NotDeclared) models=0"
    );
}

#[test]
fn pro_capabilities_answer_pro_required_while_pro_is_off() {
    let env = maker_env(test_limits());
    env.install_with("appearance", &look_manifest(), LOOK, LOOK_SCHEMA, &[]);
    env.host.set_pro(false);
    assert_eq!(recv(env.host.maker_plan(MAKER)), Err(CallError::ProRequired));
    let access = Arc::new(FakeAccess::default());
    assert_eq!(
        recv(env.host.make_lyrics(MAKER, job("Invented Song", &access, &[]))),
        Err(CallError::ProRequired)
    );
    assert_eq!(recv(env.host.appearance_look(LOOK)), Err(CallError::ProRequired));
    // The plugins stay installed and their other calls work.
    assert!(env.host.plugin(MAKER).unwrap().enabled);
    assert!(recv(env.host.capabilities(LOOK)).is_ok());
    env.host.set_pro(true);
    assert!(recv(env.host.maker_plan(MAKER)).is_ok());
    assert!(recv(env.host.appearance_look(LOOK)).is_ok());
}

#[test]
fn make_counts_only_the_plugins_own_cpu_and_has_a_wall_clock_cap() {
    let limits = Limits {
        make_cpu: Duration::from_millis(400),
        make_wall: Duration::from_secs(3),
        ..test_limits()
    };
    let env = maker_env(limits);
    settings(&env, json!({ "model": "base" }));
    // Recognition taking far longer than the CPU budget is the host's time.
    let slow = Arc::new(FakeAccess {
        delay: Duration::from_millis(1200),
        ..FakeAccess::default()
    });
    let answer = report(recv(
        env.host.make_lyrics(MAKER, job("do:slow-import", &slow, &[])),
    ));
    assert_eq!(answer, "segments=2");

    // The plugin's own loop is stopped by the CPU budget.
    let access = Arc::new(FakeAccess::default());
    let (result, elapsed) = timed(|| recv(env.host.make_lyrics(MAKER, job("do:spin", &access, &[]))));
    assert_eq!(result.map(|m| m.engine), Err(CallError::CpuLimit));
    assert!(elapsed < Duration::from_secs(3), "took {elapsed:?}");

    // Recognition outlasting the wall clock ends the call when it returns.
    let slower = Arc::new(FakeAccess {
        delay: Duration::from_millis(3500),
        ..FakeAccess::default()
    });
    let result = recv(env.host.make_lyrics(MAKER, job("do:slow-import", &slower, &[])));
    assert_eq!(result.map(|m| m.engine), Err(CallError::Timeout));
}

#[test]
fn make_runs_beside_the_plugins_other_calls_and_can_be_cancelled() {
    let env = maker_env(test_limits());
    env.host.set_speech_models(models(true));
    let waiting = Arc::new(FakeAccess {
        wait_for_cancel: true,
        ..FakeAccess::default()
    });
    let job_in_import = job("do:cancel-in-import", &waiting, &[]);
    let cancel = job_in_import.cancel.clone();
    let reply = env.host.make_lyrics(MAKER, job_in_import);
    std::thread::sleep(Duration::from_millis(200));
    // Meanwhile the plugin answers its other calls at once.
    let (plan, elapsed) = timed(|| recv(env.host.maker_plan(MAKER)));
    assert!(plan.is_ok());
    assert!(elapsed < Duration::from_secs(2), "the plan waited {elapsed:?}");
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(recv(reply).map(|m| m.engine), Err(CallError::Cancelled));

    // A cancel during the plugin's own code stops it at the next tick.
    let access = Arc::new(FakeAccess::default());
    let spinning = job("do:spin", &access, &[]);
    let cancel = spinning.cancel.clone();
    let reply = env.host.make_lyrics(MAKER, spinning);
    std::thread::sleep(Duration::from_millis(50));
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(recv(reply).map(|m| m.engine), Err(CallError::Cancelled));
    // Cancelling is not a strike.
    for _ in 0..3 {
        let job = job("do:spin", &access, &[]);
        job.cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            recv(env.host.make_lyrics(MAKER, job)).map(|m| m.engine),
            Err(CallError::Cancelled)
        );
    }
    assert!(env.host.plugin(MAKER).unwrap().enabled);
}

#[test]
fn make_has_more_memory_and_its_answers_are_checked() {
    let env = maker_env(test_limits());
    let access = Arc::new(FakeAccess::default());
    let answer = report(recv(env.host.make_lyrics(MAKER, job("do:memory", &access, &[]))));
    assert!(answer.starts_with("allocated "), "{answer}");
    assert!(matches!(
        recv(env.host.make_lyrics(MAKER, job("do:plain", &access, &[]))),
        Err(CallError::InvalidResponse(why)) if why.contains("synced")
    ));
}

const LOOK_SCHEMA: &str = r#"{
    "type": "object",
    "title": "Look",
    "properties": {
        "look": { "type": "string", "maxLength": 4096, "default": "{}", "title": "Look", "x-jf-group": "Look" },
        "nan_glass": { "type": "boolean", "default": false, "title": "NaN Glass" },
        "corners": { "type": "number", "minimum": 0, "maximum": 2, "multipleOf": 0.25, "default": 1, "title": "Corners", "x-jf-group": "Look" },
        "layout": { "type": "string", "oneOf": [{ "const": "stage", "title": "Stage" }, { "const": "lyrics", "title": "Lyrics" }], "default": "stage", "title": "Now Playing" }
    }
}"#;

fn look_manifest() -> String {
    format!(
        "id = {LOOK:?}\nname = \"Test Look\"\nversion = \"1.0.0\"\napi_version = \"1.2\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"appearance\"]\nplayback_mode = \"stream\"\n\
         \n[settings]\nschema = \"settings.schema.json\"\n"
    )
}

#[test]
fn appearance_looks_are_checked() {
    let env = Env::new();
    env.install_with("appearance", &look_manifest(), LOOK, LOOK_SCHEMA, &[]);
    env.host.set_pro(true);
    assert_eq!(
        recv(env.host.appearance_look(LOOK)).unwrap(),
        jf_plugin_host::Look::default()
    );
    let look = json!({
        "corner_scale": 9.0,
        "aurora": 0.25,
        "text_scale": 0.5,
        "density": "Roomy",
        "lyrics_highlight": { "Custom": 0xFF8800 },
        "sidebar": ["Search", "Sources", "Search"],
        "home": ["MostPlayed", "Summary", "MostPlayed"]
    });
    env.host
        .set_settings(
            LOOK,
            &json!({ "look": look.to_string(), "nan_glass": true }).to_string(),
        )
        .unwrap();
    let checked = recv(env.host.appearance_look(LOOK)).unwrap();
    assert_eq!(checked.corner_scale, Some(2.0));
    assert_eq!(checked.glass, None, "NaN is ignored");
    assert_eq!(checked.aurora, Some(0.25));
    assert_eq!(checked.text_scale, Some(0.85));
    assert_eq!(checked.density, Some(Density::Roomy));
    assert_eq!(checked.lyrics_highlight, Some(Highlight::Custom(0xFF8800)));
    assert_eq!(
        checked.sidebar,
        Some(vec![Place::Search, Place::Sources, Place::Albums])
    );
    assert_eq!(checked.home.map(|h| h.len()), Some(2));
}

#[test]
fn schema_profile_1_2_translations_reach_the_app() {
    let env = Env::new();
    let ja = r#"{"title": "見た目", "properties.corners.title": "角", "properties.corners.x-jf-group": "外観", "choices.lyrics": "歌詞"}"#;
    let hans = r#"{"properties.corners.title": "圆角"}"#;
    env.install_with(
        "appearance",
        &look_manifest(),
        LOOK,
        LOOK_SCHEMA,
        &[("l10n/ja.json", ja), ("l10n/zh-Hans.json", hans)],
    );
    let info = env.host.plugin(LOOK).unwrap();
    assert_eq!(info.languages, ["ja", "zh-Hans"]);
    let localized = env.host.localized_settings_schema(LOOK, "ja-JP").unwrap();
    let value: serde_json::Value = serde_json::from_str(&localized).unwrap();
    assert_eq!(value["title"], "見た目");
    assert_eq!(value["properties"]["corners"]["title"], "角");
    assert_eq!(value["properties"]["corners"]["x-jf-group"], "外観");
    assert_eq!(value["properties"]["layout"]["oneOf"][1]["title"], "歌詞");
    assert_eq!(
        value["properties"]["nan_glass"]["title"], "NaN Glass",
        "English where a word is missing"
    );
    let chinese = env.host.localized_settings_schema(LOOK, "zh-CN").unwrap();
    assert!(chinese.contains("圆角"));
    assert_eq!(
        env.host.localized_settings_schema(LOOK, "en").unwrap(),
        LOOK_SCHEMA
    );
    assert_eq!(env.host.localized_settings_schema("org.nope.nope", "ja"), None);
    // Profile 1.2 values validate.
    assert!(
        env.host
            .set_settings(LOOK, r#"{"corners": 1.5, "layout": "lyrics"}"#)
            .is_ok()
    );
    assert!(env.host.set_settings(LOOK, r#"{"corners": 1.3}"#).is_err());
    // A translation naming a setting the schema lacks fails the install.
    let path = package_with(
        &env.incoming(),
        "bad.jfpkg",
        &look_manifest(),
        component("appearance"),
        LOOK_SCHEMA,
        &[("l10n/ja.json", r#"{"properties.missing.title": "x"}"#)],
    );
    assert!(matches!(env.host.install(&path), Err(HostError::Manifest(_))));
}

const DISCOVER_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "server_url": { "type": "string", "format": "uri" },
        "hostile": { "type": "boolean", "default": false }
    }
}"#;

fn discover_env(server: &TestServer) -> Env {
    let env = Env::new();
    let manifest = format!(
        "id = {DISCOVER:?}\nname = \"Test Discover\"\nversion = \"1.0.0\"\napi_version = \"1.2\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"browse\", \"discover\", \"details\", \"comments\"]\n\
         playback_mode = \"stream\"\n\
         \n[network]\nallow = [\"${{settings.server_url}}\"]\n\
         \n[settings]\nschema = \"settings.schema.json\"\n\
         \n[content_policy]\nattribution = \"Invented Service\"\n"
    );
    env.install_with("discover", &manifest, DISCOVER, DISCOVER_SCHEMA, &[]);
    env.host
        .set_settings(DISCOVER, &json!({ "server_url": server.url() }).to_string())
        .unwrap();
    env
}

#[test]
fn discover_details_and_comments_are_checked() {
    let server = TestServer::start(|_| Response::new(200, "png"));
    let env = discover_env(&server);
    let host = &env.host;
    assert_eq!(
        recv(host.capabilities(DISCOVER)).unwrap(),
        vec![
            Capability::Browse,
            Capability::Discover,
            Capability::Details,
            Capability::Comments
        ]
    );

    let shelves = recv(host.discover(DISCOVER)).unwrap();
    assert_eq!(shelves.len(), 2);
    assert_eq!(shelves[0].title, "New Releases");
    assert_eq!(shelves[1].style, ShelfStyle::Chart);
    assert_eq!(shelves[1].more.as_deref(), Some("chart:top"));
    let art = shelves[0].items[0].artwork.as_ref().expect("on the allowlist");
    assert_eq!(art.headers[0].name, "x-art-key");

    let details = recv(host.details(DISCOVER, track("tr:1", "Invented Song 1"))).unwrap();
    assert_eq!(
        details.description.as_deref(),
        Some("Invented notes about Invented Song 1.")
    );
    assert_eq!(details.facts[0].key, FactKey::Released);
    assert_eq!(details.facts[2].key, FactKey::Other("Mood".into()));
    assert_eq!(details.comment_count, Some(3));
    assert!(details.image.is_some());

    let first = recv(host.comments(DISCOVER, track("tr:1", "x"), page(20))).unwrap();
    assert_eq!(first.hot.len(), 1);
    assert_eq!(first.comments.len(), 2);
    assert_eq!(first.total, Some(3));
    let second = recv(host.comments(
        DISCOVER,
        track("tr:1", "x"),
        jf_plugin_host::PageRequest {
            cursor: first.next.clone(),
            limit: 20,
        },
    ))
    .unwrap();
    assert_eq!(second.comments[0].id, "c3");
    assert_eq!(second.next, None);

    // Hostile answers are cut to the limits.
    host.set_settings(
        DISCOVER,
        &json!({ "server_url": server.url(), "hostile": true }).to_string(),
    )
    .unwrap();
    let shelves = recv(host.discover(DISCOVER)).unwrap();
    assert_eq!(shelves.len(), jf_plugin_host::sanitize::MAX_SHELVES);
    assert!(
        shelves
            .iter()
            .all(|s| s.items.len() == jf_plugin_host::sanitize::MAX_SHELF_ITEMS)
    );
    let details = recv(host.details(DISCOVER, track("tr:1", "x"))).unwrap();
    assert_eq!(details.facts.len(), jf_plugin_host::sanitize::MAX_FACTS);
    assert_eq!(details.image, None, "not on the allowlist");
    assert_eq!(
        details.description.map(|d| d.chars().count()),
        Some(jf_plugin_host::sanitize::MAX_DESCRIPTION)
    );
    let comments = recv(host.comments(DISCOVER, track("tr:1", "x"), page(50))).unwrap();
    assert_eq!(comments.hot.len(), 50);
    assert_eq!(comments.comments.len(), 50);
    assert_eq!(comments.comments[0].text.chars().count(), 2000);
}

#[test]
fn the_1_2_exports_need_their_capabilities() {
    let env = Env::new();
    let server = TestServer::start(|_| Response::new(404, ""));
    env.install("echo", &echo_manifest(), ECHO, Some(&server.url()));
    env.host.set_pro(true);
    assert!(
        matches!(recv(env.host.maker_plan(ECHO)), Err(CallError::NotDeclared(what)) if what == "make_lyrics")
    );
    assert!(matches!(
        recv(env.host.appearance_look(ECHO)),
        Err(CallError::NotDeclared(_))
    ));
    assert!(matches!(
        recv(env.host.discover(ECHO)),
        Err(CallError::NotDeclared(_))
    ));
    assert!(matches!(
        recv(env.host.details(ECHO, track("a", "b"))),
        Err(CallError::NotDeclared(_))
    ));
    assert!(matches!(
        recv(env.host.comments(ECHO, track("a", "b"), page(5))),
        Err(CallError::NotDeclared(_))
    ));

    // A component built for 1.0 has no maker: a manifest declaring one is refused.
    let path = package_with(
        &env.incoming(),
        "old.jfpkg",
        &maker_manifest(""),
        component("compat-1-0"),
        MAKER_SCHEMA,
        &[],
    );
    match env.host.install(&path) {
        Err(HostError::Incompatible(why)) => assert!(why.contains("declares make_lyrics"), "{why}"),
        other => panic!("expected an incompatible component, got {other:?}"),
    }
    // A 1.1 manifest cannot declare 1.2 capabilities.
    let old = maker_manifest("").replace("api_version = \"1.2\"", "api_version = \"1.1\"");
    let path = package_with(
        &env.incoming(),
        "v11.jfpkg",
        &old,
        component("maker"),
        MAKER_SCHEMA,
        &[],
    );
    assert!(matches!(env.host.install(&path), Err(HostError::Manifest(_))));
}

#[test]
fn a_plugin_built_for_1_1_runs_in_a_1_2_host() {
    let env = Env::new();
    let manifest = format!(
        "id = {COMPAT:?}\nname = \"Compat 1.1\"\nversion = \"1.0.0\"\napi_version = \"1.1\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"search\", \"remote\"]\nplayback_mode = \"remote\"\n\
         \n[settings]\nschema = \"settings.schema.json\"\n"
    );
    env.install("compat-1-1", &manifest, COMPAT, None);
    assert_eq!(titles(&env.host, COMPAT, "hello"), ["1.1 says hello"]);
    let devices = recv(env.host.remote_devices(COMPAT)).unwrap();
    assert_eq!(devices[0].name, "Compat Speaker");
    assert_eq!(
        recv(env.host.capabilities(COMPAT)).unwrap(),
        vec![Capability::Search, Capability::Remote]
    );
}

#[test]
fn components_import_audio_and_speech_only_when_they_use_them() {
    let interfaces = |name: &str| -> (Vec<String>, Vec<String>) {
        let wasm = component(name);
        let decoded = wit_component::decode(&wasm).unwrap();
        let wit_component::DecodedWasm::Component(resolve, world) = decoded else {
            panic!("not a component");
        };
        let world = &resolve.worlds[world];
        (
            world.imports.keys().map(|k| resolve.name_world_key(k)).collect(),
            world.exports.keys().map(|k| resolve.name_world_key(k)).collect(),
        )
    };
    let (imports, exports) = interfaces("echo");
    assert!(
        !imports
            .iter()
            .any(|i| i.contains("/audio@") || i.contains("/speech@")),
        "{imports:?}"
    );
    for export in [
        "maker",
        "appearance",
        "discover",
        "details",
        "comments",
        "remote",
        "meta",
    ] {
        assert!(
            exports
                .iter()
                .any(|e| e.starts_with(&format!("jellyfesh:plugin/{export}@1.2"))),
            "SDK 1.2 exports {export}: {exports:?}"
        );
    }
    let (imports, _) = interfaces("maker");
    assert!(
        imports.iter().any(|i| i == "jellyfesh:plugin/audio@1.2.0"),
        "{imports:?}"
    );
    assert!(
        imports.iter().any(|i| i == "jellyfesh:plugin/speech@1.2.0"),
        "{imports:?}"
    );
}

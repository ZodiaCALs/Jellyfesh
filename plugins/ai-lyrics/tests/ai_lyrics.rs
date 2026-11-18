//! The AI Lyrics plugin: the real component in the real host (Jellyfesh Pro
//! on), with the app's audio and recognition served by the testkit
//! (`TestAccess`: silence for audio, invented transcripts) and an
//! OpenAI-compatible provider mocked on 127.0.0.1 as a Custom server.
//! OpenAI and Groq themselves are never contacted from tests.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use jf_plugin_host::{
    AuthKind, AuthStep, CallError, Capability, LyricFormat, MakeJob, MakeSong, MakeStage, MakerMode,
    MakerNeed, Permission, PluginError, SpeechError,
};
use jf_plugin_testkit::*;
use serde_json::{Value, json};

const ID: &str = "org.jellyfesh.ai-lyrics";

fn dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A host with the plugin installed and Pro on.
fn env() -> Env {
    let env = Env::new();
    env.install_crate(dir(), "jf-ai-lyrics");
    env.host.set_pro(true);
    env
}

/// A job for `access`'s song, with plain lyrics or none, and consent for
/// these origins.
fn job(access: &Arc<TestAccess>, duration_ms: u64, lyrics: Option<&str>, consent: &[&str]) -> MakeJob {
    MakeJob {
        song: MakeSong {
            title: "Invented Song".into(),
            artist: Some("Invented Artist".into()),
            album: None,
            duration_ms,
            text: lyrics.map(str::to_owned),
            language: None,
        },
        access: access.clone(),
        allowed_origins: consent.iter().map(|o| (*o).to_string()).collect(),
        upload_to: consent.first().map(|o| (*o).to_string()),
        cancel: Arc::new(AtomicBool::new(false)),
    }
}

/// The parts of a `multipart/form-data` request: (name, content type, bytes).
fn multipart(request: &Request) -> Vec<(String, Option<String>, Vec<u8>)> {
    let content_type = request.header("content-type").unwrap_or_default();
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .expect("a multipart boundary")
        .to_string();
    let delimiter = format!("--{boundary}").into_bytes();
    let body = &request.body;
    let mut starts = Vec::new();
    let mut at = 0;
    while let Some(found) = body[at..]
        .windows(delimiter.len())
        .position(|w| w == delimiter.as_slice())
    {
        starts.push(at + found);
        at += found + delimiter.len();
    }
    let mut out = Vec::new();
    for pair in starts.windows(2) {
        let part = &body[pair[0] + delimiter.len() + 2..pair[1] - 2];
        let split = part
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("part headers");
        let headers = String::from_utf8_lossy(&part[..split]).to_string();
        let name = headers
            .split("name=\"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .unwrap_or_default()
            .to_string();
        let kind = headers
            .lines()
            .find_map(|l| l.strip_prefix("Content-Type: "))
            .map(str::to_owned);
        out.push((name, kind, part[split + 4..].to_vec()));
    }
    out
}

fn text_fields(request: &Request) -> Vec<(String, String)> {
    multipart(request)
        .into_iter()
        .filter(|(name, _, _)| name != "file")
        .map(|(name, _, value)| (name, String::from_utf8(value).unwrap()))
        .collect()
}

/// The made lyrics read through the core's parser: the words of each line.
fn words(text: &str) -> Vec<Vec<(String, u32)>> {
    let doc = jf_lyrics::parse(text, Some(jf_lyrics::Format::EnhancedLrc)).expect("enhanced LRC");
    assert_eq!(doc.timing, jf_lyrics::Timing::Word);
    doc.lines
        .iter()
        .filter(|l| !l.text.is_empty())
        .map(|l| l.words.iter().map(|w| (w.text.clone(), w.start_ms)).collect())
        .collect()
}

fn plugin_error<T: std::fmt::Debug>(result: Result<T, CallError>) -> PluginError {
    match result {
        Err(CallError::Plugin(error)) => error,
        other => panic!("expected a plugin error, got {other:?}"),
    }
}

fn service_text<T: std::fmt::Debug>(result: Result<T, CallError>) -> String {
    match plugin_error(result) {
        PluginError::Service(text) => text,
        other => panic!("expected a service error, got {other:?}"),
    }
}

#[test]
fn installs_as_an_extension_with_its_permissions_and_translations() {
    let env = env();
    let info = env.host.plugin(ID).unwrap();
    assert_eq!(info.name, "AI Lyrics");
    assert_eq!(info.permissions, vec![Permission::Audio, Permission::Speech]);
    assert_eq!(info.auth_kinds, vec![AuthKind::ApiKey]);
    assert_eq!(info.languages, ["ja", "zh-Hans", "zh-Hant"]);
    assert_eq!(
        recv(env.host.capabilities(ID)).unwrap(),
        vec![Capability::Auth, Capability::MakeLyrics]
    );
    assert_eq!(info.settings["mode"], "on-device");
    assert_eq!(info.settings["model"], "base");
    assert_eq!(info.settings["automatic"], false);
    let ja: Value = serde_json::from_str(&env.host.localized_settings_schema(ID, "ja-JP").unwrap()).unwrap();
    assert_eq!(ja["properties"]["mode"]["oneOf"][0]["title"], "このデバイスで");
    let hant: Value =
        serde_json::from_str(&env.host.localized_settings_schema(ID, "zh-TW").unwrap()).unwrap();
    assert_eq!(hant["properties"]["language"]["oneOf"][0]["title"], "自動偵測");
    // Making lyrics is Pro: without it the host answers for the plugin.
    env.host.set_pro(false);
    assert_eq!(recv(env.host.maker_plan(ID)), Err(CallError::ProRequired));
}

#[test]
fn plans_follow_the_settings_the_models_and_the_key() {
    let server = TestServer::start(|request| match request.path() {
        "/v1/models" => Response::json(&json!({ "object": "list", "data": [] })),
        _ => Response::text(404, "not found"),
    });
    let env = env();
    let plan = || recv(env.host.maker_plan(ID)).unwrap();

    // On the device.
    env.host.set_speech_models(Vec::new());
    let p = plan();
    assert_eq!(
        (p.mode, p.upload_to.clone(), p.automatic),
        (MakerMode::OnDevice, None, false)
    );
    assert!(matches!(p.need, MakerNeed::Unavailable(why) if why.contains("Online")));
    env.host.set_speech_models(speech_models(&[]));
    let p = plan();
    assert_eq!(p.engine, "Whisper Base on this device");
    assert_eq!(p.need, MakerNeed::Model("base".into()));
    env.host.set_speech_models(speech_models(&["base"]));
    assert_eq!(plan().need, MakerNeed::Nothing);
    env.settings(ID, json!({ "model": "large-v3-turbo-q5_0", "automatic": true }));
    let p = plan();
    assert_eq!(p.engine, "Whisper Large Turbo on this device");
    assert_eq!(p.need, MakerNeed::Model("large-v3-turbo-q5_0".into()));
    assert!(p.automatic);

    // Online, at OpenAI and Groq: a key is needed first.
    env.settings(ID, json!({ "mode": "online" }));
    let p = plan();
    assert_eq!(p.mode, MakerMode::Online);
    assert_eq!(p.engine, "whisper-1 at api.openai.com");
    assert_eq!(p.upload_to.as_deref(), Some("https://api.openai.com"));
    assert_eq!(p.need, MakerNeed::SignIn);
    env.settings(ID, json!({ "mode": "online", "provider": "groq" }));
    let p = plan();
    assert_eq!(p.engine, "whisper-large-v3-turbo at api.groq.com");
    assert_eq!(p.upload_to.as_deref(), Some("https://api.groq.com"));
    assert_eq!(p.need, MakerNeed::SignIn);
    env.settings(
        ID,
        json!({ "mode": "online", "provider": "groq", "online_model": "whisper-large-v3" }),
    );
    assert_eq!(plan().engine, "whisper-large-v3 at api.groq.com");

    // A Custom server: its address first; a key only if it asks for one.
    env.settings(ID, json!({ "mode": "online", "provider": "custom" }));
    assert!(matches!(plan().need, MakerNeed::Unavailable(why) if why.contains("Server Address")));
    let base = format!("{}/v1", server.url());
    env.settings(
        ID,
        json!({ "mode": "online", "provider": "custom", "server_url": base }),
    );
    let p = plan();
    assert_eq!(p.upload_to.as_deref(), Some(server.url().as_str()));
    assert_eq!(
        p.engine,
        format!("whisper-1 at {}", &server.url()["http://".len()..])
    );
    assert_eq!(p.need, MakerNeed::Nothing);

    // A key entered for the Custom server is not OpenAI's.
    let flow = login(&env.host, ID, AuthKind::ApiKey);
    assert!(matches!(next_step(&flow), AuthStep::AskFields { .. }));
    flow.submit(fields(&[("api-key", "test-key-1234")]));
    assert!(matches!(final_step(&flow), AuthStep::Done { account } if account.ends_with("…1234")));
    assert_eq!(
        server.requests_to("/v1/models")[0].header("authorization"),
        Some("Bearer test-key-1234")
    );
    env.settings(ID, json!({ "mode": "online", "provider": "openai" }));
    assert_eq!(
        plan().need,
        MakerNeed::SignIn,
        "the key belongs to the Custom server"
    );
}

#[test]
fn sign_in_checks_the_key_with_the_provider() {
    let server = TestServer::start(|request| match request.header("authorization") {
        Some("Bearer good-key") => Response::json(&json!({ "data": [] })),
        _ => Response::new(401, "{\"error\":{\"message\":\"Incorrect API key provided.\"}}"),
    });
    let env = env();
    env.settings(
        ID,
        json!({ "mode": "online", "provider": "custom", "server_url": format!("{}/v1", server.url()) }),
    );
    let flow = login(&env.host, ID, AuthKind::ApiKey);
    next_step(&flow);
    flow.submit(fields(&[("api-key", "bad-key")]));
    match final_step(&flow) {
        AuthStep::Failed { message } => assert!(message.contains("did not accept this API key"), "{message}"),
        other => panic!("expected a failure, got {other:?}"),
    }
    let flow = login(&env.host, ID, AuthKind::ApiKey);
    next_step(&flow);
    flow.submit(fields(&[("api-key", "has spaces in it")]));
    assert!(matches!(final_step(&flow), AuthStep::Failed { .. }));
    assert!(env.secrets_are_empty());

    let flow = login(&env.host, ID, AuthKind::ApiKey);
    next_step(&flow);
    flow.submit(fields(&[("api-key", "good-key")]));
    assert!(matches!(final_step(&flow), AuthStep::Done { .. }));
    // Kept in the keystore, nowhere in the plugin's files.
    assert!(env.files_containing("good-key").is_empty());
}

trait Secrets {
    fn secrets_are_empty(&self) -> bool;
}

impl Secrets for Env {
    fn secrets_are_empty(&self) -> bool {
        use jf_plugin_host::SecretStore;
        self.secrets
            .get(&format!("jellyfesh/{ID}/api-key"))
            .ok()
            .flatten()
            .is_none()
    }
}

#[test]
fn on_the_device_the_apps_recogniser_makes_the_lyrics() {
    let env = env();
    env.host.set_speech_models(speech_models(&["base"]));
    let access = Arc::new(TestAccess::new(60_000).transcript(transcript(&[
        (2000, &["Invented", "words", "here"]),
        (6000, &["and", "a", "second", "line"]),
    ])));
    let made = recv(env.host.make_lyrics(ID, job(&access, 60_000, None, &[]))).unwrap();
    assert_eq!(made.engine, "Whisper Base on this device");
    assert_eq!(made.lyrics.format, LyricFormat::Elrc);
    assert_eq!(
        made.lyrics.text,
        "[00:02.00]<00:02.00>Invented <00:02.40>words <00:02.80>here<00:03.15>\n\
         [00:06.00]<00:06.00>and <00:06.40>a <00:06.80>second <00:07.20>line<00:07.55>\n"
    );
    assert_eq!(words(&made.lyrics.text)[1][2], ("second".to_string(), 6800));
    assert_eq!(access.stages(), [MakeStage::Recognising, MakeStage::TimingWords]);
    assert!(access.wav_requests().is_empty(), "nothing is read in the plugin");
    let options = access.options();
    assert_eq!(options[0].model, "base");
    assert_eq!(
        (options[0].language.clone(), options[0].prompt.clone()),
        (None, None)
    );
}

#[test]
fn plain_lyrics_are_timed_and_steer_recognition() {
    let env = env();
    env.host.set_speech_models(speech_models(&["small"]));
    env.settings(ID, json!({ "model": "small", "language": "en" }));
    let access = Arc::new(TestAccess::new(60_000).transcript(transcript(&[
        (3000, &["walking", "down", "the", "river", "road"]),
        (9000, &["under", "the", "silver", "sky"]),
    ])));
    let lyrics = "[Verse]\nWalking down the river road,\nUnder the silver moon\n";
    let made = recv(env.host.make_lyrics(ID, job(&access, 60_000, Some(lyrics), &[]))).unwrap();
    assert_eq!(made.engine, "Whisper Small on this device");
    let lines = words(&made.lyrics.text);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0][0], ("Walking".to_string(), 3000));
    assert_eq!(lines[0][4], ("road,".to_string(), 4600));
    assert_eq!(
        lines[1][3].0, "moon",
        "the lyrics' words, timed where the recogniser heard sky"
    );
    assert_eq!(lines[1][3].1, 10_200);
    assert_eq!(made.lyrics.language.as_deref(), Some("en"));
    let options = access.options();
    assert_eq!(options[0].language.as_deref(), Some("en"));
    assert_eq!(
        options[0].prompt.as_deref(),
        Some("Walking down the river road, Under the silver moon")
    );
}

#[test]
fn on_device_failures_say_what_to_do() {
    let env = env();
    env.host.set_speech_models(speech_models(&["base"]));
    // Silence: nothing recognised.
    let access = Arc::new(TestAccess::new(30_000));
    let text = service_text(recv(env.host.make_lyrics(ID, job(&access, 30_000, None, &[]))));
    assert!(text.contains("No singing was recognised"), "{text}");
    let missing =
        Arc::new(TestAccess::new(30_000).recognition_fails(SpeechError::ModelMissing("base".into())));
    let text = service_text(recv(env.host.make_lyrics(ID, job(&missing, 30_000, None, &[]))));
    assert!(text.contains("Base model is not downloaded"), "{text}");
    let elsewhere =
        Arc::new(TestAccess::new(30_000).transcript(transcript(&[(1000, &["paper", "lanterns", "glow"])])));
    let text = service_text(recv(env.host.make_lyrics(
        ID,
        job(
            &elsewhere,
            30_000,
            Some("Completely different\nwords entirely written here\n"),
            &[],
        ),
    )));
    assert!(text.contains("do not match its lyrics"), "{text}");
}

/// A provider that answers each request with the next of `answers`.
fn provider(answers: Vec<Value>) -> TestServer {
    let answers = std::sync::Mutex::new(answers.into_iter());
    TestServer::start(move |request| match request.path() {
        "/v1/audio/transcriptions" => match answers.lock().unwrap().next() {
            Some(answer) => Response::json(&answer),
            None => Response::text(500, "no more answers"),
        },
        "/v1/models" => Response::json(&json!({ "data": [] })),
        _ => Response::text(404, "{\"detail\":\"Not Found\"}"),
    })
}

/// A host set to the Custom provider at `server`, with a key stored.
fn online(server: &TestServer, key: bool) -> Env {
    let env = env();
    env.settings(
        ID,
        json!({ "mode": "online", "provider": "custom", "server_url": format!("{}/v1", server.url()), "language": "en" }),
    );
    if key {
        let flow = login(&env.host, ID, AuthKind::ApiKey);
        next_step(&flow);
        flow.submit(fields(&[("api-key", "sk-test-online")]));
        assert!(matches!(final_step(&flow), AuthStep::Done { .. }));
    }
    env
}

#[test]
fn online_the_audio_goes_to_the_provider_after_consent() {
    let server = provider(vec![json!({
        "task": "transcribe", "language": "english", "duration": 12.0, "text": "Hello there again",
        "words": [
            { "word": "Hello", "start": 1.0, "end": 1.4 },
            { "word": "there", "start": 1.5, "end": 2.0 },
            { "word": "again", "start": 4.0, "end": 4.6 }
        ],
        "segments": [
            { "id": 0, "start": 0.9, "end": 2.1, "text": " Hello there", "no_speech_prob": 0.01, "avg_logprob": -0.2 },
            { "id": 1, "start": 3.9, "end": 4.7, "text": " again", "no_speech_prob": 0.02, "avg_logprob": -0.3 }
        ]
    })]);
    let env = online(&server, true);
    let origin = server.url();

    // No consent yet: no audio, nothing sent.
    let access = Arc::new(TestAccess::new(12_000));
    let refused = recv(env.host.make_lyrics(ID, job(&access, 12_000, None, &[])));
    assert!(service_text(refused).contains("has not been allowed"));
    assert_eq!(server.count("/v1/audio/transcriptions"), 0);

    let made = recv(
        env.host
            .make_lyrics(ID, job(&access, 12_000, Some("Hello there\nAgain\n"), &[&origin])),
    )
    .unwrap();
    assert_eq!(
        made.engine,
        format!("whisper-1 at {}", &origin["http://".len()..])
    );
    assert_eq!(
        made.lyrics.text,
        "[00:01.00]<00:01.00>Hello <00:01.50>there<00:02.00>\n[00:04.00]<00:04.00>Again<00:04.60>\n"
    );
    let request = &server.requests_to("/v1/audio/transcriptions")[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.header("authorization"), Some("Bearer sk-test-online"));
    assert_eq!(
        text_fields(request),
        [
            ("model".to_string(), "whisper-1".to_string()),
            ("response_format".into(), "verbose_json".into()),
            ("timestamp_granularities[]".into(), "word".into()),
            ("timestamp_granularities[]".into(), "segment".into()),
            ("temperature".into(), "0".into()),
            ("language".into(), "en".into()),
            ("prompt".into(), "Hello there Again".into()),
        ]
    );
    let file = multipart(request)
        .into_iter()
        .find(|(name, _, _)| name == "file")
        .unwrap();
    assert_eq!(file.1.as_deref(), Some("audio/wav"));
    assert!(file.2.starts_with(b"RIFF") && file.2.len() == 44 + 12_000 * 32);
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
    assert_eq!(access.wav_requests(), [(0, 12_000)]);
}

#[test]
fn online_segments_without_words_are_spread() {
    let server = provider(vec![json!({
        "text": "One two three four",
        "segments": [{ "start": 2.0, "end": 4.0, "text": " One two three four" }]
    })]);
    let env = online(&server, false);
    let access = Arc::new(TestAccess::new(10_000));
    let made = recv(
        env.host
            .make_lyrics(ID, job(&access, 10_000, None, &[&server.url()])),
    )
    .unwrap();
    assert_eq!(
        words(&made.lyrics.text),
        [vec![
            ("One".to_string(), 2000),
            ("two".into(), 2400),
            ("three".into(), 2800),
            ("four".into(), 3460)
        ]]
    );
    let request = &server.requests_to("/v1/audio/transcriptions")[0];
    assert_eq!(request.header("authorization"), None, "no key stored, none sent");
}

#[test]
fn online_errors_are_the_providers() {
    let consent = |server: &TestServer| server.url();
    let failing = TestServer::start(|request| match request.path() {
        "/v1/audio/transcriptions" => {
            Response::text(500, "{\"error\":{\"message\":\"The server had an error\"}}")
        }
        _ => Response::json(&json!({ "data": [] })),
    });
    let env = online(&failing, true);
    let access = Arc::new(TestAccess::new(10_000));
    let origin = consent(&failing);
    let text = service_text(recv(
        env.host.make_lyrics(ID, job(&access, 10_000, None, &[&origin])),
    ));
    assert!(
        text.contains("HTTP 500") && text.contains("The server had an error"),
        "{text}"
    );

    let refusing = TestServer::start(|request| match request.path() {
        "/v1/audio/transcriptions" => {
            Response::text(401, "{\"error\":{\"message\":\"Incorrect API key provided\"}}")
        }
        _ => Response::json(&json!({ "data": [] })),
    });
    let env = online(&refusing, true);
    let origin = consent(&refusing);
    assert_eq!(
        plugin_error(recv(
            env.host.make_lyrics(ID, job(&access, 10_000, None, &[&origin]))
        )),
        PluginError::AuthRequired,
        "a key the provider no longer takes asks for Sign In again"
    );

    let elsewhere = TestServer::start(|_| Response::text(404, "{\"detail\":\"Not Found\"}"));
    let wrong_address = self::env();
    wrong_address.settings(
        ID,
        json!({ "mode": "online", "provider": "custom", "server_url": format!("{}/api", elsewhere.url()) }),
    );
    let origin = consent(&elsewhere);
    let text = service_text(recv(
        wrong_address
            .host
            .make_lyrics(ID, job(&access, 10_000, None, &[&origin])),
    ));
    assert!(text.contains("Server Address"), "{text}");

    let text_only = provider(vec![json!({ "text": "words without times" })]);
    let env = online(&text_only, false);
    let origin = consent(&text_only);
    let text = service_text(recv(
        env.host.make_lyrics(ID, job(&access, 10_000, None, &[&origin])),
    ));
    assert!(text.contains("gave no times"), "{text}");
}

#[test]
fn online_long_songs_go_in_overlapping_parts_and_words_count_once() {
    // A Custom server gets two-minute parts: 0–120 s, 118–238 s, 236–300 s.
    let word = |text: &str, start: f64| json!({ "word": text, "start": start, "end": start + 0.3 });
    let server = provider(vec![
        json!({ "words": [word("one", 1.0), word("two", 118.5)] }),
        json!({ "words": [word("two", 0.5), word("three", 60.0), word("four", 118.6)] }),
        json!({ "words": [word("four", 0.6), word("five", 30.0)] }),
    ]);
    let env = online(&server, false);
    let access = Arc::new(TestAccess::new(300_000));
    let made = recv(
        env.host
            .make_lyrics(ID, job(&access, 300_000, None, &[&server.url()])),
    )
    .unwrap();
    assert_eq!(
        access.wav_requests(),
        [(0, 120_000), (118_000, 120_000), (236_000, 64_000)]
    );
    assert_eq!(server.count("/v1/audio/transcriptions"), 3);
    let all: Vec<(String, u32)> = words(&made.lyrics.text).into_iter().flatten().collect();
    assert_eq!(
        all,
        [
            ("one".to_string(), 1000),
            ("two".into(), 118_500),
            ("three".into(), 178_000),
            ("four".into(), 236_600),
            ("five".into(), 266_000)
        ]
    );
}

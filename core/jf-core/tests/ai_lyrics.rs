//! AI Lyrics through the core, with the maker test plugin: on this device (a recogniser made up
//! here hears the same invented words in any audio; the speech model is a few invented bytes
//! downloaded from a local server and verified by their SHA-1 like a real one) and online (a
//! local server answers with invented words), consent, the job queue, cancelling, made lyrics
//! shown and deleted, and automatic lyrics. The songs are generated tones.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use common::*;
use jf_core::{
    AiLyricsMode, AiLyricsNeed, Command, Config, Event, LyricsJobStage, LyricsSource, ProConfig, SecretStore,
    SongRef, SpeechModelStatus, TrackRef,
};
use jf_speech::{ModelInfo, ModelStore, Options, Recognizer, Segment, SpeechError, Transcript, Word};
use serde_json::json;

const MAKER: &str = "org.jellyfesh.test.maker";

const MAKER_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "mode": { "type": "string", "enum": ["on-device", "online"], "default": "on-device" },
        "model": { "type": "string", "default": "base" },
        "server_url": { "type": "string", "format": "uri" },
        "other_url": { "type": "string", "format": "uri" }
    }
}"#;

fn maker_manifest() -> String {
    format!(
        "id = {MAKER:?}\nname = \"Test Maker\"\nversion = \"1.0.0\"\napi_version = \"1.2\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"make_lyrics\"]\nplayback_mode = \"stream\"\n\
         permissions = [\"audio\", \"speech\"]\n\
         \n[network]\nallow = [\"${{settings.server_url}}\", \"${{settings.other_url}}\"]\nmax_rps = 20\n\
         \n[settings]\nschema = \"settings.schema.json\"\n"
    )
}

/// The invented model's bytes and their SHA-1.
const MODEL_BYTES: &[u8] = b"an invented speech model for the core tests\n";
const MODEL_SHA1: &str = "0f3a798d131ba61d18c20334e466337ce5e984f2";

/// Two models made of the invented bytes: `tiny` (which the test plugin makes lyrics with by
/// itself) and `base`.
fn models() -> &'static [ModelInfo] {
    let model = |id: &'static str, name: &'static str, file: &'static str| ModelInfo {
        id,
        name,
        file,
        url: "http://test.invalid/model.bin",
        approx_bytes: MODEL_BYTES.len() as u64,
        sha1: MODEL_SHA1,
        sha256: None,
        multilingual: true,
    };
    Box::leak(Box::new([
        model("tiny", "Tiny", "test-tiny.bin"),
        model("base", "Base", "test-base.bin"),
    ]))
}

/// A recogniser that hears "invented words here" in any audio, after it saw some.
#[derive(Default)]
struct Hears {
    calls: AtomicUsize,
    /// Waits until the job is cancelled.
    stall: AtomicBool,
}

impl Recognizer for Hears {
    fn transcribe(
        &self,
        model: &ModelInfo,
        model_path: &Path,
        pcm: &[f32],
        _options: &Options,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Transcript, SpeechError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(model_path.ends_with(model.file), "the verified model");
        assert!(pcm.len() > 16_000, "the song, decoded to 16 kHz");
        progress(0.5);
        while self.stall.load(Ordering::SeqCst) {
            if cancel.load(Ordering::SeqCst) {
                return Err(SpeechError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let word = |text: &str, start_ms, end_ms| Word {
            text: text.into(),
            start_ms,
            end_ms,
            confidence: 0.9,
        };
        Ok(Transcript {
            language: Some("en".into()),
            segments: vec![Segment {
                text: "invented words here".into(),
                start_ms: 1_000,
                end_ms: 2_500,
                words: vec![
                    word("invented", 1_000, 1_400),
                    word("words", 1_500, 1_900),
                    word("here", 2_000, 2_500),
                ],
            }],
        })
    }
}

/// The models' server.
fn model_server() -> TestServer {
    TestServer::start(|req| {
        if req.path().starts_with("/models/") {
            Response::ranged(req, MODEL_BYTES, "application/octet-stream")
        } else {
            Response::new(404, "")
        }
    })
}

/// A core with Pro, the recogniser and the models of `server`.
fn ai_env(server: &TestServer, hears: Arc<Hears>) -> Env {
    let base = format!("{}/models", server.url());
    Env::with(move |c: &mut Config| {
        c.pro = ProConfig {
            preview: true,
            licence_keys: Vec::new(),
        };
        c.speech = Some(hears as Arc<dyn Recognizer>);
        c.speech_store = Some(ModelStore::with_models(
            c.data_dir.join("speech-models"),
            &base,
            models(),
        ));
        c.update_url = "http://127.0.0.1:9/releases".into();
        if let jf_core::OutputKind::WavFile { realtime, .. } = &mut c.output {
            *realtime = true;
        }
    })
}

/// A generated tone, long enough to stay the song playing.
fn tone(env: &Env, name: &str) -> PathBuf {
    let path = env.dir.path().join(name);
    sine_wav(&path, 440.0, 20.0);
    path
}

fn play(env: &Env, path: &Path) {
    env.run(
        Command::PlayTracks {
            tracks: vec![TrackRef::File(path.to_owned())],
            start: 0,
        },
        |e| matches!(e, Event::Lyrics(_)),
    );
}

fn need(env: &Env) -> AiLyricsNeed {
    env.core.snapshot().ai_lyrics.need.clone()
}

#[test]
fn lyrics_made_on_this_device_show_until_deleted() {
    let server = model_server();
    let hears = Arc::new(Hears::default());
    let env = ai_env(&server, hears.clone());
    assert!(!env.core.snapshot().ai_lyrics.available);
    env.install_with_schema("maker", &maker_manifest(), MAKER, MAKER_SCHEMA);
    env.settings(MAKER, json!({ "mode": "on-device", "model": "base" }));
    assert!(until(Duration::from_secs(20), || matches!(
        need(&env),
        AiLyricsNeed::Model { .. }
    )));
    let ai = env.core.snapshot().ai_lyrics.clone();
    assert!(ai.available);
    assert_eq!(ai.plugin.as_deref(), Some(MAKER));
    assert_eq!(ai.plan.as_ref().map(|p| p.mode), Some(AiLyricsMode::OnDevice));
    assert_eq!(
        need(&env),
        AiLyricsNeed::Model {
            id: "base".into(),
            name: "Base".into(),
            bytes: MODEL_BYTES.len() as u64
        }
    );
    let song = tone(&env, "invented-tone.wav");
    play(&env, &song);
    assert!(
        env.fails(Command::MakeLyrics {
            song: SongRef::Current
        })
        .contains("download")
    );

    // The model, downloaded and verified.
    env.run(Command::DownloadSpeechModel { id: "base".into() }, |e| {
        matches!(e, Event::AiLyrics(a) if a.models.iter().any(|m| m.id == "base" && m.state == SpeechModelStatus::Ready))
    });
    assert!(until(Duration::from_secs(20), || need(&env) == AiLyricsNeed::Nothing));
    assert_eq!(server.count("/models/test-base.bin"), 1);

    env.run(
        Command::MakeLyrics {
            song: SongRef::Current,
        },
        |e| matches!(e, Event::Lyrics(l) if matches!(l.source, Some(LyricsSource::Made { .. }))),
    );
    let lyrics = env.core.snapshot().lyrics.clone().unwrap();
    assert_eq!(
        lyrics.source,
        Some(LyricsSource::Made {
            plugin: MAKER.into(),
            engine: "test-whisper on this device".into()
        })
    );
    let doc = lyrics.doc.as_ref().unwrap();
    assert_eq!(doc.lines.len(), 1);
    assert!(lyrics.timeline.as_ref().is_some_and(|t| t.word_starts.len() == 3));
    let jobs = env.core.snapshot().ai_lyrics.jobs.clone();
    assert_eq!(jobs.last().map(|j| &j.stage), Some(&LyricsJobStage::Done));
    assert_eq!(jobs.last().map(|j| j.title.as_str()), Some("invented-tone"));
    assert_eq!(hears.calls.load(Ordering::SeqCst), 1);
    let kept = env.dir.path().join("data/made-lyrics");
    assert_eq!(
        std::fs::read_dir(&kept).unwrap().count(),
        2,
        "the lyrics and who made them"
    );

    // Shown again when the song plays again.
    env.run(Command::Stop, |e| matches!(e, Event::NowPlaying(None)));
    play(&env, &song);
    assert!(matches!(
        env.core.snapshot().lyrics.as_ref().and_then(|l| l.source.clone()),
        Some(LyricsSource::Made { .. })
    ));

    // Deleted: the song has no lyrics again.
    env.run(
        Command::DeleteMadeLyrics {
            song: SongRef::Current,
        },
        |e| matches!(e, Event::Lyrics(l) if l.source.is_none()),
    );
    assert_eq!(std::fs::read_dir(&kept).unwrap().count(), 0);
    assert!(
        env.fails(Command::DeleteMadeLyrics {
            song: SongRef::Current
        })
        .contains("no lyrics made")
    );

    // The model, deleted.
    env.run(Command::DeleteSpeechModel { id: "base".into() }, |e| {
        matches!(e, Event::AiLyrics(a) if a.models.iter().any(|m| m.id == "base" && m.state == SpeechModelStatus::NotDownloaded))
    });
    assert!(until(Duration::from_secs(20), || matches!(
        need(&env),
        AiLyricsNeed::Model { .. }
    )));
}

#[test]
fn jobs_wait_their_turn_and_can_be_cancelled() {
    let server = model_server();
    let hears = Arc::new(Hears::default());
    let env = ai_env(&server, hears.clone());
    env.install_with_schema("maker", &maker_manifest(), MAKER, MAKER_SCHEMA);
    env.settings(MAKER, json!({ "mode": "on-device", "model": "base" }));
    env.run(Command::DownloadSpeechModel { id: "base".into() }, |e| {
        matches!(e, Event::AiLyrics(a) if a.models.iter().any(|m| m.id == "base" && m.state == SpeechModelStatus::Ready))
    });
    assert!(until(Duration::from_secs(20), || need(&env) == AiLyricsNeed::Nothing));
    // Two library songs.
    let music = env.dir.path().join("music");
    std::fs::create_dir_all(&music).unwrap();
    sine_wav(&music.join("one.wav"), 440.0, 3.0);
    sine_wav(&music.join("two.wav"), 550.0, 3.0);
    env.run(Command::AddRoot { path: music }, |e| {
        matches!(e, Event::Scan(jf_core::ScanStatus::Finished(_)))
    });
    let ids: Vec<i64> = env
        .core
        .tracks(&jf_core::TrackQuery::default())
        .unwrap()
        .iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(ids.len(), 2);

    hears.stall.store(true, Ordering::SeqCst);
    env.run(Command::MakeLyrics {
        song: SongRef::Library { id: ids[0] },
    }, |e| matches!(e, Event::AiLyrics(a) if a.jobs.iter().any(|j| matches!(j.stage, LyricsJobStage::Recognising { .. }))));
    env.run(
        Command::MakeLyrics {
            song: SongRef::Library { id: ids[1] },
        },
        |e| matches!(e, Event::AiLyrics(a) if a.jobs.len() == 2),
    );
    let jobs = env.core.snapshot().ai_lyrics.jobs.clone();
    assert_eq!(jobs[1].stage, LyricsJobStage::Waiting, "one at a time");
    assert_eq!(jobs[0].library_id, Some(ids[0]));
    // The same song again keeps its job.
    env.core.send(Command::MakeLyrics {
        song: SongRef::Library { id: ids[1] },
    });
    // Cancelling the waiting one, then the running one.
    let (first, second) = (jobs[0].id, jobs[1].id);
    env.run(Command::CancelLyrics { job: second }, |e| {
        matches!(e, Event::AiLyrics(a) if a.jobs.iter().any(|j| j.id == second && j.stage == LyricsJobStage::Cancelled))
    });
    env.run(Command::CancelLyrics { job: first }, |e| {
        matches!(e, Event::AiLyrics(a) if a.jobs.iter().any(|j| j.id == first && j.stage == LyricsJobStage::Cancelled))
    });
    let jobs = env.core.snapshot().ai_lyrics.jobs.clone();
    assert_eq!(jobs.len(), 2, "no third job for the same song");
    assert!(
        env.fails(Command::CancelLyrics { job: 999 })
            .contains("no lyrics job")
    );
    assert_eq!(
        std::fs::read_dir(env.dir.path().join("data/made-lyrics"))
            .map(|d| d.count())
            .unwrap_or(0),
        0
    );
}

#[test]
fn online_lyrics_need_a_key_and_consent() {
    // The provider: answers with invented words for the WAV it is sent.
    let server = TestServer::start(|req| match req.path() {
        "/v1/audio/transcriptions" if req.body.starts_with(b"RIFF") => Response::json(json!({
            "words": [ { "word": "invented", "start": 1.0 }, { "word": "online", "start": 1.5 } ]
        })),
        _ => Response::new(404, ""),
    });
    let env = ai_env(&server, Arc::new(Hears::default()));
    env.install_with_schema("maker", &maker_manifest(), MAKER, MAKER_SCHEMA);
    env.settings(
        MAKER,
        json!({ "mode": "online", "server_url": server.url(), "other_url": "https://elsewhere.invalid" }),
    );
    assert!(until(Duration::from_secs(20), || need(&env) == AiLyricsNeed::SignIn));

    // The person's API key (as AI Lyrics settings would keep it), then the plan again.
    env.secrets
        .set(&format!("jellyfesh/{MAKER}/api-key"), "invented-key")
        .unwrap();
    env.settings(
        MAKER,
        json!({ "mode": "online", "server_url": server.url(), "other_url": "https://elsewhere.invalid", "model": "tiny" }),
    );
    let origin = server.url().trim_end_matches('/').to_owned();
    assert!(until(Duration::from_secs(20), || need(&env)
        == AiLyricsNeed::Consent {
            origin: origin.clone()
        }));
    let song = tone(&env, "invented-online.wav");
    play(&env, &song);
    assert!(
        env.fails(Command::MakeLyrics {
            song: SongRef::Current
        })
        .contains("allow sending")
    );
    assert_eq!(
        server.count("/v1/audio/transcriptions"),
        0,
        "nothing sent before consent"
    );

    env.run(
        Command::AllowAudioUpload {
            plugin: MAKER.into(),
            origin: origin.clone(),
        },
        |e| matches!(e, Event::AiLyrics(a) if a.need == AiLyricsNeed::Nothing),
    );
    let events = env.run(
        Command::MakeLyrics {
            song: SongRef::Current,
        },
        |e| matches!(e, Event::Lyrics(l) if matches!(l.source, Some(LyricsSource::Made { .. }))),
    );
    let stages: Vec<LyricsJobStage> = events
        .iter()
        .filter_map(|e| match e {
            Event::AiLyrics(a) => a.jobs.last().map(|j| j.stage.clone()),
            _ => None,
        })
        .collect();
    assert!(
        stages.contains(&LyricsJobStage::Sending {
            origin: origin.clone()
        }),
        "{stages:?}"
    );
    assert_eq!(server.count("/v1/audio/transcriptions"), 1);
    let consents = env.core.snapshot().ai_lyrics.consents.clone();
    assert_eq!(consents.len(), 1);
    assert_eq!(consents[0].origin, origin);

    // Stop Allowing: kept across restarts until then.
    env.run(
        Command::RevokeAudioUpload {
            plugin: MAKER.into(),
            origin: origin.clone(),
        },
        |e| matches!(e, Event::AiLyrics(a) if a.consents.is_empty()),
    );
    assert!(until(Duration::from_secs(20), || matches!(
        need(&env),
        AiLyricsNeed::Consent { .. }
    )));
    assert!(
        env.fails(Command::AllowAudioUpload {
            plugin: MAKER.into(),
            origin: "not an origin at all".into()
        })
        .contains("not an origin")
    );
}

#[test]
fn songs_without_synced_lyrics_get_them_by_themselves() {
    let server = model_server();
    let hears = Arc::new(Hears::default());
    let env = ai_env(&server, hears.clone());
    env.install_with_schema("maker", &maker_manifest(), MAKER, MAKER_SCHEMA);
    // With `tiny` the test plugin asks to make lyrics by itself.
    env.settings(MAKER, json!({ "mode": "on-device", "model": "tiny" }));
    env.run(Command::DownloadSpeechModel { id: "tiny".into() }, |e| {
        matches!(e, Event::AiLyrics(a) if a.models.iter().any(|m| m.id == "tiny" && m.state == SpeechModelStatus::Ready))
    });
    assert!(until(Duration::from_secs(20), || {
        let ai = env.core.snapshot().ai_lyrics.clone();
        ai.need == AiLyricsNeed::Nothing && ai.plan.as_ref().is_some_and(|p| p.automatic)
    }));
    let song = tone(&env, "invented-automatic.wav");
    env.core.send(Command::PlayTracks {
        tracks: vec![TrackRef::File(song)],
        start: 0,
    });
    wait_for(
        &env.events,
        Duration::from_secs(60),
        |e| matches!(e, Event::Lyrics(l) if matches!(l.source, Some(LyricsSource::Made { .. }))),
    );
    assert_eq!(hears.calls.load(Ordering::SeqCst), 1);

    // Without Pro nothing is made, and the need says so.
    let env = env.restart_with(|c| {
        c.pro = ProConfig {
            preview: false,
            licence_keys: Vec::new(),
        };
        c.update_url = "http://127.0.0.1:9/releases".into();
    });
    assert!(until(Duration::from_secs(10), || need(&env) == AiLyricsNeed::Pro));
    assert!(env.core.snapshot().ai_lyrics.plan.is_none());
}

#[test]
fn turning_pro_off_stops_the_jobs() {
    let server = model_server();
    let hears = Arc::new(Hears::default());
    let (secret, public) = jf_licence::generate_keypair_with_id(5);
    let key = jf_licence::issue(&secret, 5, &jf_licence::new_licence(20_409));
    let base = format!("{}/models", server.url());
    let recogniser = hears.clone();
    let env = Env::with(move |c: &mut Config| {
        c.pro = ProConfig {
            preview: false,
            licence_keys: vec![public],
        };
        c.speech = Some(recogniser as Arc<dyn Recognizer>);
        c.speech_store = Some(ModelStore::with_models(
            c.data_dir.join("speech-models"),
            &base,
            models(),
        ));
        c.update_url = "http://127.0.0.1:9/releases".into();
    });
    env.run(
        Command::ActivateLicence { key },
        |e| matches!(e, Event::Pro(p) if p.active),
    );
    env.install_with_schema("maker", &maker_manifest(), MAKER, MAKER_SCHEMA);
    env.settings(MAKER, json!({ "mode": "on-device", "model": "base" }));
    env.run(Command::DownloadSpeechModel { id: "base".into() }, |e| {
        matches!(e, Event::AiLyrics(a) if a.models.iter().any(|m| m.id == "base" && m.state == SpeechModelStatus::Ready))
    });
    assert!(until(Duration::from_secs(20), || need(&env) == AiLyricsNeed::Nothing));
    let music = env.dir.path().join("music");
    std::fs::create_dir_all(&music).unwrap();
    sine_wav(&music.join("one.wav"), 440.0, 3.0);
    env.run(Command::AddRoot { path: music }, |e| {
        matches!(e, Event::Scan(jf_core::ScanStatus::Finished(_)))
    });
    let id = env.core.tracks(&jf_core::TrackQuery::default()).unwrap()[0].id;
    hears.stall.store(true, Ordering::SeqCst);
    env.run(
        Command::MakeLyrics {
            song: SongRef::Library { id },
        },
        |e| matches!(e, Event::AiLyrics(a) if a.jobs.iter().any(|j| matches!(j.stage, LyricsJobStage::Recognising { .. }))),
    );
    env.run(
        Command::RemoveLicence,
        |e| matches!(e, Event::AiLyrics(a) if a.jobs.iter().all(|j| j.stage == LyricsJobStage::Cancelled)),
    );
    assert_eq!(need(&env), AiLyricsNeed::Pro);
}

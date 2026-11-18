//! Making lyrics in tests (API 1.2): a [`TestAccess`] serves a maker
//! plugin's `audio` and `speech` imports the way the app does, with silence
//! for audio and the transcript the test gives, and records what the plugin
//! asked for. Turn Jellyfesh Pro on first (`env.host.set_pro(true)`):
//! `make_lyrics` is a Pro capability.
//!
//! ```no_run
//! use std::sync::Arc;
//! use jf_plugin_testkit::*;
//!
//! let env = Env::new();
//! let info = env.install_crate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")), "my-maker");
//! env.host.set_pro(true);
//! env.host.set_speech_models(speech_models(&["base"]));
//! let access = Arc::new(TestAccess::new(180_000).transcript(transcript(&[(1000, &["la", "la"])])));
//! let made = recv(env.host.make_lyrics(&info.id, make_job("A Song", &access, &[]))).unwrap();
//! assert!(made.lyrics.text.contains("la"));
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jf_plugin_host::{
    AudioError, MakeJob, MakeSong, MakeStage, MakerAccess, SpeechError, SpeechModel, SpeechOptions,
    SpeechSegment, SpeechWord, Transcript,
};

/// The song's audio and recognition, as the app serves them, for one test.
#[derive(Debug)]
pub struct TestAccess {
    duration_ms: u64,
    transcript: Result<Transcript, SpeechError>,
    delay: Duration,
    stages: Mutex<Vec<MakeStage>>,
    wav_requests: Mutex<Vec<(u64, u64)>>,
    options: Mutex<Vec<SpeechOptions>>,
}

impl TestAccess {
    /// A song of `duration_ms` whose audio is silence and whose recognition
    /// finds nothing (until [`TestAccess::transcript`]).
    pub fn new(duration_ms: u64) -> TestAccess {
        TestAccess {
            duration_ms,
            transcript: Ok(Transcript {
                language: None,
                segments: Vec::new(),
            }),
            delay: Duration::ZERO,
            stages: Mutex::new(Vec::new()),
            wav_requests: Mutex::new(Vec::new()),
            options: Mutex::new(Vec::new()),
        }
    }

    /// What `speech.transcribe` answers.
    pub fn transcript(mut self, transcript: Transcript) -> TestAccess {
        self.transcript = Ok(transcript);
        self
    }

    /// Makes `speech.transcribe` fail.
    pub fn recognition_fails(mut self, error: SpeechError) -> TestAccess {
        self.transcript = Err(error);
        self
    }

    /// Recognition takes this long (cancellable meanwhile).
    pub fn delay(mut self, delay: Duration) -> TestAccess {
        self.delay = delay;
        self
    }

    /// The stages the host reported, in order.
    pub fn stages(&self) -> Vec<MakeStage> {
        self.stages.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Every `audio.wav` request: (start, length) in milliseconds.
    pub fn wav_requests(&self) -> Vec<(u64, u64)> {
        self.wav_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The options of every `speech.transcribe` call.
    pub fn options(&self) -> Vec<SpeechOptions> {
        self.options.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// `ms` of 16 kHz mono 16-bit PCM silence as a WAV file.
pub fn silent_wav(ms: u64) -> Vec<u8> {
    let data = u32::try_from(ms * 32).unwrap_or(u32::MAX - 64);
    let mut wav = Vec::with_capacity(44 + data as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&16_000u32.to_le_bytes());
    wav.extend_from_slice(&32_000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    wav.resize(44 + data as usize, 0);
    wav
}

impl MakerAccess for TestAccess {
    fn duration_ms(&self) -> Result<u64, AudioError> {
        Ok(self.duration_ms)
    }

    fn wav(&self, start_ms: u64, length_ms: u64) -> Result<Vec<u8>, AudioError> {
        self.wav_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((start_ms, length_ms));
        let end = start_ms.saturating_add(length_ms).min(self.duration_ms);
        Ok(silent_wav(end.saturating_sub(start_ms)))
    }

    fn transcribe(&self, options: &SpeechOptions, cancel: &AtomicBool) -> Result<Transcript, SpeechError> {
        self.options
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(options.clone());
        let started = Instant::now();
        while started.elapsed() < self.delay {
            if cancel.load(Ordering::Relaxed) {
                return Err(SpeechError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.transcript.clone()
    }

    fn stage(&self, stage: MakeStage) {
        self.stages.lock().unwrap_or_else(|e| e.into_inner()).push(stage);
    }
}

/// A transcript of lines, each `(start_ms, words)`: words 400 ms apart.
pub fn transcript(lines: &[(u32, &[&str])]) -> Transcript {
    Transcript {
        language: Some("en".into()),
        segments: lines
            .iter()
            .map(|(start, words)| {
                let words: Vec<SpeechWord> = words
                    .iter()
                    .enumerate()
                    .map(|(i, text)| {
                        let at = start + 400 * u32::try_from(i).unwrap_or(0);
                        SpeechWord {
                            text: (*text).to_string(),
                            start_ms: at,
                            end_ms: at + 350,
                            confidence: 0.9,
                        }
                    })
                    .collect();
                SpeechSegment {
                    text: words
                        .iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                    start_ms: *start,
                    end_ms: words.last().map_or(*start, |w| w.end_ms),
                    words,
                }
            })
            .collect(),
    }
}

/// A `maker.make` call for a song called `title`, served by `access`; the
/// person allowed sending audio to `consent` (origins such as
/// `https://api.openai.com`).
pub fn make_job(title: &str, access: &Arc<TestAccess>, consent: &[&str]) -> MakeJob {
    MakeJob {
        song: MakeSong {
            title: title.into(),
            artist: Some("Invented Artist".into()),
            album: None,
            duration_ms: access.duration_ms,
            text: None,
            language: None,
        },
        access: access.clone(),
        allowed_origins: consent.iter().map(|o| (*o).to_string()).collect(),
        upload_to: None,
        cancel: Arc::new(AtomicBool::new(false)),
    }
}

/// The app's four speech models (ADR-0019), the ones in `ready` downloaded.
pub fn speech_models(ready: &[&str]) -> Vec<SpeechModel> {
    [
        ("tiny", "Tiny", 77_691_713),
        ("base", "Base", 147_951_465),
        ("small", "Small", 487_601_967),
        ("large-v3-turbo-q5_0", "Large Turbo", 574_041_195),
    ]
    .into_iter()
    .map(|(id, name, bytes)| SpeechModel {
        id: id.into(),
        name: name.into(),
        bytes,
        ready: ready.contains(&id),
    })
    .collect()
}

//! Making lyrics (API 1.2, ADR-0019): what the embedder gives a
//! `maker.make` call.
//!
//! The host has no audio and no speech recognition of its own. The core
//! hands each [`PluginHost::make_lyrics`](crate::PluginHost::make_lyrics)
//! call a [`MakerAccess`] for that call's song, which serves the plugin's
//! `audio` and `speech` imports, and hears the stages the host sees
//! ([`MakeStage`]). The plugin gets a track handle that means nothing
//! outside its call.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::types::{AudioError, MakeSong, SpeechError, SpeechOptions, Transcript};

/// What the host has seen a `maker.make` call do, for the job's progress
/// ("Reading Audio…", "Sending to {origin}…", "Recognising…", "Timing
/// Words…").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MakeStage {
    /// The plugin reads the song's audio (`audio.wav`).
    ReadingAudio,
    /// The plugin sends a request while it holds audio; `origin` as
    /// [`crate::sanitize::origin`] writes it.
    Sending { origin: String },
    /// The app recognises the song (`speech.transcribe`); the embedder
    /// knows the progress.
    Recognising,
    /// The last import returned: the plugin times the words.
    TimingWords,
}

/// The song of one `maker.make` call, served by the embedder. Called on the
/// call's own thread; each method may take long (decoding, recognition).
pub trait MakerAccess: Send + Sync {
    /// The song's length in milliseconds (`audio.info`).
    fn duration_ms(&self) -> Result<u64, AudioError>;

    /// 16 kHz mono 16-bit PCM WAV of `[start_ms, start_ms + length_ms)`, cut
    /// at the song's end (`audio.wav`). The host has checked the permission,
    /// the handle, consent and the 8-minute cap.
    fn wav(&self, start_ms: u64, length_ms: u64) -> Result<Vec<u8>, AudioError>;

    /// Recognises the song (`speech.transcribe`), stopping early when
    /// `cancel` is set. The host has checked the permission and the handle,
    /// and checks the transcript it returns.
    fn transcribe(&self, options: &SpeechOptions, cancel: &AtomicBool) -> Result<Transcript, SpeechError>;

    /// The call reached `stage`. Each stage is reported once in a row.
    fn stage(&self, stage: MakeStage) {
        let _ = stage;
    }
}

/// One `maker.make` call.
#[derive(Clone)]
pub struct MakeJob {
    /// The song, as the plugin's `maker.song` gets it.
    pub song: MakeSong,
    /// Its audio and recognition.
    pub access: Arc<dyn MakerAccess>,
    /// Where the person allowed this plugin to send audio, as
    /// [`crate::sanitize::origin`] writes origins. Empty: `audio.wav`
    /// answers `no-consent`.
    pub allowed_origins: Vec<String>,
    /// The plan's `upload-to`, named in a `no-consent` answer.
    pub upload_to: Option<String>,
    /// Set it to stop the call: imports answer `cancelled`, the plugin's
    /// code traps at the next tick, and the reply is `CallError::Cancelled`.
    pub cancel: Arc<AtomicBool>,
}

impl std::fmt::Debug for MakeJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MakeJob")
            .field("song", &self.song.title)
            .field("allowed_origins", &self.allowed_origins)
            .field("upload_to", &self.upload_to)
            .finish_non_exhaustive()
    }
}

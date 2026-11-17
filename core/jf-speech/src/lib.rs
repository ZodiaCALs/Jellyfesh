//! On-device speech recognition for AI Lyrics (ADR-0019 §5).
//!
//! * [`catalogue`] lists the Whisper models the app offers: whisper.cpp's
//!   converted `ggml` files, with the SHA-1 sums whisper.cpp publishes.
//! * [`ModelStore`] downloads a model only when asked, into a folder the app
//!   chooses (`<data>/speech-models/`): resumable with HTTP ranges,
//!   verified before use, cancellable and deletable.
//! * [`Recognizer`] turns 16 kHz mono audio into a [`Transcript`] with
//!   word timing. [`recognizer`] gives whisper.cpp's when the crate is built
//!   with the feature `whisper`, and `None` without it (Jellyfesh Web).
//!
//! Times are milliseconds from the start of the audio passed in.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

mod catalogue;
mod store;

pub use catalogue::{DEFAULT_MODEL, ModelInfo, catalogue, model};
pub use store::ModelStore;

/// What steers one recognition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// The language sung or spoken, as a BCP 47 tag ("en", "zh-Hans", "ja");
    /// `None` detects it.
    pub language: Option<String>,
    /// Text that steers recognition, such as the lyrics being timed
    /// (Whisper's initial prompt).
    pub prompt: Option<String>,
    /// Threads to recognise with; `None` takes `min(4, cores − 1)`.
    pub threads: Option<usize>,
}

/// A recognised word.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    /// The word without surrounding spaces. For Chinese, Japanese and Korean
    /// text, one character.
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    /// How sure the recogniser is, from 0 to 1.
    pub confidence: f32,
}

/// A stretch of speech the recogniser decoded at once, usually a line or two.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// The segment's text, trimmed.
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    /// The segment's words in order; their times never decrease.
    pub words: Vec<Word>,
}

/// What a recognition found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Transcript {
    /// The language recognised (the hint, or the one detected), such as "en".
    pub language: Option<String>,
    pub segments: Vec<Segment>,
}

/// Why a download or a recognition did not finish.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpeechError {
    /// The model with this id is not downloaded (or failed verification).
    #[error("the speech model {0} is not downloaded")]
    ModelMissing(String),
    /// Recognition is not available in this build or on this device.
    #[error("speech recognition is unavailable: {0}")]
    Unavailable(String),
    /// The cancel flag was set.
    #[error("cancelled")]
    Cancelled,
    /// A model download failed: an unknown id, the network, the disk or
    /// verification. A partial download is kept for the next attempt unless
    /// it failed verification.
    #[error("the speech model download failed: {0}")]
    Download(String),
    /// Recognition failed.
    #[error("speech recognition failed: {0}")]
    Failed(String),
}

/// A speech recogniser.
pub trait Recognizer: Send + Sync {
    /// Recognises `pcm` with the model at `model_path` (described by `model`).
    ///
    /// `pcm`: 16 kHz mono f32 in [-1, 1]. `progress` receives the fraction
    /// done, from 0 to 1. Setting `cancel` stops the recognition soon after,
    /// with [`SpeechError::Cancelled`]. Runs on the caller's thread and may
    /// take minutes; one recognition at a time.
    fn transcribe(
        &self,
        model: &ModelInfo,
        model_path: &Path,
        pcm: &[f32],
        options: &Options,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Transcript, SpeechError>;
}

/// The recogniser this build has: whisper.cpp's with the feature `whisper`,
/// `None` without it. (Not yet: the whisper.cpp recogniser comes next.)
pub fn recognizer() -> Option<Arc<dyn Recognizer>> {
    None
}

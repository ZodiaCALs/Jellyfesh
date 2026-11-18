//! Speech recognition in the app (API 1.2, permission `speech`): Whisper on
//! the device, with the models the person downloads in the app. Nothing
//! leaves the device; Jellyfesh Web answers `unavailable`.

use crate::bindings::jellyfesh::plugin::speech as wit;

pub use wit::{Model, Options, Segment, SpeechError, Transcript, Word};

/// The models the app offers and whether each is downloaded (empty without
/// the `speech` permission). Cheap: call it from [`crate::Plugin::maker_plan`].
pub fn models() -> Vec<Model> {
    wit::models()
}

/// Whether the model `id` is downloaded and ready.
pub fn ready(id: &str) -> bool {
    models().iter().any(|m| m.id == id && m.ready)
}

/// Recognises the song of the current [`crate::Plugin::make_lyrics`] call.
/// Takes as long as it takes; that time never counts against the plugin's
/// CPU budget.
pub fn transcribe(
    track: crate::TrackHandle,
    options: &Options,
) -> core::result::Result<Transcript, SpeechError> {
    wit::transcribe(track, options)
}

impl Options {
    /// Recognition with model `model`, the language detected.
    pub fn new(model: impl Into<String>) -> Options {
        Options {
            model: model.into(),
            language: None,
            prompt: None,
        }
    }
}

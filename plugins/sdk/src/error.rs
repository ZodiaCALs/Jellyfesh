//! [`PluginError`] helpers and conversions from the imports' error types,
//! so `?` works everywhere in a plugin.

use crate::PluginError;
use crate::bindings::jellyfesh::plugin::audio::AudioError;
use crate::bindings::jellyfesh::plugin::crypto::CryptoError;
use crate::bindings::jellyfesh::plugin::http::HttpError;
use crate::bindings::jellyfesh::plugin::kv::KvError;
use crate::bindings::jellyfesh::plugin::speech::SpeechError;

impl PluginError {
    /// The service answered with an error; `message` is shown to the person.
    pub fn service(message: impl Into<String>) -> PluginError {
        PluginError::Service(message.into())
    }

    /// A bug or unexpected state in the plugin.
    pub fn internal(message: impl Into<String>) -> PluginError {
        PluginError::Internal(message.into())
    }

    /// The host passed something the plugin cannot use.
    pub fn invalid_input(message: impl Into<String>) -> PluginError {
        PluginError::InvalidInput(message.into())
    }
}

impl From<HttpError> for PluginError {
    fn from(error: HttpError) -> PluginError {
        match error {
            HttpError::RateLimited => PluginError::RateLimited,
            HttpError::NotAllowed(m) => PluginError::Network(format!("not allowed: {m}")),
            HttpError::BlockedAddress(m) => PluginError::Network(format!("blocked address: {m}")),
            HttpError::InvalidRequest(m) => PluginError::Internal(format!("invalid request: {m}")),
            HttpError::Timeout => PluginError::Network("timed out".into()),
            HttpError::TooLarge => PluginError::Network("response too large".into()),
            HttpError::TooManyRedirects => PluginError::Network("too many redirects".into()),
            HttpError::Dns(m) => PluginError::Network(format!("DNS: {m}")),
            HttpError::Connection(m) => PluginError::Network(m),
        }
    }
}

impl From<KvError> for PluginError {
    fn from(error: KvError) -> PluginError {
        match error {
            KvError::InvalidKey(m) => PluginError::Internal(format!("storage key: {m}")),
            KvError::QuotaExceeded => PluginError::Internal("storage quota exceeded".into()),
            KvError::Forbidden(m) => PluginError::Internal(format!("storage refused: {m}")),
            KvError::Unavailable(m) => PluginError::Internal(format!("storage unavailable: {m}")),
        }
    }
}

impl From<CryptoError> for PluginError {
    fn from(error: CryptoError) -> PluginError {
        PluginError::Internal(format!("crypto: {error:?}"))
    }
}

/// The song's audio could not be read (API 1.2). A cancelled job reports
/// itself as `service("cancelled")`; the host ends the call anyway.
impl From<AudioError> for PluginError {
    fn from(error: AudioError) -> PluginError {
        match error {
            AudioError::NotDeclared => {
                PluginError::Internal("the manifest does not declare the audio permission".into())
            }
            AudioError::NoConsent(origins) => PluginError::Service(if origins.is_empty() {
                "sending audio has not been allowed".into()
            } else {
                format!("sending audio to {} has not been allowed", origins.join(", "))
            }),
            AudioError::Unavailable(m) => PluginError::Service(m),
            AudioError::Cancelled => PluginError::Service("cancelled".into()),
        }
    }
}

/// Recognition failed (API 1.2).
impl From<SpeechError> for PluginError {
    fn from(error: SpeechError) -> PluginError {
        match error {
            SpeechError::NotDeclared => {
                PluginError::Internal("the manifest does not declare the speech permission".into())
            }
            SpeechError::ModelMissing(id) => {
                PluginError::Service(format!("the {id} model is not downloaded"))
            }
            SpeechError::Unavailable(m) | SpeechError::Failed(m) => PluginError::Service(m),
            SpeechError::Cancelled => PluginError::Service("cancelled".into()),
        }
    }
}

impl From<serde_json::Error> for PluginError {
    fn from(error: serde_json::Error) -> PluginError {
        PluginError::Service(format!("unexpected answer: {error}"))
    }
}

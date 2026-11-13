//! The one error type the bindings throw: `CoreFfiError` (Swift `CoreFfiError`, Kotlin and C#
//! `CoreFfiException`).

/// Why a core call failed. Swift and Kotlin get the kind as the case or subclass and the text as
/// its message (a UniFFI flat error); C# gets `{"kind": "...", "message": "..."}`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreFfiError {
    /// A library, session, audio or file error; the message says which.
    #[error("{0}")]
    Core(String),
    /// The feature is not in this build yet (parts of the core arrive during this release).
    #[error("{0}")]
    NotAvailable(String),
    /// The core was shut down; start a new one.
    #[error("the core has been shut down")]
    ShutDown,
    /// An argument could not be used: unreadable JSON over the C ABI, or a bad value.
    #[error("{0}")]
    InvalidArgument(String),
}

impl CoreFfiError {
    /// The variant name, as C# receives it in `kind`.
    pub fn kind(&self) -> &'static str {
        match self {
            CoreFfiError::Core(_) => "Core",
            CoreFfiError::NotAvailable(_) => "NotAvailable",
            CoreFfiError::ShutDown => "ShutDown",
            CoreFfiError::InvalidArgument(_) => "InvalidArgument",
        }
    }

    /// `{"kind": "...", "message": "..."}`, the error as the C ABI returns it.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "kind": self.kind(), "message": self.to_string() })
    }
}

/// The core reports features that arrive later in this release as `CoreError::Other` ending in
/// this text (`jf-core`'s `not_yet` and the actor's `unavailable`).
const NOT_AVAILABLE: &str = "not available in this build yet";

impl From<jf_core::CoreError> for CoreFfiError {
    fn from(error: jf_core::CoreError) -> Self {
        match error {
            jf_core::CoreError::Other(message) if message.ends_with(NOT_AVAILABLE) => {
                CoreFfiError::NotAvailable(message)
            }
            other => CoreFfiError::Core(other.to_string()),
        }
    }
}

impl From<std::io::Error> for CoreFfiError {
    fn from(error: std::io::Error) -> Self {
        CoreFfiError::Core(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_yet_available_features_have_their_own_kind() {
        let err: CoreFfiError =
            jf_core::CoreError::Other("playlists are not available in this build yet".into()).into();
        assert_eq!(err.kind(), "NotAvailable");
        assert_eq!(err.to_string(), "playlists are not available in this build yet");
        let err: CoreFfiError = jf_core::CoreError::Other("nothing to play".into()).into();
        assert_eq!(err, CoreFfiError::Core("nothing to play".into()));
    }

    #[test]
    fn json_carries_kind_and_message() {
        let json = CoreFfiError::InvalidArgument("bad".into()).to_json().to_string();
        assert_eq!(json, r#"{"kind":"InvalidArgument","message":"bad"}"#);
        let json = CoreFfiError::ShutDown.to_json().to_string();
        assert_eq!(
            json,
            r#"{"kind":"ShutDown","message":"the core has been shut down"}"#
        );
    }
}

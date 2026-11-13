//! The app's secret store as a foreign trait (ADR-0015): the shells implement [`SecretStore`]
//! over the platform keystore (Keychain in Swift, Android Keystore in Kotlin) and pass it to
//! `JellyfeshCore::with_secret_store`; C# passes a callback to `jf_core_new_with_secret_store`
//! (`c_api`), which ends up here too.

use std::sync::Arc;

use jf_core as jf;

/// Where the core keeps the sign-in tokens, API keys and cookie-jar keys of online sources:
/// implement it over the platform keystore and pass it to `JellyfeshCore::with_secret_store`.
///
/// Keys look like `jellyfesh/<plugin-id>/<name>`; values are short strings. Passwords never
/// reach the store. The core calls it from its own threads (never the UI thread), possibly from
/// several at once, and may block on it briefly. It reads one entry no plugin can name when it
/// starts: if that call fails, the store counts as unreachable for this run, sign-ins are kept
/// in memory until the app quits, and the `Sources` slice says so (`SecretStorage::SessionOnly`).
#[uniffi::export(foreign)]
pub trait SecretStore: Send + Sync {
    /// The value stored under `key`, or `None` when there is none.
    fn get(&self, key: String) -> Result<Option<String>, SecretStoreError>;
    /// Stores `value` under `key`, replacing any earlier value.
    fn set(&self, key: String, value: String) -> Result<(), SecretStoreError>;
    /// Removes `key`; removing a key that is not there is not an error.
    fn delete(&self, key: String) -> Result<(), SecretStoreError>;
}

/// Why a [`SecretStore`] call failed (the keystore is locked or cannot be reached). Throw it
/// from the store; any other exception is turned into it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum SecretStoreError {
    /// The store cannot answer; `reason` says why. (Not `message`, which Kotlin exceptions
    /// already have.)
    #[error("{reason}")]
    Unavailable { reason: String },
}

impl From<uniffi::UnexpectedUniFFICallbackError> for SecretStoreError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        SecretStoreError::Unavailable { reason: error.reason }
    }
}

/// An app's store as the core's [`jf::SecretStore`].
pub(crate) struct Foreign(pub(crate) Arc<dyn SecretStore>);

impl std::fmt::Debug for Foreign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the app's secret store")
    }
}

impl jf::SecretStore for Foreign {
    fn get(&self, key: &str) -> Result<Option<String>, jf::SecretError> {
        self.0.get(key.to_owned()).map_err(core_error)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), jf::SecretError> {
        self.0.set(key.to_owned(), value.to_owned()).map_err(core_error)
    }

    fn delete(&self, key: &str) -> Result<(), jf::SecretError> {
        self.0.delete(key.to_owned()).map_err(core_error)
    }
}

fn core_error(error: SecretStoreError) -> jf::SecretError {
    jf::SecretError(error.to_string())
}

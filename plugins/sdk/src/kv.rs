//! Persistent storage private to the plugin.
//!
//! Plain values: at most 1 MiB per plugin, keys of 1–128 printable ASCII
//! bytes. Credentials go to [`secret`], which the host keeps in the
//! operating system's keystore.

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::Result;
use crate::bindings::jellyfesh::plugin::kv as wit;

pub use wit::KvError;

/// Reads a value.
pub fn get(key: &str) -> Result<Option<Vec<u8>>> {
    Ok(wit::get(key)?)
}

/// Writes a value.
pub fn set(key: &str, value: &[u8]) -> Result<()> {
    Ok(wit::set(key, value)?)
}

/// Removes a value.
pub fn delete(key: &str) -> Result<()> {
    Ok(wit::delete(key)?)
}

/// Reads a JSON value.
pub fn get_json<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
    match get(key)? {
        Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        None => Ok(None),
    }
}

/// Writes a JSON value.
pub fn set_json<T: Serialize + ?Sized>(key: &str, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| crate::PluginError::internal(format!("JSON: {e}")))?;
    set(key, &bytes)
}

/// Secrets in the keystore, under `jellyfesh/<plugin-id>/<name>`. Names are
/// 1–64 characters of `a-z 0-9 . _ -`. Never store a password: during a
/// password-exchange login the host refuses to.
pub mod secret {
    use crate::Result;
    use crate::bindings::jellyfesh::plugin::kv as wit;

    /// Reads a secret.
    pub fn get(name: &str) -> Result<Option<String>> {
        Ok(wit::get_secret(name)?)
    }

    /// Stores a secret.
    pub fn set(name: &str, value: &str) -> Result<()> {
        Ok(wit::set_secret(name, value)?)
    }

    /// Removes a secret.
    pub fn delete(name: &str) -> Result<()> {
        Ok(wit::delete_secret(name)?)
    }
}

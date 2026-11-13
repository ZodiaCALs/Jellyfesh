//! Where plugin secrets live.
//!
//! The core plugs in the operating system's keystore (ADR-0015: `keyring` on
//! desktop, Android Keystore); tests use [`MemorySecretStore`]. Keys are
//! `jellyfesh/<plugin-id>/<name>`. A plugin names its secrets with 1–64
//! characters of `a-z 0-9 . _ -`, so it can never reach outside its own
//! namespace; the host's own entries for a plugin start with `~` (for example
//! the key of the plugin's cookie jar), which plugin names cannot.

use std::collections::BTreeMap;
use std::fmt;

use parking_lot::Mutex;

/// A secret store failure (locked keystore, no Secret Service, …).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SecretError(pub String);

/// A store for secrets, keyed by `jellyfesh/<plugin-id>/<name>`.
pub trait SecretStore: Send + Sync + fmt::Debug {
    /// Reads a secret.
    fn get(&self, key: &str) -> Result<Option<String>, SecretError>;
    /// Writes a secret, replacing any previous value.
    fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    /// Removes a secret; removing a missing key is not an error.
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

/// An in-memory [`SecretStore`] for tests and for "session only" logins.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    entries: Mutex<BTreeMap<String, String>>,
}

impl MemorySecretStore {
    /// An empty store.
    pub fn new() -> MemorySecretStore {
        MemorySecretStore::default()
    }

    /// Every key currently stored, sorted.
    pub fn keys(&self) -> Vec<String> {
        self.entries.lock().keys().cloned().collect()
    }

    /// Whether any stored value contains `needle` (tests use this to prove a
    /// password was never stored).
    pub fn any_value_contains(&self, needle: &str) -> bool {
        self.entries.lock().values().any(|v| v.contains(needle))
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        Ok(self.entries.lock().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.entries.lock().insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.entries.lock().remove(key);
        Ok(())
    }
}

/// The keystore key of a plugin's secret.
pub fn secret_key(plugin_id: &str, name: &str) -> String {
    format!("jellyfesh/{plugin_id}/{name}")
}

/// Whether a plugin may use `name` for a secret: 1–64 characters of
/// `a-z 0-9 . _ -`, starting with a letter or digit, no `..`.
pub fn valid_secret_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
}

/// The host's own entry holding the key of a plugin's cookie jar.
pub(crate) const COOKIE_KEY_NAME: &str = "~cookie-key";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_names_stay_in_their_namespace() {
        for ok in ["token", "access-token", "refresh_token.v2", "a", "0"] {
            assert!(valid_secret_name(ok), "{ok}");
        }
        for bad in [
            "",
            "../other/token",
            "jellyfesh/other/token",
            "Token",
            "~cookie-key",
            ".hidden",
            "a..b",
            "a/b",
            "a\\b",
            "a\0b",
            &"x".repeat(65),
        ] {
            assert!(!valid_secret_name(bad), "{bad:?}");
        }
        assert!(!valid_secret_name(COOKIE_KEY_NAME));
        assert_eq!(
            secret_key("org.example.a", "token"),
            "jellyfesh/org.example.a/token"
        );
    }
}

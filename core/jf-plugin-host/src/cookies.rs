//! One cookie jar per plugin, persisted encrypted (ADR-0015).
//!
//! The jar file holds the jar's persistent cookies as JSON, encrypted with
//! AES-256-GCM under a random key kept in the secret store
//! (`jellyfesh/<plugin-id>/~cookie-key`), with the plugin id as associated
//! data so a file cannot be swapped between plugins. The jar is loaded on
//! first use, so starting the app never touches the keystore. If the key or
//! the file cannot be read, the jar starts empty (a fresh login fixes it)
//! rather than failing requests.

use std::path::PathBuf;
use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use cookie_store::CookieStore;
use parking_lot::Mutex;
use url::Url;

use crate::secrets::{COOKIE_KEY_NAME, SecretStore, secret_key};
use crate::storage::write_atomic;

const MAGIC: &[u8] = b"JFCJ\x01";
const NONCE_LEN: usize = 12;
/// Largest jar file the host reads.
const MAX_JAR_FILE: u64 = 4 * 1024 * 1024;
/// Imported cookies have no expiry of their own; keep them this long.
const IMPORTED_MAX_AGE_SECS: u64 = 365 * 24 * 3600;

/// A plugin's cookie jar.
#[derive(Debug)]
pub(crate) struct CookieJar {
    plugin_id: String,
    file: PathBuf,
    secrets: Arc<dyn SecretStore>,
    store: Mutex<Option<CookieStore>>,
}

impl CookieJar {
    pub(crate) fn new(plugin_id: &str, file: PathBuf, secrets: Arc<dyn SecretStore>) -> CookieJar {
        CookieJar {
            plugin_id: plugin_id.to_string(),
            file,
            secrets,
            store: Mutex::new(None),
        }
    }

    fn with_store<R>(&self, f: impl FnOnce(&mut CookieStore) -> R) -> R {
        let mut guard = self.store.lock();
        let store = guard.get_or_insert_with(|| self.load());
        f(store)
    }

    /// The `Cookie` header value for a request to `url`, if any cookie matches.
    pub(crate) fn header_for(&self, url: &Url) -> Option<String> {
        self.with_store(|store| {
            let pairs: Vec<String> = store
                .get_request_values(url)
                .map(|(name, value)| format!("{name}={value}"))
                .collect();
            (!pairs.is_empty()).then(|| pairs.join("; "))
        })
    }

    /// Stores the `Set-Cookie` headers of a response from `url`.
    pub(crate) fn store_response(&self, url: &Url, set_cookie: &[String]) {
        if set_cookie.is_empty() {
            return;
        }
        self.with_store(|store| {
            let cookies = set_cookie
                .iter()
                .filter_map(|header| cookie::Cookie::parse(header.clone()).ok());
            store.store_response_cookies(cookies, url);
        });
        self.save();
    }

    /// Imports `name=value` pairs (a cookie-import login) for `url`.
    pub(crate) fn import(&self, url: &Url, pairs: &[(String, String)]) {
        self.with_store(|store| {
            for (name, value) in pairs {
                let text = format!("{name}={value}; Path=/; Max-Age={IMPORTED_MAX_AGE_SECS}");
                if let Ok(cookie) = cookie::Cookie::parse(text)
                    && let Err(error) = store.insert_raw(&cookie, url)
                {
                    tracing::warn!(plugin = %self.plugin_id, %error, "cookie import skipped a cookie");
                }
            }
        });
        self.save();
    }

    /// Empties the jar and deletes its file and key.
    pub(crate) fn clear(&self) {
        *self.store.lock() = Some(CookieStore::new());
        let _ = std::fs::remove_file(&self.file);
        let _ = self.secrets.delete(&secret_key(&self.plugin_id, COOKIE_KEY_NAME));
    }

    fn load(&self) -> CookieStore {
        let Ok(meta) = std::fs::metadata(&self.file) else {
            return CookieStore::new();
        };
        let result = (|| -> Result<CookieStore, String> {
            if meta.len() > MAX_JAR_FILE {
                return Err("file too large".into());
            }
            let data = std::fs::read(&self.file).map_err(|e| e.to_string())?;
            let key = self
                .key(false)?
                .ok_or("the jar's key is missing from the secret store")?;
            let rest = data.strip_prefix(MAGIC).ok_or("not a cookie jar")?;
            if rest.len() < NONCE_LEN {
                return Err("truncated".into());
            }
            let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
            let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
            let nonce = Nonce::try_from(nonce).map_err(|_| "bad nonce")?;
            let json = cipher
                .decrypt(
                    &nonce,
                    Payload {
                        msg: ciphertext,
                        aad: self.plugin_id.as_bytes(),
                    },
                )
                .map_err(|_| "cannot decrypt (wrong key or tampered file)")?;
            cookie_store::serde::json::load(json.as_slice()).map_err(|e| e.to_string())
        })();
        result.unwrap_or_else(|error| {
            tracing::warn!(plugin = %self.plugin_id, %error, "starting with an empty cookie jar");
            CookieStore::new()
        })
    }

    /// The jar's key; with `create`, makes one when there is none.
    fn key(&self, create: bool) -> Result<Option<[u8; 32]>, String> {
        let name = secret_key(&self.plugin_id, COOKIE_KEY_NAME);
        match self.secrets.get(&name).map_err(|e| e.to_string())? {
            Some(text) => {
                let bytes = BASE64.decode(text.as_bytes()).map_err(|e| e.to_string())?;
                let key: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| "the jar's key has the wrong length")?;
                Ok(Some(key))
            }
            None if create => {
                let mut key = [0u8; 32];
                getrandom::fill(&mut key).map_err(|e| e.to_string())?;
                self.secrets
                    .set(&name, &BASE64.encode(key))
                    .map_err(|e| e.to_string())?;
                Ok(Some(key))
            }
            None => Ok(None),
        }
    }

    /// Writes the jar (best effort: a failure only costs the cookies on restart).
    fn save(&self) {
        let result = (|| -> Result<(), String> {
            let mut json = Vec::new();
            {
                let guard = self.store.lock();
                let Some(store) = guard.as_ref() else {
                    return Ok(());
                };
                cookie_store::serde::json::save(store, &mut json).map_err(|e| e.to_string())?;
            }
            let key = self.key(true)?.ok_or("no key")?;
            let mut nonce = [0u8; NONCE_LEN];
            getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
            let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
            let ciphertext = cipher
                .encrypt(
                    &Nonce::from(nonce),
                    Payload {
                        msg: &json,
                        aad: self.plugin_id.as_bytes(),
                    },
                )
                .map_err(|_| "encryption failed")?;
            let mut file = Vec::with_capacity(MAGIC.len() + NONCE_LEN + ciphertext.len());
            file.extend_from_slice(MAGIC);
            file.extend_from_slice(&nonce);
            file.extend_from_slice(&ciphertext);
            write_atomic(&self.file, &file).map_err(|e| e.to_string())
        })();
        if let Err(error) = result {
            tracing::warn!(plugin = %self.plugin_id, %error, "could not save the cookie jar");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecretStore;

    /// The cookies of a header in name order (the jar's order is unspecified).
    fn sorted(header: Option<String>) -> String {
        let header = header.expect("no cookies");
        let mut pairs: Vec<&str> = header.split("; ").collect();
        pairs.sort_unstable();
        pairs.join("; ")
    }

    #[test]
    fn persists_encrypted_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let secrets: Arc<dyn SecretStore> = Arc::new(MemorySecretStore::new());
        let file = dir.path().join("cookies.jar");
        let url = Url::parse("https://music.example.com/api").unwrap();

        let jar = CookieJar::new("org.example.a", file.clone(), secrets.clone());
        jar.store_response(&url, &["sid=abc123; Max-Age=3600; Path=/".to_string()]);
        jar.import(&url, &[("csrf".to_string(), "tok".to_string())]);
        assert_eq!(sorted(jar.header_for(&url)), "csrf=tok; sid=abc123");

        let on_disk = std::fs::read(&file).unwrap();
        assert!(on_disk.starts_with(MAGIC));
        let text = String::from_utf8_lossy(&on_disk);
        assert!(
            !text.contains("abc123") && !text.contains("sid"),
            "jar is not encrypted"
        );

        let reloaded = CookieJar::new("org.example.a", file.clone(), secrets.clone());
        assert_eq!(sorted(reloaded.header_for(&url)), "csrf=tok; sid=abc123");

        // Another plugin's id (the associated data) cannot open the same file.
        let other = CookieJar::new("org.example.b", file.clone(), secrets.clone());
        assert_eq!(other.header_for(&url), None);

        reloaded.clear();
        assert!(!file.exists());
        assert!(
            secrets
                .get("jellyfesh/org.example.a/~cookie-key")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn cookies_stay_with_their_site() {
        let dir = tempfile::tempdir().unwrap();
        let jar = CookieJar::new(
            "org.example.a",
            dir.path().join("cookies.jar"),
            Arc::new(MemorySecretStore::new()),
        );
        let a = Url::parse("https://a.example.com/").unwrap();
        let b = Url::parse("https://b.example.org/").unwrap();
        jar.store_response(&a, &["k=v; Max-Age=60".to_string()]);
        assert!(jar.header_for(&a).is_some());
        assert!(jar.header_for(&b).is_none());
    }
}

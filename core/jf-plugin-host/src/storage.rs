//! Files the host keeps per plugin, under `<dir>/data/<plugin-id>/`:
//!
//! | File | Contents |
//! | --- | --- |
//! | `state.json` | enabled, account name, strikes, disabled reason, secret names, signer |
//! | `settings.json` | the person's settings |
//! | `kv.json` | the plugin's plain values (base64), at most 1 MiB |
//! | `cookies.jar` | the encrypted cookie jar ([`crate::cookies`]) |
//!
//! Every write goes to a temporary file first and is renamed into place, so
//! a crash never leaves half a file.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};

/// The kv quota per plugin: keys and values together.
pub const KV_QUOTA: usize = 1024 * 1024;
/// Longest kv key.
pub const KV_MAX_KEY: usize = 128;

/// Writes `data` to `path` atomically (temporary file, then rename).
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// What the host remembers about an installed plugin.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct PluginState {
    pub enabled: bool,
    pub account: Option<String>,
    pub strikes: u32,
    pub disabled_reason: Option<String>,
    /// Names of secrets the plugin stored, so logout and uninstall can delete
    /// them (keystores cannot list entries).
    pub secret_names: BTreeSet<String>,
    /// The signing key id remembered at first install (`None`: unsigned).
    pub signer: Option<String>,
}

/// Paths of one plugin's files.
#[derive(Debug, Clone)]
pub(crate) struct PluginPaths {
    pub data: PathBuf,
}

impl PluginPaths {
    pub(crate) fn new(dir: &Path, id: &str) -> PluginPaths {
        PluginPaths {
            data: dir.join("data").join(id),
        }
    }
    pub(crate) fn state(&self) -> PathBuf {
        self.data.join("state.json")
    }
    pub(crate) fn settings(&self) -> PathBuf {
        self.data.join("settings.json")
    }
    pub(crate) fn kv(&self) -> PathBuf {
        self.data.join("kv.json")
    }
    pub(crate) fn cookies(&self) -> PathBuf {
        self.data.join("cookies.jar")
    }
}

/// Reads a JSON file, or `None` when it does not exist or cannot be parsed.
pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "ignoring an unreadable file");
            None
        }
    }
}

/// Writes a JSON file atomically.
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    write_atomic(path, &bytes)
}

/// Why a kv operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KvFailure {
    InvalidKey,
    Quota,
    Io(String),
}

/// A plugin's plain key-value store, loaded on first use.
#[derive(Debug)]
pub(crate) struct KvStore {
    path: PathBuf,
    entries: Option<BTreeMap<String, Vec<u8>>>,
}

impl KvStore {
    pub(crate) fn new(path: PathBuf) -> KvStore {
        KvStore { path, entries: None }
    }

    fn entries(&mut self) -> &mut BTreeMap<String, Vec<u8>> {
        let path = &self.path;
        self.entries.get_or_insert_with(|| {
            let raw: BTreeMap<String, String> = read_json(path).unwrap_or_default();
            raw.into_iter()
                .filter_map(|(k, v)| Some((k, BASE64.decode(v).ok()?)))
                .collect()
        })
    }

    fn used(entries: &BTreeMap<String, Vec<u8>>) -> usize {
        entries.iter().map(|(k, v)| k.len() + v.len()).sum()
    }

    pub(crate) fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, KvFailure> {
        check_key(key)?;
        Ok(self.entries().get(key).cloned())
    }

    pub(crate) fn set(&mut self, key: &str, value: &[u8]) -> Result<(), KvFailure> {
        check_key(key)?;
        let entries = self.entries();
        let current = entries.get(key).map_or(0, |v| key.len() + v.len());
        if Self::used(entries) - current + key.len() + value.len() > KV_QUOTA {
            return Err(KvFailure::Quota);
        }
        entries.insert(key.to_string(), value.to_vec());
        self.save()
    }

    pub(crate) fn delete(&mut self, key: &str) -> Result<(), KvFailure> {
        check_key(key)?;
        if self.entries().remove(key).is_some() {
            self.save()?;
        }
        Ok(())
    }

    /// Forgets everything (uninstall).
    pub(crate) fn clear(&mut self) {
        self.entries = Some(BTreeMap::new());
        let _ = std::fs::remove_file(&self.path);
    }

    fn save(&mut self) -> Result<(), KvFailure> {
        let path = self.path.clone();
        let raw: BTreeMap<&String, String> = self
            .entries()
            .iter()
            .map(|(k, v)| (k, BASE64.encode(v)))
            .collect();
        write_json(&path, &raw).map_err(|e| KvFailure::Io(e.to_string()))
    }
}

fn check_key(key: &str) -> Result<(), KvFailure> {
    let ok = !key.is_empty() && key.len() <= KV_MAX_KEY && key.bytes().all(|b| (0x20..0x7f).contains(&b));
    if ok { Ok(()) } else { Err(KvFailure::InvalidKey) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv_round_trips_and_enforces_the_quota() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kv.json");
        let mut kv = KvStore::new(path.clone());
        kv.set("token-cache", b"\x00\x01binary").unwrap();
        assert_eq!(kv.get("token-cache").unwrap().unwrap(), b"\x00\x01binary");
        assert_eq!(
            KvStore::new(path.clone()).get("token-cache").unwrap().unwrap(),
            b"\x00\x01binary"
        );

        let big = vec![7u8; KV_QUOTA / 2];
        kv.set("a", &big).unwrap();
        assert_eq!(kv.set("b", &big), Err(KvFailure::Quota));
        // Replacing a value counts only the difference.
        kv.set("a", &big).unwrap();
        kv.delete("a").unwrap();
        kv.set("b", &big).unwrap();

        assert_eq!(kv.get(""), Err(KvFailure::InvalidKey));
        assert_eq!(kv.get(&"k".repeat(KV_MAX_KEY + 1)), Err(KvFailure::InvalidKey));
        assert_eq!(kv.set("new\nline", b""), Err(KvFailure::InvalidKey));
    }

    #[test]
    fn atomic_writes_leave_no_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/state.json");
        write_json(&path, &PluginState::default()).unwrap();
        write_json(
            &path,
            &PluginState {
                enabled: true,
                ..PluginState::default()
            },
        )
        .unwrap();
        let state: PluginState = read_json(&path).unwrap();
        assert!(state.enabled);
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("state.json")]);
    }
}

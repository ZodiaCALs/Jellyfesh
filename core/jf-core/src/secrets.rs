//! Where plugin secrets go (ADR-0015).
//!
//! The plugin host keeps tokens, API keys and the keys of its cookie jars
//! in a [`SecretStore`]. The app may pass one ([`crate::SourcesConfig::secrets`]:
//! Android Keystore, or a Swift or C# implementation through the FFI);
//! otherwise macOS, Windows and Linux use the system keystore through
//! `keyring` (Keychain, Credential Manager, the Secret Service over D-Bus),
//! and other platforms have none. Windows and Linux keep one entry per
//! secret; macOS keeps every secret in one Keychain item, read once per run
//! (`secrets/bundle.rs`, ADR-0027 §1), so an update of an ad-hoc signed
//! build asks at most once.
//!
//! Whichever store is chosen is checked once, off the actor thread, by
//! reading an entry no plugin can name ([`PROBE_KEY`]). Until the check is
//! done, secret calls (which only plugin workers and login threads make)
//! wait for it. When the store cannot be reached (no Secret Service on a
//! minimal Linux desktop, a broken foreign store), secrets are kept in
//! memory for this run instead and [`Selection::check`] reports why, so the
//! UI can say that sign-ins last for this session only. Passwords are never
//! stored anywhere: the host refuses them.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use jf_plugin_host::{MemorySecretStore, SecretError, SecretStore};

use crate::types::SecretStorage;

/// The entry the check reads. Plugin secrets are `jellyfesh/<plugin-id>/<name>`
/// and plugin ids never start with `~`, so no plugin can own it.
pub(crate) const PROBE_KEY: &str = "jellyfesh/~probe";

/// Longest a secret call waits for the check before using memory. On macOS
/// the check may be waiting for the person to answer the Keychain's prompt.
#[cfg(target_os = "macos")]
const CHECK_WAIT: Duration = Duration::from_secs(10 * 60);
#[cfg(not(target_os = "macos"))]
const CHECK_WAIT: Duration = Duration::from_secs(60);

/// The outcome of the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Check {
    /// Not done yet.
    Pending,
    /// Secrets go to the chosen store.
    Keystore,
    /// Secrets stay in memory, for this reason.
    SessionOnly(String),
}

impl Check {
    pub(crate) fn storage(&self) -> SecretStorage {
        match self {
            Check::SessionOnly(_) => SecretStorage::SessionOnly,
            Check::Pending | Check::Keystore => SecretStorage::Keystore,
        }
    }
}

/// The store the plugin host uses: the chosen one, or memory once the
/// check has failed.
#[derive(Debug)]
pub(crate) struct Selection {
    primary: Option<Arc<dyn SecretStore>>,
    memory: MemorySecretStore,
    check: Mutex<Check>,
    checked: Condvar,
}

impl Selection {
    /// `primary` is the app's store or the platform keystore; `None` when the
    /// platform has none (then everything stays in memory from the start).
    pub(crate) fn new(primary: Option<Arc<dyn SecretStore>>) -> Arc<Selection> {
        let check = match primary {
            Some(_) => Check::Pending,
            None => Check::SessionOnly("this platform has no keystore the core can reach".into()),
        };
        Arc::new(Selection {
            primary,
            memory: MemorySecretStore::new(),
            check: Mutex::new(check),
            checked: Condvar::new(),
        })
    }

    /// The store for this platform: `config`'s, else the system keystore.
    pub(crate) fn for_platform(config: Option<Arc<dyn SecretStore>>) -> Arc<Selection> {
        Selection::new(config.or_else(platform_store))
    }

    /// Checks the store (blocking: call it off the actor) and returns the
    /// outcome. Later calls return the first outcome.
    pub(crate) fn run_check(&self) -> Check {
        let outcome = match &self.primary {
            None => return self.status(),
            Some(store) => match store.get(PROBE_KEY) {
                Ok(_) => Check::Keystore,
                Err(e) => Check::SessionOnly(e.0),
            },
        };
        let mut check = lock(&self.check);
        if *check == Check::Pending {
            *check = outcome;
            self.checked.notify_all();
        }
        check.clone()
    }

    /// The outcome so far.
    pub(crate) fn status(&self) -> Check {
        lock(&self.check).clone()
    }

    /// The store to use now, waiting for the check if it is still running.
    fn store(&self) -> &dyn SecretStore {
        let mut check = lock(&self.check);
        let deadline = std::time::Instant::now() + CHECK_WAIT;
        while *check == Check::Pending {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                *check = Check::SessionOnly("the keystore did not answer".into());
                self.checked.notify_all();
                break;
            }
            check = self
                .checked
                .wait_timeout(check, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        match (&*check, &self.primary) {
            (Check::Keystore, Some(primary)) => primary.as_ref(),
            _ => &self.memory,
        }
    }
}

impl SecretStore for Selection {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        self.store().get(key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.store().set(key, value)
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.store().delete(key)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The system keystore through `keyring` (macOS, Windows, Linux and the
/// BSDs), or `None`. On macOS every secret goes in one item.
fn platform_store() -> Option<Arc<dyn SecretStore>> {
    #[cfg(target_os = "macos")]
    {
        Some(Arc::new(bundle::Bundled::new(keystore::Keystore)))
    }
    #[cfg(any(
        target_os = "windows",
        all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android")))
    ))]
    {
        Some(Arc::new(keystore::Keystore))
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android")))
    )))]
    {
        None
    }
}

#[cfg(any(target_os = "macos", test))]
mod bundle;

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android")))
))]
mod keystore {
    use jf_plugin_host::{SecretError, SecretStore};

    /// The service name entries are filed under; the account is the key.
    const SERVICE: &str = "Jellyfesh";

    /// Keychain, Credential Manager or the Secret Service, as `keyring`'s
    /// `v1` store picks for the platform.
    #[derive(Debug)]
    pub(super) struct Keystore;

    fn entry(key: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, key).map_err(error)
    }

    fn error(e: keyring::Error) -> SecretError {
        SecretError(match e {
            keyring::Error::NoDefaultStore => match keyring::Entry::store_status() {
                Err(cause) => cause.to_string(),
                Ok(()) => "no keystore is available".into(),
            },
            other => other.to_string(),
        })
    }

    impl SecretStore for Keystore {
        fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
            match entry(key)?.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(error(e)),
            }
        }

        fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
            entry(key)?.set_password(value).map_err(error)
        }

        fn delete(&self, key: &str) -> Result<(), SecretError> {
            match entry(key)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(error(e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A store whose every call fails, like a keystore that cannot be reached.
    #[derive(Debug, Default)]
    struct Broken {
        calls: AtomicUsize,
    }

    impl SecretStore for Broken {
        fn get(&self, _: &str) -> Result<Option<String>, SecretError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(SecretError(
                "org.freedesktop.secrets was not provided by any .service files".into(),
            ))
        }
        fn set(&self, _: &str, _: &str) -> Result<(), SecretError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(SecretError("unreachable".into()))
        }
        fn delete(&self, _: &str) -> Result<(), SecretError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(SecretError("unreachable".into()))
        }
    }

    #[test]
    fn a_reachable_store_keeps_the_secrets() {
        let store = Arc::new(MemorySecretStore::new());
        let selection = Selection::new(Some(store.clone()));
        assert_eq!(selection.status(), Check::Pending);
        assert_eq!(selection.run_check(), Check::Keystore);
        assert_eq!(selection.status().storage(), SecretStorage::Keystore);
        selection.set("jellyfesh/p/token", "t").unwrap();
        assert_eq!(store.keys(), vec!["jellyfesh/p/token".to_string()]);
        assert_eq!(selection.get("jellyfesh/p/token").unwrap().as_deref(), Some("t"));
        selection.delete("jellyfesh/p/token").unwrap();
        assert!(store.keys().is_empty());
    }

    #[test]
    fn an_unreachable_store_falls_back_to_memory_for_the_session() {
        let broken = Arc::new(Broken::default());
        let selection = Selection::new(Some(broken.clone()));
        let check = selection.run_check();
        let Check::SessionOnly(reason) = &check else {
            panic!("expected a fallback, got {check:?}");
        };
        assert!(reason.contains("not provided"), "{reason}");
        assert_eq!(check.storage(), SecretStorage::SessionOnly);
        let calls = broken.calls.load(Ordering::Relaxed);
        selection.set("jellyfesh/p/token", "t").unwrap();
        assert_eq!(selection.get("jellyfesh/p/token").unwrap().as_deref(), Some("t"));
        assert_eq!(
            broken.calls.load(Ordering::Relaxed),
            calls,
            "the broken store is left alone"
        );
        assert_eq!(selection.run_check(), check, "the first outcome stays");
    }

    #[test]
    fn calls_wait_for_the_check() {
        let store = Arc::new(MemorySecretStore::new());
        let selection = Selection::new(Some(store.clone()));
        let waiter = {
            let selection = selection.clone();
            std::thread::spawn(move || selection.set("jellyfesh/p/token", "t"))
        };
        std::thread::sleep(Duration::from_millis(50));
        assert!(store.keys().is_empty(), "nothing is written before the check");
        selection.run_check();
        waiter.join().unwrap().unwrap();
        assert_eq!(store.keys(), vec!["jellyfesh/p/token".to_string()]);
    }

    #[test]
    fn no_store_means_session_only_from_the_start() {
        let selection = Selection::new(None);
        assert_eq!(selection.status().storage(), SecretStorage::SessionOnly);
        selection.set("jellyfesh/p/token", "t").unwrap();
        assert_eq!(selection.get("jellyfesh/p/token").unwrap().as_deref(), Some("t"));
    }
}

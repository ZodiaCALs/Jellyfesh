//! Every secret in one keystore entry, for macOS (ADR-0027 §1).
//!
//! The login keychain ties each item to the build that saved it. An ad-hoc
//! signed build is a different build after every update, so macOS asks once
//! per item before the new build may read it. [`Bundled`] keeps every secret
//! in one entry, [`BUNDLE_KEY`], read once per run and kept in memory. The
//! keystore check's probe read is the first call, so the one prompt comes at
//! start.
//!
//! * **Writes** read the entry again, apply the change and write it back, so
//!   the app and the `jf` CLI don't drop each other's keys. The copy in
//!   memory changes only once the write succeeded.
//! * **Entries from before.** Entries saved one per key by v0.0.8 and earlier
//!   move in the first time each is read, and are deleted once the bundle
//!   holds them. A deleted key is remembered as retired, so a leftover old
//!   entry never brings it back.
//! * **Denied access.** If the entry cannot be read, every call fails with
//!   that error and nothing is ever written; the check then keeps this run's
//!   sign-ins in memory. A failed read of an old entry is not tried again in
//!   the run either, so the person is never asked twice.
//! * Unreadable JSON counts as no secrets (logged, never the values). A newer
//!   format is an error, so an older version never overwrites it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::Mutex;

use jf_plugin_host::{SecretError, SecretStore};
use serde::{Deserialize, Serialize};

use super::{PROBE_KEY, lock};

/// The entry that holds every secret. Plugin keys are
/// `jellyfesh/<plugin-id>/<name>` (two slashes) and the probe is
/// [`PROBE_KEY`], so neither can name it.
pub(crate) const BUNDLE_KEY: &str = "jellyfesh/secrets";

/// The format this version writes and reads.
const VERSION: u32 = 1;

/// What the entry holds: `{"version":1,"secrets":{key:value},"retired":[key,…]}`.
/// No `Debug`: it holds the values.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Bundle {
    version: u32,
    #[serde(default)]
    secrets: BTreeMap<String, String>,
    /// Keys deleted since: an entry from before left behind for one of them
    /// is never read.
    #[serde(default)]
    retired: BTreeSet<String>,
}

impl Default for Bundle {
    fn default() -> Bundle {
        Bundle {
            version: VERSION,
            secrets: BTreeMap::new(),
            retired: BTreeSet::new(),
        }
    }
}

impl Bundle {
    /// The entry's text: unreadable text is an empty bundle, a newer format
    /// an error.
    fn parse(json: &str) -> Result<Bundle, SecretError> {
        match serde_json::from_str::<Bundle>(json) {
            Ok(bundle) if bundle.version > VERSION => Err(SecretError(format!(
                "the stored secrets are in a newer format (version {}) than this version of Jellyfesh reads",
                bundle.version
            ))),
            Ok(bundle) => Ok(bundle),
            Err(e) => {
                // serde_json's message can quote the text: only say where it broke.
                tracing::warn!(
                    "the stored secrets are unreadable ({:?} at line {}, column {}); they count as none",
                    e.classify(),
                    e.line(),
                    e.column()
                );
                Ok(Bundle::default())
            }
        }
    }
}

/// What reading a key's entry from before found, remembered for the run.
enum Legacy {
    Absent,
    /// Found, but the bundle could not take it.
    Found(String),
    Failed(SecretError),
}

/// The entry as this run has it.
struct State {
    /// `None` until the entry is read; then the bundle, or why it could not
    /// be read (for the rest of the run).
    loaded: Option<Result<Bundle, SecretError>>,
    legacy: HashMap<String, Legacy>,
}

/// Every secret of `raw`'s in one entry of it (see the module docs).
pub(crate) struct Bundled<S> {
    raw: S,
    /// Held around the calls to `raw` too, so threads never interleave a
    /// read and a write.
    state: Mutex<State>,
}

impl<S: SecretStore> Bundled<S> {
    /// Over a store that keeps one entry per key (the system keystore).
    pub(crate) fn new(raw: S) -> Bundled<S> {
        Bundled {
            raw,
            state: Mutex::new(State {
                loaded: None,
                legacy: HashMap::new(),
            }),
        }
    }

    /// The bundle, read on first use; after a failed read, that error.
    fn bundle<'a>(&self, state: &'a mut State) -> Result<&'a mut Bundle, SecretError> {
        let loaded = state.loaded.get_or_insert_with(|| {
            let read = self.read();
            if let Err(e) = &read {
                tracing::warn!("the stored secrets could not be read: {e}; nothing is written in this run");
            }
            read
        });
        loaded.as_mut().map_err(|e| e.clone())
    }

    /// The entry as it is stored now; none yet is an empty bundle.
    fn read(&self) -> Result<Bundle, SecretError> {
        match self.raw.get(BUNDLE_KEY)? {
            Some(json) => Bundle::parse(&json),
            None => Ok(Bundle::default()),
        }
    }

    /// Reads the entry again, applies `change` and writes the result if it
    /// differs, so keys another process wrote meanwhile stay. The copy in
    /// memory becomes the result only once that worked.
    fn write(&self, state: &mut State, change: impl FnOnce(&mut Bundle)) -> Result<(), SecretError> {
        let mut stored = self.read()?;
        let before = stored.clone();
        change(&mut stored);
        if stored != before {
            let json = serde_json::to_string(&stored)
                .map_err(|e| SecretError(format!("the secrets could not be encoded: {e}")))?;
            self.raw.set(BUNDLE_KEY, &json)?;
        }
        state.loaded = Some(Ok(stored));
        Ok(())
    }

    /// Moves `value`, read from `key`'s entry from before, into the bundle,
    /// then deletes that entry. If the bundle cannot take it, it stays where
    /// it is and this run keeps using it.
    fn adopt(&self, state: &mut State, key: &str, value: String) -> Result<Option<String>, SecretError> {
        let moved = self.write(state, |bundle| {
            // Another process may have stored or deleted the key meanwhile: that stands.
            if !bundle.retired.contains(key) {
                bundle
                    .secrets
                    .entry(key.to_owned())
                    .or_insert_with(|| value.clone());
            }
        });
        match moved {
            Ok(()) => {
                // A leftover entry is never read again, so a failure here does not matter.
                let _ = self.raw.delete(key);
                Ok(self.bundle(state)?.secrets.get(key).cloned())
            }
            Err(e) => {
                tracing::warn!("a secret saved by an earlier version stays in its own entry: {e}");
                state.legacy.insert(key.to_owned(), Legacy::Found(value.clone()));
                Ok(Some(value))
            }
        }
    }
}

impl<S: SecretStore> SecretStore for Bundled<S> {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        let mut state = lock(&self.state);
        let bundle = self.bundle(&mut state)?;
        if let Some(value) = bundle.secrets.get(key) {
            return Ok(Some(value.clone()));
        }
        if key == PROBE_KEY || key == BUNDLE_KEY || bundle.retired.contains(key) {
            return Ok(None);
        }
        match state.legacy.get(key) {
            Some(Legacy::Absent) => return Ok(None),
            Some(Legacy::Found(value)) => return Ok(Some(value.clone())),
            Some(Legacy::Failed(e)) => return Err(e.clone()),
            None => {}
        }
        // Not in the bundle: maybe an entry saved by v0.0.8 or earlier.
        match self.raw.get(key) {
            Ok(Some(value)) => self.adopt(&mut state, key, value),
            Ok(None) => {
                state.legacy.insert(key.to_owned(), Legacy::Absent);
                Ok(None)
            }
            Err(e) => {
                state.legacy.insert(key.to_owned(), Legacy::Failed(e.clone()));
                Err(e)
            }
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        let mut state = lock(&self.state);
        self.bundle(&mut state)?;
        self.write(&mut state, |bundle| {
            bundle.secrets.insert(key.to_owned(), value.to_owned());
            bundle.retired.remove(key);
        })
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        let mut state = lock(&self.state);
        self.bundle(&mut state)?;
        self.write(&mut state, |bundle| {
            bundle.secrets.remove(key);
            bundle.retired.insert(key.to_owned());
        })?;
        // An entry from before, if there is one: retired either way, so
        // errors (and none being there) don't matter.
        let _ = self.raw.delete(key);
        Ok(())
    }
}

impl<S: fmt::Debug> fmt::Debug for Bundled<S> {
    /// Never the secrets.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bundled")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::secrets::{Check, Selection};

    const TOKEN: &str = "jellyfesh/org.example.music/token";
    const COOKIE_KEY: &str = "jellyfesh/org.example.music/~cookie-key";
    const OTHER: &str = "jellyfesh/org.example.video/token";

    /// A keystore with one entry per key, in memory, that logs every call
    /// (keys only) and can deny reads, refuse writes or fail deletes.
    #[derive(Clone, Default)]
    struct Raw(Arc<Mutex<RawState>>);

    impl fmt::Debug for Raw {
        /// As the real keystore's: no entries.
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("Raw")
        }
    }

    #[derive(Default)]
    struct RawState {
        entries: BTreeMap<String, String>,
        /// "get <key>", "set <key>" or "delete <key>", in order.
        calls: Vec<String>,
        /// Reads of these keys fail, as when the person denies access.
        denied: BTreeSet<String>,
        read_only: bool,
        deletes_fail: bool,
    }

    impl Raw {
        fn with(entries: &[(&str, &str)]) -> Raw {
            let raw = Raw::default();
            lock(&raw.0).entries = entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            raw
        }
        fn deny(&self, key: &str) {
            lock(&self.0).denied.insert(key.into());
        }
        fn calls(&self) -> Vec<String> {
            lock(&self.0).calls.clone()
        }
        fn entry(&self, key: &str) -> Option<String> {
            lock(&self.0).entries.get(key).cloned()
        }
        fn keys(&self) -> Vec<String> {
            lock(&self.0).entries.keys().cloned().collect()
        }
        /// The bundle entry, parsed.
        fn stored(&self) -> serde_json::Value {
            serde_json::from_str(&self.entry(BUNDLE_KEY).expect("the bundle entry")).unwrap()
        }
    }

    impl SecretStore for Raw {
        fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
            let mut raw = lock(&self.0);
            raw.calls.push(format!("get {key}"));
            if raw.denied.contains(key) {
                return Err(SecretError("the person denied access".into()));
            }
            Ok(raw.entries.get(key).cloned())
        }
        fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
            let mut raw = lock(&self.0);
            raw.calls.push(format!("set {key}"));
            if raw.read_only {
                return Err(SecretError("the keychain is read-only".into()));
            }
            raw.entries.insert(key.into(), value.into());
            Ok(())
        }
        fn delete(&self, key: &str) -> Result<(), SecretError> {
            let mut raw = lock(&self.0);
            raw.calls.push(format!("delete {key}"));
            if raw.deletes_fail {
                return Err(SecretError("the item could not be deleted".into()));
            }
            raw.entries.remove(key);
            Ok(())
        }
    }

    fn bundle(secrets: &[(&str, &str)], retired: &[&str]) -> String {
        let secrets: BTreeMap<_, _> = secrets.iter().copied().collect();
        serde_json::json!({"version": 1, "secrets": secrets, "retired": retired}).to_string()
    }

    fn get(key: &str) -> String {
        format!("get {key}")
    }

    #[test]
    fn neither_a_plugin_nor_the_probe_can_name_the_entry() {
        assert_ne!(BUNDLE_KEY, PROBE_KEY);
        assert_eq!(BUNDLE_KEY.matches('/').count(), 1);
        for (plugin, name) in [
            ("org.example.music", "token"),
            ("secrets", "a"),
            ("x", "~cookie-key"),
        ] {
            let key = jf_plugin_host::secrets::secret_key(plugin, name);
            assert!(key.matches('/').count() >= 2, "{key}");
            assert!(key != BUNDLE_KEY && key != PROBE_KEY, "{key}");
        }
    }

    #[test]
    fn one_read_serves_every_secret_in_a_run() {
        let raw = Raw::with(&[(BUNDLE_KEY, &bundle(&[(TOKEN, "t"), (COOKIE_KEY, "k")], &[]))]);
        let selection = Selection::new(Some(Arc::new(Bundled::new(raw.clone()))));
        assert_eq!(selection.run_check(), Check::Keystore);
        assert_eq!(raw.calls(), [get(BUNDLE_KEY)], "the check's probe read loads it");
        for _ in 0..3 {
            assert_eq!(selection.get(TOKEN).unwrap().as_deref(), Some("t"));
            assert_eq!(selection.get(COOKIE_KEY).unwrap().as_deref(), Some("k"));
            assert_eq!(selection.get(BUNDLE_KEY).unwrap(), None);
            assert_eq!(selection.get(PROBE_KEY).unwrap(), None);
        }
        assert_eq!(raw.calls(), [get(BUNDLE_KEY)], "no other read");
        // A key it does not hold: one look for an entry from before, then none.
        assert_eq!(selection.get(OTHER).unwrap(), None);
        assert_eq!(selection.get(OTHER).unwrap(), None);
        assert_eq!(raw.calls(), [get(BUNDLE_KEY), get(OTHER)]);
    }

    #[test]
    fn entries_from_before_move_in_once_and_are_deleted() {
        let raw = Raw::with(&[(TOKEN, "t"), (COOKIE_KEY, "k")]);
        let store = Bundled::new(raw.clone());
        assert_eq!(
            store.get(PROBE_KEY).unwrap(),
            None,
            "no bundle yet is not an error"
        );
        assert_eq!(store.get(TOKEN).unwrap().as_deref(), Some("t"));
        assert_eq!(
            raw.calls()[1..],
            [
                get(TOKEN),
                get(BUNDLE_KEY),
                format!("set {BUNDLE_KEY}"),
                format!("delete {TOKEN}")
            ],
            "read, merged into the entry as stored, then deleted"
        );
        assert_eq!(raw.stored()["secrets"][TOKEN], "t");
        let calls = raw.calls().len();
        assert_eq!(store.get(TOKEN).unwrap().as_deref(), Some("t"));
        assert_eq!(raw.calls().len(), calls, "moved once");
        assert_eq!(store.get(COOKIE_KEY).unwrap().as_deref(), Some("k"));
        assert_eq!(raw.keys(), [BUNDLE_KEY], "only the one entry is left");

        // The next run reads only that entry.
        let next = Bundled::new(raw.clone());
        let calls = raw.calls().len();
        assert_eq!(next.get(TOKEN).unwrap().as_deref(), Some("t"));
        assert_eq!(next.get(COOKIE_KEY).unwrap().as_deref(), Some("k"));
        assert_eq!(raw.calls()[calls..], [get(BUNDLE_KEY)]);
    }

    #[test]
    fn a_denied_read_is_not_retried_and_nothing_is_written() {
        let stored = bundle(&[(TOKEN, "t")], &[]);
        let raw = Raw::with(&[(BUNDLE_KEY, &stored)]);
        raw.deny(BUNDLE_KEY);
        let store = Bundled::new(raw.clone());
        let denied = store.get(PROBE_KEY).unwrap_err();
        assert_eq!(store.get(TOKEN).unwrap_err(), denied);
        assert_eq!(store.set(TOKEN, "new").unwrap_err(), denied);
        assert_eq!(store.delete(TOKEN).unwrap_err(), denied);
        assert_eq!(raw.calls(), [get(BUNDLE_KEY)], "asked once, never written");
        assert_eq!(raw.entry(BUNDLE_KEY), Some(stored));
    }

    #[test]
    fn a_denied_read_of_an_entry_from_before_is_not_retried() {
        let raw = Raw::with(&[(TOKEN, "t")]);
        raw.deny(TOKEN);
        let store = Bundled::new(raw.clone());
        let denied = store.get(TOKEN).unwrap_err();
        assert_eq!(store.get(TOKEN).unwrap_err(), denied);
        assert_eq!(raw.calls().iter().filter(|c| **c == get(TOKEN)).count(), 1);
        assert_eq!(raw.entry(TOKEN).as_deref(), Some("t"), "left where it is");
        // The other secrets work.
        store.set(OTHER, "o").unwrap();
        assert_eq!(store.get(OTHER).unwrap().as_deref(), Some("o"));
    }

    #[test]
    fn a_deleted_secret_stays_deleted_when_an_entry_from_before_is_left_behind() {
        let raw = Raw::with(&[(TOKEN, "old"), (COOKIE_KEY, "old-key")]);
        lock(&raw.0).deletes_fail = true;
        let store = Bundled::new(raw.clone());
        // Moved in (its old entry stays: deleting it fails), then deleted.
        assert_eq!(store.get(TOKEN).unwrap().as_deref(), Some("old"));
        store.delete(TOKEN).unwrap();
        // Deleted before it was ever read (a sign-out first thing).
        store.delete(COOKIE_KEY).unwrap();
        assert_eq!(raw.entry(TOKEN).as_deref(), Some("old"), "left behind");
        assert_eq!(raw.entry(COOKIE_KEY).as_deref(), Some("old-key"), "left behind");
        let calls = raw.calls().len();
        assert_eq!(store.get(TOKEN).unwrap(), None);
        assert_eq!(store.get(COOKIE_KEY).unwrap(), None);
        assert_eq!(raw.calls().len(), calls, "not read again");

        // Nor in the next run.
        let next = Bundled::new(raw.clone());
        assert_eq!(next.get(TOKEN).unwrap(), None);
        assert_eq!(next.get(COOKIE_KEY).unwrap(), None);
        assert_eq!(raw.calls()[calls..], [get(BUNDLE_KEY)]);
        // A new sign-in stores it again.
        next.set(TOKEN, "new").unwrap();
        assert_eq!(
            Bundled::new(raw.clone()).get(TOKEN).unwrap().as_deref(),
            Some("new")
        );
    }

    #[test]
    fn two_stores_writing_different_keys_keep_both() {
        let raw = Raw::with(&[(BUNDLE_KEY, &bundle(&[(COOKIE_KEY, "k")], &[]))]);
        // The app and the `jf` CLI, each having read the entry at its start.
        let (app, cli) = (Bundled::new(raw.clone()), Bundled::new(raw.clone()));
        app.get(PROBE_KEY).unwrap();
        cli.get(PROBE_KEY).unwrap();
        app.set(TOKEN, "a").unwrap();
        cli.set(OTHER, "c").unwrap();
        cli.delete(COOKIE_KEY).unwrap();
        app.set(TOKEN, "a2").unwrap();
        let next = Bundled::new(raw.clone());
        assert_eq!(next.get(TOKEN).unwrap().as_deref(), Some("a2"));
        assert_eq!(next.get(OTHER).unwrap().as_deref(), Some("c"));
        assert_eq!(next.get(COOKIE_KEY).unwrap(), None, "the CLI's delete stands");
        assert_eq!(raw.stored()["retired"], serde_json::json!([COOKIE_KEY]));
    }

    #[test]
    fn a_failed_write_leaves_the_copy_in_memory_as_stored() {
        let raw = Raw::with(&[(BUNDLE_KEY, &bundle(&[(TOKEN, "t")], &[]))]);
        let store = Bundled::new(raw.clone());
        store.get(PROBE_KEY).unwrap();
        lock(&raw.0).read_only = true;
        assert!(store.set(TOKEN, "new").is_err());
        assert!(store.set(OTHER, "o").is_err());
        assert!(store.delete(TOKEN).is_err());
        assert_eq!(store.get(TOKEN).unwrap().as_deref(), Some("t"));
        assert_eq!(store.get(OTHER).unwrap(), None);
    }

    #[test]
    fn an_entry_from_before_that_cannot_move_is_read_once() {
        let raw = Raw::with(&[(TOKEN, "t")]);
        lock(&raw.0).read_only = true;
        let store = Bundled::new(raw.clone());
        assert_eq!(store.get(TOKEN).unwrap().as_deref(), Some("t"));
        assert_eq!(store.get(TOKEN).unwrap().as_deref(), Some("t"));
        assert_eq!(raw.calls().iter().filter(|c| **c == get(TOKEN)).count(), 1);
        assert_eq!(
            raw.entry(TOKEN).as_deref(),
            Some("t"),
            "kept: the bundle could not take it"
        );
    }

    #[test]
    fn an_unreadable_entry_counts_as_empty() {
        let raw = Raw::with(&[(BUNDLE_KEY, r#"{"version":1,"secrets":{"#), (TOKEN, "t")]);
        let store = Bundled::new(raw.clone());
        assert_eq!(
            store.get(PROBE_KEY).unwrap(),
            None,
            "not an error: the check passes"
        );
        assert_eq!(
            store.get(TOKEN).unwrap().as_deref(),
            Some("t"),
            "entries from before still move in"
        );
        assert_eq!(raw.stored()["secrets"][TOKEN], "t", "written whole again");
        assert_eq!(raw.stored()["version"], 1);
    }

    #[test]
    fn a_newer_format_is_never_overwritten() {
        let newer = r#"{"version":2,"secrets":{},"retired":[],"more":true}"#;
        let raw = Raw::with(&[(BUNDLE_KEY, newer)]);
        let store = Bundled::new(raw.clone());
        let e = store.get(PROBE_KEY).unwrap_err();
        assert!(e.0.contains("newer format"), "{e}");
        assert!(store.set(TOKEN, "t").is_err());
        assert_eq!(raw.entry(BUNDLE_KEY).as_deref(), Some(newer));
    }

    #[test]
    fn a_denied_entry_keeps_this_runs_sign_ins_in_memory() {
        let stored = bundle(&[(TOKEN, "t")], &[]);
        let raw = Raw::with(&[(BUNDLE_KEY, &stored)]);
        raw.deny(BUNDLE_KEY);
        let selection = Selection::new(Some(Arc::new(Bundled::new(raw.clone()))));
        let check = selection.run_check();
        assert!(
            matches!(&check, Check::SessionOnly(why) if why.contains("denied")),
            "{check:?}"
        );
        selection.set(OTHER, "o").unwrap();
        assert_eq!(selection.get(OTHER).unwrap().as_deref(), Some("o"));
        selection.delete(TOKEN).unwrap();
        assert_eq!(raw.calls(), [get(BUNDLE_KEY)], "asked once; nothing written");
        assert_eq!(raw.entry(BUNDLE_KEY), Some(stored));
    }

    #[test]
    fn its_debug_text_shows_no_secret() {
        let raw = Raw::with(&[(BUNDLE_KEY, &bundle(&[(TOKEN, "hunter2")], &[]))]);
        let store = Bundled::new(raw);
        assert_eq!(store.get(TOKEN).unwrap().as_deref(), Some("hunter2"));
        let shown = format!("{store:?}");
        assert!(!shown.contains("hunter2"), "{shown}");
    }
}

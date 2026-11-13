//! What one plugin's host imports need, shared by all of its instances:
//! HTTP client, cookie jar, kv store, secret namespace, log limiter,
//! allowlist, settings and persisted state.

use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use serde_json::Value;

use crate::allow::Allowlist;
use crate::cookies::CookieJar;
use crate::http::HttpClient;
use crate::manifest::{AllowRule, CookieImport, CookieUrl};
use crate::net::Resolve;
use crate::plugin_log::LogLimiter;
use crate::schema::SettingsSchema;
use crate::secrets::{COOKIE_KEY_NAME, SecretStore, secret_key};
use crate::storage::{KvStore, PluginPaths, PluginState, write_json};

/// Per-plugin services.
#[derive(Debug)]
pub(crate) struct Services {
    pub id: String,
    pub paths: PluginPaths,
    pub http: HttpClient,
    pub cookies: Arc<CookieJar>,
    pub kv: Mutex<KvStore>,
    pub secrets: Arc<dyn SecretStore>,
    pub log: Mutex<LogLimiter>,
    allowlist: Arc<RwLock<Allowlist>>,
    rules: Vec<AllowRule>,
    pub cookie_import: Option<CookieImport>,
    state: Mutex<PluginState>,
    settings: RwLock<Value>,
}

impl Services {
    #[allow(clippy::too_many_arguments, reason = "constructed in one place")]
    pub(crate) fn new(
        id: &str,
        paths: PluginPaths,
        rules: Vec<AllowRule>,
        max_rps: u32,
        cookie_import: Option<CookieImport>,
        settings: Value,
        state: PluginState,
        secrets: Arc<dyn SecretStore>,
        resolve: Arc<dyn Resolve>,
    ) -> Services {
        let allowlist = Arc::new(RwLock::new(Allowlist::new(&rules, &settings)));
        let cookies = Arc::new(CookieJar::new(id, paths.cookies(), secrets.clone()));
        Services {
            id: id.to_string(),
            http: HttpClient::new(id, allowlist.clone(), max_rps, cookies.clone(), resolve),
            cookies,
            kv: Mutex::new(KvStore::new(paths.kv())),
            secrets,
            log: Mutex::new(LogLimiter::default()),
            allowlist,
            rules,
            cookie_import,
            state: Mutex::new(state),
            settings: RwLock::new(settings),
            paths,
        }
    }

    /// The settings as JSON text, for `meta.settings-changed`.
    pub(crate) fn settings_json(&self) -> String {
        self.settings.read().to_string()
    }

    /// The settings.
    pub(crate) fn settings(&self) -> Value {
        self.settings.read().clone()
    }

    /// Replaces the settings (already validated) and rebuilds the allowlist.
    pub(crate) fn set_settings(&self, settings: Value) -> std::io::Result<()> {
        write_json(&self.paths.settings(), &settings)?;
        *self.allowlist.write() = Allowlist::new(&self.rules, &settings);
        *self.settings.write() = settings;
        Ok(())
    }

    /// A copy of the persisted state.
    pub(crate) fn state(&self) -> PluginState {
        self.state.lock().clone()
    }

    /// Changes the persisted state and writes it.
    pub(crate) fn update_state<R>(&self, f: impl FnOnce(&mut PluginState) -> R) -> R {
        let mut state = self.state.lock();
        let result = f(&mut state);
        if let Err(error) = write_json(&self.paths.state(), &*state) {
            tracing::warn!(plugin = %self.id, %error, "could not save the plugin state");
        }
        result
    }

    /// Clears the strike count after a successful call (writes only if needed).
    pub(crate) fn reset_strikes(&self) {
        let mut state = self.state.lock();
        if state.strikes != 0 {
            state.strikes = 0;
            if let Err(error) = write_json(&self.paths.state(), &*state) {
                tracing::warn!(plugin = %self.id, %error, "could not save the plugin state");
            }
        }
    }

    /// The URL a cookie import stores cookies for, if it is configured.
    pub(crate) fn cookie_import_url(&self) -> Option<url::Url> {
        match &self.cookie_import.as_ref()?.url {
            CookieUrl::Fixed(url) => Some(url.clone()),
            CookieUrl::Setting(key) => {
                let text = self.settings.read().get(key)?.as_str()?.to_string();
                url::Url::parse(&text).ok()
            }
        }
    }

    /// Deletes every secret the plugin stored, the cookie jar and its key
    /// (logout and uninstall).
    pub(crate) fn forget_credentials(&self) {
        let names = self.update_state(|state| {
            state.account = None;
            std::mem::take(&mut state.secret_names)
        });
        for name in names {
            if let Err(error) = self.secrets.delete(&secret_key(&self.id, &name)) {
                tracing::warn!(plugin = %self.id, %error, "could not delete a secret");
            }
        }
        self.cookies.clear();
        let _ = self.secrets.delete(&secret_key(&self.id, COOKIE_KEY_NAME));
    }

    /// Whether the plugin's allowlist covers `url`.
    pub(crate) fn allows(&self, url: &url::Url) -> bool {
        self.http.allows(url)
    }
}

/// The settings a plugin starts with: the stored ones fitted to the schema
/// (an update may have changed it), or the defaults.
pub(crate) fn effective_settings(schema: &SettingsSchema, stored: Option<Value>) -> Value {
    match stored {
        Some(value) => schema.fit(&value),
        None => schema.defaults(),
    }
}

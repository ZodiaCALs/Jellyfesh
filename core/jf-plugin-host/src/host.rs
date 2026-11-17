//! [`PluginHost`]: the API the core calls.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::RwLock;

use crate::bindings::exports::jellyfesh::plugin::{catalog, playback};
use crate::bindings::jellyfesh::plugin::types as wit;
use crate::error::{CallError, HostError};
use crate::install::{self, MAX_PACKAGE_FILE};
use crate::net::{Resolve, SystemResolver};
use crate::package::{self, Package};
use crate::plugin::{Job, JobContext, Plugin};
use crate::pool::{Scheduler, WORKERS};
use crate::runtime::{Budget, Limits, Protected, Runtime};
use crate::sanitize::{self, Checker};
use crate::secrets::SecretStore;
use crate::services::{Services, effective_settings};
use crate::storage::{PluginPaths, PluginState, read_json, write_atomic, write_json};
use crate::trust::TrustList;
use crate::types::{
    Capability, ImageRef, Item, ItemPage, LyricPayload, PageRequest, PluginInfo, Quality, RemoteDevice,
    RemoteRepeat, RemoteState, Reply, SearchFilters, SignatureStatus, StreamInfo,
};

/// Where the host keeps its files and secrets.
#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Installed packages (`packages/<id>.jfpkg`) and per-plugin data
    /// (`data/<id>/`): app-private storage.
    pub dir: PathBuf,
    /// Reserved for compiled-component caches (ADR-0003); v0.0.3 compiles on
    /// load and stores nothing here yet.
    pub cache_dir: PathBuf,
    /// Tokens, API keys and cookie-jar keys (the OS keystore in the app).
    pub secrets: Arc<dyn SecretStore>,
    /// "Allow community plugins" (ADR-0012): packages not signed by a key in
    /// the trust list are refused at install and cannot be called while this
    /// is off.
    pub allow_community: bool,
}

/// Knobs with production defaults; tests change them.
#[derive(Debug, Clone)]
pub struct HostOptions {
    /// Keys that make a package official. Default: [`TrustList::builtin`].
    pub trust: TrustList,
    /// Name resolution for the `http` import. Default: the system resolver.
    pub resolver: Arc<dyn Resolve>,
    /// Sandbox limits. Default: ADR-0004's.
    pub limits: Limits,
    /// Worker threads. Default: [`WORKERS`] (2).
    pub workers: usize,
}

impl Default for HostOptions {
    fn default() -> Self {
        HostOptions {
            trust: TrustList::builtin(),
            resolver: Arc::new(SystemResolver),
            limits: Limits::default(),
            workers: WORKERS,
        }
    }
}

/// The plugin host. Cheap to share: clones refer to the same host.
#[derive(Debug, Clone)]
pub struct PluginHost {
    pub(crate) inner: Arc<Inner>,
}

#[derive(Debug)]
pub(crate) struct Inner {
    pub config: HostConfig,
    pub options: HostOptions,
    pub runtime: Arc<Runtime>,
    pub scheduler: Scheduler,
    pub plugins: RwLock<BTreeMap<String, Arc<Plugin>>>,
    allow_community: AtomicBool,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Workers finish the call they are running (bounded by its budget)
        // and exit; queued calls are dropped, which disconnects their replies.
        self.scheduler.shutdown();
    }
}

impl PluginHost {
    /// Opens the host on `config.dir` with production options, loading
    /// every installed plugin (components compile on first use).
    pub fn new(config: HostConfig) -> Result<PluginHost, HostError> {
        PluginHost::with_options(config, HostOptions::default())
    }

    /// Like [`PluginHost::new`] with explicit options.
    pub fn with_options(config: HostConfig, options: HostOptions) -> Result<PluginHost, HostError> {
        std::fs::create_dir_all(config.dir.join("packages"))?;
        std::fs::create_dir_all(config.dir.join("data"))?;
        std::fs::create_dir_all(&config.cache_dir)?;
        let runtime = Arc::new(Runtime::new(options.limits.clone())?);
        let (scheduler, _workers) = Scheduler::start(runtime.clone(), options.workers)?;
        let host = PluginHost {
            inner: Arc::new(Inner {
                allow_community: AtomicBool::new(config.allow_community),
                config,
                options,
                runtime,
                scheduler,
                plugins: RwLock::new(BTreeMap::new()),
            }),
        };
        host.load_installed();
        Ok(host)
    }

    fn package_path(&self, id: &str) -> PathBuf {
        self.inner.config.dir.join("packages").join(format!("{id}.jfpkg"))
    }

    fn load_installed(&self) {
        let Ok(entries) = std::fs::read_dir(self.inner.config.dir.join("packages")) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jfpkg") {
                continue;
            }
            let loaded = read_limited(&path)
                .and_then(|bytes| package::read(&bytes, &self.inner.options.trust))
                .and_then(|package| {
                    let expected = self.package_path(&package.manifest.id);
                    if expected != path {
                        return Err(HostError::Package(format!(
                            "{} holds plugin {}",
                            path.display(),
                            package.manifest.id
                        )));
                    }
                    let paths = PluginPaths::new(&self.inner.config.dir, &package.manifest.id);
                    let state: PluginState = read_json(&paths.state()).unwrap_or_default();
                    Ok(self.make_plugin(package, state, None))
                });
            match loaded {
                Ok(plugin) => {
                    self.inner
                        .plugins
                        .write()
                        .insert(plugin.id().to_string(), Arc::new(plugin));
                }
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "skipping an installed package");
                }
            }
        }
    }

    fn make_plugin(
        &self,
        package: Package,
        state: PluginState,
        prepared: Option<crate::runtime::Prepared>,
    ) -> Plugin {
        let id = package.manifest.id.clone();
        let paths = PluginPaths::new(&self.inner.config.dir, &id);
        let settings = effective_settings(&package.settings_schema, read_json(&paths.settings()));
        let services = Arc::new(Services::new(
            &id,
            paths,
            package.manifest.allow.clone(),
            package.manifest.max_rps,
            package.manifest.cookie_import.clone(),
            settings,
            state,
            self.inner.config.secrets.clone(),
            self.inner.options.resolver.clone(),
        ));
        Plugin::new(
            package.manifest,
            package.settings_schema,
            package.signature,
            package.icon,
            package.wasm,
            prepared,
            services,
        )
    }

    /// Installs (or updates) the package at `path` and returns its info.
    ///
    /// Checks the package, its signature and API version; refuses community
    /// packages while they are turned off and packages signed by a different
    /// key than the installed version; compiles the component, links it
    /// against the host's imports (no WASI, matching WIT version), starts it
    /// once and checks that the capabilities it reports are declared.
    /// Official packages start enabled; community ones start disabled until
    /// the person confirms the disclosure and calls
    /// [`PluginHost::set_enabled`]. An update keeps the settings, account and
    /// secrets.
    pub fn install(&self, path: &Path) -> Result<PluginInfo, HostError> {
        let bytes = read_limited(path)?;
        let package = package::read(&bytes, &self.inner.options.trust)?;
        let official = matches!(package.signature, SignatureStatus::Trusted { .. });
        let id = package.manifest.id.clone();
        let signer = install::signer(&package.signature);
        let paths = PluginPaths::new(&self.inner.config.dir, &id);
        let installed = self.inner.plugins.read().contains_key(&id);
        let remembered = read_json::<PluginState>(&paths.state()).filter(|_| installed);
        install::check(
            &id,
            &package.signature,
            remembered.as_ref().map(|state| state.signer.as_deref()),
            self.allow_community(),
        )?;
        let state = remembered.unwrap_or_else(|| PluginState {
            enabled: official,
            signer,
            ..PluginState::default()
        });

        let remote = package.manifest.capabilities.contains(&Capability::Remote);
        let prepared = self.inner.runtime.prepare(&package.wasm, remote)?;
        let plugin = Arc::new(self.make_plugin(package, state, Some(prepared.clone())));

        // Start it once: instantiation, settings, and its capabilities.
        let mut instance = self
            .inner
            .runtime
            .instantiate(&prepared, &plugin.services)
            .map_err(|e| HostError::Start(e.to_string()))?;
        let reported = instance
            .call(
                Budget::start(&self.inner.runtime.limits),
                Protected::default(),
                |exports, store| exports.source.jellyfesh_plugin_meta().call_capabilities(store),
            )
            .map_err(|e| HostError::Start(e.to_string()))?;
        let reported: Vec<Capability> = reported.into_iter().map(Into::into).collect();
        if let Some(extra) = reported
            .iter()
            .find(|c| !plugin.manifest.capabilities.contains(c))
        {
            return Err(HostError::Manifest(format!(
                "the plugin reports capability {extra}, which the manifest does not declare"
            )));
        }

        write_atomic(&self.package_path(&id), &bytes)?;
        write_json(&plugin.services.paths.state(), &plugin.services.state())?;
        write_json(&plugin.services.paths.settings(), &plugin.services.settings())?;
        plugin.slot.lock().instance = Some(instance);
        if let Some(old) = self.inner.plugins.write().insert(id.clone(), plugin.clone()) {
            close(&old);
        }
        tracing::info!(plugin = %id, version = %plugin.manifest.version, "installed plugin");
        Ok(self.info_of(&plugin))
    }

    /// Removes a plugin with its package, settings, kv values, cookies and
    /// secrets. Pending calls are dropped.
    pub fn uninstall(&self, id: &str) -> Result<(), HostError> {
        let plugin = self
            .inner
            .plugins
            .write()
            .remove(id)
            .ok_or_else(|| HostError::NotInstalled(id.to_string()))?;
        close(&plugin);
        plugin.services.forget_credentials();
        plugin.services.kv.lock().clear();
        let _ = std::fs::remove_dir_all(&plugin.services.paths.data);
        std::fs::remove_file(self.package_path(id))?;
        tracing::info!(plugin = %id, "uninstalled plugin");
        Ok(())
    }

    /// Every installed plugin, by id.
    pub fn plugins(&self) -> Vec<PluginInfo> {
        let plugins: Vec<Arc<Plugin>> = self.inner.plugins.read().values().cloned().collect();
        plugins.iter().map(|p| self.info_of(p)).collect()
    }

    /// One installed plugin.
    pub fn plugin(&self, id: &str) -> Option<PluginInfo> {
        let plugin = self.inner.plugins.read().get(id).cloned()?;
        Some(self.info_of(&plugin))
    }

    /// The package's `icon.png`, if it has one.
    pub fn icon(&self, id: &str) -> Option<Vec<u8>> {
        self.inner.plugins.read().get(id)?.icon.clone()
    }

    fn info_of(&self, plugin: &Plugin) -> PluginInfo {
        let state = plugin.services.state();
        let manifest = &plugin.manifest;
        PluginInfo {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.to_string(),
            author: manifest.author.clone(),
            description: manifest.description.clone(),
            official: matches!(plugin.signature, SignatureStatus::Trusted { .. }),
            signature: plugin.signature.clone(),
            api_version: manifest.api_version.to_string(),
            capabilities: manifest.capabilities.clone(),
            playback_mode: manifest.playback_mode,
            auth_kinds: manifest.auth.clone(),
            enabled: state.enabled,
            disabled_reason: state.disabled_reason,
            account: state.account,
            settings_schema: plugin.schema.json().clone(),
            settings_schema_text: plugin.schema.text().to_owned(),
            settings: plugin.services.settings(),
            content_policy: manifest.content_policy.clone(),
            links: manifest.links.clone(),
            has_icon: plugin.icon.is_some(),
        }
    }

    /// Turns a plugin on or off. Turning it on clears the strikes and the
    /// reason it was turned off; a community plugin can only be turned on
    /// while community plugins are allowed.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), HostError> {
        let plugin = self.get(id)?;
        if enabled && !is_official(&plugin) && !self.allow_community() {
            return Err(HostError::CommunityDisabled);
        }
        plugin.services.update_state(|state| {
            state.enabled = enabled;
            if enabled {
                state.strikes = 0;
                state.disabled_reason = None;
            }
        });
        Ok(())
    }

    /// Whether community plugins are allowed.
    pub fn allow_community(&self) -> bool {
        self.inner.allow_community.load(Ordering::Relaxed)
    }

    /// The "Allow community plugins" switch.
    pub fn set_allow_community(&self, allow: bool) {
        self.inner.allow_community.store(allow, Ordering::Relaxed);
    }

    /// Validates `json` against the plugin's settings schema, stores it,
    /// updates the network allowlist and tells the running plugin
    /// (`meta.settings-changed`; its answer is only logged).
    pub fn set_settings(&self, id: &str, json: &str) -> Result<(), HostError> {
        let plugin = self.get(id)?;
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| HostError::Settings(format!("not JSON: {e}")))?;
        let value = plugin.schema.validate(&value).map_err(HostError::Settings)?;
        plugin.services.set_settings(value)?;
        let job: Job = Box::new(|ctx: &mut JobContext<'_>| {
            if !ctx.is_live() {
                return; // a new instance gets the settings when it starts
            }
            let settings = ctx.plugin.services.settings_json();
            let budget = Budget::call(&ctx.runtime.limits);
            match ctx.call(budget, Protected::default(), |source, store| {
                source
                    .jellyfesh_plugin_meta()
                    .call_settings_changed(store, &settings)
            }) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => tracing::warn!(
                    plugin = %ctx.plugin.id(),
                    error = %crate::error::plugin_error_text(&error.into()),
                    "the plugin did not accept its new settings"
                ),
                Err(error) => tracing::warn!(plugin = %ctx.plugin.id(), %error, "settings update failed"),
            }
        });
        self.inner.scheduler.submit(&plugin, job);
        Ok(())
    }

    pub(crate) fn get(&self, id: &str) -> Result<Arc<Plugin>, HostError> {
        self.inner
            .plugins
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| HostError::NotInstalled(id.to_string()))
    }

    /// The plugin, if it may be called for `capability` right now.
    pub(crate) fn callable(
        &self,
        id: &str,
        capability: Option<Capability>,
    ) -> Result<Arc<Plugin>, CallError> {
        let plugin = self
            .inner
            .plugins
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| CallError::NotInstalled(id.to_string()))?;
        if !is_official(&plugin) && !self.allow_community() {
            return Err(CallError::CommunityDisabled);
        }
        check_enabled(&plugin)?;
        if let Some(capability) = capability
            && !plugin.manifest.capabilities.contains(&capability)
        {
            return Err(CallError::NotDeclared(capability.to_string()));
        }
        Ok(plugin)
    }

    /// Runs `f` on the plugin's worker and returns the reply channel.
    fn call<T: Send + 'static>(
        &self,
        id: &str,
        capability: Option<Capability>,
        f: impl FnOnce(&mut JobContext<'_>) -> Result<T, CallError> + Send + 'static,
    ) -> Reply<T> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        match self.callable(id, capability) {
            Err(error) => {
                let _ = tx.send(Err(error));
            }
            Ok(plugin) => {
                let job: Job = Box::new(move |ctx: &mut JobContext<'_>| {
                    // It may have been turned off while the call waited.
                    let result = check_enabled(ctx.plugin).and_then(|()| f(ctx));
                    let _ = tx.send(result);
                });
                if !self.inner.scheduler.submit(&plugin, job) {
                    let (tx, rx) = crossbeam_channel::bounded(1);
                    let _ = tx.send(Err(CallError::Shutdown));
                    return rx;
                }
            }
        }
        rx
    }

    /// What the plugin says it can do right now (`meta.capabilities`),
    /// limited to what its manifest declares. `remote` is a manifest-only
    /// capability (ADR-0018): it is listed whenever the manifest declares it.
    pub fn capabilities(&self, id: &str) -> Reply<Vec<Capability>> {
        self.call(id, None, |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let reported: Vec<Capability> = ctx
                .call(budget, Protected::default(), |source, store| {
                    source.jellyfesh_plugin_meta().call_capabilities(store)
                })?
                .into_iter()
                .map(Into::into)
                .collect();
            let declared = &ctx.plugin.manifest.capabilities;
            Ok(declared
                .iter()
                .copied()
                .filter(|c| *c == Capability::Remote || reported.contains(c))
                .collect())
        })
    }

    /// `catalog.search`.
    pub fn search(
        &self,
        id: &str,
        query: &str,
        filters: SearchFilters,
        page: PageRequest,
    ) -> Reply<ItemPage> {
        let query = sanitize::text(query, 1024);
        let page = clamp_page(page);
        self.call(id, Some(Capability::Search), move |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let (filters, request) = (
                catalog::SearchFilters::from(filters),
                wit::PageRequest::from(page.clone()),
            );
            let result = ctx.call(budget, Protected::default(), |source, store| {
                source
                    .jellyfesh_plugin_catalog()
                    .call_search(store, &query, &filters, &request)
            })?;
            let answer = result.map_err(|e| CallError::Plugin(e.into()))?;
            Ok(checked_page(answer, page.limit, &ctx.plugin.services))
        })
    }

    /// `catalog.browse`; `node` none lists the plugin's root sections.
    pub fn browse(&self, id: &str, node: Option<Item>, page: PageRequest) -> Reply<ItemPage> {
        let page = clamp_page(page);
        self.call(id, Some(Capability::Browse), move |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let (node, request) = (node.map(wit::Item::from), wit::PageRequest::from(page.clone()));
            let result = ctx.call(budget, Protected::default(), |source, store| {
                source
                    .jellyfesh_plugin_catalog()
                    .call_browse(store, node.as_ref(), &request)
            })?;
            let answer = result.map_err(|e| CallError::Plugin(e.into()))?;
            Ok(checked_page(answer, page.limit, &ctx.plugin.services))
        })
    }

    /// `playback.resolve-stream`. The URL must be on the plugin's allowlist.
    pub fn resolve_stream(&self, id: &str, item: Item, quality: Quality) -> Reply<StreamInfo> {
        self.call(id, Some(Capability::ResolveStream), move |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let (item, quality) = (wit::Item::from(item), playback::Quality::from(quality));
            let result = ctx.call(budget, Protected::default(), |source, store| {
                source
                    .jellyfesh_plugin_playback()
                    .call_resolve_stream(store, &item, quality)
            })?;
            let info = result.map_err(|e| CallError::Plugin(e.into()))?;
            let services = &ctx.plugin.services;
            let allows = |url: &url::Url| services.allows(url);
            sanitize::stream(info.into(), &Checker::new(&services.id, &allows))
        })
    }

    /// `lyrics.lyrics`.
    pub fn lyrics(&self, id: &str, item: Item) -> Reply<Option<LyricPayload>> {
        self.call(id, Some(Capability::Lyrics), move |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let item = wit::Item::from(item);
            let result = ctx.call(budget, Protected::default(), |source, store| {
                source.jellyfesh_plugin_lyrics().call_lyrics(store, &item)
            })?;
            result
                .map_err(|e| CallError::Plugin(e.into()))?
                .map(|payload| sanitize::lyrics(payload.into()))
                .transpose()
        })
    }

    /// `artwork.artwork` for about `size` pixels (clamped to 64–3000). The
    /// URL must be on the plugin's allowlist.
    pub fn artwork(&self, id: &str, item: Item, size: u32) -> Reply<Option<ImageRef>> {
        let size = size.clamp(64, 3000);
        self.call(id, Some(Capability::Artwork), move |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let item = wit::Item::from(item);
            let result = ctx.call(budget, Protected::default(), |source, store| {
                source.jellyfesh_plugin_artwork().call_artwork(store, &item, size)
            })?;
            let services = ctx.plugin.services.clone();
            let allows = |url: &url::Url| services.allows(url);
            let checker = Checker::new(&services.id, &allows);
            result
                .map_err(|e| CallError::Plugin(e.into()))?
                .map(|image| sanitize::artwork(image.into(), &checker))
                .transpose()
        })
    }

    /// `recommendations.for-item`.
    pub fn recommendations_for_item(&self, id: &str, item: Item, page: PageRequest) -> Reply<ItemPage> {
        let page = clamp_page(page);
        self.call(id, Some(Capability::Recommendations), move |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let (item, request) = (wit::Item::from(item), wit::PageRequest::from(page.clone()));
            let result = ctx.call(budget, Protected::default(), |source, store| {
                source
                    .jellyfesh_plugin_recommendations()
                    .call_for_item(store, &item, &request)
            })?;
            let answer = result.map_err(|e| CallError::Plugin(e.into()))?;
            Ok(checked_page(answer, page.limit, &ctx.plugin.services))
        })
    }

    /// `recommendations.for-user`.
    pub fn recommendations_for_user(&self, id: &str, page: PageRequest) -> Reply<ItemPage> {
        let page = clamp_page(page);
        self.call(id, Some(Capability::Recommendations), move |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let request = wit::PageRequest::from(page.clone());
            let result = ctx.call(budget, Protected::default(), |source, store| {
                source
                    .jellyfesh_plugin_recommendations()
                    .call_for_user(store, &request)
            })?;
            let answer = result.map_err(|e| CallError::Plugin(e.into()))?;
            Ok(checked_page(answer, page.limit, &ctx.plugin.services))
        })
    }

    /// `remote.devices`: where the remote source can play (API 1.1).
    pub fn remote_devices(&self, id: &str) -> Reply<Vec<RemoteDevice>> {
        self.call(id, Some(Capability::Remote), |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let result = ctx.call_remote(budget, |remote, store| {
                remote.jellyfesh_plugin_remote().call_devices(store)
            })?;
            let devices = result.map_err(|e| CallError::Plugin(e.into()))?;
            Ok(sanitize::remote_devices(
                devices.into_iter().map(Into::into).collect(),
            ))
        })
    }

    /// `remote.state`: what the remote player is doing, `None` when nothing
    /// plays anywhere.
    pub fn remote_state(&self, id: &str) -> Reply<Option<RemoteState>> {
        self.call(id, Some(Capability::Remote), |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let result = ctx.call_remote(budget, |remote, store| {
                remote.jellyfesh_plugin_remote().call_state(store)
            })?;
            let state = result.map_err(|e| CallError::Plugin(e.into()))?;
            let services = ctx.plugin.services.clone();
            let allows = |url: &url::Url| services.allows(url);
            let checker = Checker::new(&services.id, &allows);
            Ok(state.map(|s| sanitize::remote_state(s.into(), &checker)))
        })
    }

    /// `remote.play`: plays `tracks` from `start`, or `context` (an album or
    /// playlist) from its track `start`, on `device` (the active device when
    /// `None`). At most [`sanitize::MAX_REMOTE_TRACKS`] tracks are handed on.
    pub fn remote_play(
        &self,
        id: &str,
        device: Option<String>,
        context: Option<Item>,
        tracks: Vec<Item>,
        start: u32,
    ) -> Reply<()> {
        let device = device.filter(|d| !d.is_empty() && d.len() <= 1024);
        self.call(id, Some(Capability::Remote), move |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let context = context.map(wit::Item::from);
            let tracks: Vec<wit::Item> = tracks
                .into_iter()
                .take(sanitize::MAX_REMOTE_TRACKS)
                .map(wit::Item::from)
                .collect();
            // Tracks start within the list; a container (empty `tracks`)
            // plays from its own track `start`, which only the service knows.
            let start = if tracks.is_empty() {
                start
            } else {
                start.min(u32::try_from(tracks.len() - 1).unwrap_or(u32::MAX))
            };
            let result = ctx.call_remote(budget, |remote, store| {
                remote.jellyfesh_plugin_remote().call_play(
                    store,
                    device.as_deref(),
                    context.as_ref(),
                    &tracks,
                    start,
                )
            })?;
            result.map_err(|e| CallError::Plugin(e.into()))
        })
    }

    /// `remote.resume`.
    pub fn remote_resume(&self, id: &str) -> Reply<()> {
        self.remote_command(id, |remote, store| remote.call_resume(store))
    }

    /// `remote.pause`.
    pub fn remote_pause(&self, id: &str) -> Reply<()> {
        self.remote_command(id, |remote, store| remote.call_pause(store))
    }

    /// `remote.next`.
    pub fn remote_next(&self, id: &str) -> Reply<()> {
        self.remote_command(id, |remote, store| remote.call_next(store))
    }

    /// `remote.previous`.
    pub fn remote_previous(&self, id: &str) -> Reply<()> {
        self.remote_command(id, |remote, store| remote.call_previous(store))
    }

    /// `remote.seek` to `position_ms`.
    pub fn remote_seek(&self, id: &str, position_ms: u32) -> Reply<()> {
        self.remote_command(id, move |remote, store| remote.call_seek(store, position_ms))
    }

    /// `remote.set-volume`, cut to 0–100.
    pub fn remote_set_volume(&self, id: &str, percent: u32) -> Reply<()> {
        let percent = percent.min(100);
        self.remote_command(id, move |remote, store| remote.call_set_volume(store, percent))
    }

    /// `remote.set-shuffle`.
    pub fn remote_set_shuffle(&self, id: &str, on: bool) -> Reply<()> {
        self.remote_command(id, move |remote, store| remote.call_set_shuffle(store, on))
    }

    /// `remote.set-repeat`.
    pub fn remote_set_repeat(&self, id: &str, mode: RemoteRepeat) -> Reply<()> {
        self.remote_command(id, move |remote, store| {
            remote.call_set_repeat(store, mode.into())
        })
    }

    /// `remote.transfer`: moves playback to `device`, playing if `play`.
    pub fn remote_transfer(&self, id: &str, device: String, play: bool) -> Reply<()> {
        if device.is_empty() || device.len() > 1024 {
            let (tx, rx) = crossbeam_channel::bounded(1);
            let _ = tx.send(Err(CallError::InvalidResponse("no such device".into())));
            return rx;
        }
        self.remote_command(id, move |remote, store| {
            remote.call_transfer(store, &device, play)
        })
    }

    /// A `remote` call that answers nothing but success or an error.
    fn remote_command(
        &self,
        id: &str,
        f: impl FnOnce(
            &crate::bindings::remote::exports::jellyfesh::plugin::remote::Guest,
            &mut wasmtime::Store<crate::runtime::HostState>,
        ) -> wasmtime::Result<Result<(), wit::PluginError>>
        + Send
        + 'static,
    ) -> Reply<()> {
        self.call(id, Some(Capability::Remote), move |ctx| {
            let budget = Budget::call(&ctx.runtime.limits);
            let result =
                ctx.call_remote(budget, |remote, store| f(remote.jellyfesh_plugin_remote(), store))?;
            result.map_err(|e| CallError::Plugin(e.into()))
        })
    }

    /// Logs out: calls the plugin's `auth.logout` (so it can revoke tokens),
    /// then deletes its secrets, cookie jar and account name whatever the
    /// plugin answered (ADR-0015). The reply carries the plugin's own error,
    /// if any, after the credentials are gone. A plugin that cannot be called
    /// now (turned off, or community plugins are off) is not asked; its
    /// credentials are deleted all the same and the reply is `Ok`.
    pub fn logout(&self, id: &str) -> Reply<()> {
        if let Ok(plugin) = self.get(id)
            && plugin.manifest.capabilities.contains(&Capability::Auth)
            && matches!(
                self.callable(id, Some(Capability::Auth)),
                Err(CallError::Disabled(_) | CallError::CommunityDisabled)
            )
        {
            plugin.services.forget_credentials();
            let (tx, rx) = crossbeam_channel::bounded(1);
            let _ = tx.send(Ok(()));
            return rx;
        }
        self.call(id, Some(Capability::Auth), |ctx| {
            let budget = Budget::auth_step(&ctx.runtime.limits);
            let result = ctx.call(budget, Protected::default(), |source, store| {
                source.jellyfesh_plugin_auth().call_logout(store)
            });
            ctx.plugin.services.forget_credentials();
            result?.map_err(|e| CallError::Plugin(e.into()))
        })
    }
}

/// Waits for a reply; a disconnected channel means the host shut down.
pub fn wait<T>(reply: &Reply<T>) -> Result<T, CallError> {
    reply.recv().unwrap_or(Err(CallError::Shutdown))
}

/// A page from the plugin, checked: invalid items dropped, cut to `limit`,
/// images only from the allowlist.
fn checked_page(answer: wit::ItemPage, limit: u32, services: &crate::services::Services) -> ItemPage {
    let allows = |url: &url::Url| services.allows(url);
    sanitize::page(answer.into(), limit, &Checker::new(&services.id, &allows))
}

fn clamp_page(page: PageRequest) -> PageRequest {
    PageRequest {
        cursor: page.cursor.filter(|c| c.len() <= 4096),
        limit: page.limit.clamp(1, sanitize::MAX_PAGE),
    }
}

fn is_official(plugin: &Plugin) -> bool {
    matches!(plugin.signature, SignatureStatus::Trusted { .. })
}

fn check_enabled(plugin: &Plugin) -> Result<(), CallError> {
    let state = plugin.services.state();
    if state.enabled {
        Ok(())
    } else {
        Err(CallError::Disabled(
            state
                .disabled_reason
                .unwrap_or_else(|| "turned off in Sources settings".into()),
        ))
    }
}

/// Stops a plugin: no more work, pending calls dropped, instance dropped.
fn close(plugin: &Plugin) {
    let mut slot = plugin.slot.lock();
    slot.closed = true;
    slot.queue.clear();
    slot.instance = None;
}

fn read_limited(path: &Path) -> Result<Vec<u8>, HostError> {
    let size = std::fs::metadata(path)?.len();
    if size > MAX_PACKAGE_FILE {
        return Err(HostError::Package("the package is larger than 40 MiB".into()));
    }
    Ok(std::fs::read(path)?)
}

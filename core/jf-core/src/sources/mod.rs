//! Online sources: the plugin host inside the core (ADR-0003, ADR-0004,
//! ADR-0012, ADR-0015; `docs/architecture.md` section 10).
//!
//! The actor owns a [`Sources`]: the [`PluginHost`] (plugins in
//! `<data>/plugins`, compiled-code cache reserved in `<cache>/plugins`), the
//! secret store selection, the community consent file, the `Sources` slice
//! and the logins in progress. Nothing here blocks the actor: plugin calls
//! answer on channels, and whoever waits for them is a helper thread.
//!
//! | Thread | Does |
//! | --- | --- |
//! | `jf-sources` | installs, uninstalls (compiling, keystore deletes) and logouts, in order |
//! | `jf-search` | fans a search out to the sources and merges the answers ([`search`]) |
//! | `jf-browse`, `jf-collection` (one per request) | catalogue pages, every page of an album or playlist |
//! | `jf-resolve` (one per track) | turns a source track into a stream URL for the engine |
//! | `jf-login` (one per login) | forwards a login's steps as `Event::Auth` |
//! | `jf-secrets` (once) | checks the keystore |
//!
//! Helpers report back to the actor with [`SourcesMsg`] when the actor has to
//! act (the queue, the slice), and emit events directly otherwise (search
//! results, browse pages, login steps).
//!
//! Plugins get their items back whole: every item the core lists is
//! remembered (by plugin and id, for this run), so `Browse` and
//! `Collection::Source` can pass a node id and the plugin still receives
//! the record it described, kind and all.

mod convert;
mod fetch;
mod search;

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crossbeam_channel::{Sender, unbounded};
use jf_engine::{HttpSource, StreamResolver};
use jf_lyrics::LyricDoc;
use jf_plugin_host::presets;
use jf_plugin_host::trust::TrustList;
use jf_plugin_host::{
    AuthFlow, CallError, Capability, FieldValue, HostConfig, HostError, ImageRef, Item, ItemKind as HostKind,
    PageRequest, PlaybackMode, PluginHost, Quality, Reply,
};
use jf_session::{SourceTrack, TrackRef};

pub(crate) use convert::{
    Answer, LIBRARY, library_answer, source_item, source_track, track_item, unplayable_reason,
};

/// The core's kind for a plugin item's kind.
pub(crate) fn item_kind(kind: HostKind) -> crate::types::ItemKind {
    convert::kind(kind)
}

/// An item that only has an id and a kind (its title is the id).
pub(crate) fn bare_item(id: &str, kind: HostKind) -> Item {
    convert::bare(id, kind, id)
}
pub(crate) use search::SearchJob;

use crate::CoreError;
use crate::actor::{Msg, Shared};
use crate::secrets::{Check, Selection};
use crate::types::{
    AuthMethod, AuthUpdate, BrowseUpdate, CommandId, Config, Event, SearchUpdate, SourcesSnapshot,
};

/// The quality asked of a source: the original file where the service
/// offers it (there is no quality setting yet).
const QUALITY: Quality = Quality::Lossless;
/// Longest the core waits for a stream URL (the plugin's own deadline is
/// 10 s; a call may also wait behind the plugin's other calls).
const RESOLVE_WAIT: Duration = Duration::from_secs(25);
/// Longest a browse page, lyrics or artwork call may take.
const CALL_WAIT: Duration = Duration::from_secs(25);
/// Longest a logout may take (a login step's budget is 30 s).
const LOGOUT_WAIT: Duration = Duration::from_secs(45);
/// Items per browse page.
const BROWSE_PAGE: u32 = 100;
/// Pages fetched to reach a page whose cursor is not known yet.
const MAX_CATCH_UP: usize = 20;
/// Items per page, pages and items when fetching a whole collection, and
/// albums of an artist.
const COLLECTION_PAGE: u32 = 200;
const COLLECTION_PAGES: usize = 50;
const COLLECTION_ITEMS: usize = 10_000;
const COLLECTION_ALBUMS: usize = 50;
/// Items remembered per generation (two generations are kept).
const ITEM_CACHE: usize = 10_000;
/// Resolved streams kept, and source items the URL refresher knows.
const STREAM_CACHE: usize = 32;
/// A stream URL that expires within this is resolved again before playing.
const EXPIRY_MARGIN_MS: i64 = 60_000;
/// Browse cursor tables kept.
const CURSOR_TABLES: usize = 64;
/// The artwork size asked of a plugin for theme colours and backdrops.
const ARTWORK_SIZE: u32 = 600;

/// Kinds a browse node is, most likely first (an id two kinds share).
const NODE_KINDS: &[HostKind] = &[
    HostKind::Album,
    HostKind::Playlist,
    HostKind::Artist,
    HostKind::Folder,
    HostKind::Track,
];
/// Kinds a played collection is, most likely first.
const COLLECTION_KINDS: &[HostKind] = &[
    HostKind::Album,
    HostKind::Playlist,
    HostKind::Folder,
    HostKind::Artist,
];

/// What source helpers tell the actor.
#[derive(Debug)]
pub(crate) enum SourcesMsg {
    /// A management job finished.
    Done {
        id: CommandId,
        result: Result<Done, String>,
    },
    /// Something may have changed the slice (a call that broke the plugin's
    /// limits, a plugin turned off): publish it if it did.
    Refresh,
    /// A login ended (logged in, failed or cancelled).
    LoginEnded { flow: u64 },
    /// A source track's stream URL, or why there is none.
    Resolved {
        uid: u64,
        result: Result<HttpSource, String>,
    },
    /// The items of a collection to queue.
    Collection {
        id: CommandId,
        plugin: String,
        then: Then,
        result: Result<Vec<Item>, String>,
    },
    /// The keystore check finished.
    SecretsChecked,
    /// What became of one of the packages the app ships.
    Bundled(Bundled),
}

/// What became of a package the app ships (`Config::bundled_plugins`).
#[derive(Debug)]
pub(crate) enum Bundled {
    /// Installed, or updated to the bundled version.
    Installed(String),
    /// Left alone: installed at this version or a newer one, or uninstalled
    /// by the person.
    Kept(String),
    /// A community package while community plugins are off: kept back, and
    /// installed (off, until its disclosure is accepted) once they are
    /// allowed (ADR-0012).
    Deferred(PathBuf),
    /// Unreadable, refused (its signature) or failed to install.
    Failed { path: PathBuf, error: String },
}

/// A management job that finished.
#[derive(Debug)]
pub(crate) enum Done {
    Installed(String),
    Uninstalled(String),
    /// Signed out; the plugin's own complaint, if it had one.
    LoggedOut {
        note: Option<String>,
    },
}

/// What to do with a fetched collection.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Then {
    Play { start: usize, shuffle: bool },
    Enqueue { play_next: bool },
}

enum Job {
    Install {
        id: CommandId,
        path: PathBuf,
    },
    Uninstall {
        id: CommandId,
        plugin: String,
    },
    Logout {
        id: CommandId,
        plugin: String,
    },
    /// Installs the packages the app ships that are missing or older than
    /// the bundled ones, except those in `declined`.
    Bundled {
        paths: Vec<PathBuf>,
        declined: BTreeSet<String>,
    },
}

/// A browse listing: (plugin, node); its cursor table holds the cursor of
/// each page fetched so far (`None` for the first).
type BrowseKey = (String, Option<String>);

/// What the helper threads share with the actor.
pub(crate) struct Ctx {
    pub(crate) host: PluginHost,
    shared: Arc<Shared>,
    tx: Sender<Msg>,
    items: Mutex<ItemCache>,
    cursors: Mutex<HashMap<BrowseKey, Vec<Option<String>>>>,
    latest_search: AtomicU64,
    search_timeout: Duration,
}

impl std::fmt::Debug for Ctx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx")
            .field("host", &self.host)
            .finish_non_exhaustive()
    }
}

impl Ctx {
    /// Remembers listed items so they can be passed back whole.
    fn remember(&self, plugin: &str, items: &[Item]) {
        lock(&self.items).insert(plugin, items);
    }

    /// A listed item by id, or a bare one of `fallback` kind.
    fn node(&self, plugin: &str, id: &str, prefer: &[HostKind], fallback: HostKind) -> Item {
        lock(&self.items)
            .get(plugin, id, prefer)
            .unwrap_or_else(|| convert::bare(id, fallback, id))
    }

    /// The item to pass back for a source track.
    pub(crate) fn track_item(&self, track: &SourceTrack) -> Item {
        lock(&self.items)
            .get(&track.plugin, &track.id, &[HostKind::Track])
            .filter(|i| i.kind == HostKind::Track)
            .unwrap_or_else(|| convert::track_item(track))
    }

    /// A plugin's links (ADR-0018); none when it declares none.
    pub(crate) fn links(&self, plugin: &str) -> jf_plugin_host::links::Links {
        self.host
            .plugin(plugin)
            .map(|info| info.links)
            .unwrap_or_default()
    }

    /// A plugin's display name.
    fn name(&self, plugin: &str) -> String {
        self.host
            .plugin(plugin)
            .map_or_else(|| plugin.to_owned(), |info| info.name)
    }

    fn emit(&self, event: Event) {
        self.shared.emit(event);
    }

    /// Publishes search results unless a newer search has started.
    fn emit_search(&self, update: SearchUpdate) {
        let request = update.request.0;
        self.shared.emit_if(
            || self.latest_search.load(Ordering::SeqCst) == request,
            Event::SearchResults(Arc::new(update)),
        );
    }

    fn send(&self, msg: SourcesMsg) {
        let _ = self.tx.send(Msg::Sources(msg));
    }

    /// After a failed call: a limit violation or a plugin turned off changes
    /// the slice.
    fn note(&self, error: &CallError) {
        if error.is_violation() || matches!(error, CallError::Disabled(_)) {
            self.send(SourcesMsg::Refresh);
        }
    }

    /// Lyrics for a source track from its plugin, and whether they may be
    /// shown synced (the plugin's content policy).
    pub(crate) fn lyrics(&self, plugin: &str, item: &Item) -> Option<(LyricDoc, bool)> {
        let payload = match wait(&self.host.lyrics(plugin, item.clone()), CALL_WAIT) {
            Ok(Some(payload)) => payload,
            Ok(None) | Err(CallError::NotDeclared(_)) => return None,
            Err(error) => {
                self.note(&error);
                tracing::warn!(%plugin, %error, "no lyrics from the plugin");
                return None;
            }
        };
        let Some(format) = convert::lyrics_format(payload.format) else {
            tracing::info!(%plugin, "the plugin's lyrics are in a format the core does not read");
            return None;
        };
        let mut doc = match jf_lyrics::parse(&payload.text, Some(format)) {
            Ok(doc) => doc,
            Err(error) => {
                tracing::warn!(%plugin, %error, "the plugin's lyrics could not be read");
                return None;
            }
        };
        let info = self.host.plugin(plugin);
        doc.source.origin = Some(
            payload
                .attribution
                .or_else(|| info.as_ref().map(|i| i.name.clone()))
                .unwrap_or_else(|| plugin.to_owned()),
        );
        if doc.language.is_none() {
            doc.language = payload.language;
        }
        let synced = info.is_none_or(|i| i.content_policy.lyrics_sync);
        if !synced {
            doc.timing = jf_lyrics::Timing::Unsynced;
        }
        Some((doc, synced))
    }

    /// A source track's artwork bytes: the plugin's `artwork` answer, or
    /// the picture it listed the track with.
    pub(crate) fn artwork(&self, plugin: &str, item: &Item, listed: Option<ImageRef>) -> Option<Vec<u8>> {
        let image = match wait(&self.host.artwork(plugin, item.clone(), ARTWORK_SIZE), CALL_WAIT) {
            Ok(Some(image)) => Some(image),
            Ok(None) | Err(CallError::NotDeclared(_)) => listed,
            Err(error) => {
                self.note(&error);
                tracing::warn!(%plugin, %error, "no artwork from the plugin");
                listed
            }
        }?;
        match fetch::image(&image) {
            Ok(bytes) => Some(bytes),
            Err(error) => {
                tracing::warn!(%plugin, %error, "could not fetch artwork");
                None
            }
        }
    }
}

/// Listed items by plugin and id, newest first per id; two generations of
/// [`ITEM_CACHE`] entries, so the oldest are forgotten in bulk.
#[derive(Debug, Default)]
struct ItemCache {
    current: HashMap<(String, String), Vec<Item>>,
    previous: HashMap<(String, String), Vec<Item>>,
}

impl ItemCache {
    fn insert(&mut self, plugin: &str, items: &[Item]) {
        for item in items {
            if self.current.len() >= ITEM_CACHE {
                self.previous = std::mem::take(&mut self.current);
            }
            let list = self
                .current
                .entry((plugin.to_owned(), item.id.clone()))
                .or_default();
            list.retain(|i| i.kind != item.kind);
            list.insert(0, item.clone());
        }
    }

    fn get(&self, plugin: &str, id: &str, prefer: &[HostKind]) -> Option<Item> {
        let key = (plugin.to_owned(), id.to_owned());
        let all: Vec<&Item> = self
            .current
            .get(&key)
            .into_iter()
            .chain(self.previous.get(&key))
            .flatten()
            .collect();
        prefer
            .iter()
            .find_map(|kind| all.iter().find(|i| i.kind == *kind))
            .or(all.first())
            .map(|i| (*i).clone())
    }
}

/// The community disclosures the person accepted (ADR-0012), in
/// `<data>/plugins/consent.json`.
#[derive(Debug)]
struct Consent {
    path: PathBuf,
    ids: BTreeSet<String>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct ConsentFile {
    acknowledged: BTreeSet<String>,
}

impl Consent {
    fn load(path: PathBuf) -> Consent {
        let ids = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<ConsentFile>(&bytes).ok())
            .map(|f| f.acknowledged)
            .unwrap_or_default();
        Consent { path, ids }
    }

    fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    fn add(&mut self, id: &str) -> Result<(), CoreError> {
        if self.ids.insert(id.to_owned()) {
            self.save()?;
        }
        Ok(())
    }

    fn remove(&mut self, id: &str) {
        if self.ids.remove(id)
            && let Err(e) = self.save()
        {
            tracing::warn!("could not save the community consent: {e}");
        }
    }

    fn save(&self) -> std::io::Result<()> {
        let file = ConsentFile {
            acknowledged: self.ids.clone(),
        };
        let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
        write_atomic(&self.path, &json)
    }
}

/// Plugin icons as PNG files in `<cache>/plugin-icons`, named by content.
#[derive(Debug)]
struct Icons {
    dir: PathBuf,
    /// Plugin id → (version, file).
    known: HashMap<String, (String, Option<PathBuf>)>,
}

impl Icons {
    fn path(&mut self, host: &PluginHost, id: &str, version: &str, has_icon: bool) -> Option<PathBuf> {
        if !has_icon {
            return None;
        }
        if let Some((known, path)) = self.known.get(id)
            && known == version
        {
            return path.clone();
        }
        let path = host.icon(id).and_then(|png| {
            let name = format!("{}.png", &blake3::hash(&png).to_hex()[..16]);
            let path = self.dir.join(name);
            if path.is_file() {
                return Some(path);
            }
            match write_atomic(&path, &png) {
                Ok(()) => Some(path),
                Err(e) => {
                    tracing::warn!("could not cache the icon of {id}: {e}");
                    None
                }
            }
        });
        self.known
            .insert(id.to_owned(), (version.to_owned(), path.clone()));
        path
    }

    fn forget(&mut self, id: &str) {
        if let Some((_, Some(path))) = self.known.remove(id)
            && !self.known.values().any(|(_, p)| p.as_ref() == Some(&path))
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A login in progress. Dropping it cancels the flow.
struct Login {
    plugin: String,
    flow: AuthFlow,
}

/// Online sources as the actor holds them.
pub(crate) struct Sources {
    ctx: Option<Arc<Ctx>>,
    /// Why there are no sources, when there are none.
    unavailable: Option<String>,
    secrets: Option<Arc<Selection>>,
    trust: TrustList,
    consent: Consent,
    icons: Icons,
    logins: HashMap<u64, Login>,
    next_flow: u64,
    searches: Option<Sender<SearchJob>>,
    jobs: Option<Sender<Job>>,
    refresher: Option<Arc<Refresher>>,
    /// Resolved stream URLs by queue uid, oldest first in `stream_order`.
    streams: HashMap<u64, HttpSource>,
    stream_order: VecDeque<u64>,
    /// Queue uids whose stream is being resolved.
    resolving: HashSet<u64>,
    version: u64,
    published: Option<Arc<SourcesSnapshot>>,
    /// The "sign-ins last for this session" error went out.
    warned_secrets: bool,
    /// Installs, uninstalls and logouts sent to the sources thread and not
    /// finished yet. Meanwhile only forced publishes go out, so the slice
    /// never shows a plugin the host has half added or removed (before its
    /// consent turned it on, or its icon went).
    jobs_in_flight: usize,
    /// Ids of the packages the app ships (`Config::bundled_plugins`), as
    /// the sources thread found them.
    bundled: BTreeSet<String>,
    /// Bundled plugins the person uninstalled: never installed again until
    /// they install one themselves.
    declined: BTreeSet<String>,
    /// Bundled community packages waiting for community plugins to be
    /// allowed.
    deferred: Vec<PathBuf>,
    /// A store build (ADR-0021 §4): no plugins from files, no community
    /// plugins.
    store_build: bool,
}

impl std::fmt::Debug for Sources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sources")
            .field("available", &self.ctx.is_some())
            .field("logins", &self.logins.len())
            .finish_non_exhaustive()
    }
}

impl Sources {
    /// Opens the plugin host (loading installed plugins; they compile on
    /// first use) and starts the helper threads. Never fails: without a host
    /// the slice says why and source commands fail with that reason.
    pub(crate) fn new(
        config: &Config,
        allow_community: bool,
        shared: &Arc<Shared>,
        tx: &Sender<Msg>,
    ) -> Sources {
        let plugins_dir = config.data_dir.join("plugins");
        let mut sources = Sources {
            ctx: None,
            unavailable: None,
            secrets: None,
            trust: config.sources.host.trust.clone(),
            consent: Consent::load(plugins_dir.join("consent.json")),
            icons: Icons {
                dir: config.cache_dir.join("plugin-icons"),
                known: HashMap::new(),
            },
            logins: HashMap::new(),
            next_flow: 1,
            searches: None,
            jobs: None,
            refresher: None,
            streams: HashMap::new(),
            stream_order: VecDeque::new(),
            resolving: HashSet::new(),
            version: 0,
            published: None,
            warned_secrets: false,
            jobs_in_flight: 0,
            bundled: BTreeSet::new(),
            declined: BTreeSet::new(),
            deferred: Vec::new(),
            store_build: config.distribution.store().is_some(),
        };
        if !config.sources.enabled {
            sources.unavailable = Some("online sources are turned off in this app's configuration".into());
            return sources;
        }
        let secrets = Selection::for_platform(config.sources.secrets.clone());
        let host = PluginHost::with_options(
            HostConfig {
                dir: plugins_dir,
                cache_dir: config.cache_dir.join("plugins"),
                secrets: secrets.clone(),
                allow_community,
            },
            config.sources.host.clone(),
        );
        let host = match host {
            Ok(host) => host,
            Err(e) => {
                tracing::error!("the plugin host did not start: {e}");
                sources.unavailable = Some(format!("online sources could not start: {e}"));
                return sources;
            }
        };
        let ctx = Arc::new(Ctx {
            host,
            shared: shared.clone(),
            tx: tx.clone(),
            items: Mutex::new(ItemCache::default()),
            cursors: Mutex::new(HashMap::new()),
            latest_search: AtomicU64::new(0),
            search_timeout: config.sources.search_timeout,
        });
        match search::spawn(ctx.clone()) {
            Ok(searches) => sources.searches = Some(searches),
            Err(e) => tracing::error!("the search thread did not start: {e}"),
        }
        match spawn_jobs(ctx.clone(), sources.trust.clone()) {
            Ok(jobs) => sources.jobs = Some(jobs),
            Err(e) => tracing::error!("the sources thread did not start: {e}"),
        }
        sources.refresher = Some(Arc::new(Refresher {
            ctx: ctx.clone(),
            items: Mutex::new(VecDeque::new()),
        }));
        // Check the keystore off the actor (D-Bus can be slow); secret calls
        // made meanwhile wait for the outcome.
        let check = {
            let (secrets, tx) = (secrets.clone(), tx.clone());
            std::thread::Builder::new()
                .name("jf-secrets".into())
                .spawn(move || {
                    secrets.run_check();
                    let _ = tx.send(Msg::Sources(SourcesMsg::SecretsChecked));
                })
        };
        if let Err(e) = check {
            tracing::error!("the keystore check did not start: {e}");
        }
        sources.secrets = Some(secrets);
        sources.ctx = Some(ctx);
        sources
    }

    /// The shared context, for the media worker.
    pub(crate) fn ctx(&self) -> Option<Arc<Ctx>> {
        self.ctx.clone()
    }

    /// The engine's URL refresher for source streams.
    pub(crate) fn refresher(&self) -> Option<Arc<dyn StreamResolver>> {
        self.refresher.clone().map(|r| r as Arc<dyn StreamResolver>)
    }

    fn require(&self) -> Result<&Arc<Ctx>, CoreError> {
        self.ctx.as_ref().ok_or_else(|| {
            CoreError::Other(
                self.unavailable
                    .clone()
                    .unwrap_or_else(|| "online sources are not available".into()),
            )
        })
    }

    /// The plugin, if it is installed, declares `capability` and may be
    /// called now.
    fn usable(&self, plugin: &str, capability: Capability) -> Result<jf_plugin_host::PluginInfo, CoreError> {
        let ctx = self.require()?;
        let info = ctx
            .host
            .plugin(plugin)
            .ok_or_else(|| CoreError::Other(format!("no source plugin {plugin} is installed")))?;
        if !info.capabilities.contains(&capability) {
            return Err(CoreError::Other(format!(
                "{} does not offer {}",
                info.name,
                jf_plugin_host::capability_name(capability).replace('_', " ")
            )));
        }
        if !info.official && !ctx.host.allow_community() {
            return Err(CoreError::Other(format!(
                "{} is a community plugin, and community plugins are turned off",
                info.name
            )));
        }
        if !info.enabled {
            return Err(CoreError::Other(match &info.disabled_reason {
                Some(reason) => format!("{} is turned off: {reason}", info.name),
                None => format!("{} is turned off", info.name),
            }));
        }
        Ok(info)
    }

    // ---- remote sessions --------------------------------------------------

    /// Whether `plugin` is a remote source (`playback_mode = "remote"`).
    pub(crate) fn is_remote(&self, plugin: &str) -> bool {
        self.ctx.as_ref().is_some_and(|ctx| {
            ctx.host
                .plugin(plugin)
                .is_some_and(|info| info.playback_mode == PlaybackMode::Remote)
        })
    }

    /// A plugin's name, or its id when it is not installed.
    pub(crate) fn display_name(&self, plugin: &str) -> String {
        self.ctx
            .as_ref()
            .map_or_else(|| plugin.to_owned(), |ctx| ctx.name(plugin))
    }

    /// What a remote session needs to control `plugin`: it must be usable
    /// and declare `remote`.
    pub(crate) fn remote_backend(
        &self,
        plugin: &str,
    ) -> Result<(crate::remote::PluginBackend, crate::remote::Source), CoreError> {
        let info = self.usable(plugin, Capability::Remote)?;
        let ctx = self.require()?;
        Ok((
            crate::remote::PluginBackend {
                host: ctx.host.clone(),
                plugin: plugin.to_owned(),
            },
            crate::remote::Source {
                id: plugin.to_owned(),
                name: info.name,
                links: info.links,
            },
        ))
    }

    /// The item to hand a remote source for one of its tracks.
    pub(crate) fn remote_track(&self, track: &jf_session::SourceTrack) -> Result<Item, CoreError> {
        Ok(self.require()?.track_item(track))
    }

    /// The listed container `node` of a remote source, to play as a context.
    pub(crate) fn remote_node(&self, plugin: &str, node: &str) -> Result<Item, CoreError> {
        Ok(self
            .require()?
            .node(plugin, node, COLLECTION_KINDS, HostKind::Album))
    }

    // ---- the slice ------------------------------------------------------

    /// Publishes the `Sources` slice: always with `force`, otherwise only
    /// when something in it changed.
    pub(crate) fn publish(&mut self, allow_community: bool, shared: &Shared, force: bool) {
        if !force && self.jobs_in_flight > 0 {
            return; // the job's end publishes
        }
        let mut plugins = Vec::new();
        if let Some(ctx) = &self.ctx {
            for info in ctx.host.plugins() {
                let icon = self.icons.path(&ctx.host, &info.id, &info.version, info.has_icon);
                plugins.push(convert::summary(
                    &info,
                    allow_community,
                    self.consent.contains(&info.id),
                    icon,
                ));
            }
        }
        let presets = convert::presets(&plugins);
        let mut snapshot = SourcesSnapshot {
            version: self.version,
            plugins,
            presets,
            allow_community,
            secret_storage: self
                .secrets
                .as_ref()
                .map(|s| s.status().storage())
                .unwrap_or_default(),
            unavailable: self.unavailable.clone(),
            can_install_plugins: !self.store_build,
        };
        if !force && self.published.as_deref() == Some(&snapshot) {
            return;
        }
        self.version += 1;
        snapshot.version = self.version;
        let snapshot = Arc::new(snapshot);
        shared.update(|s| s.sources = snapshot.clone());
        shared.emit(Event::Sources(snapshot.clone()));
        self.published = Some(snapshot);
        self.warn_about_secrets(shared);
    }

    /// Says once that sign-ins last for this session only, when the keystore
    /// is out of reach and a plugin that signs in is installed.
    fn warn_about_secrets(&mut self, shared: &Shared) {
        if self.warned_secrets {
            return;
        }
        let (Some(secrets), Some(ctx)) = (&self.secrets, &self.ctx) else {
            return;
        };
        let Check::SessionOnly(reason) = secrets.status() else {
            return;
        };
        if !ctx
            .host
            .plugins()
            .iter()
            .any(|p| p.capabilities.contains(&Capability::Auth))
        {
            return;
        }
        self.warned_secrets = true;
        shared.emit(Event::Error {
            message: format!(
                "Sign-ins to online sources last until Jellyfesh quits: no keystore can be reached ({reason})."
            ),
        });
    }

    // ---- management -----------------------------------------------------

    /// Whether this is a store build, which installs no plugins from files.
    pub(crate) fn store_build(&self) -> bool {
        self.store_build
    }

    pub(crate) fn install(&mut self, id: CommandId, path: PathBuf) -> Result<(), CoreError> {
        if self.store_build {
            return Err(CoreError::Other(
                "plugins cannot be installed from files in this edition of Jellyfesh".into(),
            ));
        }
        self.job(Job::Install { id, path })
    }

    pub(crate) fn uninstall(&mut self, id: CommandId, plugin: String) -> Result<(), CoreError> {
        let ctx = self.require()?;
        if ctx.host.plugin(&plugin).is_none() {
            return Err(CoreError::Other(format!(
                "no source plugin {plugin} is installed"
            )));
        }
        self.job(Job::Uninstall { id, plugin })
    }

    pub(crate) fn logout(&mut self, id: CommandId, plugin: String) -> Result<(), CoreError> {
        let ctx = self.require()?;
        let info = ctx
            .host
            .plugin(&plugin)
            .ok_or_else(|| CoreError::Other(format!("no source plugin {plugin} is installed")))?;
        if !info.capabilities.contains(&Capability::Auth) {
            return Err(CoreError::Other(format!("{} has no sign-in", info.name)));
        }
        self.job(Job::Logout { id, plugin })
    }

    fn job(&mut self, job: Job) -> Result<(), CoreError> {
        self.require()?;
        self.jobs
            .as_ref()
            .ok_or_else(|| CoreError::Other("the sources thread is not running".into()))?
            .send(job)
            .map_err(|_| CoreError::Other("the sources thread has stopped".into()))?;
        self.jobs_in_flight += 1;
        Ok(())
    }

    /// A job sent with [`Sources::job`] finished.
    pub(crate) fn job_done(&mut self) {
        self.jobs_in_flight = self.jobs_in_flight.saturating_sub(1);
    }

    /// Installs, uninstalls and logouts not finished yet.
    pub(crate) fn jobs_in_flight(&self) -> usize {
        self.jobs_in_flight
    }

    /// After an install: a community plugin whose disclosure was accepted
    /// before it was installed turns on. One the person installed
    /// (`by_person`) is no longer declined as a bundled plugin; returns
    /// whether that changed [`Sources::declined`].
    pub(crate) fn installed(&mut self, plugin: &str, by_person: bool) -> bool {
        if let Some(ctx) = &self.ctx
            && let Some(info) = ctx.host.plugin(plugin)
            && !info.official
            && !info.enabled
            && self.consent.contains(plugin)
            && ctx.host.allow_community()
            && let Err(e) = ctx.host.set_enabled(plugin, true)
        {
            tracing::warn!("could not turn on {plugin}: {e}");
        }
        by_person && self.declined.remove(plugin)
    }

    /// After an uninstall: forget its consent, icon and logins; a bundled
    /// plugin is declined from now on. Returns whether that changed
    /// [`Sources::declined`].
    pub(crate) fn uninstalled(&mut self, plugin: &str) -> bool {
        self.consent.remove(plugin);
        self.icons.forget(plugin);
        self.logins.retain(|_, l| l.plugin != plugin);
        self.bundled.contains(plugin) && self.declined.insert(plugin.to_owned())
    }

    /// Installs or updates the packages the app ships, off the actor
    /// (installing compiles the plugin), skipping the bundled plugins the
    /// person uninstalled (`declined`). Verification and trust are those of
    /// any package. Each outcome comes back as [`SourcesMsg::Bundled`].
    pub(crate) fn install_bundled(&mut self, paths: Vec<PathBuf>, declined: BTreeSet<String>) {
        self.declined = declined;
        if paths.is_empty() {
            return;
        }
        let Some(jobs) = &self.jobs else {
            return; // no host: the slice says why
        };
        let declined = self.declined.clone();
        if jobs.send(Job::Bundled { paths, declined }).is_err() {
            tracing::error!("the sources thread has stopped; bundled plugins are not installed");
        }
    }

    /// Notes what became of a bundled package; returns the plugin to treat
    /// as installed, if one was.
    pub(crate) fn bundled_outcome(&mut self, outcome: &Bundled) -> Option<String> {
        match outcome {
            Bundled::Installed(id) => {
                self.bundled.insert(id.clone());
                self.installed(id, false);
                Some(id.clone())
            }
            Bundled::Kept(id) => {
                self.bundled.insert(id.clone());
                None
            }
            Bundled::Deferred(path) => {
                if !self.deferred.contains(path) {
                    self.deferred.push(path.clone());
                }
                None
            }
            Bundled::Failed { .. } => None,
        }
    }

    /// Installs the bundled community packages kept back while community
    /// plugins were off (call it when they are allowed).
    pub(crate) fn install_deferred(&mut self) {
        let paths = std::mem::take(&mut self.deferred);
        if paths.is_empty() {
            return;
        }
        let declined = self.declined.clone();
        self.install_bundled(paths, declined);
    }

    /// The bundled plugins the person uninstalled, to remember.
    pub(crate) fn declined(&self) -> &BTreeSet<String> {
        &self.declined
    }

    pub(crate) fn set_enabled(&mut self, plugin: &str, enabled: bool) -> Result<(), CoreError> {
        let ctx = self.require()?;
        let info = ctx
            .host
            .plugin(plugin)
            .ok_or_else(|| CoreError::Other(format!("no source plugin {plugin} is installed")))?;
        if enabled && !info.official && !self.consent.contains(plugin) {
            return Err(CoreError::Other(format!(
                "{} is a community plugin: show its disclosure and send AcknowledgeUnofficial first",
                info.name
            )));
        }
        ctx.host.set_enabled(plugin, enabled).map_err(host_error)
    }

    /// Restores a plugin's settings to its schema's defaults.
    pub(crate) fn reset_settings(&self, plugin: &str) -> Result<(), CoreError> {
        let ctx = self.require()?;
        let info = ctx
            .host
            .plugin(plugin)
            .ok_or_else(|| CoreError::Other(format!("no plugin {plugin} is installed")))?;
        let defaults = jf_plugin_host::schema::SettingsSchema::parse_with(
            &info.settings_schema_text,
            jf_plugin_host::schema::Profile::V1_2,
        )
        .map(|schema| schema.defaults())
        .unwrap_or_else(|_| serde_json::json!({}));
        ctx.host
            .set_settings(plugin, &defaults.to_string())
            .map_err(host_error)
    }

    pub(crate) fn set_settings(&self, plugin: &str, json: &str) -> Result<(), CoreError> {
        self.require()?
            .host
            .set_settings(plugin, json)
            .map_err(host_error)
    }

    pub(crate) fn set_allow_community(&self, allow: bool) {
        if let Some(ctx) = &self.ctx {
            ctx.host.set_allow_community(allow);
        }
    }

    /// Records the disclosure as accepted.
    pub(crate) fn acknowledge(&mut self, plugin: &str) -> Result<(), CoreError> {
        let plugin = plugin.trim();
        if plugin.is_empty() {
            return Err(CoreError::Other("no plugin named".into()));
        }
        self.require()?;
        self.consent.add(plugin)
    }

    /// Turns a community plugin on after its disclosure was accepted (when
    /// it is installed).
    pub(crate) fn enable_acknowledged(&self, plugin: &str) -> Result<(), CoreError> {
        let ctx = self.require()?;
        match ctx.host.plugin(plugin) {
            Some(info) if !info.official && !info.enabled => {
                ctx.host.set_enabled(plugin, true).map_err(host_error)
            }
            _ => Ok(()),
        }
    }

    // ---- logins ---------------------------------------------------------

    pub(crate) fn begin_login(&mut self, plugin: &str, method: AuthMethod) -> Result<(), CoreError> {
        let ctx = self.require()?.clone();
        // One login per source: a new one replaces the one in progress
        // (dropping its flow cancels it).
        self.logins.retain(|_, l| l.plugin != plugin);
        let flow = ctx
            .host
            .begin_auth(plugin, convert::auth_kind(method))
            .map_err(|e| CoreError::Other(e.to_string()))?;
        let number = self.next_flow;
        self.next_flow += 1;
        let steps = flow.steps().clone();
        let name = plugin.to_owned();
        std::thread::Builder::new()
            .name("jf-login".into())
            .spawn(move || {
                for step in steps.iter() {
                    let last = step.is_final();
                    ctx.emit(Event::Auth(AuthUpdate {
                        plugin: name.clone(),
                        flow: number,
                        step: convert::auth_step(step),
                    }));
                    if last {
                        break;
                    }
                }
                ctx.send(SourcesMsg::LoginEnded { flow: number });
            })?;
        self.logins.insert(
            number,
            Login {
                plugin: plugin.to_owned(),
                flow,
            },
        );
        Ok(())
    }

    /// `Command::ConnectSource`: the server's address (for self-hosted
    /// servers), the plugin on, then a login as `begin_login` starts it.
    pub(crate) fn connect(
        &mut self,
        preset: &str,
        address: Option<&str>,
        method: AuthMethod,
    ) -> Result<(), CoreError> {
        let preset = presets::preset(preset)
            .ok_or_else(|| CoreError::Other(format!("there is no service {preset:?} to connect to")))?;
        let ctx = self.require()?.clone();
        let info = ctx
            .host
            .plugin(preset.plugin)
            .filter(|info| info.official)
            .ok_or_else(|| {
                CoreError::Other(format!(
                    "{} needs its plugin, which is not installed",
                    preset.name
                ))
            })?;
        if let Some(key) = preset.address_setting {
            let address = address.map(str::trim).filter(|a| !a.is_empty()).ok_or_else(|| {
                CoreError::Other(format!("Enter the address of your {} server.", preset.name))
            })?;
            let mut settings = info.settings.as_object().cloned().unwrap_or_default();
            settings.insert(key.to_owned(), serde_json::Value::String(address.to_owned()));
            ctx.host
                .set_settings(preset.plugin, &serde_json::Value::Object(settings).to_string())
                .map_err(host_error)?;
        }
        if !info.enabled {
            ctx.host.set_enabled(preset.plugin, true).map_err(host_error)?;
        }
        self.begin_login(preset.plugin, method)
    }

    fn login(&self, plugin: &str, flow: u64) -> Result<&Login, CoreError> {
        self.logins
            .get(&flow)
            .filter(|l| l.plugin == plugin)
            .ok_or_else(|| CoreError::Other(format!("there is no login {flow} to {plugin} in progress")))
    }

    pub(crate) fn submit_login(
        &self,
        plugin: &str,
        flow: u64,
        fields: Vec<(String, String)>,
    ) -> Result<(), CoreError> {
        let login = self.login(plugin, flow)?;
        login.flow.submit(
            fields
                .into_iter()
                .map(|(name, value)| FieldValue { name, value })
                .collect(),
        );
        Ok(())
    }

    pub(crate) fn cancel_login(&self, plugin: &str, flow: u64) -> Result<(), CoreError> {
        self.login(plugin, flow)?.flow.cancel();
        Ok(())
    }

    pub(crate) fn login_ended(&mut self, flow: u64) {
        self.logins.remove(&flow);
    }

    // ---- search and browse ------------------------------------------------

    /// Plugins a search of everything asks: on, callable, searchable, and
    /// whose terms allow merging their results with others.
    pub(crate) fn unified_search_plugins(&self) -> Vec<String> {
        let Some(ctx) = &self.ctx else {
            return Vec::new();
        };
        let community = ctx.host.allow_community();
        ctx.host
            .plugins()
            .into_iter()
            .filter(|p| {
                p.enabled
                    && (p.official || community)
                    && p.capabilities.contains(&Capability::Search)
                    && p.content_policy.unified_search
            })
            .map(|p| p.id)
            .collect()
    }

    /// Checks that a plugin can be searched now.
    pub(crate) fn check_search(&self, plugin: &str) -> Result<(), CoreError> {
        self.usable(plugin, Capability::Search).map(|_| ())
    }

    /// Makes `request` the newest search: results of older ones still on
    /// their way are dropped.
    pub(crate) fn begin_search(&self, request: CommandId) {
        if let Some(ctx) = &self.ctx {
            ctx.latest_search.store(request.0, Ordering::SeqCst);
        }
    }

    /// Publishes search results (the library's answer) unless a newer
    /// search has begun.
    pub(crate) fn emit_search(&self, shared: &Shared, update: SearchUpdate) {
        match &self.ctx {
            Some(ctx) => ctx.emit_search(update),
            None => shared.emit(Event::SearchResults(Arc::new(update))),
        }
    }

    /// Hands a search to the search thread.
    pub(crate) fn start_search(&self, job: SearchJob) -> Result<(), CoreError> {
        self.searches
            .as_ref()
            .ok_or_else(|| CoreError::Other("the search thread is not running".into()))?
            .send(job)
            .map_err(|_| CoreError::Other("the search thread has stopped".into()))
    }

    pub(crate) fn browse(
        &self,
        request: CommandId,
        plugin: String,
        node: Option<String>,
        page: u32,
    ) -> Result<(), CoreError> {
        self.usable(&plugin, Capability::Browse)?;
        let ctx = self.require()?.clone();
        let item = node
            .as_deref()
            .map(|id| ctx.node(&plugin, id, NODE_KINDS, HostKind::Folder));
        std::thread::Builder::new()
            .name("jf-browse".into())
            .spawn(move || {
                let update = browse_page(&ctx, request, plugin, node, item, page);
                ctx.emit(Event::BrowsePage(Arc::new(update)));
            })?;
        Ok(())
    }

    /// Fetches every page of a source container, then tells the actor.
    pub(crate) fn fetch_collection(
        &self,
        id: CommandId,
        plugin: String,
        node: String,
        then: Then,
    ) -> Result<(), CoreError> {
        self.usable(&plugin, Capability::Browse)?;
        let ctx = self.require()?.clone();
        let item = ctx.node(&plugin, &node, COLLECTION_KINDS, HostKind::Album);
        std::thread::Builder::new()
            .name("jf-collection".into())
            .spawn(move || {
                let result = collection_items(&ctx, &plugin, &item);
                ctx.send(SourcesMsg::Collection {
                    id,
                    plugin,
                    then,
                    result,
                });
            })?;
        Ok(())
    }

    // ---- streams ----------------------------------------------------------

    /// The stream URL of a source track in the queue when it is resolved;
    /// otherwise starts resolving it (once) and returns `None`. Fails when
    /// its plugin cannot stream at all.
    pub(crate) fn stream(&mut self, uid: u64, track: &SourceTrack) -> Result<Option<HttpSource>, CoreError> {
        let ctx = self.require()?.clone();
        if let Some(source) = self.streams.get(&uid)
            && !expiring(source)
        {
            let source = source.clone();
            self.register(&ctx, uid, track);
            return Ok(Some(source));
        }
        let info = ctx.host.plugin(&track.plugin).ok_or_else(|| {
            CoreError::Other(format!("its source plugin {} is not installed", track.plugin))
        })?;
        if info.playback_mode != PlaybackMode::Stream
            || !info.capabilities.contains(&Capability::ResolveStream)
        {
            return Err(CoreError::Other(format!(
                "{} does not stream audio to Jellyfesh",
                info.name
            )));
        }
        if !self.resolving.insert(uid) {
            return Ok(None);
        }
        let item = ctx.track_item(track);
        let plugin = track.plugin.clone();
        let spawned = std::thread::Builder::new().name("jf-resolve".into()).spawn({
            let ctx = ctx.clone();
            move || {
                let result = wait(&ctx.host.resolve_stream(&plugin, item, QUALITY), RESOLVE_WAIT)
                    .map(convert::http_source)
                    .map_err(|e| {
                        ctx.note(&e);
                        e.to_string()
                    });
                ctx.send(SourcesMsg::Resolved { uid, result });
            }
        });
        if let Err(e) = spawned {
            self.resolving.remove(&uid);
            return Err(e.into());
        }
        Ok(None)
    }

    /// Keeps a resolved stream for its queue entry.
    pub(crate) fn resolved(&mut self, uid: u64, result: &Result<HttpSource, String>) {
        self.resolving.remove(&uid);
        if let Ok(source) = result {
            if self.streams.insert(uid, source.clone()).is_none() {
                self.stream_order.push_back(uid);
            }
            while self.stream_order.len() > STREAM_CACHE {
                if let Some(old) = self.stream_order.pop_front() {
                    self.streams.remove(&old);
                }
            }
        }
    }

    /// Tells the URL refresher which plugin item a queue entry is.
    fn register(&self, ctx: &Ctx, uid: u64, track: &SourceTrack) {
        if let Some(refresher) = &self.refresher {
            let mut items = lock(&refresher.items);
            items.retain(|(u, _, _)| *u != uid);
            items.push_back((uid, track.plugin.clone(), ctx.track_item(track)));
            while items.len() > STREAM_CACHE {
                items.pop_front();
            }
        }
    }
}

/// Refreshes the stream URL of a source track for the engine (before it
/// expires, and after the server refused it), through the plugin that owns
/// it. Called on the stream's own thread, so it may wait for the plugin.
pub(crate) struct Refresher {
    ctx: Arc<Ctx>,
    /// (queue uid, plugin, item) of the source streams handed to the engine.
    items: Mutex<VecDeque<(u64, String, Item)>>,
}

impl StreamResolver for Refresher {
    fn resolve(&self, id: u64) -> Result<HttpSource, String> {
        let (plugin, item) = lock(&self.items)
            .iter()
            .find(|(uid, _, _)| *uid == id)
            .map(|(_, plugin, item)| (plugin.clone(), item.clone()))
            .ok_or_else(|| "this stream is not from an online source".to_owned())?;
        tracing::debug!(%plugin, uid = id, "resolving a stream again");
        wait(
            &self.ctx.host.resolve_stream(&plugin, item, QUALITY),
            RESOLVE_WAIT,
        )
        .map(convert::http_source)
        .map_err(|e| {
            self.ctx.note(&e);
            e.to_string()
        })
    }
}

/// Runs installs, uninstalls and logouts in order.
fn spawn_jobs(ctx: Arc<Ctx>, trust: TrustList) -> std::io::Result<Sender<Job>> {
    let (tx, rx) = unbounded::<Job>();
    std::thread::Builder::new()
        .name("jf-sources".into())
        .spawn(move || {
            for job in rx.iter() {
                let (id, result) = match job {
                    Job::Install { id, path } => {
                        let result = match ctx.host.install(&path) {
                            Ok(info) => Ok(Done::Installed(info.id)),
                            Err(HostError::CommunityDisabled) => Err(community_refusal(&path, &trust)),
                            Err(e) => Err(e.to_string()),
                        };
                        (id, result)
                    }
                    Job::Uninstall { id, plugin } => (
                        id,
                        ctx.host
                            .uninstall(&plugin)
                            .map(|()| Done::Uninstalled(plugin))
                            .map_err(|e| e.to_string()),
                    ),
                    Job::Bundled { paths, declined } => {
                        for path in paths {
                            let outcome = install_bundled(&ctx.host, &trust, &path, &declined);
                            ctx.send(SourcesMsg::Bundled(outcome));
                        }
                        continue;
                    }
                    Job::Logout { id, plugin } => {
                        let result = match wait(&ctx.host.logout(&plugin), LOGOUT_WAIT) {
                            Ok(()) => Ok(Done::LoggedOut { note: None }),
                            // Signed out here; the plugin could not tell the service.
                            Err(CallError::Plugin(error)) => Ok(Done::LoggedOut {
                                note: Some(format!(
                                    "Signed out of {}; the service said: {}",
                                    ctx.name(&plugin),
                                    jf_plugin_host::plugin_error_text(&error)
                                )),
                            }),
                            Err(e) if e.is_violation() => {
                                ctx.note(&e);
                                Ok(Done::LoggedOut {
                                    note: Some(format!(
                                        "Signed out of {}; the plugin failed: {e}",
                                        ctx.name(&plugin)
                                    )),
                                })
                            }
                            Err(e) => Err(e.to_string()),
                        };
                        (id, result)
                    }
                };
                ctx.send(SourcesMsg::Done { id, result });
            }
        })?;
    Ok(tx)
}

/// Installs one package the app ships unless the person uninstalled it or
/// the same or a newer version is installed.
fn install_bundled(
    host: &PluginHost,
    trust: &TrustList,
    path: &Path,
    declined: &BTreeSet<String>,
) -> Bundled {
    let failed = |error: String| Bundled::Failed {
        path: path.to_owned(),
        error,
    };
    let package = match std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| jf_plugin_host::package::read(&bytes, trust).map_err(|e| e.to_string()))
    {
        Ok(package) => package,
        Err(error) => return failed(error),
    };
    let id = package.manifest.id.clone();
    if declined.contains(&id) {
        tracing::info!(plugin = %id, "not installing a bundled plugin the person uninstalled");
        return Bundled::Kept(id);
    }
    if let Some(installed) = host.plugin(&id) {
        if !jf_plugin_host::install::bundled_updates(&installed.version, &package.manifest.version) {
            return Bundled::Kept(id);
        }
        tracing::info!(plugin = %id, from = %installed.version, to = %package.manifest.version, "updating a bundled plugin");
    }
    match host.install(path) {
        Ok(info) => Bundled::Installed(info.id),
        // Not an error: it waits until the person allows community plugins.
        Err(HostError::CommunityDisabled) => {
            tracing::info!(plugin = %id, "a bundled community plugin waits until community plugins are allowed");
            Bundled::Deferred(path.to_owned())
        }
        Err(e) => failed(e.to_string()),
    }
}

/// Why a community package was refused, naming it when it can be read.
fn community_refusal(path: &Path, trust: &TrustList) -> String {
    let package = std::fs::read(path)
        .ok()
        .and_then(|bytes| jf_plugin_host::package::read(&bytes, trust).ok());
    jf_plugin_host::install::community_refusal(
        package
            .as_ref()
            .map(|p| (p.manifest.name.as_str(), p.manifest.id.as_str())),
    )
}

/// One browse page, fetching the pages before it when their cursors are
/// not known yet.
fn browse_page(
    ctx: &Ctx,
    request: CommandId,
    plugin: String,
    node: Option<String>,
    item: Option<Item>,
    page: u32,
) -> BrowseUpdate {
    let failed = |error: String| BrowseUpdate {
        request,
        plugin: plugin.clone(),
        node: node.clone(),
        items: Vec::new(),
        next_page: None,
        error: Some(error),
    };
    let key = (plugin.clone(), node.clone());
    let mut cursors = lock(&ctx.cursors)
        .get(&key)
        .cloned()
        .unwrap_or_else(|| vec![None]);
    let wanted = page as usize;
    let mut at = wanted.min(cursors.len() - 1);
    let first = at;
    loop {
        let reply = ctx.host.browse(
            &plugin,
            item.clone(),
            PageRequest {
                cursor: cursors[at].clone(),
                limit: BROWSE_PAGE,
            },
        );
        let answer = match wait(&reply, CALL_WAIT) {
            Ok(answer) => answer,
            Err(error) => {
                ctx.note(&error);
                return failed(error.to_string());
            }
        };
        ctx.remember(&plugin, &answer.items);
        cursors.truncate(at + 1);
        if let Some(next) = &answer.next {
            cursors.push(Some(next.clone()));
        }
        {
            let mut tables = lock(&ctx.cursors);
            if tables.len() >= CURSOR_TABLES && !tables.contains_key(&key) {
                tables.clear();
            }
            tables.insert(key.clone(), cursors.clone());
        }
        if at == wanted {
            let links = ctx.links(&plugin);
            return BrowseUpdate {
                request,
                plugin: plugin.clone(),
                node: node.clone(),
                items: answer
                    .items
                    .iter()
                    .map(|i| convert::source_item(&plugin, i, &links))
                    .collect(),
                next_page: answer.next.map(|_| page.saturating_add(1)),
                error: None,
            };
        }
        if answer.next.is_none() {
            return failed(format!("there is no page {page}"));
        }
        at += 1;
        if at - first > MAX_CATCH_UP {
            return failed(format!("page {page} is too far ahead; browse the pages in order"));
        }
    }
}

/// Every child of a collection, in order. An artist (or anything that holds
/// albums and no tracks) gives its albums' tracks.
fn collection_items(ctx: &Ctx, plugin: &str, node: &Item) -> Result<Vec<Item>, String> {
    let children = all_pages(ctx, plugin, node)?;
    if children.iter().any(|i| i.kind == HostKind::Track) {
        return Ok(children);
    }
    let mut tracks = Vec::new();
    for album in children
        .iter()
        .filter(|i| i.kind == HostKind::Album)
        .take(COLLECTION_ALBUMS)
    {
        tracks.extend(
            all_pages(ctx, plugin, album)?
                .into_iter()
                .filter(|i| i.kind == HostKind::Track),
        );
        if tracks.len() >= COLLECTION_ITEMS {
            break;
        }
    }
    Ok(tracks)
}

fn all_pages(ctx: &Ctx, plugin: &str, node: &Item) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    let mut cursor = None;
    for _ in 0..COLLECTION_PAGES {
        let reply = ctx.host.browse(
            plugin,
            Some(node.clone()),
            PageRequest {
                cursor,
                limit: COLLECTION_PAGE,
            },
        );
        let page = wait(&reply, CALL_WAIT).map_err(|e| {
            ctx.note(&e);
            e.to_string()
        })?;
        ctx.remember(plugin, &page.items);
        items.extend(page.items);
        match page.next {
            Some(next) if items.len() < COLLECTION_ITEMS => cursor = Some(next),
            _ => break,
        }
    }
    items.truncate(COLLECTION_ITEMS);
    Ok(items)
}

/// Waits at most `timeout` for a plugin's answer.
fn wait<T>(reply: &Reply<T>, timeout: Duration) -> Result<T, CallError> {
    match reply.recv_timeout(timeout) {
        Ok(result) => result,
        Err(crossbeam_channel::RecvTimeoutError::Timeout) => Err(CallError::Timeout),
        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => Err(CallError::Shutdown),
    }
}

fn expiring(source: &HttpSource) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    source
        .expires_at_ms
        .is_some_and(|at| at <= now.saturating_add(EXPIRY_MARGIN_MS))
}

fn host_error(error: HostError) -> CoreError {
    CoreError::Other(error.to_string())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Writes a file through a temporary file in the same folder.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// The queue entries for a fetched collection: its playable tracks, and the
/// index among them of the listing's entry `start` (or the first playable
/// one after it). Fails, with the service's reason when it gave one, when
/// nothing can play.
pub(crate) fn collection_queue(
    plugin: &str,
    items: &[Item],
    start: usize,
) -> Result<(Vec<TrackRef>, usize), CoreError> {
    let mut tracks = Vec::new();
    let mut start_at = None;
    let mut reason = None;
    for (i, item) in items.iter().enumerate() {
        if item.kind != HostKind::Track {
            continue;
        }
        if !item.playable {
            reason = reason.or_else(|| unplayable_reason(item));
            continue;
        }
        if i >= start && start_at.is_none() {
            start_at = Some(tracks.len());
        }
        tracks.push(TrackRef::Source(source_track(plugin, item)));
    }
    if tracks.is_empty() {
        return Err(CoreError::Other(match reason {
            Some(reason) => format!("nothing to play: {reason}"),
            None => "nothing to play".into(),
        }));
    }
    let start = start_at.unwrap_or(tracks.len() - 1);
    Ok((tracks, start))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: HostKind) -> Item {
        convert::bare(id, kind, id)
    }

    #[test]
    fn the_item_cache_prefers_the_kind_asked_for_and_forgets_in_bulk() {
        let mut cache = ItemCache::default();
        cache.insert("p", &[item("5", HostKind::Artist), item("5", HostKind::Album)]);
        assert_eq!(cache.get("p", "5", NODE_KINDS).unwrap().kind, HostKind::Album);
        assert_eq!(
            cache.get("p", "5", &[HostKind::Artist]).unwrap().kind,
            HostKind::Artist
        );
        assert_eq!(
            cache.get("p", "5", &[HostKind::Track]).unwrap().kind,
            HostKind::Album,
            "newest"
        );
        assert!(cache.get("q", "5", NODE_KINDS).is_none());
        let many: Vec<Item> = (0..ITEM_CACHE * 2 + 1)
            .map(|i| item(&i.to_string(), HostKind::Track))
            .collect();
        cache.insert("p", &many);
        assert!(
            cache.get("p", "0", &[]).is_none(),
            "the oldest generation is gone"
        );
        assert!(cache.get("p", &(ITEM_CACHE * 2).to_string(), &[]).is_some());
    }

    #[test]
    fn collections_queue_playable_tracks_and_keep_the_reason_otherwise() {
        let mut locked = item("t1", HostKind::Track);
        locked.playable = false;
        locked.unplayable = Some(jf_plugin_host::Unplayable {
            reason: jf_plugin_host::UnplayableReason::Region,
            message: None,
        });
        let items = vec![
            item("disc", HostKind::Folder),
            locked.clone(),
            item("t2", HostKind::Track),
            item("t3", HostKind::Track),
        ];
        let (tracks, start) = collection_queue("p", &items, 1).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(start, 0, "the locked entry starts at the next playable one");
        assert_eq!(collection_queue("p", &items, 3).unwrap().1, 1);
        assert_eq!(
            collection_queue("p", &items, 9).unwrap().1,
            1,
            "past the end: the last"
        );
        let error = collection_queue("p", &[locked], 0).unwrap_err().to_string();
        assert_eq!(error, "nothing to play: Not available in your region");
    }

    #[test]
    fn consent_is_kept_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plugins").join("consent.json");
        let mut consent = Consent::load(path.clone());
        assert!(!consent.contains("org.example.a"));
        consent.add("org.example.a").unwrap();
        assert!(Consent::load(path.clone()).contains("org.example.a"));
        consent.remove("org.example.a");
        assert!(!Consent::load(path).contains("org.example.a"));
    }
}

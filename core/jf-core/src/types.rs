//! Commands, events and snapshots: the whole surface a UI talks to.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use jf_engine::{
    CrossfadeSettings, DeviceInfo, EqSettings, OutputKind, PlaybackAnchor, ReplayGainMode, TrackInfo,
};
use jf_library::{ScanProgress, ScanReport};
use jf_lyrics::{LyricDoc, LyricTimeline};
use jf_palette::Theme;
use jf_plugin_host::{HostOptions, SecretStore};
use jf_session::{QueueItem, RepeatMode, TrackRef};
use serde::{Deserialize, Serialize};

/// How the core is set up.
#[derive(Clone)]
pub struct Config {
    /// Holds `library.sqlite` and `session.sqlite`.
    pub data_dir: PathBuf,
    /// Holds artwork and baked backdrops; safe to delete.
    pub cache_dir: PathBuf,
    /// Where audio goes. The engine (and the audio device) is only opened
    /// when something is first played.
    pub output: OutputKind,
    /// Output buffer length.
    pub buffer_ms: u32,
    /// Restore the saved queue, position and settings on start.
    pub restore_session: bool,
    /// Save the queue, position and settings as they change.
    pub persist_session: bool,
    /// Watch the library folders for changes while the user's setting
    /// ([`Settings::watch_folders`]) is on. Short-lived tools turn it off.
    pub watch_folders: bool,
    /// Online sources: plugins in `<data_dir>/plugins`, secrets, limits.
    pub sources: SourcesConfig,
    /// `.jfpkg` plugin packages the app ships inside its bundle. At start
    /// the core installs each one that is not installed and updates one
    /// whose bundled version is newer than the installed one, verified and
    /// trusted like any other package; one the person uninstalled stays
    /// uninstalled (remembered with the settings) until they install it
    /// again themselves. Failures are reported as [`Event::Error`].
    pub bundled_plugins: Vec<PathBuf>,
    /// How this build reached the person (ADR-0021 §4): which stores can sell
    /// Jellyfesh Pro here, and whether the core checks for updates
    /// ([`Distribution::Direct`] only).
    pub distribution: Distribution,
    /// On-device speech recognition for AI Lyrics (ADR-0019 §5):
    /// `jf_speech::recognizer()` in builds with Whisper, `None` elsewhere
    /// (then the on-device mode says it is unavailable).
    pub speech: Option<Arc<dyn jf_speech::Recognizer>>,
    /// Where speech models are downloaded from: `None` for whisper.cpp's
    /// published models (`jf_speech::catalogue`); tests point it at a local
    /// server (`{url}/{file}`).
    pub speech_model_url: Option<String>,
    /// The releases API the update check asks (direct downloads only):
    /// GitHub's list for the project by default; tests point it at a local
    /// server.
    pub update_url: String,
}

/// The releases the update check reads (ADR-0021 §5).
pub const RELEASES_URL: &str = "https://api.github.com/repos/ZodiaCALs/Jellyfesh/releases?per_page=10";

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("data_dir", &self.data_dir)
            .field("cache_dir", &self.cache_dir)
            .field("output", &self.output)
            .field("buffer_ms", &self.buffer_ms)
            .field("restore_session", &self.restore_session)
            .field("persist_session", &self.persist_session)
            .field("watch_folders", &self.watch_folders)
            .field("sources", &self.sources)
            .field("bundled_plugins", &self.bundled_plugins)
            .field("distribution", &self.distribution)
            .field("speech", &self.speech.is_some())
            .field("speech_model_url", &self.speech_model_url)
            .field("update_url", &self.update_url)
            .finish()
    }
}

impl Config {
    /// Defaults: the default audio device, 200 ms buffer, session restored
    /// and saved, folders watched, sources on with the platform keystore,
    /// no bundled plugins.
    pub fn new(data_dir: impl Into<PathBuf>, cache_dir: impl Into<PathBuf>) -> Self {
        Config {
            data_dir: data_dir.into(),
            cache_dir: cache_dir.into(),
            output: OutputKind::Device { name: None },
            buffer_ms: 200,
            restore_session: true,
            persist_session: true,
            watch_folders: true,
            sources: SourcesConfig::default(),
            bundled_plugins: Vec::new(),
            distribution: Distribution::Source,
            speech: None,
            speech_model_url: None,
            update_url: RELEASES_URL.to_owned(),
        }
    }
}

/// How this build of the app reached the person (ADR-0021 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Distribution {
    /// A download from the project's release page: checks for updates itself.
    Direct,
    /// The Mac App Store.
    AppStore,
    /// The Microsoft Store.
    MicrosoftStore,
    /// Google Play.
    GooglePlay,
    /// Jellyfesh Web.
    Web,
    /// Built from source.
    #[default]
    Source,
}

impl Distribution {
    /// The store that sells Jellyfesh Pro in this build, if any.
    pub fn store(self) -> Option<Store> {
        match self {
            Distribution::AppStore => Some(Store::AppStore),
            Distribution::MicrosoftStore => Some(Store::MicrosoftStore),
            Distribution::GooglePlay => Some(Store::GooglePlay),
            Distribution::Direct | Distribution::Web | Distribution::Source => None,
        }
    }
}

/// A store that sells Jellyfesh Pro (product `org.jellyfesh.pro`; `pro` on
/// Google Play).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Store {
    AppStore,
    MicrosoftStore,
    GooglePlay,
}

/// What a store says about Jellyfesh Pro, as the app's store glue verified it
/// with the platform ([`crate::Core::set_store_entitlement`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreEntitlement {
    pub store: Store,
    /// The store's product id (`org.jellyfesh.pro`, or `pro` on Google Play).
    pub product: String,
    /// Whether the person owns it now (false after a refund or revocation).
    pub active: bool,
    /// The store's transaction or order id, if it gives one.
    pub transaction: Option<String>,
}

/// How online sources (plugins) run.
#[derive(Debug, Clone)]
pub struct SourcesConfig {
    /// Run the plugin host. Tools that never touch sources turn it off; the
    /// source commands then fail and the `Sources` slice says why.
    pub enabled: bool,
    /// Where sign-in tokens, API keys and cookie-jar keys go (ADR-0015).
    /// `None` uses the platform keystore: Keychain, Credential Manager or
    /// the Secret Service, through `keyring`. Apps pass their own (Android
    /// Keystore, or a Swift or C# implementation of the FFI's
    /// `SecretStore`). Whichever store is used is checked once at start; if
    /// it cannot be reached, sign-ins are kept in memory for this run only,
    /// and the `Sources` slice ([`SourcesSnapshot::secret_storage`]) and an
    /// [`Event::Error`] say so. Passwords are never stored.
    pub secrets: Option<Arc<dyn SecretStore>>,
    /// The plugin host's trust list, name resolution, sandbox limits and
    /// workers. Production defaults; tests change them.
    pub host: HostOptions,
    /// How long a search waits for each source before reporting it as late.
    pub search_timeout: Duration,
}

impl Default for SourcesConfig {
    fn default() -> Self {
        SourcesConfig {
            enabled: true,
            secrets: None,
            host: HostOptions::default(),
            search_timeout: Duration::from_secs(8),
        }
    }
}

/// Identifies a sent command, so a failure can be matched to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(pub u64);

/// Everything a UI can ask the core to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Replace the queue with `tracks` and play the one at `start`.
    PlayTracks {
        tracks: Vec<TrackRef>,
        start: usize,
    },
    /// Add to the queue: right after the current entry ("Play Next") or at
    /// the end ("Play Last").
    Enqueue {
        tracks: Vec<TrackRef>,
        play_next: bool,
    },
    RemoveFromQueue {
        uid: u64,
    },
    MoveInQueue {
        uid: u64,
        to: usize,
    },
    /// Play the queue entry with this uid.
    JumpTo {
        uid: u64,
    },
    ClearQueue,
    SetShuffle {
        on: bool,
    },
    SetRepeat {
        mode: RepeatMode,
    },
    /// Resume, or start the current entry when stopped.
    Play,
    Pause,
    TogglePlayPause,
    Stop,
    Next,
    /// Restarts the current track after the first three seconds, otherwise
    /// goes to the previous entry.
    Previous,
    Seek {
        position_ms: u64,
    },
    /// Linear volume, 0.0 to 1.0.
    SetVolume {
        volume: f32,
    },
    SetEq {
        eq: EqSettings,
    },
    SetReplayGain {
        mode: ReplayGainMode,
        preamp_db: f32,
    },
    AddRoot {
        path: PathBuf,
    },
    RemoveRoot {
        path: PathBuf,
    },
    /// Scan every library folder for changes.
    Rescan,

    // ---- collections ----------------------------------------------------
    /// Replace the queue with a whole collection (in its natural order:
    /// album by disc and track, artist album by album, playlist as listed)
    /// and play from `start`, shuffled when `shuffle` is set.
    PlayCollection {
        collection: Collection,
        start: usize,
        shuffle: bool,
    },
    /// Add a whole collection to the queue.
    EnqueueCollection {
        collection: Collection,
        play_next: bool,
    },

    // ---- settings and output -------------------------------------------
    /// Replace all settings (the UI sends back a modified copy of the
    /// `Settings` slice). The core applies what it owns at once.
    SetSettings {
        settings: Settings,
    },
    SetMuted {
        muted: bool,
    },
    SetCrossfade {
        crossfade: CrossfadeSettings,
    },
    /// Output device by name from the `Devices` slice; `None` follows the
    /// system default.
    SetOutputDevice {
        name: Option<String>,
    },
    /// List output devices again (shells send this on OS device
    /// notifications; the core never polls).
    RefreshDevices,

    // ---- playlists and favourites --------------------------------------
    /// A new playlist, optionally with tracks.
    CreatePlaylist {
        name: String,
        tracks: Vec<TrackRef>,
    },
    RenamePlaylist {
        id: i64,
        name: String,
    },
    DeletePlaylist {
        id: i64,
    },
    AddToPlaylist {
        id: i64,
        tracks: Vec<TrackRef>,
    },
    RemoveFromPlaylist {
        id: i64,
        entries: Vec<i64>,
    },
    MovePlaylistEntry {
        id: i64,
        entry: i64,
        to: usize,
    },
    /// Import an M3U/M3U8 file as a new playlist.
    ImportPlaylist {
        path: PathBuf,
    },
    SetFavorite {
        track: TrackRef,
        favorite: bool,
    },

    // ---- online sources (plugins) -------------------------------------------
    /// Install (or update) a `.jfpkg` plugin package. Official packages
    /// start on; community ones need community plugins allowed and start
    /// off until their disclosure is accepted (`AcknowledgeUnofficial`).
    /// The `Sources` slice shows the result.
    InstallPlugin {
        path: PathBuf,
    },
    /// Remove a plugin with its settings, data, sign-in and secrets.
    UninstallPlugin {
        id: String,
    },
    /// Turn a plugin on or off. A community plugin turns on only after its
    /// disclosure was accepted, and while community plugins are allowed.
    SetPluginEnabled {
        id: String,
        enabled: bool,
    },
    /// The plugin's settings as JSON matching its settings schema.
    SetPluginSettings {
        id: String,
        settings_json: String,
    },
    /// The person read and accepted the community-plugin disclosure for
    /// this plugin (ADR-0012): remembered for good, allows community
    /// plugins ([`Settings::allow_community_plugins`]) and turns the plugin
    /// on. An id that is not installed yet is remembered for its install.
    AcknowledgeUnofficial {
        id: String,
    },
    /// Start signing in to a source; steps arrive as `Event::Auth`, the
    /// first one carrying the flow number the other login commands take.
    /// A new login to the same source cancels the one in progress.
    BeginLogin {
        plugin: String,
        method: AuthMethod,
    },
    /// Connect to a service from [`SourcesSnapshot::presets`]: sets the
    /// server's address when the service asks for one, turns its plugin on
    /// and starts signing in with `method`, as `BeginLogin` does.
    ConnectSource {
        preset: String,
        address: Option<String>,
        method: AuthMethod,
    },
    /// Answer an `AuthStep::AskFields` step (field name → value). Secrets
    /// pass through once and are never stored (ADR-0015).
    SubmitLogin {
        plugin: String,
        flow: u64,
        fields: Vec<(String, String)>,
    },
    /// Stop a login; a `Failed` step follows.
    CancelLogin {
        plugin: String,
        flow: u64,
    },
    /// Sign out: the source's secrets, cookies and account are deleted
    /// (even when the plugin is off), and the plugin may revoke its tokens.
    Logout {
        plugin: String,
    },
    /// Search the library and every enabled source; results arrive as
    /// `Event::SearchResults` (see [`SearchUpdate`]): the library at once,
    /// then one message per source as it answers, merged and de-duplicated
    /// across sources. A newer search supersedes older ones.
    Search {
        query: String,
        scope: SearchScope,
    },
    /// Browse a source's catalogue: `node` `None` is its root, otherwise
    /// the `id` of a [`SourceItem`] the core listed for this plugin. Pages
    /// count from 0; `BrowseUpdate::next_page` names the next one.
    Browse {
        plugin: String,
        node: Option<String>,
        page: u32,
    },
    /// The remote session (ADR-0018): what the transport commands cannot
    /// say. While a session runs, Play, Pause, TogglePlayPause, Stop, Next,
    /// Previous, Seek, SetShuffle and SetRepeat control the service's player
    /// instead of the engine.
    Remote {
        action: RemoteAction,
    },

    // ---- AI Lyrics (ADR-0019) ---------------------------------------------------------------
    /// Makes synced lyrics for a song with the AI Lyrics plugin. The job
    /// joins the `AiLyrics` slice's `jobs`; one runs at a time. Fails while
    /// the slice's `need` is not `Nothing` (Pro, sign-in, consent, a model).
    MakeLyrics {
        song: SongRef,
    },
    /// Stops a waiting or running job.
    CancelLyrics {
        job: u64,
    },
    /// Deletes the lyrics AI Lyrics made for a song.
    DeleteMadeLyrics {
        song: SongRef,
    },
    /// The person allowed sending songs' audio to `origin` for this plugin
    /// (the `Consent` need's origin).
    AllowAudioUpload {
        plugin: String,
        origin: String,
    },
    /// Stop Allowing: no more audio goes to `origin` for this plugin.
    RevokeAudioUpload {
        plugin: String,
        origin: String,
    },
    /// Downloads an on-device speech model (`AiLyricsSnapshot::models`).
    DownloadSpeechModel {
        id: String,
    },
    /// Stops a model download; what came so far is kept for later.
    CancelSpeechModelDownload {
        id: String,
    },
    /// Deletes a downloaded model.
    DeleteSpeechModel {
        id: String,
    },

    // ---- plugins, Pro and updates (ADR-0020, ADR-0021) ----------------------------------------
    /// Restores a plugin's settings to the schema's defaults (Reset to
    /// Defaults).
    ResetPluginSettings {
        plugin: String,
    },
    /// Activates Jellyfesh Pro with a licence key (`JFP1-…`); a bad key
    /// leaves `ProSnapshot::licence_error` set.
    ActivateLicence {
        key: String,
    },
    /// Removes the licence key.
    RemoveLicence,
    /// Checks for a newer release now (direct downloads only).
    CheckForUpdates,

    // ---- Discover, details and comments (ADR-0022) ----------------------------------------------
    /// A source's Discover page (`library` for the library's); answered by
    /// `Event::Discover`.
    Discover {
        source: String,
    },
    /// More about an item; answered by `Event::Details`.
    Details {
        item: ItemRef,
    },
    /// A page of comments on a source item (`page` `None` is the first,
    /// otherwise the `next` of the page before); answered by
    /// `Event::Comments`.
    Comments {
        item: ItemRef,
        page: Option<String>,
    },
}

/// A song to make lyrics for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SongRef {
    /// The song playing now (a library track, a file or a source track).
    Current,
    /// A library song.
    Library { id: i64 },
}

/// An item to show details or comments for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemRef {
    /// A library track.
    Track { id: i64 },
    /// A library album.
    Album { id: i64 },
    /// A library artist.
    Artist { id: i64 },
    /// An audio file that is not in the library.
    File { path: PathBuf },
    /// An item a source listed: its `SourceItem::source`, `kind` and `id`.
    Source {
        plugin: String,
        item_kind: ItemKind,
        id: String,
    },
}

/// What [`Command::Remote`] asks of the remote session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemoteAction {
    /// Asks the service for its devices and state now (when the device
    /// picker opens).
    Refresh,
    /// Moves playback to the device with this id, playing there.
    Transfer { device: String },
    /// The playing device's volume, 0–100.
    SetVolume { percent: u32 },
    /// Stops controlling the service. Its player carries on as it was.
    End,
}

/// A group of tracks that plays as a unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Collection {
    Album {
        id: i64,
    },
    Artist {
        id: i64,
    },
    Playlist {
        id: i64,
    },
    Favorites,
    /// Every song in the library, in the Songs page's order (`sort`, and
    /// `descending` as `TrackQuery` takes them).
    Songs {
        sort: jf_library::TrackSort,
        descending: bool,
    },
    Tracks {
        tracks: Vec<TrackRef>,
    },
    /// An album, playlist or other container from an online source (the
    /// `id` of a [`SourceItem`] the core listed), fetched through its plugin:
    /// every page of it, playable tracks only (an artist plays its albums).
    /// The queue changes once the plugin has answered.
    Source {
        plugin: String,
        id: String,
    },
}

/// Where a search looks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SearchScope {
    /// The library and every enabled source.
    All,
    Library,
    Plugin {
        id: String,
    },
}

/// How a source signs in (ADR-0004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMethod {
    Qr,
    Oauth,
    CookieImport,
    PasswordExchange,
    ApiKey,
}

/// Light, dark, or following the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearancePref {
    #[default]
    System,
    Light,
    Dark,
}

/// The Now Playing backdrop: the baked artwork backdrop, or Misty's own
/// field (Step 0: both are built so you can compare).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackdropStyle {
    #[default]
    Artwork,
    Misty,
}

/// Where the theme hue comes from.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HueSource {
    /// The current track's artwork (fallback hue when there is none).
    #[default]
    Artwork,
    /// One fixed hue in degrees.
    Fixed { hue: f32 },
}

/// The size of lyrics on Now Playing: line, translation and romanization
/// sizes multiply by `jf_tokens::lyrics::SCALE_SMALL`, 1 or `SCALE_LARGE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// How lyrics lines line up on Now Playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsAlign {
    /// The start edge (left in left-to-right text).
    #[default]
    Start,
    Center,
}

/// Everything the person sets in Preferences. The core stores it, applies
/// the playback, library and source parts itself, and shells apply the
/// appearance parts (ADR-0001: shells own platform UI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub appearance: AppearancePref,
    pub style: BackdropStyle,
    /// `automatic` (Ice in light, Steel in dark) or one of the ten icon
    /// names (ADR-0017).
    pub app_icon: String,
    pub hue_source: HueSource,
    /// Fewer translucent layers and shorter motion, beyond the OS setting.
    pub reduce_effects: bool,
    /// How see-through glass is, from 0 (solid) to 1 (clearest); 0.5 keeps
    /// the tokens' own alphas. Shells apply it with
    /// [`jf_palette::glass_alpha`] (or [`Theme::with_transparency`]). The
    /// core keeps it within 0..=1.
    pub transparency: f32,
    pub lyrics_size: LyricsSize,
    pub lyrics_align: LyricsAlign,
    pub show_translation: bool,
    pub show_romanization: bool,
    pub crossfade: CrossfadeSettings,
    pub replaygain_mode: ReplayGainMode,
    pub preamp_db: f32,
    pub eq: EqSettings,
    /// `None` follows the system default output.
    pub output_device: Option<String>,
    /// Start playing the restored queue on launch (otherwise it waits,
    /// paused).
    pub resume_on_launch: bool,
    /// Rescan folders when their files change.
    pub watch_folders: bool,
    /// Community (unofficial) plugins may be installed and enabled
    /// (ADR-0012). Off by default.
    pub allow_community_plugins: bool,
    /// Check for Updates Automatically (direct downloads only; ADR-0021 §5):
    /// at most once a day. On by default.
    pub check_for_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            appearance: AppearancePref::System,
            style: BackdropStyle::Artwork,
            app_icon: "automatic".into(),
            hue_source: HueSource::Artwork,
            reduce_effects: false,
            transparency: 0.5,
            lyrics_size: LyricsSize::Medium,
            lyrics_align: LyricsAlign::Start,
            show_translation: true,
            show_romanization: true,
            crossfade: CrossfadeSettings::default(),
            replaygain_mode: ReplayGainMode::Off,
            preamp_db: 0.0,
            eq: EqSettings::flat(),
            output_device: None,
            resume_on_launch: false,
            watch_folders: true,
            allow_community_plugins: false,
            check_for_updates: true,
        }
    }
}

/// An equalizer preset ([`crate::eq_presets`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqPreset {
    /// Stable name: `flat`, `bass_boost`, `vocal`, `treble`. Apps show it translated ("Flat",
    /// "Bass Boost", "Vocal", "Treble").
    pub id: String,
    /// Its bands, switched on, with a 0 dB preamp.
    pub settings: EqSettings,
}

/// The `Devices` slice.
#[derive(Debug, Clone, PartialEq)]
pub struct DevicesSnapshot {
    pub version: u64,
    pub devices: Vec<DeviceInfo>,
    /// The chosen device; `None` follows the system default.
    pub selected: Option<String>,
}

/// The badge a source shows (ADR-0004, ADR-0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginBadge {
    /// Signed by a key in the app's trust list.
    Official,
    /// Anything else: community, unofficial, opt-in.
    Community,
}

/// One installed source.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    /// One line from the manifest, if it has one.
    pub description: Option<String>,
    pub badge: PluginBadge,
    /// Community packages: the fingerprint of the key that signed it, to
    /// show with the disclosure (`None` when unsigned, or official).
    pub signing_key: Option<String>,
    /// Vendor playback: the service plays it, so no EQ, crossfade or
    /// visualization.
    pub vendor_playback: bool,
    /// Remote playback (ADR-0018): the service plays on the person's own
    /// devices and Jellyfesh is its remote control. Playing its tracks
    /// starts a remote session (the `Remote` slice); they never join the
    /// queue.
    pub remote_playback: bool,
    /// Manifest names: `auth`, `search`, `browse`, `resolve_stream`,
    /// `lyrics`, `artwork`, `recommendations`, `remote`.
    pub capabilities: Vec<String>,
    pub auth_methods: Vec<AuthMethod>,
    /// Whether the plugin can be used now.
    pub enabled: bool,
    /// Why it cannot be used, when the host turned it off (after it broke
    /// its limits three times in a row) or community plugins are turned
    /// off.
    pub disabled_reason: Option<String>,
    /// The signed-in account's display name.
    pub account: Option<String>,
    /// The community disclosure has not been accepted yet.
    pub needs_disclosure: bool,
    /// Attribution its views must show (`[content_policy]`), if any.
    pub attribution: Option<String>,
    pub settings_schema_json: String,
    pub settings_json: String,
    /// The package's icon as a PNG file in the cache folder (named by its
    /// content, so an update gets a new path).
    pub icon: Option<PathBuf>,
    /// Source (Settings › Sources) or Extension (Settings › Extensions:
    /// plugins that make lyrics or change the look and offer no music).
    pub kind: PluginKind,
    /// It has a Jellyfesh Pro capability (`make_lyrics`, `appearance`); its
    /// calls answer "needs Jellyfesh Pro" while Pro is inactive.
    pub requires_pro: bool,
    /// The host imports the manifest declares (`audio`, `speech`), for the
    /// install screen.
    pub permissions: Vec<String>,
    /// It has a Discover page of its own (`discover`).
    pub discover: bool,
    /// It describes items (`details`): biography, notes, facts.
    pub details: bool,
    /// It shows comments (`comments`): offer a Comments pane.
    pub comments: bool,
}

/// Where Settings lists a plugin (ADR-0020 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// A music source: Settings › Sources.
    Source,
    /// AI Lyrics, Customize and the like: Settings › Extensions.
    Extension,
}

/// A music service the apps offer in Sources → Add a Source
/// (`jf_plugin_model::presets`), listed while its official plugin is
/// installed. [`Command::ConnectSource`] connects to it.
#[derive(Debug, Clone, PartialEq)]
pub struct SourcePreset {
    /// Stable name: `jellyfin`, `navidrome`, `subsonic`.
    pub id: String,
    /// The service's own name.
    pub name: String,
    /// The plugin that connects to it.
    pub plugin: String,
    /// The plugin's sign-in methods, in its order.
    pub auth_methods: Vec<AuthMethod>,
    /// Whether the person types the server's address (a self-hosted server).
    pub needs_address: bool,
    /// The address set now, to fill the field with.
    pub address: Option<String>,
    /// An address to show as an example in that field.
    pub address_example: Option<String>,
    /// The plugin's signed-in account, if any.
    pub account: Option<String>,
}

/// Where sign-ins are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretStorage {
    /// The system keystore, or the store the app passed in.
    #[default]
    Keystore,
    /// No keystore could be reached: sign-ins last until the app quits.
    SessionOnly,
}

/// The `Sources` slice.
#[derive(Debug, Clone, PartialEq)]
pub struct SourcesSnapshot {
    pub version: u64,
    pub plugins: Vec<PluginSummary>,
    /// The services offered in Sources → Add a Source, in their order.
    pub presets: Vec<SourcePreset>,
    pub allow_community: bool,
    /// Whether sign-ins outlive this run (ADR-0015).
    pub secret_storage: SecretStorage,
    /// Set when online sources cannot run at all (turned off in this
    /// build's configuration, or the plugin host failed to start).
    pub unavailable: Option<String>,
    /// Plugins may be installed from files and community plugins allowed:
    /// false in store builds (App Store, Microsoft Store, Google Play;
    /// ADR-0021 §4), where the apps hide Install from File… and Allow
    /// Community Plugins and the core refuses both.
    pub can_install_plugins: bool,
}

/// How a sign-in field is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthFieldKind {
    #[default]
    Text,
    Email,
    Url,
    /// Digits only, such as a one-time code.
    Number,
    /// A `Cookie:` header copied from a browser (cookie-import logins).
    CookieHeader,
}

/// A field to ask for during sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthField {
    pub name: String,
    pub label: String,
    pub kind: AuthFieldKind,
    /// Mask the input (passwords, keys).
    pub secret: bool,
    /// The person must fill it in.
    pub required: bool,
    pub placeholder: Option<String>,
    /// A sentence of help under the input.
    pub help: Option<String>,
}

/// One step of a sign-in flow.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthStep {
    /// Show `data` (usually a URL) as a QR code for the service's app to
    /// scan; the core polls the plugin meanwhile. The UI draws the code.
    ShowQr {
        data: String,
        /// One sentence for the person ("Scan with the Example app").
        hint: Option<String>,
        expires_at_ms: Option<i64>,
    },
    /// Show `code` large, to be entered on another device (`hint` says
    /// where); the core polls the plugin meanwhile.
    ShowCode {
        code: String,
        hint: String,
        expires_at_ms: Option<i64>,
    },
    /// Open `url` in the browser; the core captures the redirect.
    OpenUrl {
        url: String,
    },
    AskFields {
        fields: Vec<AuthField>,
    },
    Waiting {
        message: String,
    },
    Done {
        account: String,
    },
    Failed {
        message: String,
    },
}

/// A sign-in flow moved to a new step.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthUpdate {
    pub plugin: String,
    pub flow: u64,
    pub step: AuthStep,
}

/// What a list row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Track,
    Album,
    Artist,
    Playlist,
    Folder,
}

/// Artwork a UI can load: a local file or a URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ArtworkRef {
    File(PathBuf),
    Url(String),
}

/// A search or browse result from the library or a source.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceItem {
    /// `library` or the plugin's id.
    pub source: String,
    pub kind: ItemKind,
    /// The library id (as text) or the plugin's id for the item.
    pub id: String,
    pub title: String,
    /// Artist, or album artist for albums.
    pub subtitle: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub artwork: Option<ArtworkRef>,
    /// False for items the service will not play here (region, paywall);
    /// they are never bypassed.
    pub playable: bool,
    pub unplayable_reason: Option<String>,
    /// For playable tracks: what goes into the queue (or, for a remote
    /// source, what starts its remote session).
    pub track: Option<TrackRef>,
    /// Other sources that have the same item (unified search).
    pub also_in: Vec<String>,
    /// An `https` link that opens the item in its service ("Open in
    /// {Service}"), when the source's plugin declares one for its kind
    /// (ADR-0018). The main action for items that cannot play here.
    pub link: Option<String>,
    /// Go to Artist for a source track or album: the source's id of its
    /// artist (`Browse { plugin: source, node: artist_id }` lists the
    /// artist's albums; `Details` with `ItemRef::Source { item_kind: Artist }`
    /// describes the artist). `None` for library items and when the source
    /// does not say.
    pub artist_id: Option<String>,
    /// Go to Album for a source track: the source's id of its album, for
    /// `Browse` and `Details` the same way.
    pub album_id: Option<String>,
}

/// Results from one source for one search: its section of the results.
///
/// The library answers at once, then each source as it answers (or fails,
/// or misses its deadline, with `error` set). Answers are merged and
/// de-duplicated across sources (same ISRC, or the same normalised title,
/// primary artist and duration within 2 s): an item several sources have
/// appears once, in the section of its first playable copy (the library's,
/// when the library has it), and lists the other sources in `also_in`. So
/// a source's `items` are what it adds to the sources before it, in its
/// own order.
///
/// When a source's answer changes an earlier section (a library track it
/// also has gains it in `also_in`; an item locked in an earlier source
/// moves to this one, which can play it), the earlier source's section is
/// sent again, right before this one. Show the latest update per `source`,
/// library first, the others in the order they first arrived. Sources whose
/// terms forbid unified search are only searched with
/// [`SearchScope::Plugin`].
#[derive(Debug, Clone, PartialEq)]
pub struct SearchUpdate {
    /// The `Search` command these results answer.
    pub request: CommandId,
    pub query: String,
    /// `library` or the plugin's id.
    pub source: String,
    /// The source's section: its results less those an earlier source
    /// shows.
    pub items: Vec<SourceItem>,
    /// Why the source has no results (failed, or did not answer in time).
    pub error: Option<String>,
    /// True on the last message for this request.
    pub done: bool,
}

/// One page of a source's catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseUpdate {
    pub request: CommandId,
    pub plugin: String,
    pub node: Option<String>,
    pub items: Vec<SourceItem>,
    pub next_page: Option<u32>,
    pub error: Option<String>,
}

/// A row in a track list (playlist, favourites, history): the track and
/// what to show for it.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackRow {
    /// What to queue to play the row: a library track, a file (one that is
    /// not in the catalogue) or a source track.
    pub track: TrackRef,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// A source track's image URL. Library rows have `artwork_hash` instead.
    pub artwork: Option<ArtworkRef>,
    /// Library rows: the artwork's hash, for [`crate::Core::artwork_thumbnail`].
    pub artwork_hash: Option<String>,
    pub favorite: bool,
    /// The file is gone; the row cannot play.
    pub missing: bool,
}

/// A playlist in the library.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub entry_count: u32,
    pub duration_ms: u64,
    pub updated_at_ms: i64,
    pub artwork_hash: Option<String>,
}

/// One entry of a playlist.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistEntry {
    pub entry_id: i64,
    pub position: u32,
    pub row: TrackRow,
}

/// Transport state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Nothing playing (never started, or stopped).
    Stopped,
    Playing,
    Paused,
    /// The queue played to its end.
    Ended,
}

/// The `Playback` slice.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackSnapshot {
    pub version: u64,
    pub status: Status,
    /// Time base for progress bars and lyrics: UIs extrapolate from it each
    /// frame instead of asking the core.
    pub anchor: PlaybackAnchor,
    pub volume: f32,
    pub eq: EqSettings,
    pub replaygain_mode: ReplayGainMode,
    pub preamp_db: f32,
}

/// Number of upcoming entries carried in a [`QueueSnapshot`]. Longer views
/// page through [`crate::Core::queue_window`].
pub const UP_NEXT_WINDOW: usize = 50;

/// The `Queue` slice.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueSnapshot {
    pub version: u64,
    pub len: usize,
    pub position: Option<usize>,
    pub current: Option<QueueItem>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// The next entries in playback order, at most [`UP_NEXT_WINDOW`].
    pub up_next: Vec<QueueItem>,
}

/// What is playing, with display metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct NowPlaying {
    pub uid: u64,
    pub track: TrackRef,
    /// The audio file, for local tracks.
    pub path: Option<PathBuf>,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// Artwork: a cached file for local tracks, a URL for online ones.
    pub artwork: Option<ArtworkRef>,
    /// Stream format, once the engine has opened the file.
    pub format: Option<TrackInfo>,
    /// Opens the track in its service, for source tracks whose plugin
    /// declares links.
    pub link: Option<String>,
    /// What is playing, as decoded (ADR-0022 §3): codec, sample rate, bit
    /// depth, channels, bitrate and the quality label, once the engine has
    /// opened it; `None` before.
    pub quality: Option<AudioFormat>,
}

/// What a remote device is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteDeviceKind {
    Computer,
    Smartphone,
    Speaker,
    Tv,
    Car,
    Other,
}

/// A device a remote source can play on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    pub kind: RemoteDeviceKind,
    /// The device playing (or last playing) for this account.
    pub active: bool,
    /// Its volume, 0–100, when it lets Jellyfesh change it.
    pub volume_percent: Option<u32>,
}

/// The `Remote` slice: the remote session, while Jellyfesh controls a
/// service's own player (ADR-0018). Apps show it as Now Playing instead of
/// the engine's track while `source` is set.
///
/// The core asks the service for its state every second while it plays and
/// every five seconds while it is paused (less often after errors), and
/// ends the session after ten minutes without playing, after
/// [`RemoteAction::End`], or when local playback starts.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteSnapshot {
    pub version: u64,
    /// The source in control; `None` when there is no remote session.
    pub source: Option<String>,
    /// That source's name, for "Playing on {device} · {source}".
    pub source_name: Option<String>,
    pub status: Status,
    /// The track playing, as the service reports it.
    pub item: Option<SourceItem>,
    /// The album or playlist it plays from, when the service says.
    pub context: Option<SourceItem>,
    /// Time base for the progress bar, like the playback slice's.
    pub anchor: PlaybackAnchor,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// The device playing.
    pub device: Option<RemoteDevice>,
    /// Every device the service lists, for the device picker.
    pub devices: Vec<RemoteDevice>,
    /// What went wrong last talking to the service, until a call succeeds.
    pub error: Option<String>,
}

/// An item an [`ExternalPlayer`] plays or reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalItem {
    pub kind: ItemKind,
    /// The player's own id for it.
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub artwork_url: Option<String>,
}

/// What an [`ExternalPlayer`] is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalState {
    pub item: Option<ExternalItem>,
    /// The album or playlist it plays from, when it knows.
    pub context: Option<ExternalItem>,
    pub playing: bool,
    pub position_ms: u64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub device: Option<RemoteDevice>,
}

/// A player an app provides (ADR-0018 item 5), such as MusicKit's on macOS,
/// registered with [`crate::Core::register_external_player`] under a
/// reserved source id. The core drives it exactly like a remote source's
/// `remote` export: playing tracks whose `SourceTrack::plugin` is that id
/// starts a remote session, and the `Remote` slice follows it.
///
/// The core calls it from the remote session's own thread, never from the
/// actor or the app's main thread; each call may block until the player
/// answers (at most a few seconds). Errors are shown as the slice's `error`.
pub trait ExternalPlayer: Send + Sync {
    fn devices(&self) -> Result<Vec<RemoteDevice>, String>;
    /// `None` when nothing plays.
    fn state(&self) -> Result<Option<ExternalState>, String>;
    /// Plays `tracks` from `start`, or the album or playlist `context` from
    /// its track `start` (then `tracks` is empty).
    fn play(
        &self,
        context: Option<ExternalItem>,
        tracks: Vec<ExternalItem>,
        start: u32,
    ) -> Result<(), String>;
    fn resume(&self) -> Result<(), String>;
    fn pause(&self) -> Result<(), String>;
    fn next(&self) -> Result<(), String>;
    fn previous(&self) -> Result<(), String>;
    fn seek(&self, position_ms: u64) -> Result<(), String>;
    /// 0–100; players without a volume answer an error.
    fn set_volume(&self, percent: u32) -> Result<(), String>;
    fn set_shuffle(&self, on: bool) -> Result<(), String>;
    fn set_repeat(&self, mode: RepeatMode) -> Result<(), String>;
    fn transfer(&self, device: String) -> Result<(), String>;
}

/// Where lyrics came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsSource {
    /// A `.ttml`, `.lrc` or `.vtt` file next to the audio file.
    Sidecar(PathBuf),
    /// Lyrics stored in the audio file's tags.
    Embedded,
    /// The plugin with this id (its attribution, if any, is in
    /// `LyricDoc::source.origin`).
    Plugin(String),
    /// Made by AI Lyrics (ADR-0019): the plugin and the engine, shown as
    /// "Made by AI Lyrics · {engine}".
    Made { plugin: String, engine: String },
}

/// The `Lyrics` slice for the current track.
#[derive(Debug, Clone, PartialEq)]
pub struct LyricsSnapshot {
    pub uid: u64,
    pub doc: Option<LyricDoc>,
    /// Flat arrays for the per-frame cursor lookup.
    pub timeline: Option<LyricTimeline>,
    pub source: Option<LyricsSource>,
}

/// The `Theme` slice: Misty colours from the artwork's hue, and baked Now
/// Playing backdrops.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSnapshot {
    pub uid: Option<u64>,
    pub hue: f32,
    /// False when the hue is the fallback (no artwork, or artwork too grey).
    pub from_artwork: bool,
    pub light: Theme,
    pub dark: Theme,
    pub backdrop_light: Option<PathBuf>,
    pub backdrop_dark: Option<PathBuf>,
}

/// Library scan progress.
#[derive(Debug, Clone)]
pub enum ScanStatus {
    Progress(ScanProgress),
    Finished(ScanReport),
    Failed(String),
}

/// Everything a UI mirrors. Each slice carries its own version.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub playback: PlaybackSnapshot,
    pub queue: QueueSnapshot,
    pub now_playing: Option<Arc<NowPlaying>>,
    pub lyrics: Option<Arc<LyricsSnapshot>>,
    pub theme: Option<Arc<ThemeSnapshot>>,
    pub library_revision: u64,
    pub scanning: bool,
    pub settings: Arc<Settings>,
    pub devices: Arc<DevicesSnapshot>,
    pub sources: Arc<SourcesSnapshot>,
    pub remote: Arc<RemoteSnapshot>,
    pub muted: bool,
    /// AI Lyrics (ADR-0019).
    pub ai_lyrics: Arc<AiLyricsSnapshot>,
    /// The look, every value resolved (ADR-0020).
    pub look: Arc<LookSnapshot>,
    /// Jellyfesh Pro (ADR-0021).
    pub pro: Arc<ProSnapshot>,
    /// Update checks (ADR-0021 §5).
    pub update: Arc<UpdateSnapshot>,
}

/// What the core tells subscribers. Slices arrive whole whenever their
/// version changes.
#[derive(Debug, Clone)]
pub enum Event {
    Playback(PlaybackSnapshot),
    Queue(QueueSnapshot),
    NowPlaying(Option<Arc<NowPlaying>>),
    Lyrics(Arc<LyricsSnapshot>),
    Theme(Arc<ThemeSnapshot>),
    Scan(ScanStatus),
    /// The catalogue, a playlist, a favourite or a play count changed: lists
    /// query again. `revision` only grows.
    LibraryChanged {
        revision: u64,
    },
    /// `CreatePlaylist` or `ImportPlaylist` (command `request`) made playlist `id`.
    PlaylistCreated {
        request: CommandId,
        id: i64,
    },
    Settings(Arc<Settings>),
    Devices(Arc<DevicesSnapshot>),
    Sources(Arc<SourcesSnapshot>),
    /// The remote session changed (see [`RemoteSnapshot`]).
    Remote(Arc<RemoteSnapshot>),
    Auth(AuthUpdate),
    SearchResults(Arc<SearchUpdate>),
    BrowsePage(Arc<BrowseUpdate>),
    /// A source is buffering the current track (rate-0 anchors meanwhile).
    Buffering {
        uid: u64,
        buffering: bool,
    },
    CommandFailed {
        id: CommandId,
        error: String,
    },
    /// A problem not tied to a command (an unreadable file during playback).
    Error {
        message: String,
    },
    /// The `AiLyrics` slice changed.
    AiLyrics(Arc<AiLyricsSnapshot>),
    /// The `Look` slice changed.
    Look(Arc<LookSnapshot>),
    /// The `Pro` slice changed.
    Pro(Arc<ProSnapshot>),
    /// The `Update` slice changed.
    Update(Arc<UpdateSnapshot>),
    /// A Discover page (`Command::Discover`).
    Discover(Arc<DiscoverPage>),
    /// An item's details (`Command::Details`).
    Details(Arc<DetailsPage>),
    /// A page of comments (`Command::Comments`).
    Comments(Arc<CommentsPage>),
    /// A source picture asked for with [`crate::Core::source_artwork`] is in
    /// the cache (`path`), or cannot be had (`None`: stop waiting).
    Artwork(Arc<ArtworkReady>),
}

// ---- AI Lyrics (ADR-0019) ------------------------------------------------------------------------

/// Where AI Lyrics recognises the words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiLyricsMode {
    /// Whisper in the app; nothing leaves the device.
    OnDevice,
    /// The person's own provider, with their API key, after consent.
    Online,
}

/// What making lyrics would do now (the plugin's plan).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiLyricsPlan {
    pub mode: AiLyricsMode,
    /// The engine, in the plugin's words: "Whisper Base on this device".
    pub engine: String,
    /// The origin the online mode sends audio to (`https://api.openai.com`).
    pub upload_to: Option<String>,
    /// Lyrics are made for each song that starts playing without synced
    /// lyrics.
    pub automatic: bool,
}

/// What must happen before lyrics can be made. Apps show the matching
/// prompt ("Download the Base model (142 MB) to make lyrics on this device.").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiLyricsNeed {
    /// Ready: `MakeLyrics` works.
    Nothing,
    /// Jellyfesh Pro is inactive: show the Pro sheet.
    Pro,
    /// The online mode needs the person's API key: open AI Lyrics settings
    /// (its Sign In).
    SignIn,
    /// Ask "Send This Song to {origin}?" and answer with
    /// `AllowAudioUpload { plugin, origin }`.
    Consent { origin: String },
    /// Download this model first (`DownloadSpeechModel { id }`).
    Model { id: String, name: String, bytes: u64 },
    /// Making lyrics cannot work as set up; the text says why.
    Unavailable { reason: String },
}

/// Where a lyrics job is.
#[derive(Debug, Clone, PartialEq)]
pub enum LyricsJobStage {
    /// Behind another job.
    Waiting,
    /// "Reading Audio…"
    ReadingAudio,
    /// "Sending to {origin}…"
    Sending {
        origin: String,
    },
    /// "Recognising… 42%" (`fraction` 0–1).
    Recognising {
        fraction: f32,
    },
    /// "Timing Words…"
    TimingWords,
    /// Finished: the song's lyrics show as made by AI Lyrics.
    Done,
    /// "Couldn't make lyrics: {message}" with Try Again.
    Failed {
        message: String,
    },
    Cancelled,
}

/// A lyrics job. Finished jobs stay listed (the newest 20) so the apps can
/// show how they ended.
#[derive(Debug, Clone, PartialEq)]
pub struct LyricsJob {
    pub id: u64,
    pub title: String,
    pub artist: Option<String>,
    /// The queue entry it is for, when it was made for the song playing.
    pub uid: Option<u64>,
    /// The library track it is for, when the song is in the library.
    pub library_id: Option<i64>,
    pub stage: LyricsJobStage,
}

/// Where an on-device speech model is.
#[derive(Debug, Clone, PartialEq)]
pub enum SpeechModelStatus {
    NotDownloaded,
    /// "Downloading… 35%" (`fraction` 0–1).
    Downloading {
        fraction: f32,
    },
    Ready,
    Failed {
        message: String,
    },
}

/// An on-device speech model ("Base · 142 MB").
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechModelState {
    /// `tiny`, `base`, `small`, `large-v3-turbo-q5_0`.
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub state: SpeechModelStatus,
}

/// An origin the person allowed a plugin to send audio to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioConsent {
    pub plugin: String,
    pub origin: String,
}

/// The `AiLyrics` slice.
#[derive(Debug, Clone, PartialEq)]
pub struct AiLyricsSnapshot {
    pub version: u64,
    /// Offer Create Synced Lyrics: an AI Lyrics plugin is installed and on
    /// (Pro may still be needed: see `need`).
    pub available: bool,
    /// The AI Lyrics plugin (the first enabled `make_lyrics` plugin).
    pub plugin: Option<String>,
    /// What it would do now; `None` until the plugin answered (or while Pro
    /// is inactive).
    pub plan: Option<AiLyricsPlan>,
    /// What is needed first.
    pub need: AiLyricsNeed,
    /// Waiting, running and the latest finished jobs, oldest first.
    pub jobs: Vec<LyricsJob>,
    /// The on-device models, in the catalogue's order.
    pub models: Vec<SpeechModelState>,
    /// Where audio may go (Settings › AI Lyrics › Sending Audio).
    pub consents: Vec<AudioConsent>,
}

// ---- the look (ADR-0020) ------------------------------------------------------------------------

/// The face text uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Typeface {
    System,
    Rounded,
    Serif,
    Mono,
}

/// Spacing: × 0.8, 1.0 or 1.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Compact,
    Standard,
    Roomy,
}

/// The weight of lyric lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsWeight {
    Regular,
    Medium,
    Semibold,
    Bold,
    Heavy,
}

/// The colour of the sung part of lyrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LyricsHighlight {
    /// The artwork's colour.
    Artwork,
    /// The accent colour from Appearance.
    Accent,
    /// This colour, `0xRRGGBB`.
    Custom { rgb: u32 },
}

/// A place in the sidebar (navigation pane, Library switcher).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    Home,
    Albums,
    Artists,
    Songs,
    Playlists,
    Favourites,
    Search,
    Sources,
}

/// A section of Home.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HomeSection {
    Summary,
    RecentlyAdded,
    RecentlyPlayed,
    MostPlayed,
    Favourites,
}

/// How Now Playing is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NowPlayingLayout {
    /// The v0.0.6 stage.
    Stage,
    /// A large cover with the current line below.
    Artwork,
    /// Lyrics full width with a small cover in the corner.
    Lyrics,
}

/// The mini-player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiniPlayerStyle {
    Bar,
    Compact,
    Floating,
}

/// The `Look` slice: every value resolved, defaults filled in, so the apps
/// apply one thing (ADR-0020 §2). With `customized` false every value is the
/// Luminous default. Reduce Transparency and the Transparency setting still
/// win over `glass`; Reduce Motion still wins over motion.
#[derive(Debug, Clone, PartialEq)]
pub struct LookSnapshot {
    pub version: u64,
    /// Radii × this (0–2).
    pub corner_scale: f32,
    /// Glass alpha and blur × this (0–1).
    pub glass: f32,
    /// The aurora field's strength (0–1).
    pub aurora: f32,
    pub typeface: Typeface,
    /// Every type size × this (0.85–1.30).
    pub text_scale: f32,
    pub density: Density,
    pub lyrics_weight: LyricsWeight,
    /// The glow around the sung line (0–1).
    pub lyrics_glow: f32,
    pub lyrics_highlight: LyricsHighlight,
    /// The places shown, in order (Settings is never listed, always shown).
    pub sidebar: Vec<Place>,
    /// Home's sections, in order.
    pub home: Vec<HomeSection>,
    pub now_playing: NowPlayingLayout,
    pub mini_player: MiniPlayerStyle,
    /// An appearance plugin sets the look (Pro active).
    pub customized: bool,
    /// That plugin.
    pub plugin: Option<String>,
}

impl LookSnapshot {
    /// The Luminous defaults.
    pub fn defaults(version: u64) -> LookSnapshot {
        LookSnapshot {
            version,
            corner_scale: 1.0,
            glass: 1.0,
            aurora: jf_tokens::field::AURORA_STRENGTH,
            typeface: Typeface::System,
            text_scale: 1.0,
            density: Density::Standard,
            lyrics_weight: LyricsWeight::Bold,
            lyrics_glow: 0.6,
            lyrics_highlight: LyricsHighlight::Artwork,
            sidebar: vec![
                Place::Home,
                Place::Albums,
                Place::Artists,
                Place::Songs,
                Place::Playlists,
                Place::Favourites,
                Place::Search,
                Place::Sources,
            ],
            home: vec![
                HomeSection::Summary,
                HomeSection::RecentlyAdded,
                HomeSection::RecentlyPlayed,
                HomeSection::MostPlayed,
            ],
            now_playing: NowPlayingLayout::Stage,
            mini_player: MiniPlayerStyle::Bar,
            customized: false,
            plugin: None,
        }
    }
}

// ---- Jellyfesh Pro (ADR-0021) ----------------------------------------------------------------------

/// Where Pro comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProSource {
    /// Pro is inactive.
    None,
    /// A Pro Preview build: "Pro features are free during the public beta."
    Preview,
    /// Bought in a store: "Jellyfesh Pro · Purchased".
    Store { store: Store },
    /// A licence key: "Jellyfesh Pro · Licence …ABCD".
    Licence {
        /// The last four characters of the licence id ("ABCD").
        id_suffix: String,
        /// When the key was issued, midnight UTC, in ms since the Unix epoch.
        issued_at_ms: i64,
    },
}

/// Why a licence key was not accepted. The apps word them: "That doesn't
/// look like a Jellyfesh licence key." (malformed), "This licence key isn't
/// valid." (unknown key, bad signature), "This key is for another product."
/// and "This licence key has been withdrawn."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenceError {
    Malformed,
    UnknownKey,
    BadSignature,
    WrongProduct,
    Revoked,
}

/// The `Pro` slice.
#[derive(Debug, Clone, PartialEq)]
pub struct ProSnapshot {
    pub version: u64,
    /// Pro features work.
    pub active: bool,
    pub source: ProSource,
    /// This build treats Pro as active for the public beta.
    pub preview: bool,
    /// Why the last licence key was refused, until a key is accepted or the
    /// licence removed.
    pub licence_error: Option<LicenceError>,
    /// The stores this build can sell Pro in (show Buy and Restore
    /// Purchases only for these).
    pub stores: Vec<Store>,
    /// Licence keys work in this build: show Enter Licence Key…. False in
    /// App Store and Google Play builds, which sell Pro only through their
    /// store (`ActivateLicence` fails there).
    pub licence_keys: bool,
}

// ---- updates (ADR-0021 §5) -------------------------------------------------------------------------

/// A newer release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    /// "0.0.8" (pre-release suffixes included: "0.0.8-beta.1").
    pub version: String,
    /// Its release page (View Release).
    pub url: String,
}

/// The `Update` slice. Only direct downloads check (`supported`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateSnapshot {
    pub version: u64,
    /// Show Check for Updates and its setting (direct downloads).
    pub supported: bool,
    /// A check is running.
    pub checking: bool,
    /// "Jellyfesh {version} Is Available", when a newer release exists.
    pub available: Option<UpdateInfo>,
    /// When the last check finished, in ms since the Unix epoch.
    pub last_checked_ms: Option<i64>,
    /// Checks run by themselves (supported and the setting is on).
    pub auto: bool,
    /// Why the last check failed, until one succeeds.
    pub error: Option<String>,
}

// ---- Discover (ADR-0022) ------------------------------------------------------------------------

/// How a shelf is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShelfStyle {
    /// One scrolling row of covers.
    Row,
    /// A grid of covers.
    Grid,
    /// A numbered list.
    Chart,
}

/// One shelf of a Discover page.
#[derive(Debug, Clone, PartialEq)]
pub struct Shelf {
    pub id: String,
    /// The heading. Shelves the core makes have stable ids instead of
    /// words the apps must translate: `for_you`, `recently_added`,
    /// `most_played`, `rediscover`; their titles are English ("For You").
    pub title: String,
    pub style: ShelfStyle,
    pub items: Vec<SourceItem>,
    /// A node to browse for See All (`Browse { plugin, node }`).
    pub more: Option<String>,
}

/// A Discover page.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoverPage {
    /// The `Discover` command it answers.
    pub request: CommandId,
    /// `library` or the plugin.
    pub source: String,
    pub shelves: Vec<Shelf>,
    /// Why there is no page.
    pub error: Option<String>,
}

// ---- details and comments (ADR-0022) ------------------------------------------------------------

/// What a fact is; the apps word the keys ("Released", "Composer").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FactKey {
    /// A version or subtitle ("Live", "2011 Remaster").
    Version,
    Genre,
    Language,
    Released,
    Label,
    Composer,
    Lyricist,
    Arranger,
    Producer,
    Popularity,
    Quality,
    /// International Standard Recording Code.
    Isrc,
    /// "Disc 1 of 2".
    Disc,
    /// "Track 3 of 12".
    Track,
    /// The service's own label.
    Other {
        label: String,
    },
}

/// One fact about an item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fact {
    pub key: FactKey,
    pub value: String,
}

/// How good what plays is, as the core judges it (ADR-0022 §3): Hi-Res
/// Lossless (lossless above 16-bit or 48 kHz), Lossless, High (lossy at
/// 256 kbit/s or more), Standard. The apps word it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityLabel {
    HiResLossless,
    Lossless,
    High,
    Standard,
}

/// The format of what plays or of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFormat {
    /// Short lowercase codec name: `flac`, `mp3`, `aac`, `alac`, `vorbis`,
    /// `opus`, `pcm`.
    pub codec: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub bit_depth: Option<u32>,
    pub channels: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub lossless: Option<bool>,
    pub quality: Option<QualityLabel>,
}

/// ReplayGain values (gains in dB, peaks as linear amplitude).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReplayGainValues {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

/// Technical information about a file or a stream.
#[derive(Debug, Clone, PartialEq)]
pub struct TechnicalInfo {
    pub format: AudioFormat,
    /// The container: `flac`, `mp3`, `mp4`, `ogg`, `wav`, …
    pub container: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub replaygain: Option<ReplayGainValues>,
    /// The file, for local items.
    pub path: Option<PathBuf>,
}

/// An item's details.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailsPage {
    /// The `Details` command it answers.
    pub request: CommandId,
    pub item: ItemRef,
    pub title: String,
    /// The artist (or album artist).
    pub subtitle: Option<String>,
    /// Album notes, an artist's biography.
    pub description: Option<String>,
    /// A large cover or an artist photo: a cached file, or (for a source
    /// picture still to fetch) its URL for [`crate::Core::source_artwork`].
    pub image: Option<ArtworkRef>,
    pub facts: Vec<Fact>,
    /// For songs: the file's format, or the stream's once it played.
    pub technical: Option<TechnicalInfo>,
    /// How many comments the service has for it.
    pub comment_count: Option<u64>,
    /// The source's attribution, when its terms ask for one.
    pub attribution: Option<String>,
    /// Why there are no details (or only some).
    pub error: Option<String>,
}

/// A comment on a source item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentEntry {
    pub id: String,
    pub author: String,
    pub avatar: Option<ArtworkRef>,
    pub text: String,
    pub likes: Option<u64>,
    /// When it was written, in ms since the Unix epoch.
    pub time_ms: Option<i64>,
    pub replies: Option<u64>,
}

/// A page of comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentsPage {
    /// The `Comments` command it answers.
    pub request: CommandId,
    pub item: ItemRef,
    /// The service's highlighted comments (first page only).
    pub hot: Vec<CommentEntry>,
    pub comments: Vec<CommentEntry>,
    pub total: Option<u64>,
    /// Ask for it with `Comments { page: next }`.
    pub next: Option<String>,
    /// Show it with the comments ("Comments from {attribution}").
    pub attribution: Option<String>,
    pub error: Option<String>,
}

/// A source picture fetched into the artwork cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkReady {
    /// The URL asked for (`ArtworkRef::Url`).
    pub url: String,
    /// The size asked for.
    pub size_px: u32,
    /// The cached file, or `None` when it cannot be had.
    pub path: Option<PathBuf>,
}

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
#[derive(Debug, Clone)]
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
        }
    }
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
        }
    }
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
    /// Manifest names: `auth`, `search`, `browse`, `resolve_stream`,
    /// `lyrics`, `artwork`, `recommendations`.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// For playable tracks: what goes into the queue.
    pub track: Option<TrackRef>,
    /// Other sources that have the same item (unified search).
    pub also_in: Vec<String>,
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
    pub muted: bool,
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
}

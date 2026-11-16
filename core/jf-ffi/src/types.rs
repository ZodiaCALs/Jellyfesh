//! Mirror types: the `jf-core` surface in shapes every binding can carry.
//!
//! Each type has the name and fields of its `jf-core` original (`docs/core-api.md`), with these
//! changes, which `docs/ffi.md` lists:
//!
//! * Paths are `String`s. `Arc`s are flattened. `CommandId` is a `u64`.
//! * `usize` counts, indices and limits are `u32`.
//! * Tuple variants get named fields: slice events carry `snapshot`, the other events `status` or
//!   `update`; [`ArtworkRef`], [`LyricsSource`], [`ScanStatus`] and [`TrackRef`] name theirs.
//! * Tuples and ordered maps become records and hash maps: [`ScanError`], `SubmitLogin.fields`,
//!   [`ThemeColor`].
//!
//! Every type derives both UniFFI and serde: UniFFI carries them to Swift and Kotlin, and the same
//! types go to C# as JSON (`c_api`). In JSON, enums with data are objects tagged by `"type"` (the
//! variant name), plain enums are their variant name, and fields keep their Rust names.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---- setup --------------------------------------------------------------------------------------

/// How the core is set up.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct CoreConfig {
    /// Holds `library.sqlite` and `session.sqlite`.
    pub data_dir: String,
    /// Holds artwork and baked backdrops; safe to delete.
    pub cache_dir: String,
    /// Where audio goes. The audio device is only opened when something is first played.
    pub output: OutputKind,
    /// Output buffer length in milliseconds.
    #[uniffi(default = 200)]
    pub buffer_ms: u32,
    /// Restore the saved queue, position and settings on start.
    #[uniffi(default = true)]
    pub restore_session: bool,
    /// Save the queue, position and settings as they change.
    #[uniffi(default = true)]
    pub persist_session: bool,
    /// Watch the library folders for changes while `Settings.watch_folders` is on. Short-lived
    /// tools turn it off.
    #[uniffi(default = true)]
    pub watch_folders: bool,
    /// `.jfpkg` plugin packages the app ships in its bundle: installed at start when missing,
    /// updated when the bundled version is newer, never reinstalled once the person uninstalled
    /// one (until they install it again).
    #[uniffi(default = [])]
    #[serde(default)]
    pub bundled_plugins: Vec<String>,
}

/// Where the engine sends audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum OutputKind {
    /// An audio device by id or name (see `DeviceInfo`); `None` follows the system default.
    Device { name: Option<String> },
    /// A 32-bit float WAV file, rendered as fast as possible unless `realtime`.
    WavFile { path: String, realtime: bool },
    /// Discards the audio; paced like a device when `realtime`.
    Null { realtime: bool },
}

// ---- commands -----------------------------------------------------------------------------------

/// Everything a UI can ask the core to do. Failures come back as `Event::CommandFailed`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum Command {
    /// Replace the queue with `tracks` and play the one at `start`.
    PlayTracks {
        tracks: Vec<TrackRef>,
        start: u32,
    },
    /// Add to the queue: right after the current entry ("Play Next") or at the end ("Play Last").
    Enqueue {
        tracks: Vec<TrackRef>,
        play_next: bool,
    },
    RemoveFromQueue {
        uid: u64,
    },
    MoveInQueue {
        uid: u64,
        to: u32,
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
    /// Restarts the current track after its first three seconds, otherwise goes back one entry.
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
        path: String,
    },
    RemoveRoot {
        path: String,
    },
    /// Scan every library folder for changes.
    Rescan,
    /// Replace the queue with a whole collection in its natural order and play from `start`,
    /// shuffled when `shuffle` is set.
    PlayCollection {
        collection: Collection,
        start: u32,
        shuffle: bool,
    },
    /// Add a whole collection to the queue.
    EnqueueCollection {
        collection: Collection,
        play_next: bool,
    },
    /// Replace all settings: send back a modified copy of the `Settings` slice.
    SetSettings {
        settings: Settings,
    },
    SetMuted {
        muted: bool,
    },
    SetCrossfade {
        crossfade: CrossfadeSettings,
    },
    /// Output device by id (or name) from the `Devices` slice; `None` follows the system
    /// default.
    SetOutputDevice {
        name: Option<String>,
    },
    /// List output devices again (send on OS device notifications; the core never polls).
    RefreshDevices,
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
        to: u32,
    },
    /// Import an M3U/M3U8 file as a new playlist (`Event::PlaylistCreated` names it). Export is
    /// the `export_playlist` query.
    ImportPlaylist {
        path: String,
    },
    SetFavorite {
        track: TrackRef,
        favorite: bool,
    },
    /// Install a `.jfpkg` plugin package.
    InstallPlugin {
        path: String,
    },
    UninstallPlugin {
        id: String,
    },
    SetPluginEnabled {
        id: String,
        enabled: bool,
    },
    /// The plugin's settings as JSON matching its settings schema.
    SetPluginSettings {
        id: String,
        settings_json: String,
    },
    /// The person read and accepted the community-plugin disclosure for this plugin.
    AcknowledgeUnofficial {
        id: String,
    },
    /// Start signing in to a source; steps arrive as `Event::Auth`.
    BeginLogin {
        plugin: String,
        method: AuthMethod,
    },
    /// Connect to a service from `SourcesSnapshot::presets`: sets the server's address when the
    /// service asks for one, turns its plugin on and starts signing in, as `BeginLogin` does.
    ConnectSource {
        preset: String,
        address: Option<String>,
        method: AuthMethod,
    },
    /// Answer an `AuthStep::AskFields` step: field name to value. Secrets pass through once and
    /// are never stored.
    SubmitLogin {
        plugin: String,
        flow: u64,
        #[serde(serialize_with = "json_maps::sorted")]
        fields: HashMap<String, String>,
    },
    CancelLogin {
        plugin: String,
        flow: u64,
    },
    Logout {
        plugin: String,
    },
    /// Search the library and every enabled source; results arrive as `Event::SearchResults`.
    Search {
        query: String,
        scope: SearchScope,
    },
    /// Browse a source's catalogue: `node` `None` is its root.
    Browse {
        plugin: String,
        node: Option<String>,
        page: u32,
    },
    /// The remote session (ADR-0018): what the transport commands cannot say. While a session
    /// runs, Play, Pause, TogglePlayPause, Stop, Next, Previous, Seek, SetShuffle and SetRepeat
    /// control the service's player instead of the engine.
    Remote {
        action: RemoteAction,
    },
}

/// What `Command::Remote` asks of the remote session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum RemoteAction {
    /// Asks the service for its devices and state now (when the device picker opens).
    Refresh,
    /// Moves playback to the device with this id, playing there.
    Transfer { device: String },
    /// The playing device's volume, 0–100.
    SetVolume { percent: u32 },
    /// Stops controlling the service; its player carries on as it was.
    End,
}

/// What a queue entry plays.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum TrackRef {
    /// A track in the library, by its library id.
    Library { id: i64 },
    /// A file played directly, outside the library.
    File { path: String },
    /// A track from an online source (a plugin).
    Source { track: SourceTrack },
}

/// A track from an online source, with the display data the source gave when it was listed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Record)]
pub struct SourceTrack {
    /// The plugin's id, e.g. `org.jellyfesh.subsonic`.
    pub plugin: String,
    /// The plugin's own id for the track.
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// Artwork URL as the source gave it.
    pub artwork_url: Option<String>,
    pub isrc: Option<String>,
}

/// A group of tracks that plays as a unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
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
    /// An album, playlist or other container from an online source.
    Source {
        plugin: String,
        id: String,
    },
}

/// Where a search looks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum SearchScope {
    /// The library and every enabled source.
    All,
    Library,
    Plugin {
        id: String,
    },
}

/// How a source signs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum AuthMethod {
    Qr,
    Oauth,
    CookieImport,
    PasswordExchange,
    ApiKey,
}

/// Repeat mode of the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum RepeatMode {
    /// Stop after the last entry.
    Off,
    /// Start again from the first entry (reshuffled when shuffle is on).
    All,
    /// Replay the current entry when it ends; Next and Previous still move.
    One,
}

// ---- settings -----------------------------------------------------------------------------------

/// Crossfade between tracks. With album continuity, tracks of the same album join gaplessly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct CrossfadeSettings {
    pub enabled: bool,
    /// 1 000 to 12 000 ms.
    pub duration_ms: u32,
    pub album_continuity: bool,
}

/// Light, dark, or following the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum AppearancePref {
    System,
    Light,
    Dark,
}

/// The Now Playing backdrop: the baked artwork backdrop, or Misty's own field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum BackdropStyle {
    Artwork,
    Misty,
}

/// The size of lyrics on Now Playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum LyricsSize {
    Small,
    Medium,
    Large,
}

/// How lyrics lines line up on Now Playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum LyricsAlign {
    /// The start edge (left in left-to-right text).
    Start,
    Center,
}

/// Where the theme hue comes from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum HueSource {
    /// The current track's artwork (the fallback hue when there is none).
    Artwork,
    /// One fixed hue in degrees.
    Fixed { hue: f32 },
}

/// Everything the person sets in Preferences. The core applies the playback, library and source
/// parts; shells apply the appearance parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Settings {
    pub appearance: AppearancePref,
    pub style: BackdropStyle,
    /// `automatic` (Ice in light, Steel in dark) or one of the ten icon names.
    pub app_icon: String,
    pub hue_source: HueSource,
    /// Fewer translucent layers and shorter motion, beyond the OS setting.
    pub reduce_effects: bool,
    /// How see-through glass is, from 0 (solid) to 1 (clearest); 0.5 keeps the tokens' own
    /// alphas. Apply it with the glass-alpha function (`tests/contract/glass-alpha.json`).
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
    /// Start playing the restored queue on launch (otherwise it waits, paused).
    pub resume_on_launch: bool,
    /// Rescan folders when their files change.
    pub watch_folders: bool,
    /// Community (unofficial) plugins may be installed and enabled. Off by default.
    pub allow_community_plugins: bool,
}

/// Which ReplayGain value to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum ReplayGainMode {
    Off,
    /// Track gain (album gain when the track has none).
    Track,
    /// Album gain (track gain when the album has none).
    Album,
}

/// Equalizer settings: a preamp and up to ten bands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct EqSettings {
    pub enabled: bool,
    pub preamp_db: f32,
    pub bands: Vec<EqBand>,
}

/// One EQ band.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct EqBand {
    pub kind: BandKind,
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub enabled: bool,
}

/// Filter shape of an EQ band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum BandKind {
    Peaking,
    LowShelf,
    HighShelf,
}

// ---- devices and sources ------------------------------------------------------------------------

/// An equalizer preset: every app offers the same ones, in this order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct EqPreset {
    /// Stable name: `flat`, `bass_boost`, `vocal`, `treble`. Show it translated ("Flat", "Bass
    /// Boost", "Vocal", "Treble").
    pub id: String,
    /// Its bands, switched on, with a 0 dB preamp.
    pub settings: EqSettings,
}

/// A sound output device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct DeviceInfo {
    /// Stable identifier, unique per device (for example `alsa:default`). `OutputKind::Device`
    /// and `SetOutputDevice` accept it as the name.
    pub id: String,
    /// Human-readable name. Several devices can share one.
    pub name: String,
    pub is_default: bool,
    /// Common sample rates the device accepts, ascending.
    pub sample_rates: Vec<u32>,
}

/// The `Devices` slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct DevicesSnapshot {
    pub version: u64,
    pub devices: Vec<DeviceInfo>,
    /// The chosen device; `None` follows the system default.
    pub selected: Option<String>,
}

/// The badge a source shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum PluginBadge {
    /// Signed by a key in the app's trust list.
    Official,
    /// Anything else: community, unofficial, opt-in.
    Community,
}

/// One installed source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    /// One line from the manifest, if it has one.
    pub description: Option<String>,
    pub badge: PluginBadge,
    /// Community packages: the fingerprint of the key that signed it, to show with the
    /// disclosure (`None` when unsigned, or official).
    pub signing_key: Option<String>,
    /// Vendor playback: the service plays it, so no EQ, crossfade or visualization.
    pub vendor_playback: bool,
    /// Remote playback: the service plays on the person's own devices and Jellyfesh is its
    /// remote control. Playing its tracks starts a remote session; they never join the queue.
    pub remote_playback: bool,
    /// Manifest names: `auth`, `search`, `browse`, `resolve_stream`, `lyrics`, `artwork`,
    /// `recommendations`, `remote`.
    pub capabilities: Vec<String>,
    pub auth_methods: Vec<AuthMethod>,
    /// Whether the plugin can be used now.
    pub enabled: bool,
    /// Why it cannot be used, when the host turned it off (after it broke its limits three
    /// times in a row) or community plugins are turned off.
    pub disabled_reason: Option<String>,
    /// The signed-in account's display name.
    pub account: Option<String>,
    /// The community disclosure has not been accepted yet.
    pub needs_disclosure: bool,
    /// Attribution its views must show, if any.
    pub attribution: Option<String>,
    pub settings_schema_json: String,
    pub settings_json: String,
    /// The package's icon as a PNG file in the cache folder (named by its content).
    pub icon: Option<String>,
}

/// A music service the apps offer in Sources → Add a Source, listed while its official plugin
/// is installed. `CoreCommand::ConnectSource` connects to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum SecretStorage {
    /// The system keystore, or the store the app passed in.
    Keystore,
    /// No keystore could be reached: sign-ins last until the app quits.
    SessionOnly,
}

/// The `Sources` slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct SourcesSnapshot {
    pub version: u64,
    pub plugins: Vec<PluginSummary>,
    /// The services offered in Sources → Add a Source, in their order.
    pub presets: Vec<SourcePreset>,
    pub allow_community: bool,
    /// Whether sign-ins outlive this run.
    pub secret_storage: SecretStorage,
    /// Set when online sources cannot run at all.
    pub unavailable: Option<String>,
}

/// How a sign-in field is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum AuthFieldKind {
    Text,
    Email,
    Url,
    /// Digits only, such as a one-time code.
    Number,
    /// A `Cookie:` header copied from a browser (cookie-import logins).
    CookieHeader,
}

/// A field to ask for during sign-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum AuthStep {
    /// Show `data` (usually a URL) as a QR code for the service's app to scan; the core polls
    /// meanwhile. The UI draws the code.
    ShowQr {
        data: String,
        /// One sentence for the person ("Scan with the Example app").
        hint: Option<String>,
        expires_at_ms: Option<i64>,
    },
    /// Show `code` large, to be entered on another device (`hint` says where); the core polls
    /// meanwhile.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AuthUpdate {
    pub plugin: String,
    pub flow: u64,
    pub step: AuthStep,
}

// ---- search, browse and lists -------------------------------------------------------------------

/// What a list row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum ItemKind {
    Track,
    Album,
    Artist,
    Playlist,
    Folder,
}

/// Artwork a UI can load: a local file or a URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum ArtworkRef {
    File { path: String },
    Url { url: String },
}

/// A search or browse result from the library or a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
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
    /// False for items the service will not play here (region, paywall).
    pub playable: bool,
    pub unplayable_reason: Option<String>,
    /// For playable tracks: what goes into the queue (or, for a remote source, what starts its
    /// remote session).
    pub track: Option<TrackRef>,
    /// Other sources that have the same item (unified search).
    pub also_in: Vec<String>,
    /// An `https` link that opens the item in its service ("Open in {Service}"), when its
    /// source declares one. The main action for items that cannot play here.
    pub link: Option<String>,
}

/// Results from one source for one search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct SearchUpdate {
    /// The id `send` returned for the `Search` command these results answer.
    pub request: u64,
    pub query: String,
    /// `library` or the plugin's id.
    pub source: String,
    pub items: Vec<SourceItem>,
    pub error: Option<String>,
    /// True on the last message for this request.
    pub done: bool,
}

/// One page of a source's catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct BrowseUpdate {
    /// The id `send` returned for the `Browse` command.
    pub request: u64,
    pub plugin: String,
    pub node: Option<String>,
    pub items: Vec<SourceItem>,
    pub next_page: Option<u32>,
    pub error: Option<String>,
}

/// A row in a track list (playlist, favourites, history): the track and what to show for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct TrackRow {
    /// What to queue to play the row: a library track, a file (one that is not in the
    /// catalogue) or a source track.
    pub track: TrackRef,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// A source track's image URL. Library rows have `artwork_hash` instead.
    pub artwork: Option<ArtworkRef>,
    /// Library rows: the artwork's hash, for `artwork_thumbnail`.
    pub artwork_hash: Option<String>,
    pub favorite: bool,
    /// The file is gone; the row cannot play.
    pub missing: bool,
}

/// A playlist in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub entry_count: u32,
    pub duration_ms: u64,
    pub updated_at_ms: i64,
    pub artwork_hash: Option<String>,
}

/// One entry of a playlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct PlaylistEntry {
    pub entry_id: i64,
    pub position: u32,
    pub row: TrackRow,
}

// ---- library ------------------------------------------------------------------------------------

/// One audio file in the library. Missing tags are `None`; only the title falls back, to the
/// file name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    /// The album this track is grouped into; `None` without an album tag.
    pub album_id: Option<i64>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub duration_ms: Option<u64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bit_depth: Option<u8>,
    /// Short lowercase codec name: "mp3", "flac", "aac", "alac", "vorbis", "opus", "pcm", …
    pub codec: Option<String>,
    pub replaygain: ReplayGain,
    pub isrc: Option<String>,
    pub has_embedded_lyrics: bool,
    /// Content hash of the embedded front cover, for `artwork_thumbnail`.
    pub artwork_hash: Option<String>,
    pub file_size: u64,
    pub mtime_ns: i64,
}

/// ReplayGain values from the tags (gains in dB, peaks as linear amplitude).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct ReplayGain {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

/// Tracks grouped by album title and album artist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub album_artist: Option<String>,
    /// Earliest year among the album's tracks.
    pub year: Option<i32>,
    pub track_count: u32,
    /// Artwork of the first track that has some.
    pub artwork_hash: Option<String>,
}

/// One artist, after splitting multi-artist tags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub track_count: u32,
}

/// Sort order for `tracks`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum TrackSort {
    /// Artist, then album, disc, track number and title. Unknown artists last.
    Artist,
    Title,
    /// Album, then disc, track number and title. Unknown albums last.
    Album,
    Year,
    Duration,
    Path,
    /// When the file was first scanned.
    Added,
}

/// Sort order for `albums_sorted`. Unknown artists and years sort last in both directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum AlbumSort {
    /// Title, then album artist.
    Title,
    /// Album artist (or the primary track artist without one), then year and title.
    Artist,
    /// Earliest year among the album's tracks, then title.
    Year,
    /// When the album's first track was first scanned.
    Added,
}

/// A page of tracks for `tracks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct TrackQuery {
    #[uniffi(default = 0)]
    pub offset: u32,
    /// Maximum number of tracks returned.
    #[uniffi(default = 100)]
    pub limit: u32,
    pub sort: TrackSort,
    #[uniffi(default = false)]
    pub descending: bool,
    /// Only tracks of this album.
    #[uniffi(default = None)]
    pub album_id: Option<i64>,
    /// Only tracks credited to this artist.
    #[uniffi(default = None)]
    pub artist_id: Option<i64>,
}

/// Library search results, each list ranked best first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
}

/// Counts for a status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct LibraryStats {
    pub roots: u64,
    pub tracks: u64,
    pub albums: u64,
    pub artists: u64,
    /// Distinct cached artwork images.
    pub artwork: u64,
    pub total_duration_ms: u64,
    pub total_bytes: u64,
}

/// Reported while a scan runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ScanProgress {
    /// Audio files visited so far.
    pub files_seen: u64,
    /// Of those, files that were new or changed and had their tags read.
    pub files_changed: u64,
    pub current: String,
}

/// What a scan did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ScanReport {
    pub added: u64,
    pub updated: u64,
    pub removed: u64,
    pub unchanged: u64,
    /// Files (or, for walk errors, roots) that could not be read.
    pub errors: Vec<ScanError>,
}

/// A file a scan could not read, and why (a `(path, message)` pair in `jf-core`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ScanError {
    pub path: String,
    pub message: String,
}

/// Library scan progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum ScanStatus {
    Progress { progress: ScanProgress },
    Finished { report: ScanReport },
    Failed { message: String },
}

// ---- playback and queue -------------------------------------------------------------------------

/// Transport state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum Status {
    /// Nothing playing (never started, or stopped).
    Stopped,
    Playing,
    Paused,
    /// The queue played to its end.
    Ended,
}

/// A point on the playback timeline that UIs extrapolate from:
/// `position(now) = position_us + (now - anchor_mono_ns) * rate / 1000`, on the clock of
/// `monotonic_now_ns()`. Feed it to a `DisplayClock` rather than computing this by hand.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct PlaybackAnchor {
    /// Changes on every track change and seek; extrapolations across generations are invalid.
    pub track_generation: u64,
    /// Audible position in the current track at `anchor_mono_ns`, in microseconds.
    pub position_us: i64,
    /// When the position was measured, on `monotonic_now_ns()`'s clock.
    pub anchor_mono_ns: u64,
    /// 1.0 while playing, 0.0 while paused, stopped or ended.
    pub rate: f32,
    /// Duration of the current track, when known.
    pub duration_us: Option<i64>,
}

/// The `Playback` slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct PlaybackSnapshot {
    pub version: u64,
    pub status: Status,
    /// Time base for progress bars and lyrics.
    pub anchor: PlaybackAnchor,
    pub volume: f32,
    pub eq: EqSettings,
    pub replaygain_mode: ReplayGainMode,
    pub preamp_db: f32,
}

/// One entry in the queue; `uid` is unique for the queue's whole life.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct QueueItem {
    pub uid: u64,
    pub track: TrackRef,
}

/// The `Queue` slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct QueueSnapshot {
    pub version: u64,
    pub len: u32,
    pub position: Option<u32>,
    pub current: Option<QueueItem>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// The next entries in playback order, at most 50; `queue_window` pages the rest.
    pub up_next: Vec<QueueItem>,
}

/// A track that started playing, for the history list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct HistoryEntry {
    pub track: TrackRef,
    /// Wall-clock time in milliseconds since the Unix epoch.
    pub played_at_ms: i64,
}

/// Facts about an opened track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct TrackInfo {
    /// Short codec name, for example `flac`, `mp3`, `aac`, `opus`, `vorbis` or `pcm_s16le`.
    pub codec: String,
    /// Sample rate of the source, in Hz.
    pub sample_rate: u32,
    /// Channel count of the source.
    pub channels: u16,
    /// Playable duration, when the container states it.
    pub duration_us: Option<i64>,
    /// Whether joins into and out of this track are sample-exact.
    pub gapless: bool,
    /// Bit rate in bits per second, when known (the file's average for compressed formats).
    pub bitrate: Option<u32>,
}

/// What is playing, with display metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct NowPlaying {
    pub uid: u64,
    pub track: TrackRef,
    /// The audio file, for local tracks.
    pub path: Option<String>,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// A cached file for local tracks, a URL for online ones.
    pub artwork: Option<ArtworkRef>,
    /// Stream format, once the engine has opened the file.
    pub format: Option<TrackInfo>,
    /// Opens the track in its service, for source tracks whose plugin declares links.
    pub link: Option<String>,
}

// ---- lyrics -------------------------------------------------------------------------------------

/// One lyric document, whatever format it came from. Times are milliseconds from the start of
/// the audio with any source offset already applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct LyricDoc {
    /// BCP 47 language of the lyrics when the source declares it.
    pub language: Option<String>,
    /// The best timing precision present in the document.
    pub timing: Timing,
    /// The offset the source declared, already applied to every time.
    pub offset_ms: i32,
    /// Singers and voices referenced by `Line.agent`.
    pub agents: Vec<Agent>,
    /// Lines in display order: sorted by start time when the document is synced.
    pub lines: Vec<Line>,
    /// Translation and romanization tracks, aligned to `Line.id`.
    pub tracks: Vec<AuxTrack>,
    /// Where the document came from.
    pub source: SourceInfo,
    /// Source metadata such as LRC `ti`/`ar`/`al` tags, keys lowercased.
    #[serde(serialize_with = "json_maps::sorted")]
    pub metadata: HashMap<String, String>,
}

/// Timing precision of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum Timing {
    /// No times: static text.
    Unsynced,
    /// Each line has a start (and usually an end).
    Line,
    /// At least one line has word (or syllable) times.
    Word,
}

/// A singer or voice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct Agent {
    /// Identifier referenced by `Line.agent` (for example `v1`).
    pub id: String,
    pub name: Option<String>,
    pub kind: AgentKind,
}

/// Kind of agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum AgentKind {
    Person,
    Group,
    Other,
}

/// One lyric line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct Line {
    /// Position of the line in `LyricDoc.lines`; auxiliary tracks are keyed by it.
    pub id: u32,
    /// Start time; `None` only in unsynced documents.
    pub start_ms: Option<u32>,
    /// End time; the last line of a document may have none.
    pub end_ms: Option<u32>,
    /// The singer's agent id, for duets.
    pub agent: Option<String>,
    /// Display text. Empty for instrumental gaps.
    pub text: String,
    /// Word (or syllable) times; empty when the line only has line timing.
    pub words: Vec<Word>,
    /// Background vocals, shown smaller.
    pub background: Vec<Word>,
    /// Writing direction, from the first strong character.
    pub direction: Direction,
}

/// A timed word or syllable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct Word {
    pub start_ms: u32,
    /// Never before `start_ms`; equal to it for zero-length words.
    pub end_ms: u32,
    pub text: String,
    /// True when no space separates this word from the next one (CJK, syllables).
    pub joins_next: bool,
}

/// Writing direction of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum Direction {
    Ltr,
    Rtl,
    /// No strong character: the shell decides.
    Auto,
}

/// A translation or romanization track, aligned line by line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AuxTrack {
    pub kind: AuxKind,
    /// BCP 47 language of the track when known.
    pub language: Option<String>,
    /// Lines of the track keyed by `Line.id`. Lines without an entry have no counterpart.
    #[serde(
        serialize_with = "json_maps::sorted",
        deserialize_with = "json_maps::u32_keys"
    )]
    pub lines: HashMap<u32, AuxLine>,
}

/// Kind of auxiliary lyric track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum AuxKind {
    /// The meaning in another language.
    Translation,
    /// The same words in another script (for example romaji).
    Romanization,
}

/// One line of an auxiliary track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AuxLine {
    pub text: String,
    /// Word times when the source provides them.
    pub words: Vec<Word>,
}

/// Where a lyric document came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct SourceInfo {
    pub format: Format,
    /// Free-form origin set by the core (file path, tag name, plugin id).
    pub origin: Option<String>,
}

/// Lyric source formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum Format {
    Lrc,
    EnhancedLrc,
    Ttml,
    WebVtt,
    Plain,
}

/// Flat lookup arrays for the per-frame lyric cursor (ported to every shell as
/// `LyricTimeline.cursor` and `nextChange`). `segment_lines` and `line_ends` use
/// `u32::MAX` (4294967295) for "no line" and "no end".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct LyricTimeline {
    /// Start of each line, ascending.
    pub line_starts: Vec<u32>,
    /// End of each line; `u32::MAX` when the line never ends.
    pub line_ends: Vec<u32>,
    /// Breakpoints of the active-line step function, strictly ascending.
    pub segment_starts: Vec<u32>,
    /// Active line from the matching breakpoint until the next one, or `u32::MAX` for none.
    pub segment_lines: Vec<u32>,
    /// Words of line `i` are at `word_offsets[i]..word_offsets[i + 1]` in the word arrays.
    pub word_offsets: Vec<u32>,
    /// Word starts, ascending within each line.
    pub word_starts: Vec<u32>,
    /// Word ends, never before the word's start.
    pub word_ends: Vec<u32>,
}

/// Where lyrics came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum LyricsSource {
    /// A `.ttml`, `.lrc` or `.vtt` file next to the audio file.
    Sidecar { path: String },
    /// Lyrics stored in the audio file's tags.
    Embedded,
    /// The plugin with this id (its attribution, if any, is in `LyricDoc.source.origin`).
    Plugin { id: String },
}

/// The `Lyrics` slice for the current track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct LyricsSnapshot {
    pub uid: u64,
    pub doc: Option<LyricDoc>,
    /// Flat arrays for the per-frame cursor lookup.
    pub timeline: Option<LyricTimeline>,
    pub source: Option<LyricsSource>,
}

// ---- theme --------------------------------------------------------------------------------------

/// Light or dark appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum Appearance {
    Light,
    Dark,
}

/// One theme colour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ThemeColor {
    /// The token name, `group.token` (for example `ink.ink-90`).
    pub name: String,
    /// 8-bit sRGB with alpha, packed as `0xRRGGBBAA`.
    pub rgba: u32,
}

/// Every theme colour resolved for one hue and appearance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Theme {
    /// The theme hue in degrees, 0 to 360.
    pub hue: f32,
    pub appearance: Appearance,
    /// Every colour, sorted by name.
    pub colors: Vec<ThemeColor>,
}

/// The `Theme` slice: Misty colours from the artwork's hue, and baked Now Playing backdrops.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct ThemeSnapshot {
    pub uid: Option<u64>,
    pub hue: f32,
    /// False when the hue is the fallback (no artwork, or artwork too grey).
    pub from_artwork: bool,
    pub light: Theme,
    pub dark: Theme,
    pub backdrop_light: Option<String>,
    pub backdrop_dark: Option<String>,
}

// ---- remote sessions ----------------------------------------------------------------------------

/// What a remote device is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum RemoteDeviceKind {
    Computer,
    Smartphone,
    Speaker,
    Tv,
    Car,
    Other,
}

/// A device a remote source can play on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    pub kind: RemoteDeviceKind,
    /// The device playing (or last playing) for this account.
    pub active: bool,
    /// Its volume, 0–100, when it lets Jellyfesh change it.
    pub volume_percent: Option<u32>,
}

/// The `Remote` slice: the remote session, while Jellyfesh controls a service's own player.
/// Apps show it as Now Playing instead of the engine's track while `source` is set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
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

// ---- snapshot and events ------------------------------------------------------------------------

/// Everything a UI mirrors. Each slice carries its own version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Snapshot {
    pub playback: PlaybackSnapshot,
    pub queue: QueueSnapshot,
    pub now_playing: Option<NowPlaying>,
    pub lyrics: Option<LyricsSnapshot>,
    pub theme: Option<ThemeSnapshot>,
    pub library_revision: u64,
    pub scanning: bool,
    pub settings: Settings,
    pub devices: DevicesSnapshot,
    pub sources: SourcesSnapshot,
    pub remote: RemoteSnapshot,
    pub muted: bool,
}

/// What the core tells the listener. Slices arrive whole whenever their version changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
#[allow(
    clippy::large_enum_variant,
    reason = "UniFFI enums cannot hold boxes; each event is built once and handed to the listener"
)]
pub enum Event {
    Playback {
        snapshot: PlaybackSnapshot,
    },
    Queue {
        snapshot: QueueSnapshot,
    },
    NowPlaying {
        snapshot: Option<NowPlaying>,
    },
    Lyrics {
        snapshot: LyricsSnapshot,
    },
    Theme {
        snapshot: ThemeSnapshot,
    },
    Scan {
        status: ScanStatus,
    },
    /// The catalogue, a playlist, a favourite or a play count changed: lists query again.
    /// `revision` only grows.
    LibraryChanged {
        revision: u64,
    },
    /// `CreatePlaylist` or `ImportPlaylist` (the command `send` returned `request` for) made
    /// playlist `id`.
    PlaylistCreated {
        request: u64,
        id: i64,
    },
    Settings {
        snapshot: Settings,
    },
    Devices {
        snapshot: DevicesSnapshot,
    },
    Sources {
        snapshot: SourcesSnapshot,
    },
    /// The remote session changed.
    Remote {
        snapshot: RemoteSnapshot,
    },
    Auth {
        update: AuthUpdate,
    },
    SearchResults {
        update: SearchUpdate,
    },
    BrowsePage {
        update: BrowseUpdate,
    },
    /// A source is buffering the current track (rate-0 anchors meanwhile).
    Buffering {
        uid: u64,
        buffering: bool,
    },
    /// The command with this id (as returned by `send`) failed.
    CommandFailed {
        id: u64,
        error: String,
    },
    /// A problem not tied to a command (an unreadable file during playback).
    Error {
        message: String,
    },
}

/// JSON for the map fields: keys sorted, so the same value always gives the same text, and
/// integer keys read back from their string form, which serde cannot do on its own inside an
/// internally tagged enum such as [`Event`].
mod json_maps {
    use std::collections::{BTreeMap, HashMap};

    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn sorted<S: Serializer, K: Ord + Serialize, V: Serialize>(
        map: &HashMap<K, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        map.iter().collect::<BTreeMap<_, _>>().serialize(serializer)
    }

    pub fn u32_keys<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
        deserializer: D,
    ) -> Result<HashMap<u32, V>, D::Error> {
        HashMap::<String, V>::deserialize(deserializer)?
            .into_iter()
            .map(|(key, value)| match key.parse() {
                Ok(key) => Ok((key, value)),
                Err(_) => Err(D::Error::custom(format!("{key:?} is not a line id"))),
            })
            .collect()
    }
}

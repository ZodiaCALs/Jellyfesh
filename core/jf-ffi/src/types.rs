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
    /// Let `Settings.resume_on_launch` start playback when the core starts. Apps started
    /// without their window (a media button, a start by the system) pass false, so no audio
    /// starts before their media session exists, and send `Play` themselves later.
    #[uniffi(default = true)]
    #[serde(default = "yes")]
    pub resume_on_start: bool,
    /// `.jfpkg` plugin packages the app ships in its bundle: installed at start when missing,
    /// updated when the bundled version is newer, never reinstalled once the person uninstalled
    /// one (until they install it again).
    #[uniffi(default = [])]
    #[serde(default)]
    pub bundled_plugins: Vec<String>,
    /// How this build reached the person (ADR-0021 §4): which stores sell Jellyfesh Pro here,
    /// and whether the core checks for updates (`Direct` only). `Source` when left out of JSON.
    #[serde(default)]
    pub distribution: Distribution,
    /// The platform's default downloads folder (ADR-0025 §2): `Jellyfesh` in the person's Music
    /// folder (`~/Music/Jellyfesh`, `Music\Jellyfesh`, `Music/Jellyfesh` in Android's shared
    /// storage). `None` lets the core work one out. `Settings.downloads.folder` replaces it.
    #[uniffi(default = None)]
    #[serde(default)]
    pub downloads_dir: Option<String>,
}

/// How this build of the app reached the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, uniffi::Enum)]
pub enum Distribution {
    /// A download from the project's release page: checks for updates itself.
    Direct,
    AppStore,
    MicrosoftStore,
    GooglePlay,
    /// Jellyfesh Web.
    Web,
    /// Built from source.
    #[default]
    Source,
}

/// A store that sells Jellyfesh Pro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum Store {
    AppStore,
    MicrosoftStore,
    GooglePlay,
}

/// What a store says about Jellyfesh Pro, as the app's store glue verified it with the platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct StoreEntitlement {
    pub store: Store,
    /// The store's product id (`org.jellyfesh.pro`, or `pro` on Google Play).
    pub product: String,
    /// Whether the person owns it now (false after a refund or revocation).
    pub active: bool,
    /// The store's transaction or order id, if it gives one.
    pub transaction: Option<String>,
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
    /// shuffled when `shuffle` is set. With `shuffle` and `start` 0 (a Shuffle button: no song
    /// chosen) any song can come first. A source collection plays as soon as its first page is
    /// in; later pages join the queue as they arrive.
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
    /// Makes synced lyrics for a song with the AI Lyrics plugin (a job in the `AiLyrics` slice).
    MakeLyrics {
        song: SongRef,
    },
    /// Stops a waiting or running lyrics job.
    CancelLyrics {
        job: u64,
    },
    /// Deletes the lyrics AI Lyrics made for a song.
    DeleteMadeLyrics {
        song: SongRef,
    },
    /// The person allowed sending songs' audio to `origin` for this plugin.
    AllowAudioUpload {
        plugin: String,
        origin: String,
    },
    /// Stop Allowing: no more audio goes to `origin` for this plugin.
    RevokeAudioUpload {
        plugin: String,
        origin: String,
    },
    /// Downloads an on-device speech model.
    DownloadSpeechModel {
        id: String,
    },
    /// Stops a model download (what came so far is kept).
    CancelSpeechModelDownload {
        id: String,
    },
    /// Deletes a downloaded speech model.
    DeleteSpeechModel {
        id: String,
    },
    /// Restores a plugin's settings to the schema's defaults (Reset to Defaults).
    ResetPluginSettings {
        plugin: String,
    },
    /// Activates Jellyfesh Pro with a licence key (`JFP1-…`).
    ActivateLicence {
        key: String,
    },
    /// Removes the licence key.
    RemoveLicence,
    /// Checks for a newer release now (direct downloads only).
    CheckForUpdates,
    /// A source's Discover page (`library` for the library's); answered by `Event::Discover`.
    Discover {
        source: String,
    },
    /// More about an item; answered by `Event::Details`.
    Details {
        item: ItemRef,
    },
    /// A page of comments (`page` `None` is the first, else the previous page's `next`);
    /// answered by `Event::Comments`.
    Comments {
        item: ItemRef,
        page: Option<String>,
    },
    /// The network the platform reports (send at start and on every change): the core decides
    /// Auto quality (High while metered) and whether downloads run (not offline, not metered with
    /// Wi-Fi Only on).
    SetNetworkState {
        online: bool,
        metered: bool,
    },
    /// Show More: more of one kind of a source's results for the newest search (`cursor`: the
    /// group's `next`); answered by `Event::SearchPage`.
    SearchMore {
        plugin: String,
        kind: ItemKind,
        cursor: String,
    },
    /// Reloads a source's likes and playlists (`None`: every source); send when Favourites or
    /// Playlists opens, or is returned to after ten minutes.
    RefreshSourceLibrary {
        plugin: Option<String>,
    },
    /// The next page of a source's list; answered by `Event::SourceLibraryPage`.
    SourceLibraryMore {
        plugin: String,
        part: SourceLibraryPart,
        cursor: String,
    },
    /// Save as Playlist…: a Jellyfesh playlist of a source playlist's songs; answered by
    /// `Event::PlaylistCreated`.
    SavePlaylistCopy {
        plugin: String,
        id: String,
        name: String,
    },
    /// Downloads a song, or every song of an album or playlist (ADR-0025).
    Download {
        target: DownloadTarget,
    },
    /// Stops an unfinished download and forgets it.
    CancelDownload {
        id: u64,
    },
    /// Tries a failed download again.
    RetryDownload {
        id: u64,
    },
    /// Remove Download…: deletes a finished download's file (ask first).
    RemoveDownload {
        id: u64,
    },
    /// Remove All Downloads… (ask first).
    RemoveAllDownloads,
}

/// What `Command::Download` downloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum DownloadTarget {
    /// One song: a `TrackRef::Source`.
    Track { track: TrackRef },
    /// Every song of an album or playlist a source listed.
    Collection {
        plugin: String,
        id: String,
        item_kind: ItemKind,
    },
}

/// A song to make lyrics for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum SongRef {
    /// The song playing now.
    Current,
    /// A library song.
    Library { id: i64 },
}

/// An item to show details or comments for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum ItemRef {
    /// A library track.
    Track { id: i64 },
    /// A library album.
    Album { id: i64 },
    /// A library artist.
    Artist { id: i64 },
    /// An audio file that is not in the library.
    File { path: String },
    /// An item a source listed: its `SourceItem.source`, `kind` and `id`.
    Source {
        plugin: String,
        item_kind: ItemKind,
        id: String,
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
    /// Every song in the library, in the Songs page's order.
    Songs {
        sort: TrackSort,
        descending: bool,
    },
    Tracks {
        tracks: Vec<TrackRef>,
    },
    /// An album, playlist or other container from an online source: a `SourceItem`'s `source`,
    /// `id` and `kind`.
    Source {
        plugin: String,
        id: String,
        item_kind: ItemKind,
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
    /// Check for Updates Automatically (direct downloads only): at most once a day. On by
    /// default.
    #[uniffi(default = true)]
    #[serde(default = "yes")]
    pub check_for_updates: bool,
    /// Show Lyrics Bar (Settings › Appearance › Lyrics): the one-line lyrics bar under the
    /// player (`lyrics_bar`). On by default.
    #[uniffi(default = true)]
    #[serde(default = "yes")]
    pub show_lyrics_bar: bool,
    /// Desktop Lyrics (Settings › Appearance › Lyrics): the current line in a window of its own
    /// above other apps, on desktops. Off by default.
    #[uniffi(default = false)]
    #[serde(default)]
    pub desktop_lyrics: bool,
    /// Streaming Quality (ADR-0026 §1). `Auto` by default.
    #[serde(default)]
    pub streaming_quality: StreamingQuality,
    /// Settings › Downloads (ADR-0025 §2).
    #[serde(default)]
    pub downloads: DownloadSettings,
    /// Settings › Appearance › Mini Player (ADR-0026 §4).
    #[serde(default)]
    pub mini_player: MiniPlayerSettings,
}

fn yes() -> bool {
    true
}

/// The streaming level asked of sources (ADR-0026 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, uniffi::Enum)]
pub enum StreamingQuality {
    /// Lossless, or High on a connection the platform reports as metered.
    #[default]
    Auto,
    Lossless,
    High,
    Normal,
    Low,
}

/// The quality downloads are asked for (ADR-0025 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, uniffi::Enum)]
pub enum DownloadQuality {
    /// The file exactly as the server keeps it.
    #[default]
    Original,
    High,
    Normal,
    Low,
}

/// Settings › Downloads (ADR-0025 §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(default)]
pub struct DownloadSettings {
    /// The folder the person chose; `None` uses the default (`DownloadsSnapshot.folder`).
    pub folder: Option<String>,
    pub quality: DownloadQuality,
    /// File Names: `{artist}`, `{album_artist}`, `{album}`, `{title}`, `{track}`, `{disc}`,
    /// `{year}`, `{source}`, `/` between folders (`download_preview` shows an example).
    pub name_template: String,
    /// Add Missing Cover and Lyrics (off: original files stay byte for byte).
    pub add_missing_cover_and_lyrics: bool,
    /// Lyrics in the tags (on by default).
    pub lyrics_in_tags: bool,
    /// Save .lrc Files beside the songs (off by default).
    pub save_lrc_files: bool,
    /// Wi-Fi Only: downloads wait while the network is metered (on by default).
    pub wifi_only: bool,
    /// Downloads at Once, 1–4 (2 by default).
    pub at_once: u32,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        jf_core::DownloadSettings::default().into()
    }
}

/// Settings › Appearance › Mini Player (ADR-0026 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(default)]
pub struct MiniPlayerSettings {
    /// Keep on Top (on by default).
    pub keep_on_top: bool,
    /// Open When the Window Is Closed (off by default).
    pub open_when_window_closed: bool,
    /// The lyrics bar's current line in the mini player (on by default).
    pub show_lyrics_line: bool,
    /// Android: Mini Player in Picture-in-Picture (off by default).
    pub android_pip: bool,
}

impl Default for MiniPlayerSettings {
    fn default() -> Self {
        jf_core::MiniPlayerSettings::default().into()
    }
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
    /// Source (Settings › Sources) or Extension (Settings › Extensions).
    #[serde(default = "source_kind")]
    pub kind: PluginKind,
    /// It has a Jellyfesh Pro capability (`make_lyrics`, `appearance`).
    #[serde(default)]
    pub requires_pro: bool,
    /// The host imports the manifest declares (`audio`, `speech`), for the install screen.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// It has a Discover page of its own.
    #[serde(default)]
    pub discover: bool,
    /// It describes items (biography, notes, facts).
    #[serde(default)]
    pub details: bool,
    /// It shows comments: offer a Comments pane.
    #[serde(default)]
    pub comments: bool,
    /// It lists the person's likes and playlists (`library`): it joins the `SourceLibrary`
    /// slice while on and signed in.
    #[serde(default)]
    pub library: bool,
    /// It offers downloads (`download`).
    #[serde(default)]
    pub download: bool,
    /// The streaming levels it offers, best first; empty when the service sets the quality.
    #[serde(default)]
    pub qualities: Vec<StreamingQuality>,
    /// Its server's address is on the local network (Android's definition, `address_is_local`):
    /// Android apps targeting SDK 37 ask for ACCESS_LOCAL_NETWORK before using it.
    #[serde(default)]
    pub local_network: bool,
}

/// Where Settings lists a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum PluginKind {
    /// A music source: Settings › Sources.
    Source,
    /// AI Lyrics, Customize and the like: Settings › Extensions.
    Extension,
}

fn source_kind() -> PluginKind {
    PluginKind::Source
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
    /// Plugins may be installed from files and community plugins allowed (false in store
    /// builds: hide Install from File… and Allow Community Plugins).
    #[serde(default = "yes")]
    pub can_install_plugins: bool,
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
    /// Go to Artist for a source track or album: the source's id of its artist (for `Browse`
    /// and `Details`).
    #[serde(default)]
    pub artist_id: Option<String>,
    /// Go to Album for a source track: the source's id of its album.
    #[serde(default)]
    pub album_id: Option<String>,
    /// A source track with a finished download (it plays the file): the Downloaded mark.
    #[serde(default)]
    pub downloaded: bool,
}

/// Results from one source for one search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct SearchUpdate {
    /// The id `send` returned for the `Search` command these results answer.
    pub request: u64,
    pub query: String,
    /// `library` or the plugin's id.
    pub source: String,
    /// The section for the All tab: with `merged`, its results less those an earlier section
    /// shows; otherwise all its results.
    pub items: Vec<SourceItem>,
    /// Part of the merged results (the library, From Your Sources). False for sources whose
    /// terms forbid merging (Spotify, QQ Music): never de-duplicated, never shown beside other
    /// rows; the All tab links to the source's own tab.
    #[serde(default)]
    pub merged: bool,
    /// The source's own results kind by kind, for its tab, each with Show More's cursor.
    #[serde(default)]
    pub groups: Vec<SearchGroup>,
    pub error: Option<String>,
    /// True on the last message for this request.
    pub done: bool,
}

/// One kind of a source's search results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct SearchGroup {
    pub kind: ItemKind,
    pub items: Vec<SourceItem>,
    /// `SearchMore { plugin: source, kind, cursor: next }` loads more; `None` when there is no
    /// more.
    pub next: Option<String>,
    pub total: Option<u32>,
}

/// More of one kind of a source's results (`Command::SearchMore`): add `items` to that group
/// and keep `next` as its cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct SearchPage {
    /// The id `send` returned for the `SearchMore` command.
    pub request: u64,
    pub query: String,
    pub source: String,
    pub kind: ItemKind,
    pub items: Vec<SourceItem>,
    pub next: Option<String>,
    pub error: Option<String>,
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
    /// A source track with a finished download (it plays the file): the Downloaded mark.
    #[serde(default)]
    pub downloaded: bool,
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
    /// Bits per sample of the source (PCM, FLAC, ALAC); none for lossy codecs.
    #[serde(default)]
    pub bits_per_sample: Option<u32>,
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
    /// What is playing, as decoded: codec, sample rate, bit depth, channels, bitrate and the
    /// quality label, once the engine has opened it. The facts of the quality menu's format line.
    #[serde(default)]
    pub quality: Option<AudioFormat>,
    /// The levels the quality menu offers, in its order: `Auto`, then the source's levels best
    /// first. Empty when there is no choice (`quality_note` says why).
    #[serde(default)]
    pub qualities: Vec<StreamingQuality>,
    /// Why `qualities` is empty.
    #[serde(default)]
    pub quality_note: Option<QualityNote>,
    /// The level the stream was asked for (`Auto` resolved); `None` for files.
    #[serde(default)]
    pub quality_level: Option<StreamingQuality>,
}

/// Why the quality menu offers no levels; the apps word it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum QualityNote {
    /// "Files play as they are." (library songs, files, downloads)
    FilesPlayAsTheyAre,
    /// "Quality is set in {service}." (remote playback, sources that offer no levels)
    SetInService { service: String },
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
    /// Made by AI Lyrics: "Made by AI Lyrics · {engine}".
    Made { plugin: String, engine: String },
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
    /// The quality menu's note while a session runs ("Quality is set in {service}.").
    #[serde(default)]
    pub quality_note: Option<QualityNote>,
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
    pub ai_lyrics: AiLyricsSnapshot,
    pub look: LookSnapshot,
    pub pro: ProSnapshot,
    pub update: UpdateSnapshot,
    /// The sources' likes and playlists (ADR-0024).
    pub source_library: SourceLibrarySnapshot,
    /// Downloads (ADR-0025).
    pub downloads: DownloadsSnapshot,
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
    AiLyrics {
        snapshot: AiLyricsSnapshot,
    },
    Look {
        snapshot: LookSnapshot,
    },
    Pro {
        snapshot: ProSnapshot,
    },
    Update {
        snapshot: UpdateSnapshot,
    },
    /// A Discover page (`Command::Discover`).
    Discover {
        page: DiscoverPage,
    },
    /// An item's details (`Command::Details`).
    Details {
        page: DetailsPage,
    },
    /// A page of comments (`Command::Comments`).
    Comments {
        page: CommentsPage,
    },
    /// A source picture asked for with `source_artwork` is cached (`path`), or cannot be had.
    Artwork {
        ready: ArtworkReady,
    },
    /// More of one kind of a source's search results (`Command::SearchMore`).
    SearchPage {
        page: SearchPage,
    },
    /// The `SourceLibrary` slice changed.
    SourceLibrary {
        snapshot: SourceLibrarySnapshot,
    },
    /// More of a source's list (`Command::SourceLibraryMore`).
    SourceLibraryPage {
        page: SourceLibraryPage,
    },
    /// The `Downloads` slice changed.
    Downloads {
        snapshot: DownloadsSnapshot,
    },
}

// ---- v0.0.8: source libraries and downloads -----------------------------------------------------

/// One of a source's lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum SourceLibraryPart {
    LikedSongs,
    LikedAlbums,
    LikedArtists,
    Playlists,
}

/// An entry of a source's list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct SourceLibraryEntry {
    pub item: SourceItem,
    /// Playlists: the person's own (or collaborative), not followed.
    pub owned: bool,
    /// Playlists: the source lists its songs here; when false its page says "{Service} lists this
    /// playlist's songs only in {Service}." and Play starts it by context. True for likes.
    pub songs_listed: bool,
}

/// One list of a source: the first page (at most 100), kept on disk with its age.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct SourceLibraryList {
    pub part: SourceLibraryPart,
    pub entries: Vec<SourceLibraryEntry>,
    pub total: Option<u32>,
    /// `SourceLibraryMore { plugin, part, cursor: next }` loads more.
    pub next: Option<String>,
    /// When it was last loaded, ms since the Unix epoch ("Updated 5 minutes ago").
    pub refreshed_at_ms: Option<i64>,
    pub loading: bool,
    pub error: Option<String>,
}

/// A source's likes and playlists; lists it does not keep are left out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct SourceLibrary {
    pub plugin: String,
    pub name: String,
    /// Attribution its views must show, if any.
    pub attribution: Option<String>,
    /// Liked Songs, Liked Albums, Liked Artists, Playlists, in that order.
    pub lists: Vec<SourceLibraryList>,
}

/// The `SourceLibrary` slice: every enabled, signed-in source with the `library` capability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct SourceLibrarySnapshot {
    pub version: u64,
    pub sources: Vec<SourceLibrary>,
}

/// More of a source's list (`Command::SourceLibraryMore`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct SourceLibraryPage {
    /// The id `send` returned for the `SourceLibraryMore` command.
    pub request: u64,
    pub plugin: String,
    pub part: SourceLibraryPart,
    pub entries: Vec<SourceLibraryEntry>,
    pub next: Option<String>,
    pub error: Option<String>,
}

/// Why downloads wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum DownloadsPaused {
    /// No network.
    Offline,
    /// Metered network with Wi-Fi Only on ("Waiting for Wi-Fi").
    Metered,
}

/// Why a download failed; the apps word it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum DownloadError {
    /// "Your {service} server doesn't allow downloads for this account."
    NotAllowed {
        service: String,
    },
    /// The source offers no downloads.
    NotOffered {
        service: String,
    },
    /// The connection failed after the retries.
    Network {
        message: String,
    },
    /// The file could not be written.
    Storage {
        message: String,
    },
    Other {
        message: String,
    },
}

/// Where a download is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum DownloadState {
    /// Waiting for its turn, or for the network (`DownloadsSnapshot.paused`).
    Queued,
    Downloading,
    Done,
    /// Try Again sends `RetryDownload`.
    Failed {
        error: DownloadError,
    },
}

/// One download.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct DownloadItem {
    pub id: u64,
    pub track: SourceTrack,
    pub state: DownloadState,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    /// The file, once finished.
    pub path: Option<String>,
    /// Its library track, once finished.
    pub library_id: Option<i64>,
    /// The server's original rather than a conversion.
    pub original: bool,
}

/// The `Downloads` slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct DownloadsSnapshot {
    pub version: u64,
    /// Where downloads go: the person's folder, or the default one.
    pub folder: String,
    /// In progress (queue order), then finished by album.
    pub items: Vec<DownloadItem>,
    /// Space the finished downloads take, in bytes.
    pub bytes_used: u64,
    pub paused: Option<DownloadsPaused>,
}

// ---- AI Lyrics (ADR-0019) -----------------------------------------------------------------------

/// Where AI Lyrics recognises the words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum AiLyricsMode {
    /// Whisper in the app; nothing leaves the device.
    OnDevice,
    /// The person's own provider, with their API key, after consent.
    Online,
}

/// What making lyrics would do now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AiLyricsPlan {
    pub mode: AiLyricsMode,
    /// The engine, in the plugin's words: "Whisper Base on this device".
    pub engine: String,
    /// The origin the online mode sends audio to.
    pub upload_to: Option<String>,
    /// Lyrics are made for each song that starts playing without synced lyrics.
    pub automatic: bool,
}

/// What must happen before lyrics can be made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum AiLyricsNeed {
    /// Ready: `MakeLyrics` works.
    Nothing,
    /// Jellyfesh Pro is inactive: show the Pro sheet.
    Pro,
    /// The online mode needs the person's API key: open AI Lyrics settings.
    SignIn,
    /// Ask "Send This Song to {origin}?" and answer with `AllowAudioUpload`.
    Consent { origin: String },
    /// Download this model first.
    Model { id: String, name: String, bytes: u64 },
    /// Making lyrics cannot work as set up.
    Unavailable { reason: String },
}

/// Where a lyrics job is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum LyricsJobStage {
    Waiting,
    ReadingAudio,
    Sending {
        origin: String,
    },
    /// `fraction` 0–1.
    Recognising {
        fraction: f32,
    },
    TimingWords,
    Done,
    Failed {
        message: String,
    },
    Cancelled,
}

/// A lyrics job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum SpeechModelStatus {
    NotDownloaded,
    /// `fraction` 0–1.
    Downloading {
        fraction: f32,
    },
    Ready,
    Failed {
        message: String,
    },
}

/// An on-device speech model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct SpeechModelState {
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub state: SpeechModelStatus,
}

/// An origin the person allowed a plugin to send audio to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Record)]
pub struct AudioConsent {
    pub plugin: String,
    pub origin: String,
}

/// The `AiLyrics` slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct AiLyricsSnapshot {
    pub version: u64,
    /// Offer Create Synced Lyrics (Pro may still be needed: see `need`).
    pub available: bool,
    pub plugin: Option<String>,
    pub plan: Option<AiLyricsPlan>,
    pub need: AiLyricsNeed,
    pub jobs: Vec<LyricsJob>,
    pub models: Vec<SpeechModelState>,
    pub consents: Vec<AudioConsent>,
}

// ---- the look (ADR-0020) ------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum Typeface {
    System,
    Rounded,
    Serif,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum Density {
    Compact,
    Standard,
    Roomy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum LyricsWeight {
    Regular,
    Medium,
    Semibold,
    Bold,
    Heavy,
}

/// The colour of the sung part of lyrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum LyricsHighlight {
    Artwork,
    Accent,
    /// `0xRRGGBB`.
    Custom {
        rgb: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum HomeSection {
    Summary,
    RecentlyAdded,
    RecentlyPlayed,
    MostPlayed,
    Favourites,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum NowPlayingLayout {
    Stage,
    Artwork,
    Lyrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum MiniPlayerStyle {
    Bar,
    Compact,
    Floating,
}

/// The `Look` slice: every value resolved (ADR-0020 §2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct LookSnapshot {
    pub version: u64,
    pub corner_scale: f32,
    pub glass: f32,
    pub aurora: f32,
    pub typeface: Typeface,
    pub text_scale: f32,
    pub density: Density,
    pub lyrics_weight: LyricsWeight,
    pub lyrics_glow: f32,
    pub lyrics_highlight: LyricsHighlight,
    pub sidebar: Vec<Place>,
    pub home: Vec<HomeSection>,
    pub now_playing: NowPlayingLayout,
    pub mini_player: MiniPlayerStyle,
    /// An appearance plugin sets the look; false: every value is the Luminous default.
    pub customized: bool,
    pub plugin: Option<String>,
}

// ---- Jellyfesh Pro (ADR-0021) -------------------------------------------------------------------

/// Where Pro comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum ProSource {
    None,
    /// "Pro features are free during the public beta."
    Preview,
    /// "Jellyfesh Pro · Purchased".
    Store {
        store: Store,
    },
    /// "Jellyfesh Pro · Licence …ABCD".
    Licence {
        id_suffix: String,
        issued_at_ms: i64,
    },
}

/// Why a licence key was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum LicenceError {
    Malformed,
    UnknownKey,
    BadSignature,
    WrongProduct,
    Revoked,
}

/// The `Pro` slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ProSnapshot {
    pub version: u64,
    pub active: bool,
    pub source: ProSource,
    pub preview: bool,
    pub licence_error: Option<LicenceError>,
    /// The stores this build can sell Pro in.
    pub stores: Vec<Store>,
    /// Licence keys work in this build: show Enter Licence Key… (false in App Store and
    /// Google Play builds).
    #[serde(default = "yes")]
    pub licence_keys: bool,
}

// ---- updates (ADR-0021 §5) ----------------------------------------------------------------------

/// A newer release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct UpdateInfo {
    pub version: String,
    /// Its release page.
    pub url: String,
}

/// The `Update` slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct UpdateSnapshot {
    pub version: u64,
    /// Show Check for Updates and its setting (direct downloads).
    pub supported: bool,
    pub checking: bool,
    pub available: Option<UpdateInfo>,
    pub last_checked_ms: Option<i64>,
    pub auto: bool,
    pub error: Option<String>,
}

// ---- Discover, details and comments (ADR-0022) --------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum ShelfStyle {
    Row,
    Grid,
    Chart,
}

/// One shelf of a Discover page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct Shelf {
    pub id: String,
    pub title: String,
    pub style: ShelfStyle,
    pub items: Vec<SourceItem>,
    /// A node to browse for See All.
    pub more: Option<String>,
}

/// A Discover page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct DiscoverPage {
    pub request: u64,
    pub source: String,
    pub shelves: Vec<Shelf>,
    pub error: Option<String>,
}

/// What a fact is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "type")]
pub enum FactKey {
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
    Isrc,
    Disc,
    Track,
    Other { label: String },
}

/// One fact about an item.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Record)]
pub struct Fact {
    pub key: FactKey,
    pub value: String,
}

/// How good what plays is, as the core judges it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum QualityLabel {
    HiResLossless,
    Lossless,
    High,
    Standard,
}

/// The format of what plays or of a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AudioFormat {
    pub codec: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub bit_depth: Option<u32>,
    pub channels: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub lossless: Option<bool>,
    pub quality: Option<QualityLabel>,
}

/// ReplayGain values (gains in dB, peaks as linear amplitude).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct ReplayGainValues {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

/// Technical information about a file or a stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct TechnicalInfo {
    pub format: AudioFormat,
    pub container: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub replaygain: Option<ReplayGainValues>,
    pub path: Option<String>,
}

/// An item's details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct DetailsPage {
    pub request: u64,
    pub item: ItemRef,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub image: Option<ArtworkRef>,
    pub facts: Vec<Fact>,
    pub technical: Option<TechnicalInfo>,
    pub comment_count: Option<u64>,
    pub attribution: Option<String>,
    pub error: Option<String>,
}

/// A comment on a source item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct CommentEntry {
    pub id: String,
    pub author: String,
    pub avatar: Option<ArtworkRef>,
    pub text: String,
    pub likes: Option<u64>,
    pub time_ms: Option<i64>,
    pub replies: Option<u64>,
}

/// A page of comments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct CommentsPage {
    pub request: u64,
    pub item: ItemRef,
    pub hot: Vec<CommentEntry>,
    pub comments: Vec<CommentEntry>,
    pub total: Option<u64>,
    pub next: Option<String>,
    pub attribution: Option<String>,
    pub error: Option<String>,
}

/// A source picture fetched into the artwork cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ArtworkReady {
    pub url: String,
    pub size_px: u32,
    pub path: Option<String>,
}

/// What the lyrics bar shows at a time (`lyrics_bar`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct LyricsBar {
    pub line: Option<u32>,
    pub next: Option<u32>,
    pub words: Vec<f32>,
    pub line_progress: f32,
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

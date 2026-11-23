//! Data types of the plugin API.
//!
//! The first part mirrors the records, enums and variants of `plugins/wit`
//! (`jellyfesh:plugin@1.0.0`) as plain Rust: the same names, fields and
//! `serde` shapes that wasmtime's `bindgen!` generates in `jf-plugin-host`,
//! which converts between the two at the runtime boundary. Keeping them here,
//! without wasmtime, lets other hosts (Jellyfesh Web, `core/jf-web`) share
//! the merge, the answer checks and the login state machine. The second part
//! describes installed plugins and login steps from the host's side.
//!
//! When the WIT changes, change the mirror with it; `jf-plugin-host`'s
//! conversions (`src/convert.rs`) stop compiling until both agree.

use serde::{Deserialize, Serialize};

// ---- types (interface `types`) ---------------------------------------------------------------

/// An HTTP header, or any other name/value pair the host passes through.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

/// What an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    Track,
    Album,
    Artist,
    Playlist,
    Folder,
}

/// Why the service will not play an item for this account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnplayableReason {
    Paywall,
    Region,
    Unavailable,
    Other,
}

/// The reason attached to an item that is not playable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Unplayable {
    pub reason: UnplayableReason,
    pub message: Option<String>,
}

/// A picture the host may download.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageRef {
    pub url: String,
    pub headers: Vec<Header>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// One entry in search results, browse listings and recommendations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub kind: ItemKind,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    pub duration_ms: Option<u32>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<u32>,
    pub child_count: Option<u32>,
    pub isrc: Option<String>,
    pub artwork: Option<ImageRef>,
    pub explicit: bool,
    pub playable: bool,
    pub unplayable: Option<Unplayable>,
}

/// Which slice of a list the host wants.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageRequest {
    pub cursor: Option<String>,
    pub limit: u32,
}

/// One page of items.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemPage {
    pub items: Vec<Item>,
    pub next: Option<String>,
    pub total: Option<u32>,
}

/// Why a plugin export failed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginError {
    Unsupported,
    AuthRequired,
    NotFound,
    InvalidInput(String),
    RateLimited,
    Network(String),
    Service(String),
    Internal(String),
}

// ---- meta ------------------------------------------------------------------------------------

/// Something a plugin can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    Auth,
    Search,
    Browse,
    ResolveStream,
    Lyrics,
    Artwork,
    Recommendations,
    /// Remote control of the service's own player (API 1.1, `remote`). A
    /// manifest capability only: the WIT `capability` enum of 1.0 is unchanged.
    Remote,
    /// Making synced lyrics (API 1.2, the `maker` export). A Pro capability.
    MakeLyrics,
    /// The app's look and layout (API 1.2, the `appearance` export). A Pro
    /// capability.
    Appearance,
    /// A Discover page of shelves (API 1.2, the `discover` export).
    Discover,
    /// More about an item: notes, biography, facts (API 1.2, `details`).
    Details,
    /// What people say about an item (API 1.2, `comments`).
    Comments,
    /// The person's likes and playlists on the service (API 1.3, the
    /// `library` export).
    Library,
    /// Files from the person's own server (API 1.3, the `download` export).
    Download,
}

impl Capability {
    /// Whether the capability is part of Jellyfesh Pro (ADR-0021): the host
    /// refuses its calls (`CallError::ProRequired`) while Pro is inactive.
    pub fn requires_pro(self) -> bool {
        matches!(self, Capability::MakeLyrics | Capability::Appearance)
    }

    /// Whether the capability exists only in the manifest: the 1.0 `meta`
    /// enum cannot report it, so the host lists it whenever it is declared.
    pub fn manifest_only(self) -> bool {
        matches!(
            self,
            Capability::Remote
                | Capability::MakeLyrics
                | Capability::Appearance
                | Capability::Discover
                | Capability::Details
                | Capability::Comments
                | Capability::Library
                | Capability::Download
        )
    }

    /// Whether the capability makes a plugin a source of music (as opposed
    /// to an extension such as AI Lyrics or Customize, ADR-0020).
    pub fn is_source(self) -> bool {
        !matches!(
            self,
            Capability::Auth | Capability::MakeLyrics | Capability::Appearance
        )
    }
}

/// Whether a plugin with these capabilities needs Jellyfesh Pro for any of
/// them.
pub fn requires_pro(capabilities: &[Capability]) -> bool {
    capabilities.iter().any(|c| c.requires_pro())
}

/// Whether a plugin with these capabilities is an extension (ADR-0020): it
/// makes lyrics or changes the look, and offers no music (Settings lists it
/// under Extensions, not Sources).
pub fn is_extension(capabilities: &[Capability]) -> bool {
    capabilities
        .iter()
        .any(|c| matches!(c, Capability::MakeLyrics | Capability::Appearance))
        && !capabilities.iter().any(|c| c.is_source())
}

/// A host import a plugin must declare in its manifest's `permissions`
/// (API 1.2); undeclared, the import answers `not-declared`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// The song's audio during `maker.make` (`audio`).
    Audio,
    /// Speech recognition in the app (`speech`).
    Speech,
}

/// The manifest name of a permission.
pub fn permission_name(permission: Permission) -> &'static str {
    match permission {
        Permission::Audio => "audio",
        Permission::Speech => "speech",
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(permission_name(*self))
    }
}

// ---- auth ------------------------------------------------------------------------------------

/// The ways to log in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthKind {
    Qr,
    Oauth,
    CookieImport,
    PasswordExchange,
    ApiKey,
}

/// What the host prepared for an `oauth` flow.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OauthRequest {
    pub redirect_uri: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub state: String,
}

/// A login flow in progress, as the host passes it to every call.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Flow {
    pub id: String,
    pub kind: AuthKind,
    pub state: String,
    pub oauth: Option<OauthRequest>,
}

/// How a field should be entered and shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldKind {
    Text,
    Email,
    Url,
    Number,
    CookieHeader,
}

/// One input the person fills in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldSpec {
    pub name: String,
    pub label: String,
    pub kind: FieldKind,
    pub secret: bool,
    pub required: bool,
    pub placeholder: Option<String>,
    pub help: Option<String>,
}

/// A submitted value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldValue {
    pub name: String,
    pub value: String,
}

/// A QR code for the person to scan with the service's app.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QrCode {
    pub data: String,
    pub hint: Option<String>,
    pub expires_at_ms: Option<u64>,
}

/// A short code for the person to enter on another device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodeDisplay {
    pub code: String,
    pub hint: String,
    pub expires_at_ms: Option<u64>,
}

/// How the host learns that the browser part of a flow is finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RedirectCapture {
    None,
    Loopback,
}

/// A web page to open in the system browser.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OpenUrl {
    pub url: String,
    pub capture: RedirectCapture,
}

/// What the UI shows next, as the plugin says it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Step {
    ShowQr(QrCode),
    ShowCode(CodeDisplay),
    OpenUrl(OpenUrl),
    AskFields(Vec<FieldSpec>),
    Waiting(String),
    Done(String),
    Failed(String),
}

/// A step plus the plugin's bookkeeping for it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowStep {
    pub step: Step,
    pub state: String,
    pub poll_after_ms: Option<u32>,
}

// ---- catalog, playback, lyrics ----------------------------------------------------------------

/// Narrows a search.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SearchFilters {
    pub kinds: Vec<ItemKind>,
}

/// The quality the person chose for this network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quality {
    Low,
    Normal,
    High,
    Lossless,
}

/// Where and how the engine fetches the audio.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StreamInfo {
    pub url: String,
    pub headers: Vec<Header>,
    pub expires_at_ms: Option<u64>,
    pub codec: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub cacheable: bool,
}

/// Format of `LyricPayload::text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LyricFormat {
    Plain,
    Lrc,
    Elrc,
    Ttml,
    Vtt,
    JsonSyllables,
}

/// Lyrics as the service provides them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LyricPayload {
    pub format: LyricFormat,
    pub text: String,
    pub language: Option<String>,
    pub attribution: Option<String>,
}

// ---- crypto (interface `crypto`) --------------------------------------------------------------

/// Hash function for `digest` and `hmac`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HashAlgorithm {
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

/// Parameters for AES-GCM.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GcmParams {
    pub nonce: Vec<u8>,
    pub aad: Vec<u8>,
}

/// AES mode; the key length picks AES-128, -192 or -256.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AesMode {
    /// CBC with PKCS#7 padding; the payload is the 16-byte IV.
    Cbc(Vec<u8>),
    /// GCM; the 16-byte tag follows the ciphertext.
    Gcm(GcmParams),
    /// ECB with PKCS#7 padding. Insecure; only for services that need it.
    Ecb,
}

/// RSA encryption padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RsaPadding {
    Pkcs1v15,
    OaepSha1,
    OaepSha256,
}

/// The raw numbers of an RSA public key, big-endian.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RsaComponents {
    pub modulus: Vec<u8>,
    pub exponent: Vec<u8>,
}

/// An RSA public key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RsaPublicKey {
    Pem(String),
    Der(Vec<u8>),
    Components(RsaComponents),
}

/// Why a crypto call failed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CryptoError {
    InvalidKey(String),
    InvalidInput(String),
    DecryptFailed,
    TooLarge,
}

// ---- audio and speech (API 1.2) --------------------------------------------------------------

/// Why a plugin gets no audio (`audio.audio-error`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioError {
    NotDeclared,
    NoConsent(Vec<String>),
    Unavailable(String),
    Cancelled,
}

/// A recognition model the app offers (`speech.model`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpeechModel {
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub ready: bool,
}

/// How to recognise (`speech.options`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpeechOptions {
    pub model: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
}

/// A recognised word (`speech.word`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeechWord {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub confidence: f32,
}

/// A recognised stretch of speech (`speech.segment`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeechSegment {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub words: Vec<SpeechWord>,
}

/// Everything recognised in a song (`speech.transcript`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub language: Option<String>,
    pub segments: Vec<SpeechSegment>,
}

/// Why there is no transcript (`speech.speech-error`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpeechError {
    NotDeclared,
    ModelMissing(String),
    Unavailable(String),
    Cancelled,
    Failed(String),
}

// ---- maker (API 1.2) -------------------------------------------------------------------------

/// Where recognition happens (`maker.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MakerMode {
    OnDevice,
    Online,
}

/// What making lyrics needs first (`maker.need`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MakerNeed {
    Nothing,
    SignIn,
    Model(String),
    Unavailable(String),
}

/// What `maker.make` would do now (`maker.plan`), checked: `upload_to` is
/// an origin (`https://host[:port]`) on the plugin's allowlist.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MakerPlan {
    pub mode: MakerMode,
    pub engine: String,
    pub upload_to: Option<String>,
    pub need: MakerNeed,
    pub automatic: bool,
}

/// The song the host asks a maker about (`maker.song` without its track
/// handle, which the host assigns per call).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MakeSong {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: u64,
    /// Plain lyrics to time.
    pub text: Option<String>,
    pub language: Option<String>,
}

/// Lyrics a plugin made (`maker.made`), checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Made {
    pub lyrics: LyricPayload,
    pub engine: String,
}

// ---- appearance (API 1.2) ----------------------------------------------------------------------

/// `appearance.typeface`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Typeface {
    System,
    Rounded,
    Serif,
    Mono,
}

/// `appearance.density`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Density {
    Compact,
    Standard,
    Roomy,
}

/// `appearance.lyrics-weight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LyricsWeight {
    Regular,
    Medium,
    Semibold,
    Bold,
    Heavy,
}

/// `appearance.highlight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Highlight {
    Artwork,
    Accent,
    /// 0xRRGGBB.
    Custom(u32),
}

/// `appearance.place`: a place in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl Place {
    /// Every place, in the docs/ui.md order (the default sidebar).
    pub const ALL: [Place; 8] = [
        Place::Home,
        Place::Albums,
        Place::Artists,
        Place::Songs,
        Place::Playlists,
        Place::Favourites,
        Place::Search,
        Place::Sources,
    ];

    /// Whether the place shows the person's library.
    pub fn is_library(self) -> bool {
        matches!(
            self,
            Place::Albums | Place::Artists | Place::Songs | Place::Playlists | Place::Favourites
        )
    }
}

/// `appearance.home-section`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HomeSection {
    Summary,
    RecentlyAdded,
    RecentlyPlayed,
    MostPlayed,
    Favourites,
}

impl HomeSection {
    /// Home's sections by default, in order.
    pub const DEFAULT: [HomeSection; 4] = [
        HomeSection::Summary,
        HomeSection::RecentlyAdded,
        HomeSection::RecentlyPlayed,
        HomeSection::MostPlayed,
    ];
}

/// `appearance.now-playing-layout`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NowPlayingLayout {
    Stage,
    Artwork,
    Lyrics,
}

/// `appearance.mini-player-style`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MiniPlayerStyle {
    Bar,
    Compact,
    Floating,
}

/// `appearance.look`: what an appearance plugin sets; `None` keeps the
/// default.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Look {
    pub corner_scale: Option<f32>,
    pub glass: Option<f32>,
    pub aurora: Option<f32>,
    pub typeface: Option<Typeface>,
    pub text_scale: Option<f32>,
    pub density: Option<Density>,
    pub lyrics_weight: Option<LyricsWeight>,
    pub lyrics_glow: Option<f32>,
    pub lyrics_highlight: Option<Highlight>,
    pub sidebar: Option<Vec<Place>>,
    pub home: Option<Vec<HomeSection>>,
    pub now_playing: Option<NowPlayingLayout>,
    pub mini_player: Option<MiniPlayerStyle>,
}

// ---- discover, details, comments (API 1.2) -------------------------------------------------------

/// `discover.shelf-style`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShelfStyle {
    Row,
    Grid,
    Chart,
}

/// `discover.shelf`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Shelf {
    pub id: String,
    pub title: String,
    pub style: ShelfStyle,
    pub items: Vec<Item>,
    pub more: Option<String>,
}

/// `details.fact-key`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    Other(String),
}

/// `details.fact`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fact {
    pub key: FactKey,
    pub value: String,
}

/// `details.item-details`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemDetails {
    pub description: Option<String>,
    pub image: Option<ImageRef>,
    pub facts: Vec<Fact>,
    pub comment_count: Option<u64>,
}

/// `comments.comment`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub avatar: Option<ImageRef>,
    pub text: String,
    pub likes: Option<u64>,
    pub time_ms: Option<u64>,
    pub replies: Option<u64>,
}

/// `comments.comment-page`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommentPage {
    pub hot: Vec<Comment>,
    pub comments: Vec<Comment>,
    pub total: Option<u64>,
    pub next: Option<String>,
}

// ---- library and download (API 1.3) -------------------------------------------------------------

/// `library.playlist-entry`: a playlist in the person's library.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlaylistEntry {
    pub item: Item,
    /// The person's own playlist (or one they collaborate on).
    pub owned: bool,
    /// `catalog.browse` lists its songs; false for a playlist the service
    /// lists only in its own app.
    pub songs_listed: bool,
}

/// `library.playlist-page`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlaylistPage {
    pub entries: Vec<PlaylistEntry>,
    pub next: Option<String>,
    pub total: Option<u32>,
}

/// `download.download-quality`: the quality a download is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DownloadQuality {
    Original,
    High,
    Normal,
    Low,
}

/// `download.download-info`: where the host fetches a file, checked (the
/// URL is on the plugin's allowlist, `container` a short lowercase
/// extension, `file_name` a bare name).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub url: String,
    pub headers: Vec<Header>,
    pub container: String,
    pub size: Option<u64>,
    pub original: bool,
    pub file_name: Option<String>,
}

// ---- the host's side -------------------------------------------------------------------------

// ---- remote control (API 1.1) -------------------------------------------------------------------

/// What kind of device a remote source plays on (`remote.device-kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteDeviceKind {
    Computer,
    Smartphone,
    Speaker,
    Tv,
    Car,
    Other,
}

/// A device a remote source can play on (`remote.device`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    pub kind: RemoteDeviceKind,
    pub active: bool,
    pub supports_volume: bool,
    pub volume_percent: Option<u32>,
}

/// The repeat mode of a remote player (`remote.repeat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteRepeat {
    Off,
    All,
    One,
}

/// What a remote player is doing (`remote.state`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteState {
    pub device: Option<RemoteDevice>,
    pub item: Option<Item>,
    pub context: Option<Item>,
    pub playing: bool,
    pub position_ms: u32,
    pub shuffle: bool,
    pub repeat: RemoteRepeat,
}

/// How a source's audio reaches the speakers (ADR-0004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaybackMode {
    /// The plugin resolves stream URLs and our engine plays them.
    Stream,
    /// The vendor's SDK plays the audio (no EQ, crossfade or visualiser).
    Vendor,
    /// The plugin remote-controls another device.
    Remote,
}

/// What the manifest's `[content_policy]` asks of the host (ADR-0011). The
/// defaults are permissive, which suits self-hosted servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentPolicy {
    /// Results may be merged with other sources in unified search.
    pub unified_search: bool,
    /// Items may share a queue with other sources.
    pub mixed_queue: bool,
    /// What the library may keep from this source.
    pub library_storage: LibraryStorage,
    /// How long metadata may be cached, in seconds; `None` means no limit.
    pub metadata_ttl_secs: Option<u64>,
    /// Synced lyrics may be shown while this source plays.
    pub lyrics_sync: bool,
    /// Attribution the views must show, if any (for example `"spotify"`).
    pub attribution: Option<String>,
    /// The streaming levels `resolve-stream` honours (API 1.3), best first;
    /// empty when the plugin offers no choice (the apps then say the quality
    /// is set in the service).
    #[serde(default)]
    pub qualities: Vec<Quality>,
}

impl Default for ContentPolicy {
    fn default() -> Self {
        ContentPolicy {
            unified_search: true,
            mixed_queue: true,
            library_storage: LibraryStorage::Full,
            metadata_ttl_secs: None,
            lyrics_sync: true,
            attribution: None,
            qualities: Vec::new(),
        }
    }
}

/// What the library database may keep from a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LibraryStorage {
    /// Nothing.
    None,
    /// Metadata, but no audio.
    Metadata,
    /// Metadata and cached audio.
    Full,
}

/// Who signed a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureStatus {
    /// No `SIGNATURE` file.
    Unsigned,
    /// Signed by a key in the host's trust list: the package gets the
    /// **Official** badge.
    Trusted {
        /// Key id: the first 8 bytes of the SHA-256 of the public key, in hex.
        key_id: String,
        /// The trust list's name for the key.
        label: String,
    },
    /// A valid signature by a key the host does not know (a community
    /// author). The key is remembered on first install; a later package with
    /// another key is refused until the plugin is uninstalled.
    Community {
        /// Key id, shown to the person as the author's fingerprint.
        key_id: String,
    },
}

/// An installed plugin as the UI shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Reverse-DNS id from the manifest.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Plugin version (semver).
    pub version: String,
    /// Author as the manifest states it.
    pub author: String,
    /// One-line description, if the manifest has one.
    pub description: Option<String>,
    /// True only when a key in the trust list signed the package; the
    /// manifest's own `official` flag is not trusted.
    pub official: bool,
    /// Signature details.
    pub signature: SignatureStatus,
    /// `api_version` from the manifest (`"1.0"`).
    pub api_version: String,
    /// Capabilities the manifest declares; the host calls nothing else.
    pub capabilities: Vec<Capability>,
    /// How the plugin's audio plays.
    pub playback_mode: PlaybackMode,
    /// Login kinds the manifest lists.
    pub auth_kinds: Vec<AuthKind>,
    /// Whether calls are allowed.
    pub enabled: bool,
    /// Why the host turned the plugin off (after repeated limit violations),
    /// if it did.
    pub disabled_reason: Option<String>,
    /// Display name of the logged-in account, if any.
    pub account: Option<String>,
    /// The settings schema (JSON Schema subset, see [`crate::schema`]); `{}`
    /// when the package has none.
    pub settings_schema: serde_json::Value,
    /// The same schema as the package wrote it, with the properties in the
    /// plugin's order (the order settings forms show them in).
    #[serde(default)]
    pub settings_schema_text: String,
    /// Current settings values, defaults filled in.
    pub settings: serde_json::Value,
    /// `[content_policy]` from the manifest.
    pub content_policy: ContentPolicy,
    /// `[links]` from the manifest (API 1.1): templates that open an item in
    /// the service.
    #[serde(default)]
    pub links: crate::links::Links,
    /// Whether the package has an icon.
    pub has_icon: bool,
    /// The host imports the manifest declares (API 1.2): `audio`, `speech`.
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Languages the package translates its settings into (`l10n/<tag>.json`,
    /// API 1.2), such as `zh-Hans`.
    #[serde(default)]
    pub languages: Vec<String>,
    /// The settings whose values are servers the plugin may reach: the keys
    /// its allowlist (and cookie import) name as `${settings.<key>}`.
    #[serde(default)]
    pub server_settings: Vec<String>,
}

/// One step of a login flow, for the UI to render natively.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthStep {
    /// Show a QR code; the host polls the plugin meanwhile.
    ShowQr {
        /// Payload to encode.
        data: String,
        /// One sentence for the person.
        hint: Option<String>,
        /// When the code stops working (ms since the epoch).
        expires_at_ms: Option<u64>,
    },
    /// Show a short code to enter on another device; the host polls meanwhile.
    ShowCode {
        /// The code.
        code: String,
        /// Where to enter it.
        hint: String,
        /// When the code stops working (ms since the epoch).
        expires_at_ms: Option<u64>,
    },
    /// Open this URL in the system browser. For OAuth the host is already
    /// listening for the redirect.
    OpenUrl {
        /// The page to open.
        url: String,
    },
    /// Show a form; answer with the host's submit.
    AskFields {
        /// The inputs, in order.
        fields: Vec<FieldSpec>,
    },
    /// Show progress text.
    Waiting {
        /// The text.
        message: String,
    },
    /// Logged in. The flow is over.
    Done {
        /// The account's display name.
        account: String,
    },
    /// The flow ended without a login.
    Failed {
        /// Why, for the person.
        message: String,
    },
}

impl AuthStep {
    /// Whether the flow is over after this step.
    pub fn is_final(&self) -> bool {
        matches!(self, AuthStep::Done { .. } | AuthStep::Failed { .. })
    }
}

/// The manifest name of a capability (`resolve_stream`, …).
pub fn capability_name(capability: Capability) -> &'static str {
    match capability {
        Capability::Auth => "auth",
        Capability::Search => "search",
        Capability::Browse => "browse",
        Capability::ResolveStream => "resolve_stream",
        Capability::Lyrics => "lyrics",
        Capability::Artwork => "artwork",
        Capability::Recommendations => "recommendations",
        Capability::Remote => "remote",
        Capability::MakeLyrics => "make_lyrics",
        Capability::Appearance => "appearance",
        Capability::Discover => "discover",
        Capability::Details => "details",
        Capability::Comments => "comments",
        Capability::Library => "library",
        Capability::Download => "download",
    }
}

/// The manifest name of a login kind (`password-exchange`, …).
pub fn auth_kind_name(kind: AuthKind) -> &'static str {
    match kind {
        AuthKind::Qr => "qr",
        AuthKind::Oauth => "oauth",
        AuthKind::CookieImport => "cookie-import",
        AuthKind::PasswordExchange => "password-exchange",
        AuthKind::ApiKey => "api-key",
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(capability_name(*self))
    }
}

impl std::fmt::Display for AuthKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(auth_kind_name(*self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_shapes_match_the_generated_types() {
        // The core and the FFI hand these to UIs as JSON: unit variants as
        // names, payload variants as one-key maps, fields in snake_case.
        assert_eq!(serde_json::to_string(&ItemKind::Track).unwrap(), "\"Track\"");
        assert_eq!(
            serde_json::to_string(&PluginError::InvalidInput("x".into())).unwrap(),
            "{\"InvalidInput\":\"x\"}"
        );
        let page = PageRequest {
            cursor: None,
            limit: 5,
        };
        assert_eq!(
            serde_json::to_string(&page).unwrap(),
            "{\"cursor\":null,\"limit\":5}"
        );
        assert_eq!(capability_name(Capability::ResolveStream), "resolve_stream");
        assert_eq!(AuthKind::PasswordExchange.to_string(), "password-exchange");
    }
}

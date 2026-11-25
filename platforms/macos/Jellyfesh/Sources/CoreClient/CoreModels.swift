// The app's own copies of the core's types (docs/core-api.md, core/jf-ffi/src/types.rs).
//
// Views and stores use only these; FFIConversions.swift maps the generated JellyfeshFFI types onto
// them, so a renamed generated type or field is fixed in this folder alone. They carry data, never
// behaviour: sorting, filtering, matching and persistence stay in the core (ADR-0001).

import Foundation

typealias CommandID = UInt64

// MARK: - References

/// What a queue entry plays (`TrackRef`).
enum TrackReference: Hashable, Sendable {
    case library(id: Int64)
    case file(path: String)
    case source(SourceSong)
}

/// A track from an online source, with the display data the source gave (`SourceTrack`).
struct SourceSong: Hashable, Sendable {
    var plugin: String
    var id: String
    var title: String
    var artist: String?
    var album: String?
    var durationMs: UInt64?
    var artworkURL: String?
    var isrc: String?
}

/// A group of tracks that plays as a unit (`Collection`).
enum CollectionReference: Hashable, Sendable {
    case album(id: Int64)
    case artist(id: Int64)
    case playlist(id: Int64)
    case favorites
    /// Every song in the library, in the Songs page's order (`Collection::Songs`).
    case songs(sort: SongSort, descending: Bool)
    case tracks([TrackReference])
    /// A source's album, playlist or other container: its `SourceItem`'s source, id and kind.
    case source(plugin: String, id: String, kind: ResultKind)
}

/// A song to make lyrics for (`SongRef`).
enum SongReference: Hashable, Sendable {
    /// The song playing now.
    case current
    case library(id: Int64)
}

/// An item to show details or comments for (`ItemRef`).
enum ItemReference: Hashable, Sendable {
    case track(id: Int64)
    case album(id: Int64)
    case artist(id: Int64)
    /// An audio file that is not in the library.
    case file(path: String)
    /// An item a source listed: its `SourceItem.source`, kind and id.
    case source(plugin: String, kind: ResultKind, id: String)
}

/// Artwork a view can load: a local file or a URL (`ArtworkRef`).
enum ArtworkLocation: Hashable, Sendable {
    case file(path: String)
    case url(String)
}

// MARK: - Library

/// One audio file in the library (`Track`).
struct Song: Identifiable, Hashable, Sendable {
    var id: Int64
    var path: String
    var title: String
    var artist: String?
    var album: String?
    var albumArtist: String?
    var albumID: Int64?
    var trackNumber: UInt32?
    var discNumber: UInt32?
    var year: Int32?
    var genre: String?
    var durationMs: UInt64?
    var sampleRate: UInt32?
    var channels: UInt8?
    var bitDepth: UInt8?
    var codec: String?
    var isrc: String?
    var artworkHash: String?

    var reference: TrackReference { .library(id: id) }
}

/// Tracks grouped by album title and album artist (`Album`).
struct AlbumSummary: Identifiable, Hashable, Sendable {
    var id: Int64
    var title: String
    var albumArtist: String?
    var year: Int32?
    var trackCount: UInt32
    var artworkHash: String?
}

/// One artist (`Artist`).
struct ArtistSummary: Identifiable, Hashable, Sendable {
    var id: Int64
    var name: String
    var trackCount: UInt32
}

/// A row of a track list: playlists, favourites, history (`TrackRow`).
struct SongRow: Hashable, Sendable {
    var track: TrackReference
    var title: String
    var artist: String?
    var album: String?
    var durationMs: UInt64?
    var artwork: ArtworkLocation?
    var artworkHash: String?
    var favorite: Bool
    var missing: Bool
    /// A source song with a finished download (it plays the file): the Downloaded mark.
    var downloaded = false
}

/// A playlist in the library (`PlaylistSummary`).
struct PlaylistInfo: Identifiable, Hashable, Sendable {
    var id: Int64
    var name: String
    var entryCount: UInt32
    var durationMs: UInt64
    var updatedAtMs: Int64
    var artworkHash: String?
}

/// One entry of a playlist (`PlaylistEntry`).
struct PlaylistRow: Identifiable, Hashable, Sendable {
    var entryID: Int64
    var position: UInt32
    var row: SongRow

    var id: Int64 { entryID }
}

/// Order of `songs` (`TrackSort`).
enum SongSort: Hashable, Sendable, CaseIterable {
    case artist, title, album, year, duration, path, added
}

/// Order of `albums` (`AlbumSort`).
enum AlbumOrder: Hashable, Sendable, CaseIterable {
    case title, artist, year, added
}

/// A page of songs (`TrackQuery`).
struct SongQuery: Hashable, Sendable {
    var offset: Int = 0
    var limit: Int = 100
    var sort: SongSort = .artist
    var descending: Bool = false
    var albumID: Int64?
    var artistID: Int64?
}

/// Library search results, best first (`SearchResults`).
struct LibrarySearch: Hashable, Sendable {
    var songs: [Song]
    var albums: [AlbumSummary]
    var artists: [ArtistSummary]

    static let empty = LibrarySearch(songs: [], albums: [], artists: [])
    var isEmpty: Bool { songs.isEmpty && albums.isEmpty && artists.isEmpty }
}

/// Counts for a status line (`LibraryStats`).
struct LibraryStatistics: Hashable, Sendable {
    var roots: UInt64
    var tracks: UInt64
    var albums: UInt64
    var artists: UInt64
    var artwork: UInt64
    var totalDurationMs: UInt64
    var totalBytes: UInt64
}

/// A library scan (`ScanStatus`).
enum ScanState: Hashable, Sendable {
    case progress(filesSeen: UInt64, filesChanged: UInt64, current: String)
    case finished(ScanSummary)
    case failed(String)
}

/// What a scan did (`ScanReport`).
struct ScanSummary: Hashable, Sendable {
    var added: UInt64
    var updated: UInt64
    var removed: UInt64
    var unchanged: UInt64
    var errors: [ScanIssue]
}

/// A file a scan could not read (`ScanError`).
struct ScanIssue: Hashable, Sendable {
    var path: String
    var message: String
}

// MARK: - Playback

/// Transport state (`Status`).
enum PlaybackStatus: Hashable, Sendable {
    case stopped, playing, paused, ended
}

/// The point on the timeline views extrapolate from (`PlaybackAnchor`). Feed it to a
/// `PlaybackClock`; never compute positions from it by hand.
struct Anchor: Hashable, Sendable {
    var trackGeneration: UInt64
    var positionUs: Int64
    var anchorMonoNs: UInt64
    var rate: Float
    var durationUs: Int64?

    static let zero = Anchor(trackGeneration: 0, positionUs: 0, anchorMonoNs: 0, rate: 0, durationUs: nil)
}

/// Equalizer: a preamp and up to ten bands (`EqSettings`).
struct EqualizerConfig: Hashable, Sendable {
    var enabled: Bool
    var preampDb: Float
    var bands: [EqualizerBand]
}

/// One EQ band (`EqBand`).
struct EqualizerBand: Hashable, Sendable {
    var kind: EqualizerBandKind
    var freqHz: Float
    var gainDb: Float
    var q: Float
    var enabled: Bool
}

enum EqualizerBandKind: Hashable, Sendable {
    case peaking, lowShelf, highShelf
}

/// An equalizer preset (`EqPreset`): every app offers the same ones, in the core's order.
/// Applying one sets `AppSettings.eq` to its settings.
struct EqualizerPreset: Identifiable, Hashable, Sendable {
    /// Stable name: `flat`, `bass_boost`, `vocal`, `treble`.
    var id: String
    /// Its bands, switched on, with a 0 dB preamp.
    var settings: EqualizerConfig
}

enum ReplayGainOption: Hashable, Sendable, CaseIterable {
    case off, track, album
}

/// The `Playback` slice.
struct PlaybackState: Hashable, Sendable {
    var version: UInt64
    var status: PlaybackStatus
    var anchor: Anchor
    var volume: Float
    var eq: EqualizerConfig
    var replayGain: ReplayGainOption
    var preampDb: Float
}

enum RepeatOption: Hashable, Sendable, CaseIterable {
    case off, all, one

    /// The next mode when the repeat button is pressed: off, all, one, off.
    var next: RepeatOption {
        switch self {
        case .off: return .all
        case .all: return .one
        case .one: return .off
        }
    }
}

/// One queue entry; `uid` is unique for the queue's life (`QueueItem`).
struct QueueEntry: Identifiable, Hashable, Sendable {
    var uid: UInt64
    var track: TrackReference

    var id: UInt64 { uid }
}

/// The `Queue` slice.
struct QueueState: Hashable, Sendable {
    var version: UInt64
    var length: UInt32
    var position: UInt32?
    var current: QueueEntry?
    var shuffle: Bool
    var repeatMode: RepeatOption
    /// The next entries in playback order, at most 50; `queueWindow` pages the rest.
    var upNext: [QueueEntry]

    static let empty = QueueState(version: 0, length: 0, position: nil, current: nil, shuffle: false,
                                  repeatMode: .off, upNext: [])
}

/// Facts about the opened stream (`TrackInfo`).
struct StreamFormat: Hashable, Sendable {
    var codec: String
    var sampleRate: UInt32
    var channels: UInt16
    var durationUs: Int64?
    var gapless: Bool
    var bitrate: UInt32?
    /// Bits per sample of the source (PCM, FLAC, ALAC); none for lossy codecs.
    var bitsPerSample: UInt32? = nil
}

/// How good what plays is, as the core judges it (`QualityLabel`): the apps word it, never
/// compute it.
enum AudioQualityLabel: Hashable, Sendable {
    /// Lossless above 16-bit or above 48 kHz.
    case hiResLossless
    case lossless
    /// Lossy at 256 kbit/s or more.
    case high
    case standard
}

/// The format of what plays or of a file (`AudioFormat`).
struct AudioQuality: Hashable, Sendable {
    var codec: String?
    var sampleRateHz: UInt32?
    var bitDepth: UInt32?
    var channels: UInt32?
    var bitrateKbps: UInt32?
    var lossless: Bool?
    var label: AudioQualityLabel?
}

/// What is playing (`NowPlaying`).
struct NowPlayingInfo: Hashable, Sendable {
    var uid: UInt64
    var track: TrackReference
    var path: String?
    var title: String
    var artist: String?
    var album: String?
    var durationMs: UInt64?
    var artwork: ArtworkLocation?
    var format: StreamFormat?
    /// Opens a source track in its service (Open in {Service}).
    var link: String? = nil
    /// What plays as decoded, with the core's quality label, once the engine has opened it.
    var quality: AudioQuality? = nil
    /// The levels the quality menu offers, Auto first (ADR-0026 §2); empty when there is no
    /// choice, and `qualityNote` says why.
    var qualities: [StreamingQualityOption] = []
    var qualityNote: QualityNoteOption? = nil
}

/// Why the quality menu offers no levels (`QualityNote`); the app words it.
enum QualityNoteOption: Hashable, Sendable {
    /// "Files play as they are." (library songs, files, downloads)
    case filesPlayAsTheyAre
    /// "Quality is set in {service}." (remote playback, sources that offer no levels)
    case setInService(String)
}

/// A sound output (`DeviceInfo`).
struct OutputDevice: Identifiable, Hashable, Sendable {
    var id: String
    var name: String
    var isDefault: Bool
    var sampleRates: [UInt32]
}

/// The `Devices` slice.
struct DevicesState: Hashable, Sendable {
    var version: UInt64
    var devices: [OutputDevice]
    /// `nil` follows the system default.
    var selected: String?

    static let empty = DevicesState(version: 0, devices: [], selected: nil)
}

// MARK: - Lyrics

enum LyricsTiming: Hashable, Sendable {
    case unsynced, line, word
}

enum TextDirection: Hashable, Sendable {
    case leftToRight, rightToLeft, automatic
}

/// A timed word or syllable (`Word`).
struct LyricsWord: Hashable, Sendable {
    var startMs: UInt32
    var endMs: UInt32
    var text: String
    /// No space between this word and the next (CJK, syllables).
    var joinsNext: Bool
}

/// One lyric line (`Line`).
struct LyricsLine: Hashable, Sendable {
    var id: UInt32
    var startMs: UInt32?
    var endMs: UInt32?
    var agent: String?
    var text: String
    var words: [LyricsWord]
    var background: [LyricsWord]
    var direction: TextDirection
}

enum LyricsAuxKind: Hashable, Sendable {
    case translation, romanization
}

/// A translation or romanization track keyed by `LyricsLine.id` (`AuxTrack`).
struct LyricsAuxTrack: Hashable, Sendable {
    var kind: LyricsAuxKind
    var language: String?
    var lines: [UInt32: String]
}

/// One lyric document (`LyricDoc`).
struct LyricsDocument: Hashable, Sendable {
    var language: String?
    var timing: LyricsTiming
    var lines: [LyricsLine]
    var tracks: [LyricsAuxTrack]
    var format: String
    var origin: String?
}

/// Where lyrics came from (`LyricsSource`).
enum LyricsOrigin: Hashable, Sendable {
    case sidecar(path: String)
    case embedded
    /// Made by AI Lyrics: "Made by AI Lyrics · {engine}".
    case made(plugin: String, engine: String)
}

/// The `Lyrics` slice for the current track.
struct LyricsState: Equatable, Sendable {
    var uid: UInt64
    var document: LyricsDocument?
    var timeline: LyricsTimeline?
    var origin: LyricsOrigin?
}

// MARK: - Theme

enum AppearanceOption: Hashable, Sendable, CaseIterable {
    case system, light, dark
}

/// Every theme colour for one appearance, as 0xRRGGBBAA by token name (`Theme`).
struct ThemePalette: Hashable, Sendable {
    var hue: Float
    var colors: [String: UInt32]
}

/// The `Theme` slice: Misty colours from the artwork hue and the baked backdrops.
struct ThemeState: Hashable, Sendable {
    var uid: UInt64?
    var hue: Float
    var fromArtwork: Bool
    var light: ThemePalette
    var dark: ThemePalette
    var backdropLight: String?
    var backdropDark: String?
}

// MARK: - Settings

enum BackdropOption: Hashable, Sendable, CaseIterable {
    case artwork, misty
}

enum HueSourceOption: Hashable, Sendable {
    case artwork
    case fixed(hue: Float)
}

/// The size of lyrics on Now Playing (`LyricsSize`): lines, translations and romanizations
/// scale together (`lyrics.scale-small`, 1, `lyrics.scale-large`).
enum LyricsSizeOption: Hashable, Sendable, CaseIterable {
    case small, medium, large
}

/// How lyrics lines line up on Now Playing (`LyricsAlign`).
enum LyricsAlignOption: Hashable, Sendable, CaseIterable {
    /// The start edge (left in left-to-right text).
    case start
    case center
}

/// Crossfade between tracks (`CrossfadeSettings`).
struct CrossfadeConfig: Hashable, Sendable {
    var enabled: Bool
    /// 1 000 to 12 000 ms.
    var durationMs: UInt32
    var albumContinuity: Bool
}

/// Everything the person sets (`Settings`). Send a modified copy back with `.setSettings`.
struct AppSettings: Hashable, Sendable {
    var appearance: AppearanceOption
    var style: BackdropOption
    /// `automatic` or one of the ten icon ids (ADR-0017).
    var appIcon: String
    var hueSource: HueSourceOption
    var reduceEffects: Bool
    /// How see-through glass is, 0 (solid) to 1 (clearest); 0.5 keeps the tokens' alphas.
    /// The core keeps it within 0...1.
    var transparency: Float
    var lyricsSize: LyricsSizeOption
    var lyricsAlign: LyricsAlignOption
    var showTranslation: Bool
    var showRomanization: Bool
    var crossfade: CrossfadeConfig
    var replayGain: ReplayGainOption
    var preampDb: Float
    var eq: EqualizerConfig
    var outputDevice: String?
    var resumeOnLaunch: Bool
    var watchFolders: Bool
    var allowCommunityPlugins: Bool
    /// Check for Updates Automatically (direct downloads only): at most once a day.
    var checkForUpdates: Bool = true
    /// Show Lyrics Bar: the current line under the player bar's title.
    var showLyricsBar: Bool = true
    /// Desktop Lyrics: the current and next line in a floating panel above other apps.
    var desktopLyrics: Bool = false
    /// Streaming Quality (ADR-0026 §1), the same setting as the bar's quality menu.
    var streamingQuality: StreamingQualityOption = .auto
    /// Settings › Downloads (ADR-0025 §2).
    var downloads: DownloadOptions = .defaults
    /// Settings › Appearance › Mini Player (ADR-0026 §4).
    var miniPlayer: MiniPlayerOptions = .defaults
}

/// The quality downloads are asked for (`DownloadQuality`): the server's original file, or its
/// conversion.
enum DownloadQualityOption: Hashable, Sendable, CaseIterable {
    case original, high, normal, low
}

/// Settings › Downloads (`DownloadSettings`).
struct DownloadOptions: Hashable, Sendable {
    /// The folder the person chose; nil keeps the default (`~/Music/Jellyfesh`).
    var folder: String?
    var quality: DownloadQualityOption
    /// File Names: `{artist}`, `{album_artist}`, `{album}`, `{title}`, `{track}`, `{disc}`,
    /// `{year}`, `{source}`, with `/` between folders.
    var nameTemplate: String
    /// Add Missing Cover and Lyrics (off: original files stay byte for byte).
    var addMissingCoverAndLyrics: Bool
    /// Lyrics in the tags.
    var lyricsInTags: Bool
    /// Save .lrc Files beside the songs.
    var saveLrcFiles: Bool
    /// Pause on Metered Connections (`wifi_only`): downloads wait while the connection is metered.
    var wifiOnly: Bool
    /// Downloads at Once, 1–4.
    var atOnce: UInt32

    /// The core's defaults (ADR-0025 §2).
    static let defaults = DownloadOptions(folder: nil, quality: .original,
                                          nameTemplate: "{album_artist}/{album}/{track} {title}",
                                          addMissingCoverAndLyrics: false, lyricsInTags: true, saveLrcFiles: false,
                                          wifiOnly: true, atOnce: 2)
}

// MARK: - Downloads (ADR-0025)

/// What `download` downloads (`DownloadTarget`).
enum DownloadRequest: Hashable, Sendable {
    /// One song from a source.
    case track(SourceSong)
    /// Every song of a source's album or playlist.
    case collection(plugin: String, id: String, kind: ResultKind)
}

/// Why downloads wait (`DownloadsPaused`).
enum DownloadsPause: Hashable, Sendable {
    /// No network.
    case offline
    /// A metered network with Pause on Metered Connections on.
    case metered
}

/// Why a download failed (`DownloadError`); the app words it.
enum DownloadProblem: Hashable, Sendable {
    case notAllowed(service: String)
    case notOffered(service: String)
    case network(String)
    case storage(String)
    case other(String)
}

/// Where a download is (`DownloadState`).
enum DownloadPhase: Hashable, Sendable {
    /// Waiting for its turn, or for the network.
    case queued
    case downloading
    case done
    case failed(DownloadProblem)
}

/// One download (`DownloadItem`).
struct DownloadEntry: Identifiable, Hashable, Sendable {
    var id: UInt64
    var track: SourceSong
    var phase: DownloadPhase
    var bytesDone: UInt64
    var bytesTotal: UInt64?
    /// The file, once finished.
    var path: String?
    /// Its library track, once finished.
    var libraryID: Int64?
    /// The server's original rather than a conversion.
    var original: Bool
}

/// The `Downloads` slice.
struct DownloadsState: Hashable, Sendable {
    var version: UInt64
    /// Where downloads go: the person's folder, or the default one.
    var folder: String
    /// In progress (queue order), then finished by album.
    var items: [DownloadEntry]
    /// Space the finished downloads take, in bytes.
    var bytesUsed: UInt64
    var paused: DownloadsPause?

    static let empty = DownloadsState(version: 0, folder: "", items: [], bytesUsed: 0, paused: nil)
}

// MARK: - The sources' likes and playlists (ADR-0024)

/// One of a source's lists (`SourceLibraryPart`).
enum SourceListPart: Hashable, Sendable {
    case likedSongs, likedAlbums, likedArtists, playlists
}

/// An entry of a source's list (`SourceLibraryEntry`).
struct SourceListEntry: Hashable, Sendable, Identifiable {
    var item: SourceResult
    /// Playlists: the person's own (or collaborative), not followed.
    var owned: Bool
    /// Playlists: the source lists its songs here; otherwise only Play by context and Open in
    /// {Service}.
    var songsListed: Bool

    var id: String { item.id }
}

/// One list of a source (`SourceLibraryList`): its first page, kept with its age.
struct SourceList: Hashable, Sendable {
    var part: SourceListPart
    var entries: [SourceListEntry]
    var total: UInt32?
    /// `SourceLibraryMore`'s cursor; nil when there is no more.
    var next: String?
    /// When it was last loaded, in ms since the Unix epoch ("Updated 5 minutes ago").
    var refreshedAtMs: Int64?
    var loading: Bool
    var error: String?
}

/// A source's likes and playlists (`SourceLibrary`).
struct SourceLibraryInfo: Hashable, Sendable, Identifiable {
    var plugin: String
    var name: String
    var attribution: String?
    /// Liked Songs, Liked Albums, Liked Artists, Playlists, in that order; lists it does not keep
    /// are left out.
    var lists: [SourceList]

    var id: String { plugin }
}

/// The `SourceLibrary` slice.
struct SourceLibraryState: Hashable, Sendable {
    var version: UInt64
    var sources: [SourceLibraryInfo]

    static let empty = SourceLibraryState(version: 0, sources: [])
}

/// More of a source's list (`SourceLibraryPage`).
struct SourceLibraryPageBatch: Hashable, Sendable {
    var request: CommandID
    var plugin: String
    var part: SourceListPart
    var entries: [SourceListEntry]
    var next: String?
    var error: String?
}

/// More of one kind of a source's search results (`SearchPage`).
struct SearchPageBatch: Hashable, Sendable {
    var request: CommandID
    var query: String
    var source: String
    var kind: ResultKind
    var items: [SourceResult]
    var next: String?
    var error: String?
}

/// Settings › Appearance › Mini Player (`MiniPlayerSettings`).
struct MiniPlayerOptions: Hashable, Sendable {
    var keepOnTop: Bool
    var openWhenWindowClosed: Bool
    var showLyricsLine: Bool
    /// Android's Mini Player in Picture-in-Picture; kept as the core has it.
    var androidPip: Bool

    /// The core's defaults (ADR-0026 §4).
    static let defaults = MiniPlayerOptions(keepOnTop: true, openWhenWindowClosed: false, showLyricsLine: true,
                                            androidPip: false)
}

// MARK: - Sources

enum PluginBadgeKind: Hashable, Sendable {
    case official, community
}

/// How a source signs in (`AuthMethod`).
enum SignInMethod: Hashable, Sendable, CaseIterable {
    case qr, oauth, cookieImport, passwordExchange, apiKey
}

/// One installed source (`PluginSummary`).
struct SourcePlugin: Identifiable, Hashable, Sendable {
    var id: String
    var name: String
    var version: String
    var author: String
    var badge: PluginBadgeKind
    var vendorPlayback: Bool
    var capabilities: [String]
    var signInMethods: [SignInMethod]
    var enabled: Bool
    var account: String?
    var needsDisclosure: Bool
    var settingsSchemaJSON: String
    var settingsJSON: String
    var iconPath: String?
    /// The service plays on the person's own devices; Jellyfesh is its remote control (ADR-0018).
    /// Its songs never join the queue.
    var remotePlayback = false
    /// What its views must credit ("Content from {Service}"), when it asks for that.
    var attribution: String? = nil
    /// An extension (`PluginKind::Extension`: AI Lyrics, Customize; Settings › Extensions) rather
    /// than a music source (Settings › Sources).
    var isExtension = false
    /// It has a Jellyfesh Pro capability (AI Lyrics, Customize).
    var requiresPro = false
    /// The host imports the manifest declares (`audio`, `speech`).
    var permissions: [String] = []
    /// It has a Discover page of its own.
    var hasDiscover = false
    /// It describes items (biography, notes, facts).
    var hasDetails = false
    /// It shows comments: Now Playing offers a Comments pane.
    var hasComments = false
    /// One line from its manifest.
    var summary: String? = nil
    /// It lists the person's likes and playlists (ADR-0024).
    var hasLibrary = false
    /// It offers downloads from the person's server (ADR-0025): Download in its menus.
    var offersDownloads = false
    /// The streaming levels it offers, best first; empty when its service sets the quality.
    var qualities: [StreamingQualityOption] = []

    var canBrowse: Bool { capabilities.contains("browse") }
    var canSearch: Bool { capabilities.contains("search") }
}


/// A music service offered in Add a Source while its official plugin is installed
/// (`SourcePreset`). `.connectSource` connects to it.
struct SourcePreset: Identifiable, Hashable, Sendable {
    /// Stable name: `jellyfin`, `navidrome`, `subsonic`.
    var id: String
    /// The service's own name.
    var name: String
    /// The plugin that connects to it; sign-in steps arrive under this id.
    var plugin: String
    /// The plugin's sign-in methods, in its order.
    var signInMethods: [SignInMethod]
    /// The person types the server's address (a self-hosted server).
    var needsAddress: Bool
    /// The address set now, to fill the field with.
    var address: String?
    /// An address to show as an example in that field.
    var addressExample: String?
    /// The plugin's signed-in account.
    var account: String?
}

/// The `Sources` slice.
struct SourcesState: Hashable, Sendable {
    var version: UInt64
    var plugins: [SourcePlugin]
    /// The services Add a Source offers, in their order. Empty by default, so a state built from
    /// plugins alone (tests, the frames before the core's snapshot) needs no services.
    var presets: [SourcePreset] = []
    var allowCommunity: Bool
    /// Plugins may be installed from files and community plugins allowed: false in store builds,
    /// which hide Install from File… and Allow Community Plugins (ADR-0021 §4).
    var canInstallPlugins = true

    static let empty = SourcesState(version: 0, plugins: [], allowCommunity: false)

    /// The music sources (Settings › Sources, the Sources page).
    var sourcePlugins: [SourcePlugin] { plugins.filter { !$0.isExtension } }
    /// The extensions (Settings › Extensions).
    var extensionPlugins: [SourcePlugin] { plugins.filter(\.isExtension) }
}

/// What a sign-in field holds (`AuthFieldKind`): it picks the field and what the system offers to
/// fill it with.
enum SignInFieldKind: Hashable, Sendable {
    case text, email, url
    /// Digits only, such as a one-time code.
    case number
    /// A `Cookie:` header copied from a browser (Paste Cookies).
    case cookieHeader
}

/// A field to ask for during sign-in (`AuthField`).
struct SignInField: Hashable, Sendable {
    var name: String
    var label: String
    var kind: SignInFieldKind = .text
    /// Masks the input (passwords, keys).
    var secret: Bool
    /// The person must fill it in: Sign In waits for it.
    var required: Bool = true
    var placeholder: String?
    /// A sentence of help under the field.
    var help: String? = nil
}

/// One step of a sign-in flow (`AuthStep`).
enum SignInStep: Hashable, Sendable {
    /// A QR code for the service's app to scan, with the plugin's sentence about it.
    case showQR(data: String, hint: String? = nil, expiresAtMs: Int64?)
    /// A code to enter on another device (Jellyfin's Quick Connect), shown large; `hint` says where.
    case showCode(code: String, hint: String, expiresAtMs: Int64?)
    case openURL(String)
    case askFields([SignInField])
    case waiting(String)
    case done(account: String)
    case failed(String)
}

/// A sign-in flow moved on (`AuthUpdate`).
struct SignInUpdate: Hashable, Sendable {
    var plugin: String
    var flow: UInt64
    var step: SignInStep
}

enum ResultKind: Hashable, Sendable {
    case track, album, artist, playlist, folder
}

/// A search or browse result from the library or a source (`SourceItem`).
struct SourceResult: Identifiable, Hashable, Sendable {
    var source: String
    var kind: ResultKind
    var itemID: String
    var title: String
    var subtitle: String?
    var album: String?
    var durationMs: UInt64?
    var artwork: ArtworkLocation?
    var playable: Bool
    var unplayableReason: String?
    var track: TrackReference?
    var alsoIn: [String]
    /// An `https` link that opens the item in its service (Open in {Service}); the main action
    /// of an item that cannot play here.
    var link: String? = nil
    /// Go to Artist for a source's track or album: the source's id of its artist.
    var artistID: String? = nil
    /// Go to Album for a source's track: the source's id of its album.
    var albumID: String? = nil
    /// A source track with a finished download (it plays the file): the Downloaded mark.
    var downloaded = false

    var id: String { "\(source)\u{1F}\(kind)\u{1F}\(itemID)" }

    /// What `Details` and `Comments` name it by.
    var itemReference: ItemReference? {
        if source == "library", let id = Int64(itemID) {
            switch kind {
            case .track: return .track(id: id)
            case .album: return .album(id: id)
            case .artist: return .artist(id: id)
            case .playlist, .folder: return nil
            }
        }
        return .source(plugin: source, kind: kind, id: itemID)
    }
}

/// Results from one source for one search (`SearchUpdate`).
struct SearchBatch: Hashable, Sendable {
    var request: CommandID
    var query: String
    var source: String
    /// The All tab's rows.
    var items: [SourceResult]
    var error: String?
    var done: Bool
    /// Part of the merged results (the library, From Your Sources); false for a source whose terms
    /// forbid merging, whose results stay in a section of their own (ADR-0024 §4).
    var merged = true
    /// The source's own results kind by kind, for its tab.
    var groups: [SearchKindGroup] = []
}

/// One kind of a source's search results, for its tab (`SearchGroup`).
struct SearchKindGroup: Hashable, Sendable {
    var kind: ResultKind
    var items: [SourceResult]
    /// Show More's cursor; nil when there is no more.
    var next: String?
    var total: UInt32?
}

/// One page of a source's catalogue (`BrowseUpdate`).
struct BrowseBatch: Hashable, Sendable {
    var request: CommandID
    var plugin: String
    var node: String?
    var items: [SourceResult]
    var nextPage: UInt32?
    var error: String?
}

enum SearchArea: Hashable, Sendable {
    case all, library
    case plugin(String)
}

// MARK: - Remote sessions (ADR-0018)

/// What a device of a remote source is (`RemoteDeviceKind`).
enum ServiceDeviceKind: Hashable, Sendable, CaseIterable {
    case computer, smartphone, speaker, tv, car, other
}

/// A device a remote source plays on (`RemoteDevice`).
struct ServiceDevice: Identifiable, Hashable, Sendable {
    var id: String
    var name: String
    var kind: ServiceDeviceKind
    /// The device playing (or last playing) for the account.
    var active: Bool
    /// Its volume, 0–100, when Jellyfesh may change it.
    var volumePercent: UInt32?
}

/// The `Remote` slice: while `source` is set, Jellyfesh controls a service's own player and
/// shows its track instead of the engine's (`RemoteSnapshot`).
struct RemoteSession: Equatable, Sendable {
    var version: UInt64
    /// The source in control; nil without a session.
    var source: String?
    /// That source's name: Open in {Service}, "Up Next is in {Service}."
    var sourceName: String?
    var status: PlaybackStatus
    /// The track playing, as the service reports it.
    var item: SourceResult?
    /// The album or playlist it plays from, when the service says.
    var context: SourceResult?
    /// Time base for the progress bar, like the playback slice's.
    var anchor: Anchor
    var shuffle: Bool
    var repeatMode: RepeatOption
    /// The device playing.
    var device: ServiceDevice?
    /// Every device the service lists (the Devices picker).
    var devices: [ServiceDevice]
    /// What went wrong last talking to the service; on a slice without a source, why the session
    /// ended.
    var error: String?
    /// The quality menu's note while the session runs ("Quality is set in {service}.").
    var qualityNote: QualityNoteOption? = nil

    static let idle = RemoteSession(version: 0, source: nil, sourceName: nil, status: .stopped, item: nil,
                                    context: nil, anchor: .zero, shuffle: false, repeatMode: .off, device: nil,
                                    devices: [], error: nil)
}

/// What `CoreCommand.remote` asks of the remote session (`RemoteAction`).
enum RemoteRequest: Hashable, Sendable {
    /// Asks the service for its devices and state now (the Devices picker opens).
    case refresh
    /// Moves playback to this device.
    case transfer(device: String)
    /// The playing device's volume, 0–100.
    case setVolume(percent: UInt32)
    /// Stops controlling the service; its player carries on.
    case end
}

// MARK: - Distribution and Pro (ADR-0021)

/// How this build reached the person (`Distribution`): the macOS app is a direct download, a Mac
/// App Store build or built from source.
enum AppDistribution: Hashable, Sendable {
    case direct, appStore, source
}

/// A store that sells Jellyfesh Pro (`Store`).
enum ProShop: Hashable, Sendable {
    case appStore, microsoftStore, googlePlay
}

/// What a store says about Jellyfesh Pro, as the app verified it with the platform
/// (`StoreEntitlement`).
struct ProEntitlement: Hashable, Sendable {
    var store: ProShop
    /// The store's product id (`org.jellyfesh.pro`).
    var product: String
    /// The person owns it now (false after a refund or revocation).
    var active: Bool
    /// The store's transaction id.
    var transaction: String?
}

/// Where Pro comes from (`ProSource`).
enum ProOrigin: Hashable, Sendable {
    case none
    /// A Pro Preview build: "Pro features are free during the public beta."
    case preview
    /// "Jellyfesh Pro · Purchased".
    case store(ProShop)
    /// "Jellyfesh Pro · Licence …ABCD".
    case licence(idSuffix: String, issuedAtMs: Int64)
}

/// Why a licence key was not accepted (`LicenceError`).
enum LicenceProblem: Hashable, Sendable {
    case malformed, unknownKey, badSignature, wrongProduct, revoked
}

/// The `Pro` slice.
struct ProState: Hashable, Sendable {
    var version: UInt64
    var active: Bool
    var origin: ProOrigin
    var preview: Bool
    var licenceProblem: LicenceProblem?
    /// The stores this build can sell Pro in: Buy and Restore Purchases only for these.
    var stores: [ProShop]
    /// Enter Licence Key… works in this build (false in App Store builds).
    var licenceKeys: Bool

    static let inactive = ProState(version: 0, active: false, origin: .none, preview: false, licenceProblem: nil,
                                   stores: [], licenceKeys: true)
}

// MARK: - Updates (ADR-0021 §5)

/// A newer release (`UpdateInfo`).
struct ReleaseInfo: Hashable, Sendable {
    var version: String
    /// Its release page.
    var url: String
}

/// The `Update` slice.
struct UpdateState: Hashable, Sendable {
    var version: UInt64
    /// Direct downloads: show Check for Updates and its setting.
    var supported: Bool
    var checking: Bool
    var available: ReleaseInfo?
    var lastCheckedMs: Int64?
    var auto: Bool
    var error: String?

    static let unsupported = UpdateState(version: 0, supported: false, checking: false, available: nil,
                                         lastCheckedMs: nil, auto: false, error: nil)
}

// MARK: - AI Lyrics (ADR-0019)

/// Where AI Lyrics recognises the words (`AiLyricsMode`).
enum LyricsMakingMode: Hashable, Sendable {
    case onDevice, online
}

/// What making lyrics would do now (`AiLyricsPlan`).
struct LyricsMakingPlan: Hashable, Sendable {
    var mode: LyricsMakingMode
    /// In the plugin's words: "Whisper Base on this device".
    var engine: String
    /// The origin the online mode sends audio to.
    var uploadTo: String?
    var automatic: Bool
}

/// What must happen before lyrics can be made (`AiLyricsNeed`).
enum LyricsMakingNeed: Hashable, Sendable {
    case nothing
    /// Jellyfesh Pro is inactive: the Pro sheet.
    case pro
    /// The online mode needs the person's API key.
    case signIn
    /// "Send This Song to {origin}?", answered with `AllowAudioUpload`.
    case consent(origin: String)
    /// Download this model first.
    case model(id: String, name: String, bytes: UInt64)
    case unavailable(reason: String)
}

/// Where a lyrics job is (`LyricsJobStage`).
enum LyricsJobStep: Hashable, Sendable {
    case waiting
    case readingAudio
    case sending(origin: String)
    /// 0–1.
    case recognising(fraction: Float)
    case timingWords
    case done
    case failed(message: String)
    case cancelled

    /// Still to finish: waiting or running.
    var isActive: Bool {
        switch self {
        case .done, .failed, .cancelled: return false
        default: return true
        }
    }
}

/// A lyrics job (`LyricsJob`).
struct LyricsMakingJob: Identifiable, Hashable, Sendable {
    var id: UInt64
    var title: String
    var artist: String?
    /// The queue entry it is for, when it was made for the song playing.
    var uid: UInt64?
    /// The library track it is for.
    var libraryID: Int64?
    var step: LyricsJobStep
}

/// Where an on-device speech model is (`SpeechModelStatus`).
enum SpeechModelProgress: Hashable, Sendable {
    case notDownloaded
    /// 0–1.
    case downloading(fraction: Float)
    case ready
    case failed(message: String)
}

/// An on-device speech model (`SpeechModelState`).
struct SpeechModelInfo: Identifiable, Hashable, Sendable {
    var id: String
    var name: String
    var bytes: UInt64
    var progress: SpeechModelProgress
}

/// An origin the person allowed a plugin to send audio to (`AudioConsent`).
struct AudioUploadConsent: Hashable, Sendable {
    var plugin: String
    var origin: String
}

/// The `AiLyrics` slice.
struct AiLyricsState: Hashable, Sendable {
    var version: UInt64
    /// Offer Create Synced Lyrics (Pro may still be needed: see `need`).
    var available: Bool
    var plugin: String?
    var plan: LyricsMakingPlan?
    var need: LyricsMakingNeed
    var jobs: [LyricsMakingJob]
    var models: [SpeechModelInfo]
    var consents: [AudioUploadConsent]

    static let idle = AiLyricsState(version: 0, available: false, plugin: nil, plan: nil,
                                    need: .unavailable(reason: ""), jobs: [], models: [], consents: [])
}

// MARK: - The look (ADR-0020)

enum LookTypeface: Hashable, Sendable {
    case system, rounded, serif, mono
}

enum LookDensity: Hashable, Sendable {
    case compact, standard, roomy
}

enum LookLyricsWeight: Hashable, Sendable {
    case regular, medium, semibold, bold, heavy
}

/// The colour of the sung part of lyrics (`LyricsHighlight`).
enum LookHighlight: Hashable, Sendable {
    case artwork, accent
    /// `0xRRGGBB`.
    case custom(rgb: UInt32)
}

/// A place of the sidebar (`Place`).
enum LookPlace: Hashable, Sendable, CaseIterable {
    case home, albums, artists, songs, playlists, favourites, search, sources
}

/// A section of Home (`HomeSection`).
enum LookHomeSection: Hashable, Sendable, CaseIterable {
    case summary, recentlyAdded, recentlyPlayed, mostPlayed, favourites
}

/// How Now Playing is laid out (`NowPlayingLayout`).
enum LookNowPlayingLayout: Hashable, Sendable {
    /// The v0.0.6 stage.
    case stage
    /// A large cover with the line being sung below it.
    case artwork
    /// Lyrics full width, a small cover in the corner.
    case lyrics
}

/// The player at the bottom of the window (`MiniPlayerStyle`).
enum LookMiniPlayerStyle: Hashable, Sendable {
    case bar, compact, floating
}

/// The `Look` slice: every value resolved, so the app applies one thing (ADR-0020).
struct LookState: Hashable, Sendable {
    var version: UInt64
    /// Radii ×, 0–2.
    var cornerScale: Float
    /// Glass alpha and blur ×, 0–1.
    var glass: Float
    /// The field's strength, 0–1.
    var aurora: Float
    var typeface: LookTypeface
    /// Every type size ×, 0.85–1.30.
    var textScale: Float
    var density: LookDensity
    var lyricsWeight: LookLyricsWeight
    /// 0–1.
    var lyricsGlow: Float
    var lyricsHighlight: LookHighlight
    /// The places shown, in order; Settings is never listed and always shown.
    var sidebar: [LookPlace]
    var home: [LookHomeSection]
    var nowPlaying: LookNowPlayingLayout
    var miniPlayer: LookMiniPlayerStyle
    /// An appearance plugin sets the look; false: every value is the Luminous default.
    var customized: Bool
    var plugin: String?

    /// The Luminous look (`LookSnapshot::defaults`).
    static let luminous = LookState(
        version: 0, cornerScale: 1, glass: 1, aurora: 0.75, typeface: .system, textScale: 1,
        density: .standard, lyricsWeight: .bold, lyricsGlow: 0.6, lyricsHighlight: .artwork,
        sidebar: LookPlace.allCases, home: [.summary, .recentlyAdded, .recentlyPlayed, .mostPlayed],
        nowPlaying: .stage, miniPlayer: .bar, customized: false, plugin: nil
    )
}

// MARK: - Discover, details, comments, covers (ADR-0022)

/// How a shelf lays out its items (`ShelfStyle`).
enum ShelfLayout: Hashable, Sendable {
    /// One row of covers that scrolls sideways.
    case row
    /// Covers that wrap.
    case grid
    /// A numbered list.
    case chart
}

/// One shelf of a Discover page (`Shelf`).
struct DiscoverShelf: Identifiable, Hashable, Sendable {
    var id: String
    var title: String
    var layout: ShelfLayout
    var items: [SourceResult]
    /// A node to browse for See All.
    var more: String?
}

/// A Discover page (`DiscoverPage`).
struct DiscoverAnswer: Hashable, Sendable {
    var request: CommandID
    var source: String
    var shelves: [DiscoverShelf]
    var error: String?
}

/// What a fact is (`FactKey`).
enum DetailFactKey: Hashable, Sendable {
    case version, genre, language, released, label, composer, lyricist, arranger, producer, popularity,
         quality, isrc, disc, track
    /// The service's own label.
    case other(label: String)
}

/// One fact about an item (`Fact`).
struct DetailFact: Hashable, Sendable {
    var key: DetailFactKey
    var value: String
}

/// ReplayGain values (`ReplayGainValues`): gains in dB, peaks as linear amplitude.
struct ReplayGainInfo: Hashable, Sendable {
    var trackGainDb: Float?
    var trackPeak: Float?
    var albumGainDb: Float?
    var albumPeak: Float?
}

/// Technical information about a file or a stream (`TechnicalInfo`).
struct TechnicalFacts: Hashable, Sendable {
    var format: AudioQuality
    var container: String?
    var fileSizeBytes: UInt64?
    var durationMs: UInt64?
    var replayGain: ReplayGainInfo?
    var path: String?
}

/// An item's details (`DetailsPage`).
struct DetailsAnswer: Hashable, Sendable {
    var request: CommandID
    var item: ItemReference
    var title: String
    var subtitle: String?
    /// Album notes, an artist's biography.
    var notes: String?
    var image: ArtworkLocation?
    var facts: [DetailFact]
    var technical: TechnicalFacts?
    var commentCount: UInt64?
    var attribution: String?
    var error: String?
}

/// A comment on a source item (`CommentEntry`).
struct SourceComment: Identifiable, Hashable, Sendable {
    var id: String
    var author: String
    var avatar: ArtworkLocation?
    var text: String
    var likes: UInt64?
    var timeMs: Int64?
    var replies: UInt64?
}

/// A page of comments (`CommentsPage`).
struct CommentsAnswer: Hashable, Sendable {
    var request: CommandID
    var item: ItemReference
    /// The hot comments, on the first page only.
    var hot: [SourceComment]
    var comments: [SourceComment]
    var total: UInt64?
    /// The page to ask for next.
    var next: String?
    var attribution: String?
    var error: String?
}

/// A source picture is in the artwork cache, or cannot be had (`ArtworkReady`).
struct ArtworkFetched: Hashable, Sendable {
    var url: String
    var sizePx: UInt32
    var path: String?
}

// MARK: - Players the app provides (ADR-0018 item 5)

/// An item a player the app provides plays or reports (`ExternalItem`).
struct ExternalMedia: Hashable, Sendable {
    var kind: ResultKind
    /// The player's own id for it.
    var id: String
    var title: String
    var artist: String?
    var album: String?
    var durationMs: UInt64?
    var artworkURL: String?
}

/// What a player the app provides is doing (`ExternalState`).
struct ExternalPlayerReport: Hashable, Sendable {
    var item: ExternalMedia?
    /// The album or playlist it plays from, when it knows.
    var context: ExternalMedia?
    var playing: Bool
    var positionMs: UInt64
    var shuffle: Bool
    var repeatMode: RepeatOption
    var device: ServiceDevice?
}

/// Why a call to a player the app provides failed: `reason` is shown to the person
/// (`ExternalPlayerError.Failed`).
struct AppPlayerError: Error, Equatable, LocalizedError {
    var reason: String

    var errorDescription: String? { reason }
}

/// A player the app provides to the core (`ExternalPlayer`), registered under a source id without
/// dots. The core calls it from a remote session's own thread, never the main thread, and each
/// call may block until the player answers (a few seconds at most). Never call back into the core
/// from inside a call. Throw `AppPlayerError` with a sentence the person can read.
protocol AppPlayer: AnyObject, Sendable {
    func devices() throws -> [ServiceDevice]
    /// Nil when nothing plays.
    func state() throws -> ExternalPlayerReport?
    /// Plays `tracks` from `start`, or the album or playlist `context` from its track `start`
    /// (then `tracks` is empty).
    func play(context: ExternalMedia?, tracks: [ExternalMedia], start: Int) throws
    func resume() throws
    func pause() throws
    func next() throws
    func previous() throws
    func seek(positionMs: UInt64) throws
    /// 0–100; a player without a volume throws.
    func setVolume(percent: UInt32) throws
    func setShuffle(_ on: Bool) throws
    func setRepeat(_ mode: RepeatOption) throws
    func transfer(device: String) throws
}

// MARK: - Snapshot, events, commands

/// Every slice at once (`Snapshot`).
struct CoreState: Equatable, Sendable {
    var playback: PlaybackState
    var queue: QueueState
    var nowPlaying: NowPlayingInfo?
    var lyrics: LyricsState?
    var theme: ThemeState?
    var libraryRevision: UInt64
    var scanning: Bool
    var settings: AppSettings
    var devices: DevicesState
    var sources: SourcesState
    var muted: Bool
    /// The remote session, if one runs.
    var remote: RemoteSession = .idle
    var aiLyrics: AiLyricsState = .idle
    var look: LookState = .luminous
    var pro: ProState = .inactive
    var update: UpdateState = .unsupported
    /// The sources' likes and playlists (ADR-0024).
    var sourceLibrary: SourceLibraryState = .empty
    /// Downloads (ADR-0025).
    var downloads: DownloadsState = .empty
}

/// What the core reports (`Event`).
enum CoreEvent: Equatable, Sendable {
    case playback(PlaybackState)
    case queue(QueueState)
    case nowPlaying(NowPlayingInfo?)
    case lyrics(LyricsState)
    case theme(ThemeState)
    case scan(ScanState)
    case libraryChanged(revision: UInt64)
    case playlistCreated(request: CommandID, id: Int64)
    case settings(AppSettings)
    case devices(DevicesState)
    case sources(SourcesState)
    /// The remote session changed.
    case remote(RemoteSession)
    case signIn(SignInUpdate)
    case searchResults(SearchBatch)
    case browsePage(BrowseBatch)
    case buffering(uid: UInt64, buffering: Bool)
    case commandFailed(id: CommandID, message: String)
    case error(String)
    case aiLyrics(AiLyricsState)
    case look(LookState)
    case pro(ProState)
    case update(UpdateState)
    /// Answers to `.discover`, `.details` and `.comments`.
    case discover(DiscoverAnswer)
    case details(DetailsAnswer)
    case comments(CommentsAnswer)
    /// A source picture asked for with `sourceArtwork` is cached, or cannot be had.
    case artwork(ArtworkFetched)
    /// More of one kind of a source's search results (`SearchMore`).
    case searchPage(SearchPageBatch)
    case sourceLibrary(SourceLibraryState)
    /// More of a source's list (`SourceLibraryMore`).
    case sourceLibraryPage(SourceLibraryPageBatch)
    case downloads(DownloadsState)
}

/// What a view can ask the core to do (`Command`). Failures come back as `.commandFailed`.
enum CoreCommand: Equatable, Sendable {
    // Transport
    case play, pause, togglePlayPause, stop, next, previous
    case seek(positionMs: UInt64)
    case setVolume(Float)
    case setMuted(Bool)
    // Queue
    case playTracks([TrackReference], start: UInt32)
    case playCollection(CollectionReference, start: UInt32, shuffle: Bool)
    case enqueue([TrackReference], playNext: Bool)
    case enqueueCollection(CollectionReference, playNext: Bool)
    case removeFromQueue(uid: UInt64)
    case moveInQueue(uid: UInt64, to: UInt32)
    case jumpTo(uid: UInt64)
    case clearQueue
    case setShuffle(Bool)
    case setRepeat(RepeatOption)
    // Settings and output
    case setSettings(AppSettings)
    case setEq(EqualizerConfig)
    case setReplayGain(ReplayGainOption, preampDb: Float)
    case setCrossfade(CrossfadeConfig)
    case setOutputDevice(String?)
    case refreshDevices
    // Library
    case addRoot(path: String)
    case removeRoot(path: String)
    case rescan
    case createPlaylist(name: String, tracks: [TrackReference])
    case renamePlaylist(id: Int64, name: String)
    case deletePlaylist(id: Int64)
    case addToPlaylist(id: Int64, tracks: [TrackReference])
    case removeFromPlaylist(id: Int64, entries: [Int64])
    case movePlaylistEntry(id: Int64, entry: Int64, to: UInt32)
    case importPlaylist(path: String)
    case setFavorite(TrackReference, favorite: Bool)
    // Sources
    case installPlugin(path: String)
    case uninstallPlugin(id: String)
    case setPluginEnabled(id: String, enabled: Bool)
    case setPluginSettings(id: String, json: String)
    case acknowledgeUnofficial(id: String)
    case beginLogin(plugin: String, method: SignInMethod)
    /// Sets the service's server address (nil when it needs none), turns its plugin on and signs
    /// in as `.beginLogin` does; the steps arrive for the preset's plugin.
    case connectSource(preset: String, address: String?, method: SignInMethod)
    case submitLogin(plugin: String, flow: UInt64, fields: [String: String])
    case cancelLogin(plugin: String, flow: UInt64)
    case logout(plugin: String)
    case search(query: String, area: SearchArea)
    case browse(plugin: String, node: String?, page: UInt32)
    /// What the transport commands cannot say to a remote session (they reach it by themselves).
    case remote(RemoteRequest)
    /// Restores a plugin's settings to its schema's defaults (Reset to Defaults).
    case resetPluginSettings(plugin: String)
    // AI Lyrics
    case makeLyrics(SongReference)
    case cancelLyrics(job: UInt64)
    case deleteMadeLyrics(SongReference)
    case allowAudioUpload(plugin: String, origin: String)
    case revokeAudioUpload(plugin: String, origin: String)
    case downloadSpeechModel(id: String)
    case cancelSpeechModelDownload(id: String)
    case deleteSpeechModel(id: String)
    // Pro and updates
    case activateLicence(key: String)
    case removeLicence
    case checkForUpdates
    // Discover, details, comments
    case discover(source: String)
    case details(ItemReference)
    /// `page` nil is the first page, else the previous page's `next`.
    case comments(ItemReference, page: String?)
    // v0.0.8: the network, search by source, the sources' likes and playlists, downloads
    /// Online, and metered (Auto quality and downloads follow it); at start and on every change.
    case setNetworkState(online: Bool, metered: Bool)
    /// Show More: more of one kind of a source's results for the newest search.
    case searchMore(plugin: String, kind: ResultKind, cursor: String)
    /// Reloads a source's likes and playlists (nil: every source's).
    case refreshSourceLibrary(plugin: String?)
    case sourceLibraryMore(plugin: String, part: SourceListPart, cursor: String)
    /// Save as Playlist…: a Jellyfesh playlist of a source playlist's songs.
    case savePlaylistCopy(plugin: String, id: String, name: String)
    case download(DownloadRequest)
    case cancelDownload(id: UInt64)
    case retryDownload(id: UInt64)
    /// Remove Download… (asked first).
    case removeDownload(id: UInt64)
    /// Remove All Downloads… (asked first).
    case removeAllDownloads
}

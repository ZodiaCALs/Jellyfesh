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
    case tracks([TrackReference])
    case source(plugin: String, id: String)
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

    static let empty = SourcesState(version: 0, plugins: [], allowCommunity: false)
}

/// A field to ask for during sign-in (`AuthField`).
struct SignInField: Hashable, Sendable {
    var name: String
    var label: String
    var secret: Bool
    var placeholder: String?
}

/// One step of a sign-in flow (`AuthStep`).
enum SignInStep: Hashable, Sendable {
    case showQR(data: String, expiresAtMs: Int64?)
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

    var id: String { "\(source)\u{1F}\(kind)\u{1F}\(itemID)" }
}

/// Results from one source for one search (`SearchUpdate`).
struct SearchBatch: Hashable, Sendable {
    var request: CommandID
    var query: String
    var source: String
    var items: [SourceResult]
    var error: String?
    var done: Bool
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
    case signIn(SignInUpdate)
    case searchResults(SearchBatch)
    case browsePage(BrowseBatch)
    case buffering(uid: UInt64, buffering: Bool)
    case commandFailed(id: CommandID, message: String)
    case error(String)
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
}

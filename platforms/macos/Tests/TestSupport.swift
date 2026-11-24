// A CoreClient for tests: records every command, answers queries from canned values, and lets a
// test hand events to whoever subscribed.

import Foundation
import XCTest
@testable import Jellyfesh

final class FakeCoreClient: CoreClient, @unchecked Sendable {
    var state = CoreState.offline
    private(set) var sent: [CoreCommand] = []
    private var lastID: CommandID = 0
    var nowNs: UInt64 = 10_000_000_000
    private(set) var onEvents: (@MainActor ([CoreEvent]) -> Void)?

    // Canned answers.
    var albumsResult: [AlbumSummary] = []
    var artistsResult: [ArtistSummary] = []
    var songsResult: [Song] = []
    var songsByID: [Int64: Song] = [:]
    var librarySearchResult = LibrarySearch.empty
    var favoriteSet = Set<TrackReference>()
    var queueWindowResult: [QueueEntry] = []
    var rootsResult: [String] = []
    var statsResult = LibraryStatistics(roots: 0, tracks: 0, albums: 0, artists: 0, artwork: 0,
                                        totalDurationMs: 0, totalBytes: 0)
    var playlistsResult: [PlaylistInfo] = []
    var playlistEntriesResult: [Int64: [PlaylistRow]] = [:]
    var equalizerPresetsResult: [EqualizerPreset] = []
    var mutedResult = false
    /// Source pictures the core has cached, by URL (others answer nil: the core is fetching).
    var cachedSourceArtwork: [String: String] = [:]
    /// Plugins' settings schemas by plugin id and language.
    var localizedSchemas: [String: String] = [:]
    /// Entitlements reported with `setStoreEntitlement`, in order.
    private(set) var storeEntitlements: [ProEntitlement] = []
    private(set) var sourceArtworkRequests: [(url: String, sizePx: Int)] = []
    /// When set, every query throws it.
    var failure: CoreClientError?

    /// Players registered with `registerExternalPlayer`, by source id: their names and players.
    private(set) var externalPlayers: [String: (name: String, player: AppPlayer)] = [:]
    /// When set, `registerExternalPlayer` throws it.
    var registerFailure: CoreClientError?

    private(set) var songIDRequests: [[Int64]] = []
    private(set) var queueWindowRequests: [(offset: Int, limit: Int)] = []
    private(set) var librarySearches: [String] = []

    func start(onEvents: @escaping @MainActor ([CoreEvent]) -> Void) -> CoreState {
        self.onEvents = onEvents
        return state
    }

    @discardableResult
    func send(_ command: CoreCommand) -> CommandID {
        sent.append(command)
        lastID += 1
        return lastID
    }

    /// The id the last `send` returned.
    var lastCommandID: CommandID { lastID }

    func monotonicNowNs() -> UInt64 { nowNs }
    func equalizerPresets() -> [EqualizerPreset] { equalizerPresetsResult }
    func downloadPreview(template: String) -> String { "Preview/\(template)" }
    func shutdown() {}

    func registerExternalPlayer(id: String, name: String, player: AppPlayer) throws {
        if let registerFailure { throw registerFailure }
        externalPlayers[id] = (name, player)
    }

    func unregisterExternalPlayer(id: String) {
        externalPlayers[id] = nil
    }

    private func check() throws {
        if let failure { throw failure }
    }

    func albums(order: AlbumOrder, descending: Bool, offset: Int, limit: Int) async throws -> [AlbumSummary] {
        try check()
        return Array(albumsResult.dropFirst(offset).prefix(limit))
    }

    func artists(offset: Int, limit: Int) async throws -> [ArtistSummary] {
        try check()
        return Array(artistsResult.dropFirst(offset).prefix(limit))
    }

    func songs(_ query: SongQuery) async throws -> [Song] {
        try check()
        return Array(songsResult.dropFirst(query.offset).prefix(query.limit))
    }

    func song(id: Int64) async throws -> Song? {
        try check()
        return songsByID[id]
    }

    func songs(ids: [Int64]) async throws -> [Int64: Song] {
        try check()
        songIDRequests.append(ids)
        return songsByID.filter { ids.contains($0.key) }
    }

    func albumSongs(albumID: Int64) async throws -> [Song] {
        try check()
        return songsResult.filter { $0.albumID == albumID }
    }

    func artistAlbums(artistID: Int64) async throws -> [AlbumSummary] {
        try check()
        return albumsResult
    }

    func searchLibrary(_ text: String, limit: Int) async throws -> LibrarySearch {
        librarySearches.append(text)
        try check()
        return librarySearchResult
    }

    func recentlyAdded(limit: Int) async throws -> [Song] { try check(); return [] }
    func recentlyPlayed(limit: Int) async throws -> [SongRow] { try check(); return [] }
    func mostPlayed(limit: Int) async throws -> [SongRow] { try check(); return [] }
    func playlists() async throws -> [PlaylistInfo] { try check(); return playlistsResult }
    func playlist(id: Int64) async throws -> PlaylistInfo? { try check(); return playlistsResult.first { $0.id == id } }
    func playlistEntries(id: Int64, offset: Int, limit: Int) async throws -> [PlaylistRow] {
        try check()
        return Array((playlistEntriesResult[id] ?? []).dropFirst(offset).prefix(limit))
    }

    func exportPlaylist(id: Int64, to path: String) async throws { try check() }
    func favorites(offset: Int, limit: Int) async throws -> [SongRow] { try check(); return [] }

    func isFavorite(_ track: TrackReference) async throws -> Bool {
        try check()
        return favoriteSet.contains(track)
    }

    func favoriteTracks(among tracks: [TrackReference]) async throws -> Set<TrackReference> {
        try check()
        return favoriteSet.intersection(tracks)
    }

    func artworkThumbnail(hash: String, sizePx: Int) async throws -> String? { try check(); return nil }

    func queueWindow(offset: Int, limit: Int) async throws -> [QueueEntry] {
        try check()
        queueWindowRequests.append((offset, limit))
        return queueWindowResult
    }

    func roots() async throws -> [String] { try check(); return rootsResult }
    func stats() async throws -> LibraryStatistics { try check(); return statsResult }
    func outputDevices() async throws -> [OutputDevice] { try check(); return [] }
    func isMuted() async -> Bool { mutedResult }

    func sourceArtwork(url: String, sizePx: Int) async throws -> String? {
        try check()
        sourceArtworkRequests.append((url, sizePx))
        return cachedSourceArtwork[url]
    }

    func localizedSettingsSchema(plugin: String, language: String) async throws -> String? {
        try check()
        return localizedSchemas["\(plugin)|\(language)"]
    }

    func setStoreEntitlement(_ entitlement: ProEntitlement) async throws {
        try check()
        storeEntitlements.append(entitlement)
    }
}

/// Waits (at most about two seconds) for work started on the main actor to finish.
@MainActor
func eventually(_ condition: () -> Bool, file: StaticString = #filePath, line: UInt = #line) async {
    for _ in 0..<400 {
        if condition() { return }
        try? await Task.sleep(for: .milliseconds(5))
    }
    XCTFail("condition not met in time", file: file, line: line)
}

enum Fixture {
    static func song(_ id: Int64, title: String? = nil, album: Int64? = nil, artwork: String? = nil) -> Song {
        Song(id: id, path: "/Music/\(id).flac", title: title ?? "Song \(id)", artist: "Artist", album: "Album",
             albumID: album, durationMs: 180_000, artworkHash: artwork)
    }

    static func playback(status: PlaybackStatus = .playing, generation: UInt64 = 1, positionUs: Int64 = 0,
                         monoNs: UInt64 = 10_000_000_000, rate: Float = 1, durationUs: Int64? = 200_000_000,
                         volume: Float = 1) -> PlaybackState {
        PlaybackState(
            version: generation,
            status: status,
            anchor: Anchor(trackGeneration: generation, positionUs: positionUs, anchorMonoNs: monoNs, rate: rate,
                           durationUs: durationUs),
            volume: volume,
            eq: EqualizerConfig(enabled: false, preampDb: 0, bands: []),
            replayGain: .off,
            preampDb: 0
        )
    }

    static func queue(position: UInt32?, length: UInt32, current: QueueEntry?, upNext: [QueueEntry],
                      version: UInt64 = 1) -> QueueState {
        QueueState(version: version, length: length, position: position, current: current, shuffle: false,
                   repeatMode: .off, upNext: upNext)
    }

    static func entry(_ uid: UInt64, track id: Int64) -> QueueEntry {
        QueueEntry(uid: uid, track: .library(id: id))
    }

    static func plugin(_ id: String = "org.example.subsonic", badge: PluginBadgeKind = .official,
                       methods: [SignInMethod] = [.passwordExchange], enabled: Bool = true,
                       needsDisclosure: Bool = false, schema: String = "", settings: String = "{}") -> SourcePlugin {
        SourcePlugin(id: id, name: "Example", version: "1.0.0", author: "Jellyfesh", badge: badge, vendorPlayback: false,
                     capabilities: ["search", "browse"], signInMethods: methods, enabled: enabled, account: nil,
                     needsDisclosure: needsDisclosure, settingsSchemaJSON: schema, settingsJSON: settings, iconPath: nil)
    }

    /// Navidrome as the core offers it in Add a Source: through the Subsonic plugin, on a server
    /// whose address the person types.
    static func preset(_ id: String = "navidrome", name: String = "Navidrome",
                       plugin: String = "org.jellyfesh.subsonic",
                       methods: [SignInMethod] = [.apiKey, .passwordExchange],
                       needsAddress: Bool = true) -> SourcePreset {
        SourcePreset(id: id, name: name, plugin: plugin, signInMethods: methods, needsAddress: needsAddress,
                     address: nil, addressExample: needsAddress ? "http://192.168.1.20:4533" : nil, account: nil)
    }

    /// Ten one-octave peaking bands with these gains, as the core's presets have them.
    static func equalizer(_ gains: [Float], preampDb: Float = 0, enabled: Bool = true) -> EqualizerConfig {
        let frequencies: [Float] = [31, 62, 125, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000]
        let bands = zip(frequencies, gains).map { frequency, gain in
            EqualizerBand(kind: .peaking, freqHz: frequency, gainDb: gain, q: 1.41, enabled: true)
        }
        return EqualizerConfig(enabled: enabled, preampDb: preampDb, bands: bands)
    }

    /// The core's four presets (`eq_presets()`), in its order.
    static let equalizerPresets: [EqualizerPreset] = [
        EqualizerPreset(id: "flat", settings: Fixture.equalizer([0, 0, 0, 0, 0, 0, 0, 0, 0, 0])),
        EqualizerPreset(id: "bass_boost", settings: Fixture.equalizer([6, 5, 4, 2, 0.5, 0, 0, 0, 0, 0])),
        EqualizerPreset(id: "vocal", settings: Fixture.equalizer([-2, -2, -1, 0, 1.5, 3, 3, 2, 0, -1])),
        EqualizerPreset(id: "treble", settings: Fixture.equalizer([0, 0, 0, 0, 0, 0, 1.5, 3, 4.5, 6])),
    ]

    static func result(_ id: String, source: String, kind: ResultKind = .track,
                       track: TrackReference? = nil) -> SourceResult {
        SourceResult(source: source, kind: kind, itemID: id, title: "Item \(id)", subtitle: nil, album: nil,
                     durationMs: nil, artwork: nil, playable: true, unplayableReason: nil, track: track, alsoIn: [])
    }

    static func device(_ id: String, name: String? = nil, kind: ServiceDeviceKind = .speaker, active: Bool = false,
                       volume: UInt32? = 40) -> ServiceDevice {
        ServiceDevice(id: id, name: name ?? "Device \(id)", kind: kind, active: active, volumePercent: volume)
    }

    /// A remote session of Spotify's plugin playing on `device`.
    static func remote(version: UInt64 = 1, status: PlaybackStatus = .playing, title: String = "Remote Song",
                       positionUs: Int64 = 0, monoNs: UInt64 = 10_000_000_000, generation: UInt64 = 1,
                       device: ServiceDevice? = Fixture.device("phone", active: true),
                       devices: [ServiceDevice] = [], error: String? = nil) -> RemoteSession {
        let item = SourceResult(source: "org.example.spotify", kind: .track, itemID: "t1", title: title,
                                subtitle: "Remote Artist", album: "Remote Album", durationMs: 180_000,
                                artwork: .url("https://img.example.com/t1.jpg"), playable: true,
                                unplayableReason: nil, track: nil, alsoIn: [],
                                link: "https://open.example.com/track/t1")
        return RemoteSession(version: version, source: "org.example.spotify", sourceName: "Spotify",
                             status: status, item: item, context: nil,
                             anchor: Anchor(trackGeneration: generation, positionUs: positionUs, anchorMonoNs: monoNs,
                                            rate: status == .playing ? 1 : 0, durationUs: 180_000_000),
                             shuffle: true, repeatMode: .one, device: device, devices: devices, error: error)
    }

    /// The slice once a session has ended, with why.
    static func ended(version: UInt64, error: String?) -> RemoteSession {
        var session = RemoteSession.idle
        session.version = version
        session.error = error
        return session
    }
}

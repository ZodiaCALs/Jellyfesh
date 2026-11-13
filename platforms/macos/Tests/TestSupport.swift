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
    var mutedResult = false
    /// When set, every query throws it.
    var failure: CoreClientError?

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
    func shutdown() {}

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
    func playlistEntries(id: Int64, offset: Int, limit: Int) async throws -> [PlaylistRow] { try check(); return [] }
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

    static func result(_ id: String, source: String, kind: ResultKind = .track,
                       track: TrackReference? = nil) -> SourceResult {
        SourceResult(source: source, kind: kind, itemID: id, title: "Item \(id)", subtitle: nil, album: nil,
                     durationMs: nil, artwork: nil, playable: true, unplayableReason: nil, track: track, alsoIn: [])
    }
}

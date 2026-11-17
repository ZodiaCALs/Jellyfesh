// The client the app runs on when the core could not start (a damaged library file, a full
// disk): every query fails with the reason and commands go nowhere, so the window can explain
// what happened instead of the app quitting.

import Foundation

final class OfflineCoreClient: CoreClient {
    let reason: String

    init(reason: String) {
        self.reason = reason
    }

    func start(onEvents: @escaping @MainActor ([CoreEvent]) -> Void) -> CoreState {
        .offline
    }

    @discardableResult
    func send(_ command: CoreCommand) -> CommandID { 0 }

    func monotonicNowNs() -> UInt64 {
        DispatchTime.now().uptimeNanoseconds
    }

    /// None: without the core there is no equalizer to set.
    func equalizerPresets() -> [EqualizerPreset] { [] }

    func shutdown() {}

    func registerExternalPlayer(id: String, name: String, player: AppPlayer) throws {
        throw CoreClientError.core(reason)
    }

    func unregisterExternalPlayer(id: String) {}

    private func fail<T>() throws -> T {
        throw CoreClientError.core(reason)
    }

    func albums(order: AlbumOrder, descending: Bool, offset: Int, limit: Int) async throws -> [AlbumSummary] { try fail() }
    func artists(offset: Int, limit: Int) async throws -> [ArtistSummary] { try fail() }
    func songs(_ query: SongQuery) async throws -> [Song] { try fail() }
    func song(id: Int64) async throws -> Song? { try fail() }
    func songs(ids: [Int64]) async throws -> [Int64: Song] { try fail() }
    func albumSongs(albumID: Int64) async throws -> [Song] { try fail() }
    func artistAlbums(artistID: Int64) async throws -> [AlbumSummary] { try fail() }
    func searchLibrary(_ text: String, limit: Int) async throws -> LibrarySearch { try fail() }
    func recentlyAdded(limit: Int) async throws -> [Song] { try fail() }
    func recentlyPlayed(limit: Int) async throws -> [SongRow] { try fail() }
    func mostPlayed(limit: Int) async throws -> [SongRow] { try fail() }
    func playlists() async throws -> [PlaylistInfo] { try fail() }
    func playlist(id: Int64) async throws -> PlaylistInfo? { try fail() }
    func playlistEntries(id: Int64, offset: Int, limit: Int) async throws -> [PlaylistRow] { try fail() }
    func exportPlaylist(id: Int64, to path: String) async throws { throw CoreClientError.core(reason) }
    func favorites(offset: Int, limit: Int) async throws -> [SongRow] { try fail() }
    func isFavorite(_ track: TrackReference) async throws -> Bool { try fail() }
    func favoriteTracks(among tracks: [TrackReference]) async throws -> Set<TrackReference> { try fail() }
    func artworkThumbnail(hash: String, sizePx: Int) async throws -> String? { try fail() }
    func queueWindow(offset: Int, limit: Int) async throws -> [QueueEntry] { try fail() }
    func roots() async throws -> [String] { try fail() }
    func stats() async throws -> LibraryStatistics { try fail() }
    func outputDevices() async throws -> [OutputDevice] { try fail() }
    func isMuted() async -> Bool { false }
}

extension CoreState {
    /// Every slice empty, with default settings: the state before the core says anything.
    static let offline = CoreState(
        playback: PlaybackState(
            version: 0, status: .stopped, anchor: .zero, volume: 1,
            eq: EqualizerConfig(enabled: false, preampDb: 0, bands: []),
            replayGain: .off, preampDb: 0
        ),
        queue: .empty,
        nowPlaying: nil,
        lyrics: nil,
        theme: nil,
        libraryRevision: 0,
        scanning: false,
        settings: .defaults,
        devices: .empty,
        sources: .empty,
        muted: false
    )
}

extension AppSettings {
    /// The core's defaults (`Settings::default()`), for the frames before its snapshot.
    static let defaults = AppSettings(
        appearance: .system,
        style: .artwork,
        appIcon: "automatic",
        hueSource: .artwork,
        reduceEffects: false,
        transparency: 0.5,
        lyricsSize: .medium,
        lyricsAlign: .start,
        showTranslation: true,
        showRomanization: true,
        crossfade: CrossfadeConfig(enabled: false, durationMs: 6_000, albumContinuity: true),
        replayGain: .off,
        preampDb: 0,
        eq: EqualizerConfig(enabled: false, preampDb: 0, bands: []),
        outputDevice: nil,
        resumeOnLaunch: false,
        watchFolders: true,
        allowCommunityPlugins: false
    )
}

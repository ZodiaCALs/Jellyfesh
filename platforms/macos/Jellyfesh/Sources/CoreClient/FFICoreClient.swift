// The real CoreClient: the UniFFI-generated JellyfeshCore (docs/ffi.md).

import Foundation
import JellyfeshFFI

/// Talks to the Rust core through the generated bindings.
///
/// Commands go straight to the core (they only queue). Queries are synchronous in the core, so
/// they run on a serial background queue and resume the caller when done. Events arrive on the
/// core's event thread; they are converted there and handed to the main thread in batches.
final class FFICoreClient: CoreClient, @unchecked Sendable {
    private let core: JellyfeshCore
    private let queries = DispatchQueue(label: "org.jellyfesh.core-queries", qos: .userInitiated)
    private var listener: EventListener?

    /// Opens the library and session in `dataDirectory` and starts the core; audio opens on first
    /// playback. `bundledPlugins` are the official `.jfpkg` packages inside the app, which the core
    /// installs on first start and updates with the app.
    init(dataDirectory: String, cacheDirectory: String, bundledPlugins: [String] = []) throws {
        var config = CoreConfig(dataDir: dataDirectory, cacheDir: cacheDirectory, output: .device(name: nil))
        // `CoreConfig.bundledPlugins` arrives with the sources work in the core (Config::bundled_plugins).
        // scripts/build.sh defines JF_BUNDLED_PLUGINS when the generated bindings have the field, so
        // this builds against bindings from before and after it.
        #if JF_BUNDLED_PLUGINS
        config.bundledPlugins = bundledPlugins
        #endif
        do {
            core = try JellyfeshCore(config: config)
        } catch {
            throw CoreClientError(error)
        }
    }

    /// The official plugin packages the build copied into the app (`Resources/Plugins/*.jfpkg`).
    static func bundledPluginPaths(in bundle: Bundle = .main) -> [String] {
        guard let folder = bundle.resourceURL?.appendingPathComponent("Plugins", isDirectory: true),
              let names = try? FileManager.default.contentsOfDirectory(atPath: folder.path)
        else { return [] }
        return names.filter { $0.hasSuffix(".jfpkg") }.sorted().map { folder.appendingPathComponent($0).path }
    }

    /// The folders the core picks by default (`~/Library/Application Support/Jellyfesh` and
    /// `~/Library/Caches/Jellyfesh`, or `JF_DATA_DIR` and `JF_CACHE_DIR`).
    static func defaultDirectories() -> (data: String, cache: String) {
        let fm = FileManager.default
        let support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
            .appendingPathComponent("Jellyfesh").path ?? NSHomeDirectory() + "/Library/Application Support/Jellyfesh"
        let caches = fm.urls(for: .cachesDirectory, in: .userDomainMask).first?
            .appendingPathComponent("Jellyfesh").path ?? NSHomeDirectory() + "/Library/Caches/Jellyfesh"
        return (defaultDataDir() ?? support, defaultCacheDir() ?? caches)
    }

    /// The version of the core library.
    static var coreVersion: String { libraryVersion() }

    func start(onEvents: @escaping @MainActor ([CoreEvent]) -> Void) -> CoreState {
        let listener = EventListener(deliver: onEvents)
        self.listener = listener
        // Listener first, then the snapshot: nothing between the two is lost (docs/ffi.md).
        core.setListener(listener: listener)
        return CoreState(core.snapshot())
    }

    @discardableResult
    func send(_ command: CoreCommand) -> CommandID {
        core.send(command: command.ffi)
    }

    func monotonicNowNs() -> UInt64 {
        JellyfeshFFI.monotonicNowNs()
    }

    func equalizerPresets() -> [EqualizerPreset] {
        JellyfeshFFI.eqPresets().map(EqualizerPreset.init)
    }

    func shutdown() {
        core.shutdown()
        listener = nil
    }

    func registerExternalPlayer(id: String, name: String, player: AppPlayer) throws {
        do {
            try core.registerExternalPlayer(id: id, name: name, player: FFIExternalPlayer(player))
        } catch {
            throw CoreClientError(error)
        }
    }

    func unregisterExternalPlayer(id: String) {
        core.unregisterExternalPlayer(id: id)
    }

    // MARK: Queries

    private func query<T>(_ run: @escaping (JellyfeshCore) throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queries.async { [core] in
                do {
                    continuation.resume(returning: try run(core))
                } catch {
                    continuation.resume(throwing: CoreClientError(error))
                }
            }
        }
    }

    func albums(order: AlbumOrder, descending: Bool, offset: Int, limit: Int) async throws -> [AlbumSummary] {
        try await query {
            try $0.albumsSorted(sort: order.ffi, descending: descending, offset: UInt32(clamping: offset),
                                limit: UInt32(clamping: limit)).map(AlbumSummary.init)
        }
    }

    func artists(offset: Int, limit: Int) async throws -> [ArtistSummary] {
        try await query {
            try $0.artists(offset: UInt32(clamping: offset), limit: UInt32(clamping: limit)).map(ArtistSummary.init)
        }
    }

    func songs(_ songQuery: SongQuery) async throws -> [Song] {
        let ffiQuery = songQuery.ffi
        return try await query { try $0.tracks(query: ffiQuery).map(Song.init) }
    }

    func song(id: Int64) async throws -> Song? {
        try await query { try $0.track(id: id).map(Song.init) }
    }

    func songs(ids: [Int64]) async throws -> [Int64: Song] {
        try await query { core in
            var found: [Int64: Song] = [:]
            for id in ids where found[id] == nil {
                if let track = try core.track(id: id) {
                    found[id] = Song(track)
                }
            }
            return found
        }
    }

    func albumSongs(albumID: Int64) async throws -> [Song] {
        try await query { try $0.albumTracks(albumId: albumID).map(Song.init) }
    }

    func artistAlbums(artistID: Int64) async throws -> [AlbumSummary] {
        try await query { try $0.artistAlbums(artistId: artistID).map(AlbumSummary.init) }
    }

    func searchLibrary(_ text: String, limit: Int) async throws -> LibrarySearch {
        try await query { LibrarySearch(try $0.searchLibrary(text: text, limit: UInt32(clamping: limit))) }
    }

    func recentlyAdded(limit: Int) async throws -> [Song] {
        try await query { try $0.recentlyAdded(limit: UInt32(clamping: limit)).map(Song.init) }
    }

    func recentlyPlayed(limit: Int) async throws -> [SongRow] {
        try await query { try $0.recentlyPlayed(limit: UInt32(clamping: limit)).map(SongRow.init) }
    }

    func mostPlayed(limit: Int) async throws -> [SongRow] {
        try await query { try $0.mostPlayed(limit: UInt32(clamping: limit)).map(SongRow.init) }
    }

    func playlists() async throws -> [PlaylistInfo] {
        try await query { try $0.playlists().map(PlaylistInfo.init) }
    }

    func playlist(id: Int64) async throws -> PlaylistInfo? {
        try await query { try $0.playlist(id: id).map(PlaylistInfo.init) }
    }

    func playlistEntries(id: Int64, offset: Int, limit: Int) async throws -> [PlaylistRow] {
        try await query {
            try $0.playlistEntries(id: id, offset: UInt32(clamping: offset), limit: UInt32(clamping: limit))
                .map(PlaylistRow.init)
        }
    }

    func exportPlaylist(id: Int64, to path: String) async throws {
        try await query { try $0.exportPlaylist(id: id, path: path) }
    }

    func favorites(offset: Int, limit: Int) async throws -> [SongRow] {
        try await query {
            try $0.favorites(offset: UInt32(clamping: offset), limit: UInt32(clamping: limit)).map(SongRow.init)
        }
    }

    func isFavorite(_ track: TrackReference) async throws -> Bool {
        let ref = track.ffi
        return try await query { try $0.isFavorite(track: ref) }
    }

    func favoriteTracks(among tracks: [TrackReference]) async throws -> Set<TrackReference> {
        try await query { core in
            var found = Set<TrackReference>()
            for track in tracks {
                if try core.isFavorite(track: track.ffi) {
                    found.insert(track)
                }
            }
            return found
        }
    }

    func artworkThumbnail(hash: String, sizePx: Int) async throws -> String? {
        try await query { try $0.artworkThumbnail(hash: hash, sizePx: UInt32(clamping: sizePx)) }
    }

    func queueWindow(offset: Int, limit: Int) async throws -> [QueueEntry] {
        try await query {
            try $0.queueWindow(offset: UInt32(clamping: offset), limit: UInt32(clamping: limit)).map(QueueEntry.init)
        }
    }

    func roots() async throws -> [String] {
        try await query { try $0.roots() }
    }

    func stats() async throws -> LibraryStatistics {
        try await query { LibraryStatistics(try $0.stats()) }
    }

    func outputDevices() async throws -> [OutputDevice] {
        try await query { _ in try JellyfeshFFI.outputDevices().map(OutputDevice.init) }
    }

    func isMuted() async -> Bool {
        (try? await query { $0.snapshot().muted }) ?? false
    }
}

/// Receives events on the core's event thread. Converts each one there (lyrics and themes can be
/// large) and hands them to the main thread in batches, dropping slices a later one in the same
/// batch replaces. Never blocks: `shutdown` and `setListener` wait for a delivery in progress.
private final class EventListener: CoreListener, @unchecked Sendable {
    private let lock = NSLock()
    private var pending: [CoreEvent] = []
    private var scheduled = false
    private let deliver: @MainActor ([CoreEvent]) -> Void

    init(deliver: @escaping @MainActor ([CoreEvent]) -> Void) {
        self.deliver = deliver
    }

    func onEvent(event: Event) {
        let converted = CoreEvent(event)
        lock.lock()
        pending.append(converted)
        let schedule = !scheduled
        scheduled = true
        lock.unlock()
        guard schedule else { return }
        DispatchQueue.main.async { [self] in
            lock.lock()
            let batch = pending
            pending = []
            scheduled = false
            lock.unlock()
            MainActor.assumeIsolated {
                deliver(EventBatch.coalesced(batch))
            }
        }
    }
}

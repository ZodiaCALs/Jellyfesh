// The only way the app reaches the core (ADR-0001): commands in, events and a snapshot out,
// library queries off the main thread. FFICoreClient is the real one; tests use a fake.

import Foundation

/// Why a query failed (`CoreFfiError`).
enum CoreClientError: Error, Equatable, LocalizedError {
    /// A library, session, audio or file error; the message says which.
    case core(String)
    /// Not in this build yet: parts of the core arrive during v0.0.3 (source commands).
    case notAvailable(String)
    /// The core was shut down.
    case shutDown
    /// An argument the core could not use.
    case invalidArgument(String)

    var errorDescription: String? {
        switch self {
        case .core(let message), .notAvailable(let message), .invalidArgument(let message):
            return message
        case .shutDown:
            return String(localized: "The player has stopped.")
        }
    }

    var isNotAvailable: Bool {
        if case .notAvailable = self { return true }
        return false
    }
}

/// The core says a feature arrives later in this release by ending its message with this text
/// (jf-ffi `error.rs`); command failures carry only the message.
let notAvailableSuffix = "not available in this build yet"

func isNotAvailableMessage(_ message: String) -> Bool {
    message.hasSuffix(notAvailableSuffix)
}

protocol CoreClient: AnyObject {
    /// Starts delivering events, in order and in batches, on the main thread, and returns the state
    /// so far. Call once.
    func start(onEvents: @escaping @MainActor ([CoreEvent]) -> Void) -> CoreState

    /// Queues a command; a failure comes back as `.commandFailed` with the returned id (0 when the
    /// core has shut down).
    @discardableResult
    func send(_ command: CoreCommand) -> CommandID

    /// Nanoseconds on the clock playback anchors are stamped with. Cheap: no core call.
    func monotonicNowNs() -> UInt64

    /// The equalizer presets, in the order every app lists them (`eq_presets()`). Cheap: a fixed
    /// list from the core's library, not a query of the running core.
    func equalizerPresets() -> [EqualizerPreset]

    /// Saves the session and stops the core. Afterwards queries throw `.shutDown`.
    func shutdown()

    // Queries: synchronous in the core, run off the main thread here.
    func albums(order: AlbumOrder, descending: Bool, offset: Int, limit: Int) async throws -> [AlbumSummary]
    func artists(offset: Int, limit: Int) async throws -> [ArtistSummary]
    func songs(_ query: SongQuery) async throws -> [Song]
    func song(id: Int64) async throws -> Song?
    /// Several library tracks in one trip to the query thread (queue rows name only track ids);
    /// ids that are gone are left out.
    func songs(ids: [Int64]) async throws -> [Int64: Song]
    func albumSongs(albumID: Int64) async throws -> [Song]
    func artistAlbums(artistID: Int64) async throws -> [AlbumSummary]
    func searchLibrary(_ text: String, limit: Int) async throws -> LibrarySearch
    func recentlyAdded(limit: Int) async throws -> [Song]
    func recentlyPlayed(limit: Int) async throws -> [SongRow]
    func mostPlayed(limit: Int) async throws -> [SongRow]
    func playlists() async throws -> [PlaylistInfo]
    func playlist(id: Int64) async throws -> PlaylistInfo?
    func playlistEntries(id: Int64, offset: Int, limit: Int) async throws -> [PlaylistRow]
    func exportPlaylist(id: Int64, to path: String) async throws
    func favorites(offset: Int, limit: Int) async throws -> [SongRow]
    func isFavorite(_ track: TrackReference) async throws -> Bool
    /// Which of `tracks` are favourites, in one trip to the query thread (library rows do not
    /// carry the flag; playlist and favourite rows do).
    func favoriteTracks(among tracks: [TrackReference]) async throws -> Set<TrackReference>
    /// A cached JPEG of the artwork with this content hash at about `sizePx` (64 to 1024).
    func artworkThumbnail(hash: String, sizePx: Int) async throws -> String?
    func queueWindow(offset: Int, limit: Int) async throws -> [QueueEntry]
    func roots() async throws -> [String]
    func stats() async throws -> LibraryStatistics
    func outputDevices() async throws -> [OutputDevice]
    /// Whether output is muted. The playback slice does not carry it: `SetMuted` republishes the
    /// slice, and the state is read from the snapshot then (as the Linux app does).
    func isMuted() async -> Bool
}

// The library as the core reports it: its revision (every list queries again when it moves),
// folders, scans, playlists and favourites, and the commands that change them.

import Foundation
import Observation

@MainActor
@Observable
final class LibraryStore {
    let client: CoreClient
    private let tracker: CommandTracker
    private let toasts: ToastCenter

    /// Grows whenever the catalogue, a playlist, a favourite or a play count changes.
    private(set) var revision: UInt64 = 0
    private(set) var scanning = false
    /// The latest scan progress or result.
    private(set) var scan: ScanState?
    private(set) var roots: [String] = []
    private(set) var stats: LibraryStatistics?
    private(set) var playlists: [PlaylistInfo] = []
    /// A playlist created from this window, to open when the core names it.
    private(set) var createdPlaylist: Int64?

    @ObservationIgnored private var pendingCreate: CommandID?
    @ObservationIgnored private var loadedRevision: UInt64?

    init(client: CoreClient, tracker: CommandTracker, toasts: ToastCenter) {
        self.client = client
        self.tracker = tracker
        self.toasts = toasts
    }

    // MARK: Events

    func apply(revision value: UInt64) {
        guard value != revision else { return }
        revision = value
        Task { await refresh() }
    }

    func apply(scanning value: Bool) {
        scanning = value
    }

    func apply(scan state: ScanState) {
        scan = state
        if case .progress = state {
            scanning = true
        } else {
            scanning = false
            Task { await refresh() }
        }
    }

    func playlistCreated(request: CommandID, id: Int64) {
        guard request == pendingCreate else { return }
        pendingCreate = nil
        createdPlaylist = id
    }

    func consumeCreatedPlaylist() -> Int64? {
        defer { createdPlaylist = nil }
        return createdPlaylist
    }

    /// Folders, counts and playlists, queried again when the revision moved.
    func refresh(force: Bool = false) async {
        let current = revision
        guard force || loadedRevision != current else { return }
        loadedRevision = current
        async let roots = client.roots()
        async let stats = client.stats()
        async let playlists = client.playlists()
        if let value = try? await roots { self.roots = value }
        if let value = try? await stats { self.stats = value }
        if let value = try? await playlists { self.playlists = value }
    }

    var isEmpty: Bool { (stats?.tracks ?? 0) == 0 && roots.isEmpty }

    // MARK: Folders

    func addFolder(_ path: String) {
        send(.addRoot(path: path))
    }

    func removeFolder(_ path: String) {
        send(.removeRoot(path: path))
    }

    func rescan() {
        send(.rescan)
    }

    // MARK: Playlists

    func createPlaylist(name: String, tracks: [TrackReference] = []) {
        pendingCreate = send(.createPlaylist(name: name, tracks: tracks))
    }

    func renamePlaylist(_ id: Int64, to name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        send(.renamePlaylist(id: id, name: trimmed))
    }

    func deletePlaylist(_ id: Int64) {
        send(.deletePlaylist(id: id))
    }

    func add(_ tracks: [TrackReference], toPlaylist id: Int64) {
        guard !tracks.isEmpty else { return }
        send(.addToPlaylist(id: id, tracks: tracks))
    }

    func removeEntries(_ entries: [Int64], fromPlaylist id: Int64) {
        guard !entries.isEmpty else { return }
        send(.removeFromPlaylist(id: id, entries: entries))
    }

    /// Moves a playlist entry; indices as SwiftUI's `onMove` gives them.
    func moveEntry(from: Int, to: Int, in rows: [PlaylistRow], playlist id: Int64) {
        guard rows.indices.contains(from) else { return }
        let destination = to > from ? to - 1 : to
        guard destination != from else { return }
        send(.movePlaylistEntry(id: id, entry: rows[from].entryID, to: UInt32(clamping: destination)))
    }

    func importPlaylist(_ path: String) {
        pendingCreate = send(.importPlaylist(path: path))
    }

    func exportPlaylist(_ id: Int64, to path: String) async {
        do {
            try await client.exportPlaylist(id: id, to: path)
        } catch {
            toasts.show(CoreClientError.describe(error))
        }
    }

    // MARK: Favourites

    func setFavorite(_ track: TrackReference, _ favorite: Bool) {
        send(.setFavorite(track, favorite: favorite))
    }

    @discardableResult
    private func send(_ command: CoreCommand) -> CommandID {
        let id = client.send(command)
        tracker.watch(id) { [weak self] message in
            self?.toasts.show(message)
        }
        return id
    }
}

extension CoreClientError {
    /// A message for any error a query throws.
    static func describe(_ error: Error) -> String {
        (error as? CoreClientError)?.errorDescription ?? error.localizedDescription
    }
}

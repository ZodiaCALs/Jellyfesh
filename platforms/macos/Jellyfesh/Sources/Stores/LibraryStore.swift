// The library as the core reports it: its revision (every list queries again when it moves),
// folders, scans, playlists and favourites, and the commands that change them.

import Foundation
import Observation

/// A New Playlist… waiting for its name: the window asks for one before anything is created
/// (docs/ui.md). `contents` is what the playlist starts with: no songs (`.tracks([])`), the songs
/// chosen, or an album, artist or playlist page's songs.
struct NewPlaylistRequest: Identifiable, Equatable {
    let id = UUID()
    var contents: CollectionReference
}

@MainActor
@Observable
final class LibraryStore {
    /// Rows per query when a whole collection is read (`tracks(of:)`).
    static let pageSize = 500
    /// At most this many songs go into a playlist at once.
    static let trackLimit = 100_000

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
    /// The New Playlist… whose name the window is asking for.
    private(set) var newPlaylist: NewPlaylistRequest?

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

    /// New Playlist…: asks for the name first (the window shows the question), then makes the
    /// playlist with `contents`.
    func askForNewPlaylist(with contents: CollectionReference = .tracks([])) {
        newPlaylist = NewPlaylistRequest(contents: contents)
    }

    /// The name was given: makes the playlist. A blank name gives the default one.
    func createNewPlaylist(named name: String) {
        guard let request = newPlaylist else { return }
        newPlaylist = nil
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let title = trimmed.isEmpty ? String(localized: "New Playlist") : trimmed
        if case .tracks(let tracks) = request.contents {
            createPlaylist(name: title, tracks: tracks)
            return
        }
        Task { [weak self] in
            guard let self else { return }
            do {
                let tracks = try await self.tracks(of: request.contents)
                self.createPlaylist(name: title, tracks: tracks)
            } catch {
                self.toasts.show(CoreClientError.describe(error))
            }
        }
    }

    /// The question was dismissed: nothing is made.
    func cancelNewPlaylist() {
        newPlaylist = nil
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

    /// Add to Playlist… for songs, or for a whole album, artist or playlist: the core adds tracks,
    /// so a collection's are read first, in its order.
    func add(_ collection: CollectionReference, toPlaylist id: Int64) {
        if case .tracks(let tracks) = collection {
            add(tracks, toPlaylist: id)
            return
        }
        Task { [weak self] in
            guard let self else { return }
            do {
                let tracks = try await self.tracks(of: collection)
                self.add(tracks, toPlaylist: id)
            } catch {
                self.toasts.show(CoreClientError.describe(error))
            }
        }
    }

    /// A library collection's tracks in its order (the core's queries page them). A source's
    /// containers have none here: the core plays them, but cannot list them for a playlist.
    func tracks(of collection: CollectionReference) async throws -> [TrackReference] {
        let client = self.client
        switch collection {
        case .tracks(let tracks):
            return tracks
        case .album(let id):
            return try await client.albumSongs(albumID: id).map(\.reference)
        case .artist(let id):
            let songs: [Song] = try await Self.everyPage { offset, limit in
                try await client.songs(SongQuery(offset: offset, limit: limit, sort: .album, artistID: id))
            }
            return songs.map(\.reference)
        case .playlist(let id):
            let rows: [PlaylistRow] = try await Self.everyPage { offset, limit in
                try await client.playlistEntries(id: id, offset: offset, limit: limit)
            }
            return rows.map(\.row.track)
        case .favorites:
            let rows: [SongRow] = try await Self.everyPage { offset, limit in
                try await client.favorites(offset: offset, limit: limit)
            }
            return rows.map(\.track)
        case .source:
            return []
        }
    }

    /// Every page of a paged query, in order, up to `trackLimit` rows.
    private static func everyPage<Item>(
        _ fetch: (_ offset: Int, _ limit: Int) async throws -> [Item]
    ) async throws -> [Item] {
        var items: [Item] = []
        var page: [Item] = []
        repeat {
            page = try await fetch(items.count, pageSize)
            items.append(contentsOf: page)
        } while page.count == pageSize && items.count < trackLimit
        return items
    }

    // MARK: Lookups for a song's menu

    /// A library track, for what its row does not carry (its album, its file).
    func song(id: Int64) async -> Song? {
        try? await client.song(id: id)
    }

    /// The library's artist with this name (Go to Artist): songs carry only the artist's name.
    func artist(named name: String) async -> ArtistSummary? {
        guard !name.isEmpty, let found = try? await client.searchLibrary(name, limit: 25) else { return nil }
        return found.artists.first { $0.name.localizedCaseInsensitiveCompare(name) == .orderedSame }
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

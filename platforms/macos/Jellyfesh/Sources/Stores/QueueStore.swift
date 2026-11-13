// The queue as rows to show: the queue slice names entries by track reference only, so library
// tracks are looked up (in one trip per batch) and kept until the library changes. Up Next shows
// the slice's 50 entries and pages the rest with `queueWindow`.

import Foundation
import Observation

/// What a queue row shows.
struct QueueRow: Identifiable, Hashable {
    var entry: QueueEntry
    var title: String
    var subtitle: String?
    var durationMs: UInt64?
    var artwork: ArtworkSource?
    /// False until a library track has been looked up.
    var resolved: Bool

    var id: UInt64 { entry.uid }
}

@MainActor
@Observable
final class QueueStore {
    let client: CoreClient

    private(set) var queue: QueueState = .empty
    /// Up Next in playback order: the slice's window and any pages loaded after it.
    private(set) var upNext: [QueueEntry] = []
    private(set) var isLoadingMore = false
    /// Library tracks looked up for display.
    private(set) var songs: [Int64: Song] = [:]

    @ObservationIgnored private var resolving = Set<Int64>()
    @ObservationIgnored private var extraPages = false

    init(client: CoreClient) {
        self.client = client
    }

    func apply(_ state: QueueState) {
        queue = state
        upNext = state.upNext
        var entries = state.upNext
        if let current = state.current { entries.append(current) }
        resolve(entries)
        if extraPages {
            extraPages = false
            Task { await loadMore() }
        }
    }

    /// The library changed: titles and artwork may have too.
    func libraryChanged() {
        songs = [:]
        resolving = []
        var entries = upNext
        if let current = queue.current { entries.append(current) }
        resolve(entries)
    }

    var current: QueueRow? {
        queue.current.map(row(for:))
    }

    var upNextRows: [QueueRow] {
        upNext.map(row(for:))
    }

    /// Entries after the loaded ones.
    var hasMore: Bool {
        let position = queue.position.map { Int($0) + 1 } ?? 0
        return position + upNext.count < Int(queue.length)
    }

    /// Loads the next page of Up Next.
    func loadMore() async {
        guard hasMore, !isLoadingMore else { return }
        isLoadingMore = true
        defer { isLoadingMore = false }
        let start = (queue.position.map { Int($0) + 1 } ?? 0) + upNext.count
        let version = queue.version
        guard let page = try? await client.queueWindow(offset: start, limit: 100), version == queue.version else { return }
        let known = Set(upNext.map(\.uid))
        upNext.append(contentsOf: page.filter { !known.contains($0.uid) })
        extraPages = true
        resolve(page)
    }

    func row(for entry: QueueEntry) -> QueueRow {
        switch entry.track {
        case .library(let id):
            if let song = songs[id] {
                return QueueRow(entry: entry, title: song.title, subtitle: song.artist, durationMs: song.durationMs,
                                artwork: song.artworkHash.map(ArtworkSource.hash), resolved: true)
            }
            return QueueRow(entry: entry, title: "", subtitle: nil, durationMs: nil, artwork: nil, resolved: false)
        case .file(let path):
            let name = (path as NSString).lastPathComponent
            return QueueRow(entry: entry, title: (name as NSString).deletingPathExtension, subtitle: nil,
                            durationMs: nil, artwork: nil, resolved: true)
        case .source(let song):
            return QueueRow(entry: entry, title: song.title, subtitle: song.artist, durationMs: song.durationMs,
                            artwork: song.artworkURL.map(ArtworkSource.url), resolved: true)
        }
    }

    private func resolve(_ entries: [QueueEntry]) {
        var missing: [Int64] = []
        for entry in entries {
            if case .library(let id) = entry.track, songs[id] == nil, !resolving.contains(id) {
                missing.append(id)
                resolving.insert(id)
            }
        }
        guard !missing.isEmpty else { return }
        Task { [weak self] in
            guard let self else { return }
            let found = (try? await self.client.songs(ids: missing)) ?? [:]
            for id in missing { self.resolving.remove(id) }
            guard !found.isEmpty else { return }
            self.songs.merge(found) { _, new in new }
        }
    }
}

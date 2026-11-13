// Paged lists for views that can be long (albums, artists, songs, favourites, playlist entries):
// the core pages (docs/architecture.md §5), the view asks for the next page as it scrolls near the
// end, and a new library revision queries the loaded range again in one go.

import Foundation
import Observation

@MainActor
@Observable
final class PagedList<Item: Identifiable> {
    private(set) var items: [Item] = []
    private(set) var isLoading = false
    private(set) var reachedEnd = false
    private(set) var error: String?
    /// The revision the items were loaded at; nil before the first load.
    private(set) var loadedRevision: UInt64?

    let pageSize: Int
    private let fetch: (_ offset: Int, _ limit: Int) async throws -> [Item]
    @ObservationIgnored private var ids = Set<Item.ID>()
    @ObservationIgnored private var generation = 0

    init(pageSize: Int = 200, fetch: @escaping (_ offset: Int, _ limit: Int) async throws -> [Item]) {
        self.pageSize = pageSize
        self.fetch = fetch
    }

    /// Loads the first page, or queries the loaded range again when the library moved on.
    func load(revision: UInt64) async {
        guard loadedRevision != revision else { return }
        await reload(revision: revision)
    }

    func reload(revision: UInt64) async {
        generation += 1
        let mine = generation
        let count = max(items.count, pageSize)
        isLoading = true
        defer { if mine == generation { isLoading = false } }
        do {
            let page = try await fetch(0, count)
            guard mine == generation else { return }
            replace(with: page)
            reachedEnd = page.count < count
            loadedRevision = revision
            error = nil
        } catch {
            guard mine == generation else { return }
            self.error = CoreClientError.describe(error)
        }
    }

    /// Call from a row's `onAppear`: loads the next page when the row is near the end.
    func rowAppeared(_ item: Item) {
        guard !reachedEnd, !isLoading else { return }
        let threshold = max(0, items.count - pageSize / 4)
        guard let index = items.lastIndex(where: { $0.id == item.id }), index >= threshold else { return }
        Task { await loadMore() }
    }

    /// The same, for a row known by its index in `items`.
    func rowAppeared(index: Int) {
        guard !reachedEnd, !isLoading, index >= max(0, items.count - pageSize / 4) else { return }
        Task { await loadMore() }
    }

    func loadMore() async {
        guard !isLoading, !reachedEnd else { return }
        let mine = generation
        isLoading = true
        defer { if mine == generation { isLoading = false } }
        do {
            let page = try await fetch(items.count, pageSize)
            guard mine == generation else { return }
            append(page)
            reachedEnd = page.count < pageSize
        } catch {
            guard mine == generation else { return }
            self.error = CoreClientError.describe(error)
        }
    }

    private func replace(with page: [Item]) {
        ids.removeAll(keepingCapacity: true)
        var unique: [Item] = []
        unique.reserveCapacity(page.count)
        for item in page where ids.insert(item.id).inserted {
            unique.append(item)
        }
        items = unique
    }

    /// A page can repeat rows when the library changed between pages; lists need unique ids.
    private func append(_ page: [Item]) {
        var added = items
        for item in page where ids.insert(item.id).inserted {
            added.append(item)
        }
        items = added
    }
}

/// A row of a track list that has no id of its own (favourites, history), keyed by its track.
struct TrackRowItem: Identifiable, Hashable {
    var row: SongRow
    var id: TrackReference { row.track }
}

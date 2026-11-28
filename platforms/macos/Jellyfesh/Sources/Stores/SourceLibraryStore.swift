// The sources' likes and playlists (ADR-0024, docs/ui.md "Favourites", "Playlists"): the core's
// `SourceLibrary` slice, as Favourites and Playlists show it after the library's own. The lists
// come from the core's cache at once; Favourites and Playlists ask for fresh ones when they open,
// and again when they are returned to after ten minutes (the core never polls). More of a list
// comes a page at a time; Save as Playlist… copies a source playlist into Jellyfesh.

import Foundation
import Observation

@MainActor
@Observable
final class SourceLibraryStore {
    /// The pages that refresh the lists when they open.
    enum Page: Hashable {
        case favourites, playlists
    }

    /// How long a refresh lasts before a page that opens asks again.
    nonisolated static let refreshInterval: TimeInterval = 10 * 60

    let client: CoreClient
    private let tracker: CommandTracker
    private let toasts: ToastCenter
    private let now: () -> Date

    private(set) var state: SourceLibraryState = .empty
    /// More of a list in flight, by source and part.
    private(set) var loadingMore: Set<String> = []

    @ObservationIgnored private var refreshed: [Page: Date] = [:]
    @ObservationIgnored private var moreRequests: [CommandID: String] = [:]

    init(client: CoreClient, tracker: CommandTracker, toasts: ToastCenter, now: @escaping () -> Date = Date.init) {
        self.client = client
        self.tracker = tracker
        self.toasts = toasts
        self.now = now
    }

    func apply(_ state: SourceLibraryState) {
        self.state = state
    }

    /// Favourites or Playlists opened: fresh lists, unless that page asked less than ten minutes ago.
    func pageOpened(_ page: Page) {
        let time = now()
        if let last = refreshed[page], time.timeIntervalSince(last) < Self.refreshInterval { return }
        refreshed[page] = time
        let id = client.send(.refreshSourceLibrary(plugin: nil))
        tracker.watch(id) { [weak self] message in
            // A build without it yet says so calmly; the cached lists stay.
            guard !isNotAvailableMessage(message) else { return }
            self?.toasts.show(message)
        }
    }

    // MARK: What the pages show

    /// The sources Favourites shows (docs/ui.md, "Favourites"): each with its liked songs, albums
    /// and artists that have entries, are loading (a spinner) or failed (why). A source whose likes
    /// all came back empty still shows its name, saying it likes nothing yet; one whose lists have
    /// not come back yet shows nothing.
    var likes: [SourceLikes] {
        state.sources.compactMap { source in
            let own = source.lists.filter { $0.part != .playlists }
            let shown = own.filter { Self.shows($0) }
            if !shown.isEmpty {
                return SourceLikes(source: Self.source(source, lists: shown), nothingLiked: false)
            }
            let empty = own.filter { Self.cameBackEmpty($0) }
            return empty.isEmpty ? nil : SourceLikes(source: Self.source(source, lists: empty), nothingLiked: true)
        }
    }

    /// Some source shows likes on Favourites (or that they load, or why they failed), not only that
    /// it likes nothing yet: Favourites is not empty.
    var anyLiked: Bool {
        likes.contains { !$0.nothingLiked }
    }

    /// The sources with playlists to show (Playlists).
    var playlists: [SourceLibraryInfo] {
        state.sources.compactMap { source in
            let lists = source.lists.filter { $0.part == .playlists && Self.shows($0) }
            return lists.isEmpty ? nil : Self.source(source, lists: lists)
        }
    }

    /// A page shows a list that has entries, is loading or failed.
    private static func shows(_ list: SourceList) -> Bool {
        !list.entries.isEmpty || list.loading || list.error != nil
    }

    /// A list that came back from its refresh with nothing in it.
    private static func cameBackEmpty(_ list: SourceList) -> Bool {
        list.entries.isEmpty && !list.loading && list.error == nil && list.refreshedAtMs != nil
    }

    private static func source(_ source: SourceLibraryInfo, lists: [SourceList]) -> SourceLibraryInfo {
        SourceLibraryInfo(plugin: source.plugin, name: source.name, attribution: source.attribution, lists: lists)
    }

    /// "Updated 5 minutes ago" for a list from the cache; nil while it loads or before it ever has.
    func updatedLine(_ list: SourceList) -> String? {
        guard !list.loading, let ms = list.refreshedAtMs else { return nil }
        return Self.updatedLine(refreshedAt: Date(timeIntervalSince1970: TimeInterval(ms) / 1000), now: now())
    }

    nonisolated static func updatedLine(refreshedAt: Date, now: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .full
        let age = formatter.localizedString(for: min(refreshedAt, now), relativeTo: now)
        return String(localized: "Updated \(age)")
    }

    // MARK: More and Save as Playlist…

    static func moreKey(_ plugin: String, _ part: SourceListPart) -> String {
        "\(plugin)\u{1F}\(part)"
    }

    /// The next page of a source's list.
    func more(plugin: String, part: SourceListPart) {
        guard let list = state.sources.first(where: { $0.plugin == plugin })?.lists.first(where: { $0.part == part }),
              let cursor = list.next
        else { return }
        let key = Self.moreKey(plugin, part)
        guard !loadingMore.contains(key) else { return }
        loadingMore.insert(key)
        let id = client.send(.sourceLibraryMore(plugin: plugin, part: part, cursor: cursor))
        moreRequests[id] = key
        tracker.watch(id) { [weak self] message in
            guard let self else { return }
            self.moreRequests[id] = nil
            self.loadingMore.remove(key)
            if !isNotAvailableMessage(message) { self.toasts.show(message) }
        }
    }

    /// A page of a list: its entries join the list, which keeps the new cursor.
    func apply(page: SourceLibraryPageBatch) {
        guard let key = moreRequests.removeValue(forKey: page.request) else { return }
        loadingMore.remove(key)
        if let error = page.error {
            toasts.show(error)
        }
        guard let source = state.sources.firstIndex(where: { $0.plugin == page.plugin }),
              let list = state.sources[source].lists.firstIndex(where: { $0.part == page.part })
        else { return }
        let shown = Set(state.sources[source].lists[list].entries.map(\.id))
        state.sources[source].lists[list].entries += page.entries.filter { !shown.contains($0.id) }
        state.sources[source].lists[list].next = page.next
    }

    /// Save as Playlist…: a Jellyfesh playlist of a source playlist's songs, named `name`. The
    /// core answers with the playlist it made (`playlistCreated` for the returned id); nil when the
    /// name is blank.
    @discardableResult
    func savePlaylistCopy(plugin: String, id: String, name: String) -> CommandID? {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let command = client.send(.savePlaylistCopy(plugin: plugin, id: id, name: trimmed))
        tracker.watch(command) { [weak self] message in
            self?.toasts.show(isNotAvailableMessage(message)
                ? String(localized: "Save as Playlist isn't available in this build yet.") : message)
        }
        return command
    }
}

/// A source as Favourites shows it (docs/ui.md, "Favourites").
struct SourceLikes: Hashable, Identifiable {
    /// The source with the lists to show; when it likes nothing yet, those that came back empty
    /// (they say when they were updated).
    var source: SourceLibraryInfo
    /// Every list came back empty: its name, then "Nothing liked in {Service} yet.".
    var nothingLiked: Bool

    var id: String { source.plugin }
}

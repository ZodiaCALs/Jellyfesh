// Unified search as it streams in (docs/core-api.md: `Search` covers the library and every enabled
// source, merged and de-duplicated by the core): one section per source in the order they answer,
// the library first. The core's library results carry no artwork, so the same library search runs
// once as a query to find it. Should the command fail (sources still being integrated), the
// library query alone fills the page. Search by source (ADR-0024 §4): the tabs All, Library and one
// per source that answered; a source whose terms forbid merging keeps its results apart (linked
// from All); a source's tab shows its results kind by kind, each with Show More (`searchMore`).

import Foundation
import Observation

/// A tab of the Search page.
enum SearchTab: Hashable {
    case all, library
    /// A source that answered, by id.
    case source(String)
}

/// One source's answer to a search.
struct SearchSection: Identifiable, Equatable {
    var source: String
    /// The All tab's rows.
    var items: [SourceResult]
    var error: String?
    /// Part of the merged results; false keeps them apart, linked from the All tab.
    var merged = true
    /// Its results kind by kind, for its tab.
    var groups: [SearchKindGroup] = []

    var id: String { source }

    /// What its tab shows: its groups with results, else its rows by kind.
    var tabGroups: [SearchKindGroup] {
        if !groups.isEmpty { return groups.filter { !$0.items.isEmpty } }
        return kinds.map { SearchKindGroup(kind: $0, items: items(of: $0), next: nil, total: nil) }
    }

    /// Kinds present, in display order (songs first).
    var kinds: [ResultKind] {
        [ResultKind.track, .album, .artist, .playlist, .folder].filter { kind in
            items.contains { $0.kind == kind }
        }
    }

    func items(of kind: ResultKind) -> [SourceResult] {
        items.filter { $0.kind == kind }
    }
}

@MainActor
@Observable
final class SearchStore {
    /// The source id the core gives the local library.
    nonisolated static let library = "library"
    /// How many results per kind the core returns from the library (its `search(query, 50)`).
    nonisolated static let libraryLimit = 50

    let client: CoreClient
    private let tracker: CommandTracker

    /// What the search field holds.
    var text = ""
    /// The words of the search whose results show.
    private(set) var query = ""
    private(set) var request: CommandID?
    private(set) var sections: [SearchSection] = []
    private(set) var done = false
    /// Artwork content hashes of library results, by kind and id.
    private(set) var libraryArtwork: [String: String] = [:]
    /// The tab shown; All for every new search.
    var tab: SearchTab = .all
    /// Show More in flight, by source and kind, and the reasons the last ones failed.
    private(set) var loadingMore: Set<String> = []
    private(set) var moreErrors: [String: String] = [:]

    @ObservationIgnored private var debounce: Task<Void, Never>?
    @ObservationIgnored private var generation = 0
    /// Show More's commands, by id: the source and kind each one asked for.
    @ObservationIgnored private var moreRequests: [CommandID: String] = [:]

    init(client: CoreClient, tracker: CommandTracker) {
        self.client = client
        self.tracker = tracker
    }

    /// Call when the field changes: searches after a short pause in typing.
    func textChanged() {
        debounce?.cancel()
        let words = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !words.isEmpty else {
            clear()
            return
        }
        guard words != query else { return }
        debounce = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(200))
            guard !Task.isCancelled else { return }
            self?.search(words)
        }
    }

    /// Searches now (Return in the field, or a search started elsewhere).
    func search(_ words: String) {
        debounce?.cancel()
        let words = words.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !words.isEmpty else {
            clear()
            return
        }
        generation += 1
        query = words
        sections = []
        done = false
        libraryArtwork = [:]
        resetTabs()
        let id = client.send(.search(query: words, area: .all))
        request = id
        let mine = generation
        tracker.watch(id) { [weak self] message in
            guard let self, self.generation == mine else { return }
            self.commandFailed(message)
        }
    }

    func clear() {
        debounce?.cancel()
        generation += 1
        query = ""
        request = nil
        sections = []
        done = false
        libraryArtwork = [:]
        resetTabs()
    }

    private func resetTabs() {
        tab = .all
        loadingMore = []
        moreErrors = [:]
        moreRequests = [:]
    }

    /// One source's answer; answers to older searches are ignored.
    func apply(_ batch: SearchBatch) {
        guard let request, batch.request == request else { return }
        let section = SearchSection(source: batch.source, items: batch.items, error: batch.error,
                                    merged: batch.merged, groups: batch.groups)
        if let index = sections.firstIndex(where: { $0.source == batch.source }) {
            sections[index] = section
        } else if batch.source == Self.library {
            sections.insert(section, at: 0)
        } else {
            sections.append(section)
        }
        if batch.done { done = true }
        if batch.source == Self.library, !section.items.isEmpty {
            findLibraryArtwork(for: batch.query)
        }
    }

    /// True while waiting for the last source.
    var isWaiting: Bool { request != nil && !done }

    /// True when every source answered and none found anything.
    var foundNothing: Bool {
        done && sections.allSatisfy { $0.items.isEmpty && $0.error == nil }
    }

    /// The library's results, shown first.
    var librarySection: SearchSection? {
        sections.first { $0.source == Self.library && (!$0.items.isEmpty || $0.error != nil) }
    }

    /// The sources' results (From Your Sources), one group per source that allows merging and found
    /// something or failed, in the order they answered.
    var sourceSections: [SearchSection] {
        sections.filter { $0.source != Self.library && $0.merged && (!$0.items.isEmpty || $0.error != nil) }
    }

    /// The sources whose terms forbid merging and that found something: the All tab links to their
    /// tabs ("Results from Spotify").
    var separateSections: [SearchSection] {
        sections.filter { $0.source != Self.library && !$0.merged && !$0.tabGroups.isEmpty }
    }

    /// All, Library, then each source that found something, in the order they answered.
    var tabs: [SearchTab] {
        guard !query.isEmpty else { return [] }
        return [.all, .library] + sections
            .filter { $0.source != Self.library && !$0.tabGroups.isEmpty }
            .map { .source($0.source) }
    }

    func section(for source: String) -> SearchSection? {
        sections.first { $0.source == source }
    }

    // MARK: Show More

    static func moreKey(_ source: String, _ kind: ResultKind) -> String {
        "\(source)\u{1F}\(kind)"
    }

    /// Show More for one kind of a source's results: the next page of the newest search.
    func showMore(source: String, kind: ResultKind) {
        guard let group = section(for: source)?.groups.first(where: { $0.kind == kind }), let cursor = group.next
        else { return }
        let key = Self.moreKey(source, kind)
        guard !loadingMore.contains(key) else { return }
        loadingMore.insert(key)
        moreErrors[key] = nil
        let id = client.send(.searchMore(plugin: source, kind: kind, cursor: cursor))
        moreRequests[id] = key
        let mine = generation
        tracker.watch(id) { [weak self] message in
            guard let self, self.generation == mine else { return }
            self.moreRequests[id] = nil
            self.loadingMore.remove(key)
            self.moreErrors[key] = message
        }
    }

    /// Show More's answer: its rows join their group, which keeps the new cursor.
    func apply(page: SearchPageBatch) {
        guard let key = moreRequests.removeValue(forKey: page.request) else { return }
        loadingMore.remove(key)
        if let error = page.error {
            moreErrors[key] = error
        }
        guard let index = sections.firstIndex(where: { $0.source == page.source }),
              let group = sections[index].groups.firstIndex(where: { $0.kind == page.kind })
        else { return }
        let shown = Set(sections[index].groups[group].items.map(\.id))
        sections[index].groups[group].items += page.items.filter { !shown.contains($0.id) }
        sections[index].groups[group].next = page.next
    }

    /// The artwork of a library result.
    func artwork(for item: SourceResult) -> ArtworkSource? {
        if item.source == Self.library, let hash = libraryArtwork[Self.key(item.kind, item.itemID)] {
            return .hash(hash)
        }
        return item.artwork.flatMap { ArtworkSource(hash: nil, location: $0) }
    }

    private static func key(_ kind: ResultKind, _ id: String) -> String {
        "\(kind)-\(id)"
    }

    private func findLibraryArtwork(for words: String) {
        let mine = generation
        Task { [weak self] in
            guard let self, let found = try? await self.client.searchLibrary(words, limit: Self.libraryLimit),
                  self.generation == mine
            else { return }
            self.libraryArtwork = Self.artwork(in: found)
        }
    }

    private static func artwork(in found: LibrarySearch) -> [String: String] {
        var map: [String: String] = [:]
        for song in found.songs {
            if let hash = song.artworkHash { map[key(.track, String(song.id))] = hash }
        }
        for album in found.albums {
            if let hash = album.artworkHash { map[key(.album, String(album.id))] = hash }
        }
        return map
    }

    /// The search command failed: show what the library alone finds.
    private func commandFailed(_ message: String) {
        let words = query
        let mine = generation
        Task { [weak self] in
            guard let self else { return }
            do {
                let found = try await self.client.searchLibrary(words, limit: Self.libraryLimit)
                guard self.generation == mine else { return }
                self.sections.removeAll { $0.source == Self.library }
                self.sections.insert(SearchSection(source: Self.library, items: Self.items(from: found), error: nil),
                                     at: 0)
                self.libraryArtwork = Self.artwork(in: found)
            } catch {
                guard self.generation == mine else { return }
                let reason = isNotAvailableMessage(message) ? CoreClientError.describe(error) : message
                self.sections = [SearchSection(source: Self.library, items: [], error: reason)]
            }
            self.done = true
        }
    }

    /// Library search results as the rows the command would have sent.
    static func items(from found: LibrarySearch) -> [SourceResult] {
        var items: [SourceResult] = []
        for song in found.songs {
            items.append(SourceResult(
                source: library, kind: .track, itemID: String(song.id), title: song.title, subtitle: song.artist,
                album: song.album, durationMs: song.durationMs, artwork: nil, playable: true,
                unplayableReason: nil, track: .library(id: song.id), alsoIn: []
            ))
        }
        for album in found.albums {
            items.append(SourceResult(
                source: library, kind: .album, itemID: String(album.id), title: album.title,
                subtitle: album.albumArtist, album: nil, durationMs: nil, artwork: nil, playable: true,
                unplayableReason: nil, track: nil, alsoIn: []
            ))
        }
        for artist in found.artists {
            items.append(SourceResult(
                source: library, kind: .artist, itemID: String(artist.id), title: artist.name, subtitle: nil,
                album: nil, durationMs: nil, artwork: nil, playable: true, unplayableReason: nil, track: nil,
                alsoIn: []
            ))
        }
        return items
    }
}

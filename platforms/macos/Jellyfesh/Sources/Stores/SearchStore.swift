// Unified search as it streams in (docs/core-api.md: `Search` covers the library and every enabled
// source, merged and de-duplicated by the core): one section per source in the order they answer,
// the library first. The core's library results carry no artwork, so the same library search runs
// once as a query to find it. Should the command fail (sources still being integrated), the
// library query alone fills the page.

import Foundation
import Observation

/// One source's answer to a search.
struct SearchSection: Identifiable, Equatable {
    var source: String
    var items: [SourceResult]
    var error: String?

    var id: String { source }

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
    static let library = "library"
    /// How many results per kind the core returns from the library (its `search(query, 50)`).
    static let libraryLimit = 50

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

    @ObservationIgnored private var debounce: Task<Void, Never>?
    @ObservationIgnored private var generation = 0

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
    }

    /// One source's answer; answers to older searches are ignored.
    func apply(_ batch: SearchBatch) {
        guard let request, batch.request == request else { return }
        let section = SearchSection(source: batch.source, items: batch.items, error: batch.error)
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

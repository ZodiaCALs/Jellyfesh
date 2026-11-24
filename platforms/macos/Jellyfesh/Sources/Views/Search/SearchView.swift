// Search by source (ADR-0024 §4, docs/ui.md "Search"): the field, then the tabs All, Library and one
// per source that answered. All: the library's results first, then From Your Sources, one group per
// source that allows merging, each appearing as it arrives, grouped by kind; Apple Music, when on,
// in its own group (ADR-0010: never merged with other sources); then a link to the tab of each
// source whose terms forbid merging ("Results from Spotify"). A source's tab: its Songs, Albums,
// Artists and Playlists, each with Show More. Library albums, artists and playlists open their
// pages; a source's containers open in Browse; an item that cannot play here but has a link opens
// it in its service.

import SwiftUI

struct SearchView: View {
    @Environment(SearchStore.self) private var search
    @Environment(SourcesStore.self) private var sources
    @Environment(AppleMusicStore.self) private var appleMusic
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @FocusState private var fieldFocused: Bool

    var body: some View {
        @Bindable var search = search
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s6) {
                HStack(spacing: Tokens.Space.s2) {
                    Image(systemName: "magnifyingglass")
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                    TextField("Songs, albums and artists", text: $search.text)
                        .textFieldStyle(.plain)
                        .lookFont(size: 17)
                        .focused($fieldFocused)
                        .onSubmit { search.search(search.text) }
                        .accessibilityLabel(Text("Search"))
                    if search.isWaiting {
                        ProgressView().controlSize(.small)
                    }
                    if !search.text.isEmpty {
                        Button {
                            search.text = ""
                            search.clear()
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                        .accessibilityLabel(Text("Clear Search"))
                    }
                }
                .padding(.horizontal, Tokens.Space.s4)
                .frame(height: 44)
                .surface(in: Capsule())
                .frame(maxWidth: 640)

                if search.query.isEmpty {
                    EmptyState(title: "Search Your Music", symbol: "magnifyingglass",
                               message: "Find songs, albums and artists in your library and in your sources.")
                        .frame(maxWidth: .infinity, minHeight: 320)
                } else if search.foundNothing && !appleMusicFound {
                    EmptyState(title: "No Results", symbol: "magnifyingglass",
                               message: "Try other words.")
                        .frame(maxWidth: .infinity, minHeight: 320)
                } else {
                    PillTabs(selection: $search.tab, options: tabOptions, label: "Search")
                    switch search.tab {
                    case .all:
                        allTab
                    case .library:
                        if let library = search.librarySection {
                            SearchSectionView(section: library, showsSource: false)
                        } else {
                            EmptyState(title: "No Results", symbol: "magnifyingglass", message: "Try other words.")
                                .frame(maxWidth: .infinity, minHeight: 240)
                        }
                    case .source(let id):
                        if let section = search.section(for: id) {
                            SourceTabView(section: section)
                        }
                    }
                }
            }
            .pagePadding()
            .frame(maxWidth: 1100, alignment: .leading)
        }
        .navigationTitle(Text("Search"))
        .onChange(of: search.text) { _, _ in
            search.textChanged()
        }
        .onChange(of: router.searchFocusRequest, initial: true) { _, _ in
            fieldFocused = true
        }
        .task(id: AppleMusicQuery(words: search.query, on: appleMusic.isAvailable)) {
            // Apple Music's own search, beside the core's (its results never merge with others).
            await appleMusic.unifiedSearch.run(search.query, using: appleMusic.catalog)
        }
    }

    /// All, Library, and each source that answered, by its name.
    private var tabOptions: [PillTabs<SearchTab>.Option] {
        search.tabs.map { tab in
            switch tab {
            case .all: return PillTabs<SearchTab>.Option(value: tab, title: "All")
            case .library: return PillTabs<SearchTab>.Option(value: tab, title: "Library")
            case .source(let id): return PillTabs<SearchTab>.Option(value: tab, verbatim: sources.name(of: id))
            }
        }
    }

    /// The library first, From Your Sources, Apple Music, then the separate sources' links.
    @ViewBuilder private var allTab: some View {
        if let library = search.librarySection {
            SearchSectionView(section: library, showsSource: false)
        }
        let fromSources = search.sourceSections
        if !fromSources.isEmpty {
            VStack(alignment: .leading, spacing: Tokens.Space.s5) {
                Text("From Your Sources")
                    .lookFont(size: 22, weight: .bold)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .accessibilityAddTraits(.isHeader)
                ForEach(fromSources) { section in
                    SearchSectionView(section: section, showsSource: true)
                }
            }
        }
        if appleMusic.isAvailable {
            AppleMusicResultsView(search: appleMusic.unifiedSearch, showsTitle: true)
        }
        let separate = search.separateSections
        if !separate.isEmpty {
            VStack(alignment: .leading, spacing: 0) {
                ForEach(separate) { section in
                    Button {
                        search.tab = .source(section.source)
                    } label: {
                        HStack(spacing: Tokens.Space.s2) {
                            Text("Results from \(sources.name(of: section.source))")
                                .lookFont(size: 15, weight: .semibold)
                                .foregroundStyle(palette[Tokens.Accent.accentText])
                            Spacer(minLength: 0)
                            Image(systemName: "chevron.right")
                                .foregroundStyle(palette[Tokens.Ink.ink50])
                        }
                        .frame(minHeight: Tokens.Player.rowHeight)
                        .padding(.horizontal, Tokens.Space.s4)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .surfaceCard()
        }
    }

    private var appleMusicFound: Bool {
        appleMusic.isAvailable && !appleMusic.unifiedSearch.results.isEmpty
    }

    private struct AppleMusicQuery: Equatable {
        var words: String
        var on: Bool
    }
}

/// One source's answer, grouped by kind.
private struct SearchSectionView: View {
    var section: SearchSection
    var showsSource: Bool

    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette

    var body: some View {
        if !section.items.isEmpty || section.error != nil {
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                if showsSource {
                    Text(sources.name(of: section.source))
                        .typography(Tokens.TypeRoles.section)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .accessibilityAddTraits(.isHeader)
                    if let credit = sources.attribution(of: section.source) {
                        Text(credit)
                            .typography(Tokens.TypeRoles.listSecondary)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                    }
                }
                if let error = section.error {
                    Text(error)
                        .lookFont(size: 13)
                        .foregroundStyle(palette[Tokens.Semantic.danger])
                }
                ForEach(section.kinds, id: \.self) { kind in
                    SearchKindView(kind: kind, items: section.items(of: kind))
                }
            }
        }
    }
}

/// A source's tab: its name (and credit), then its results kind by kind, each with Show More while
/// the source has more.
private struct SourceTabView: View {
    var section: SearchSection

    @Environment(SearchStore.self) private var search
    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s6) {
            VStack(alignment: .leading, spacing: Tokens.Space.s1) {
                Text(sources.name(of: section.source))
                    .typography(Tokens.TypeRoles.section)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .accessibilityAddTraits(.isHeader)
                if let credit = sources.attribution(of: section.source) {
                    Text(credit)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
            }
            if let error = section.error {
                Text(error)
                    .lookFont(size: 13)
                    .foregroundStyle(palette[Tokens.Semantic.danger])
            }
            ForEach(section.tabGroups, id: \.kind) { group in
                let key = SearchStore.moreKey(section.source, group.kind)
                let loading = search.loadingMore.contains(key)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    SearchKindView(kind: group.kind, items: group.items)
                    if group.next != nil || loading {
                        HStack(spacing: Tokens.Space.s2) {
                            Button("Show More") {
                                search.showMore(source: section.source, kind: group.kind)
                            }
                            .disabled(loading)
                            if loading {
                                ProgressView()
                                    .controlSize(.small)
                            }
                        }
                    }
                    if let error = search.moreErrors[key] {
                        Text(error)
                            .lookFont(size: 12)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                    }
                }
            }
        }
    }
}

/// One kind of results: songs as rows that play, albums as cards, the rest as rows.
private struct SearchKindView: View {
    var kind: ResultKind
    var items: [SourceResult]

    @Environment(SearchStore.self) private var search
    @Environment(SourcesStore.self) private var sources
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    private let columns = [
        GridItem(.adaptive(minimum: 140, maximum: 190), spacing: Tokens.Space.s5, alignment: .top),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            Text(Self.title(of: kind))
                .lookFont(size: 13, weight: .semibold)
                .foregroundStyle(palette[Tokens.Ink.ink70])
            switch kind {
            case .track:
                let rows = items.map { result -> TrackItem in
                    var row = TrackItem(result: result, artwork: search.artwork(for: result))
                    row.alsoIn = sources.alsoIn(result.alsoIn)
                    return row
                }
                TrackStack(items: rows) { index in
                    let playable = rows.filter(\.canPlay)
                    guard let start = playable.firstIndex(where: { $0.id == rows[index].id }) else { return }
                    playback.play(tracks: playable.compactMap(\.track), from: start)
                }
                .surfaceCard()
            case .album:
                LazyVGrid(columns: columns, alignment: .leading, spacing: Tokens.Space.s5) {
                    ForEach(items) { item in
                        if item.source == SearchStore.library, let id = Int64(item.itemID) {
                            AlbumCard(album: AlbumSummary(id: id, title: item.title, albumArtist: item.subtitle, year: nil,
                                                          trackCount: 0, artworkHash: hash(of: search.artwork(for: item))),
                                      alsoIn: sources.alsoIn(item.alsoIn))
                        } else {
                            SourceCard(item: item, artwork: search.artwork(for: item))
                        }
                    }
                }
            default:
                VStack(spacing: 0) {
                    ForEach(items) { item in
                        ResultRow(item: item, artwork: search.artwork(for: item))
                    }
                }
                .padding(.horizontal, Tokens.Space.s2)
                .surfaceCard()
            }
        }
    }

    private func hash(of artwork: ArtworkSource?) -> String? {
        if case .hash(let value)? = artwork { return value }
        return nil
    }

    static func title(of kind: ResultKind) -> LocalizedStringKey {
        switch kind {
        case .track: return "Songs"
        case .album: return "Albums"
        case .artist: return "Artists"
        case .playlist: return "Playlists"
        case .folder: return "Folders"
        }
    }
}

/// A source's album or container as a card; opens its page, or its service when it cannot play
/// here and has a link. The second line names the other sources that have it ("Also in …").
struct SourceCard: View {
    var item: SourceResult
    var artwork: ArtworkSource?

    @Environment(Router.self) private var router
    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette

    var body: some View {
        Button {
            if ServiceLink.isMainAction(canPlay: item.playable, link: item.link) {
                ServiceLink.open(item.link)
            } else {
                router.open(.browse(plugin: item.source, node: item.itemID, title: item.title, link: item.link,
                                    item: item))
            }
        } label: {
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                ArtworkView(source: artwork, size: nil, cornerRadius: Tokens.Artwork.radiusGrid,
                            placeholder: "square.stack")
                Text(item.title)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .lineLimit(1)
                if let line = secondLine {
                    Text(line)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .lineLimit(1)
                }
            }
        }
        .buttonStyle(.plain)
        .contextMenu {
            SourceContainerActions(item: item)
        }
        .accessibilityLabel(Text([item.title, secondLine].compactMap { $0 }.joined(separator: ", ")))
    }

    /// The artist, and the other sources that have it.
    private var secondLine: String? {
        CountText.joined([item.subtitle, sources.alsoIn(item.alsoIn)]).nilIfEmpty
    }
}

/// A source container's menu: Play, Shuffle, Play Next and Play Last (not for a remote source,
/// whose songs never join the queue), Download (an album or playlist of a source that offers
/// downloads), then Open in {Service}.
struct SourceContainerActions: View {
    var item: SourceResult

    @Environment(PlaybackStore.self) private var playback
    @Environment(SourcesStore.self) private var sources
    @Environment(DownloadsStore.self) private var downloads

    var body: some View {
        if item.playable && item.kind != .track {
            let collection = CollectionReference.source(plugin: item.source, id: item.itemID, kind: item.kind)
            Button("Play") { playback.play(collection) }
            Button("Shuffle") { playback.play(collection, shuffle: true) }
            if !sources.isRemote(item.source) {
                Divider()
                Button("Play Next") { playback.enqueue(collection, next: true) }
                Button("Play Last") { playback.enqueue(collection, next: false) }
            }
        }
        if let download = SourcePage.downloadRequest(plugin: item.source, item: item,
                                                     offered: sources.offersDownloads(item.source)) {
            Button("Download") { downloads.download(download) }
        }
        OpenInServiceButton(link: item.link, source: item.source)
    }
}

/// An artist, playlist or folder result: library ones open their page, a source's open in Browse.
struct ResultRow: View {
    var item: SourceResult
    var artwork: ArtworkSource?
    /// A source playlist whose songs its service lists here (false: its page says where they are).
    var songsListed = true

    @Environment(Router.self) private var router
    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette

    var body: some View {
        Button {
            open()
        } label: {
            HStack(spacing: Tokens.Space.s3) {
                ArtworkView(source: artwork, size: 40, placeholder: symbol)
                VStack(alignment: .leading, spacing: 1) {
                    Text(item.title)
                        .typography(Tokens.TypeRoles.listTitle)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .lineLimit(1)
                    if let line = secondLine {
                        Text(line)
                            .typography(Tokens.TypeRoles.listSecondary)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                            .lineLimit(1)
                    }
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(palette[Tokens.Ink.ink30])
            }
            .padding(.horizontal, Tokens.Space.s2)
            .frame(height: Tokens.Player.rowHeight)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .contextMenu {
            if item.source != SearchStore.library {
                SourceContainerActions(item: item)
            }
        }
        .accessibilityLabel(Text([item.title, secondLine].compactMap { $0 }.joined(separator: ", ")))
    }

    private var symbol: String {
        switch item.kind {
        case .artist: return "music.mic"
        case .playlist: return "music.note.list"
        case .folder: return "folder"
        case .album: return "square.stack"
        case .track: return "music.note"
        }
    }

    /// The subtitle, and the other sources that have the same item (the core merges them).
    private var secondLine: String? {
        CountText.joined([item.subtitle, sources.alsoIn(item.alsoIn)]).nilIfEmpty
    }

    private func open() {
        if ServiceLink.isMainAction(canPlay: item.playable, link: item.link) {
            ServiceLink.open(item.link)
            return
        }
        if item.source == SearchStore.library, let id = Int64(item.itemID) {
            switch item.kind {
            case .artist:
                router.open(.artist(ArtistSummary(id: id, name: item.title, trackCount: 0)))
                return
            case .playlist:
                router.open(.playlist(id))
                return
            case .album:
                router.open(.album(AlbumSummary(id: id, title: item.title, albumArtist: item.subtitle, year: nil,
                                                trackCount: 0, artworkHash: nil)))
                return
            default:
                break
            }
        }
        router.open(.browse(plugin: item.source, node: item.itemID, title: item.title, link: item.link, item: item,
                            songsListed: songsListed))
    }
}

extension String {
    /// Nil for an empty string.
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

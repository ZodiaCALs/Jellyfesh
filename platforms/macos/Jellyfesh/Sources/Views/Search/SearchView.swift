// Unified search: the library and every enabled source, each source's results appearing as they
// arrive, grouped by kind. Library albums, artists and playlists open their pages; a source's
// containers open in Browse.

import SwiftUI

struct SearchView: View {
    @Environment(SearchStore.self) private var search
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
                    TextField("Songs, albums, artists", text: $search.text)
                        .textFieldStyle(.plain)
                        .font(.system(size: 17))
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
                .glass(.mist, in: Capsule())
                .frame(maxWidth: 640)

                if search.query.isEmpty {
                    EmptyState(title: "Search Your Music", symbol: "magnifyingglass",
                               message: "Find songs, albums and artists in your library and in your sources.")
                        .frame(maxWidth: .infinity, minHeight: 320)
                } else if search.foundNothing {
                    EmptyState(title: "No Results", symbol: "magnifyingglass",
                               message: "Try other words, or check the spelling.")
                        .frame(maxWidth: .infinity, minHeight: 320)
                } else {
                    let several = search.sections.count > 1
                    ForEach(search.sections) { section in
                        SearchSectionView(section: section,
                                          showsSource: several || section.source != SearchStore.library)
                    }
                }
            }
            .padding(.horizontal, Tokens.Space.s7)
            .padding(.vertical, Tokens.Space.s6)
            .frame(maxWidth: 1100, alignment: .leading)
        }
        .navigationTitle(Text("Search"))
        .onChange(of: search.text) { _, _ in
            search.textChanged()
        }
        .onChange(of: router.searchFocusRequest, initial: true) { _, _ in
            fieldFocused = true
        }
    }
}

/// One source's answer, grouped by kind.
private struct SearchSectionView: View {
    var section: SearchSection
    var showsSource: Bool

    @Environment(SearchStore.self) private var search
    @Environment(SourcesStore.self) private var sources
    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette

    private let columns = [
        GridItem(.adaptive(minimum: 140, maximum: 190), spacing: Tokens.Space.s5, alignment: .top),
    ]

    var body: some View {
        if !section.items.isEmpty || section.error != nil {
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                if showsSource {
                    Text(sources.name(of: section.source))
                        .typography(Tokens.TypeRoles.section)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .accessibilityAddTraits(.isHeader)
                }
                if let error = section.error {
                    Text(error)
                        .font(.system(size: 13))
                        .foregroundStyle(palette[Tokens.Semantic.danger])
                }
                ForEach(section.kinds, id: \.self) { kind in
                    kindGroup(kind)
                }
            }
        }
    }

    @ViewBuilder private func kindGroup(_ kind: ResultKind) -> some View {
        let items = section.items(of: kind)
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            Text(Self.title(of: kind))
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(palette[Tokens.Ink.ink70])
            switch kind {
            case .track:
                let rows = items.map { TrackItem(result: $0, artwork: search.artwork(for: $0)) }
                TrackStack(items: rows) { index in
                    let playable = rows.filter(\.canPlay)
                    guard let start = playable.firstIndex(where: { $0.id == rows[index].id }) else { return }
                    playback.play(tracks: playable.compactMap(\.track), from: start)
                }
                .glass(.mist, in: RoundedRectangle(cornerRadius: Tokens.Radius.rLg, style: .continuous))
            case .album:
                LazyVGrid(columns: columns, alignment: .leading, spacing: Tokens.Space.s5) {
                    ForEach(items) { item in
                        if item.source == SearchStore.library, let id = Int64(item.itemID) {
                            AlbumCard(album: AlbumSummary(id: id, title: item.title, albumArtist: item.subtitle, year: nil,
                                                          trackCount: 0, artworkHash: hash(of: search.artwork(for: item))))
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
                .glass(.mist, in: RoundedRectangle(cornerRadius: Tokens.Radius.rLg, style: .continuous))
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

/// A source's album or container as a card; opens it in Browse.
struct SourceCard: View {
    var item: SourceResult
    var artwork: ArtworkSource?

    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette

    var body: some View {
        Button {
            router.open(.browse(plugin: item.source, node: item.itemID, title: item.title))
        } label: {
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                ArtworkView(source: artwork, size: nil, cornerRadius: Tokens.Artwork.radiusGrid,
                            placeholder: "square.stack")
                Text(item.title)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .lineLimit(1)
                if let subtitle = item.subtitle {
                    Text(subtitle)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .lineLimit(1)
                }
            }
        }
        .buttonStyle(.plain)
        .accessibilityLabel(Text(item.subtitle.map { "\(item.title), \($0)" } ?? item.title))
    }
}

/// An artist, playlist or folder result: library ones open their page, a source's open in Browse.
struct ResultRow: View {
    var item: SourceResult
    var artwork: ArtworkSource?

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
        .accessibilityLabel(Text(CountText.joined([item.title, secondLine])))
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
        let also = item.alsoIn.map { sources.name(of: $0) }
        let alsoText = also.isEmpty ? nil : String(localized: "Also in \(also.joined(separator: ", "))")
        return CountText.joined([item.subtitle, alsoText]).nilIfEmpty
    }

    private func open() {
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
        router.open(.browse(plugin: item.source, node: item.itemID, title: item.title))
    }
}

extension String {
    /// Nil for an empty string.
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

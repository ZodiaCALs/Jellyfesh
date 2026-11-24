// The sources' likes and playlists on Favourites and Playlists (ADR-0024, docs/ui.md): one section
// per signed-in source after the library's own, with its name, its credit where its terms ask for
// one ("Content from Spotify"), and when the cached lists were updated ("Updated 5 minutes ago");
// then its lists: songs as rows that play, albums as cards, artists and playlists as rows that open
// their pages (a playlist whose songs its service doesn't list says so there), each with Show More
// while the source has more. They are read-only: the heart marks a song in Jellyfesh only.

import SwiftUI

struct SourceLibrarySection: View {
    var source: SourceLibraryInfo

    @Environment(SourceLibraryStore.self) private var store
    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            VStack(alignment: .leading, spacing: 2) {
                SectionTitle(verbatim: source.name)
                if let credit = sources.attribution(of: source.plugin) {
                    Text(credit)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
                if let updated = source.lists.lazy.compactMap({ store.updatedLine($0) }).first {
                    Text(updated)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                }
            }
            ForEach(source.lists, id: \.part) { list in
                SourceListView(plugin: source.plugin, list: list, showsTitle: source.lists.count > 1)
            }
        }
        .padding(.vertical, Tokens.Space.s3)
    }
}

/// One of a source's lists, with Show More while the source has more.
private struct SourceListView: View {
    var plugin: String
    var list: SourceList
    /// The list's kind as a heading, when the source shows more than one.
    var showsTitle: Bool

    @Environment(SourceLibraryStore.self) private var store
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    private let columns = [
        GridItem(.adaptive(minimum: 140, maximum: 190), spacing: Tokens.Space.s5, alignment: .top),
    ]

    var body: some View {
        let key = SourceLibraryStore.moreKey(plugin, list.part)
        let loading = store.loadingMore.contains(key)
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            if showsTitle {
                Text(Self.title(of: list.part))
                    .lookFont(size: 13, weight: .semibold)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
            }
            if let error = list.error {
                Text(error)
                    .lookFont(size: 13)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
            }
            if list.entries.isEmpty && list.loading {
                ProgressView()
                    .controlSize(.small)
            }
            entries
            if list.next != nil || loading {
                HStack(spacing: Tokens.Space.s2) {
                    Button("Show More") {
                        store.more(plugin: plugin, part: list.part)
                    }
                    .disabled(loading)
                    if loading {
                        ProgressView()
                            .controlSize(.small)
                    }
                }
            }
        }
    }

    @ViewBuilder private var entries: some View {
        switch list.part {
        case .likedSongs:
            let rows = list.entries.map { entry in
                TrackItem(result: entry.item, artwork: Self.artwork(entry.item))
            }
            if !rows.isEmpty {
                TrackStack(items: rows) { index in
                    let playable = rows.filter(\.canPlay)
                    guard let start = playable.firstIndex(where: { $0.id == rows[index].id }) else { return }
                    playback.play(tracks: playable.compactMap(\.track), from: start)
                }
                .surfaceCard()
            }
        case .likedAlbums:
            LazyVGrid(columns: columns, alignment: .leading, spacing: Tokens.Space.s5) {
                ForEach(list.entries) { entry in
                    SourceCard(item: entry.item, artwork: Self.artwork(entry.item))
                }
            }
        case .likedArtists, .playlists:
            if !list.entries.isEmpty {
                VStack(spacing: 0) {
                    ForEach(list.entries) { entry in
                        ResultRow(item: entry.item, artwork: Self.artwork(entry.item), songsListed: entry.songsListed)
                    }
                }
                .padding(.horizontal, Tokens.Space.s2)
                .surfaceCard()
            }
        }
    }

    private static func artwork(_ item: SourceResult) -> ArtworkSource? {
        item.artwork.flatMap { ArtworkSource(hash: nil, location: $0) }
    }

    static func title(of part: SourceListPart) -> LocalizedStringKey {
        switch part {
        case .likedSongs: return "Songs"
        case .likedAlbums: return "Albums"
        case .likedArtists: return "Artists"
        case .playlists: return "Playlists"
        }
    }
}

// A source's Discover page (docs/ui.md "Discover"): the source's name and status, Browse (its own
// folders) in the header, then its shelves, each with its title and See All when it has more
// (which browses it). Row shelves scroll sideways, grid shelves wrap, chart shelves are numbered
// lists. While loading, placeholders the shape of the shelves; on failure, the error and Try
// Again. A source whose terms ask for it is credited ("Content from {Service}").

import SwiftUI

struct DiscoverView: View {
    var plugin: String
    var title: String

    @Environment(AppModel.self) private var model
    @Environment(SourcesStore.self) private var sources
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @Environment(\.look) private var look
    @State private var page: DiscoverModel?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: look.spacing(Tokens.Space.s7)) {
                PageHeader(title: title, subtitle: sourceLine) {
                    if sources.plugin(plugin)?.canBrowse == true {
                        Button {
                            router.open(.browse(plugin: plugin, node: nil, title: title))
                        } label: {
                            Label("Browse", systemImage: "folder")
                        }
                        .buttonStyle(PillButtonStyle(prominent: false))
                    }
                }
                if let page {
                    if let error = page.error, page.shelves.isEmpty {
                        VStack(spacing: Tokens.Space.s3) {
                            EmptyState(title: "This Source Did Not Answer", symbol: "exclamationmark.triangle",
                                       error: error)
                            Button("Try Again") {
                                page.load(model.answers)
                            }
                            .disabled(page.loading)
                        }
                        .frame(maxWidth: .infinity, minHeight: 240)
                    } else if !page.loaded {
                        ShelfPlaceholders()
                    } else if page.shelves.isEmpty {
                        EmptyState(title: "Nothing Here", symbol: "tray")
                            .frame(maxWidth: .infinity, minHeight: 240)
                    }
                    ForEach(page.shelves) { shelf in
                        ShelfView(plugin: plugin, shelf: shelf,
                                  title: DiscoverText.title(shelf, sourceName: sources.name(of: plugin)))
                    }
                }
            }
            .pagePadding()
        }
        .navigationTitle(Text(title))
        .task(id: plugin) {
            let discover = DiscoverModel(source: plugin)
            page = discover
            discover.load(model.answers)
        }
    }

    /// Under the title: the credit, else the source's badge and account.
    private var sourceLine: String? {
        if let credit = sources.attribution(of: plugin) { return credit }
        guard let source = sources.plugin(plugin) else { return nil }
        return CountText.joined([SourceText.badge(source), SourceText.account(source)])
    }
}

/// One shelf: its title, See All, and its items as a row, a grid or a chart.
private struct ShelfView: View {
    var plugin: String
    var shelf: DiscoverShelf
    var title: String

    @Environment(Router.self) private var router
    @Environment(\.look) private var look

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s3) {
            HStack(alignment: .firstTextBaseline) {
                SectionTitle(verbatim: title)
                Spacer(minLength: 0)
                if let more = shelf.more {
                    Button("See All") {
                        router.open(.browse(plugin: plugin, node: more, title: title))
                    }
                    .buttonStyle(.link)
                }
            }
            switch shelf.layout {
            case .row:
                ScrollView(.horizontal) {
                    LazyHStack(alignment: .top, spacing: look.spacing(Tokens.Space.s5)) {
                        ForEach(shelf.items) { item in
                            ShelfItemCard(item: item, shelf: shelf)
                                .frame(width: 160)
                        }
                    }
                    .padding(.bottom, Tokens.Space.s2)
                }
                .scrollIndicators(.hidden)
            case .grid:
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 150, maximum: 220), spacing: look.spacing(Tokens.Space.s5),
                                             alignment: .top)],
                          alignment: .leading, spacing: look.spacing(Tokens.Space.s6)) {
                    ForEach(shelf.items) { item in
                        ShelfItemCard(item: item, shelf: shelf)
                    }
                }
            case .chart:
                VStack(spacing: 0) {
                    ForEach(Array(shelf.items.enumerated()), id: \.element.id) { index, item in
                        ChartRow(number: index + 1, item: item, shelf: shelf)
                    }
                }
                .padding(.horizontal, Tokens.Space.s3)
                .surfaceCard()
            }
        }
    }
}

/// An item of a row or grid shelf: a container opens its page (SourceCard); a song plays the
/// shelf's songs from it.
private struct ShelfItemCard: View {
    var item: SourceResult
    var shelf: DiscoverShelf

    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    var body: some View {
        let artwork = item.artwork.flatMap { ArtworkSource(hash: nil, location: $0) }
        if item.kind == .track {
            Button {
                ShelfPlay.play(item, of: shelf, playback: playback)
            } label: {
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    ArtworkView(source: artwork, size: nil, cornerRadius: Tokens.Artwork.radiusGrid)
                    Text(item.title)
                        .typography(Tokens.TypeRoles.listTitle)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .lineLimit(1)
                    if let artist = item.subtitle, !artist.isEmpty {
                        Text(artist)
                            .typography(Tokens.TypeRoles.listSecondary)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                            .lineLimit(1)
                    }
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .contextMenu {
                TrackActionsMenu(items: [TrackItem(result: item, artwork: nil)])
            }
            .accessibilityLabel(Text(CountText.joined([item.title, item.subtitle])))
        } else {
            SourceCard(item: item, artwork: artwork)
        }
    }
}

/// A chart shelf's row: the place, then the song (or container) as its list shows it.
private struct ChartRow: View {
    var number: Int
    var item: SourceResult
    var shelf: DiscoverShelf

    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        let artwork = item.artwork.flatMap { ArtworkSource(hash: nil, location: $0) }
        if item.kind == .track {
            let shown = SourcePage.trackItem(item, artwork: artwork, number: UInt32(number))
            TrackRowView(item: shown, style: .album,
                         isCurrent: item.track != nil && item.track == playback.nowPlaying?.track,
                         isPlaying: playback.isPlaying,
                         play: { ShelfPlay.play(item, of: shelf, playback: playback) })
                .onTapGesture(count: 2) { ShelfPlay.play(item, of: shelf, playback: playback) }
                .contextMenu {
                    TrackActionsMenu(items: [shown])
                }
                .accessibilityAction { ShelfPlay.play(item, of: shelf, playback: playback) }
        } else {
            ResultRow(item: item, artwork: artwork)
        }
    }
}

/// Playing from a shelf.
@MainActor
private enum ShelfPlay {
    /// The shelf's songs from `item`; one that cannot play here opens in its service instead.
    static func play(_ item: SourceResult, of shelf: DiscoverShelf, playback: PlaybackStore) {
        if let queue = DiscoverText.queue(of: shelf, from: item) {
            playback.play(tracks: queue.tracks, from: queue.start)
        } else if ServiceLink.isMainAction(canPlay: item.playable && item.track != nil, link: item.link) {
            ServiceLink.open(item.link)
        }
    }
}

/// While the page loads: two shelves of grey covers.
private struct ShelfPlaceholders: View {
    @Environment(\.palette) private var palette
    @Environment(\.look) private var look

    var body: some View {
        VStack(alignment: .leading, spacing: look.spacing(Tokens.Space.s7)) {
            ForEach(0..<2, id: \.self) { _ in
                VStack(alignment: .leading, spacing: Tokens.Space.s3) {
                    RoundedRectangle(cornerRadius: Tokens.Radius.rXs, style: .continuous)
                        .fill(palette[Tokens.Ink.ink06])
                        .frame(width: 140, height: 18)
                    HStack(spacing: look.spacing(Tokens.Space.s5)) {
                        ForEach(0..<5, id: \.self) { _ in
                            RoundedRectangle(cornerRadius: look.radius(Tokens.Artwork.radiusGrid), style: .continuous)
                                .fill(palette[Tokens.Ink.ink06])
                                .frame(width: 160, height: 160)
                        }
                    }
                }
            }
        }
        .accessibilityHidden(true)
        .clipped()
    }
}

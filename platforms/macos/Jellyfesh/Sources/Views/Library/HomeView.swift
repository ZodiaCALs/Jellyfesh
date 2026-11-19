// Home: the library's size, then recently added albums (newest first, in the core's order) and
// recently and most played songs; with nothing in the library and no source signed in, a welcome
// with Add Music Folder… and Add a Source instead. A customized look picks the sections and their
// order, Favourites among them (ADR-0020); the summary stays under the title while it comes first.

import SwiftUI

struct HomeView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryStore.self) private var library
    @Environment(SourcesStore.self) private var sources
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette
    @Environment(\.look) private var look

    private var home: HomeModel { model.pages.home }

    var body: some View {
        let sections = look.home
        ScrollView {
            VStack(alignment: .leading, spacing: look.spacing(Tokens.Space.s7)) {
                PageHeader(title: String(localized: "Home"), subtitle: sections.first == .summary ? statsLine : nil)
                if home.loaded && library.isEmpty && !library.scanning && !sources.hasSignedInSource {
                    WelcomeCard()
                }
                ForEach(Array(sections.enumerated()), id: \.element) { index, section in
                    switch section {
                    case .summary:
                        // First, it is the title's subtitle; elsewhere, a line of its own.
                        if index > 0, let summary = statsLine {
                            Text(summary)
                                .typography(Tokens.TypeRoles.listSecondary)
                                .foregroundStyle(palette[Tokens.Ink.ink70])
                        }
                    case .recentlyAdded:
                        recentlyAdded
                    case .recentlyPlayed:
                        trackSection("Recently Played", rows: home.recentlyPlayed, prefix: "recent")
                    case .mostPlayed:
                        trackSection("Most Played", rows: home.mostPlayed, prefix: "most")
                    case .favourites:
                        trackSection("Favourites", rows: home.favorites, prefix: "favourite")
                    }
                }
            }
            .pagePadding()
            .frame(maxWidth: 1100, alignment: .leading)
        }
        .navigationTitle(Text("Home"))
        .task(id: library.revision) {
            await home.load(revision: library.revision)
        }
    }

    @ViewBuilder private var recentlyAdded: some View {
        if !home.recentAlbums.isEmpty {
            VStack(alignment: .leading, spacing: Tokens.Space.s3) {
                SectionTitle("Recently Added")
                ScrollView(.horizontal) {
                    LazyHStack(alignment: .top, spacing: look.spacing(Tokens.Space.s5)) {
                        ForEach(home.recentAlbums) { album in
                            AlbumCard(album: album, size: 160)
                        }
                    }
                    .padding(.bottom, Tokens.Space.s2)
                }
                .scrollIndicators(.hidden)
            }
        }
    }

    @ViewBuilder
    private func trackSection(_ title: LocalizedStringKey, rows: [SongRow], prefix: String) -> some View {
        if !rows.isEmpty {
            let items = rows.enumerated().map { TrackItem(row: $0.element, id: "\(prefix)\($0.offset)") }
            VStack(alignment: .leading, spacing: Tokens.Space.s3) {
                SectionTitle(title)
                TrackStack(items: items) { index in
                    playback.play(.tracks(items.compactMap(\.track)), from: playableIndex(index, in: items))
                }
                .surfaceCard()
            }
        }
    }

    /// The row's place among the rows that have a track (the collection leaves out the others).
    private func playableIndex(_ index: Int, in items: [TrackItem]) -> Int {
        items[..<index].filter { $0.track != nil }.count
    }

    private var statsLine: String? {
        guard let stats = home.stats, stats.tracks > 0 else { return nil }
        return CountText.joined([
            CountText.songs(Int(stats.tracks)),
            CountText.albums(Int(stats.albums)),
            CountText.artists(Int(stats.artists)),
        ])
    }
}

/// Shown on Home while the library is empty and no source is signed in.
struct WelcomeCard: View {
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s3) {
            Text("Welcome to Jellyfesh")
                .typography(Tokens.TypeRoles.section)
                .foregroundStyle(palette[Tokens.Ink.ink90])
                .accessibilityAddTraits(.isHeader)
            Text("Add your music folders, or connect a server in Sources.")
                .lookFont(size: 14)
                .foregroundStyle(palette[Tokens.Ink.ink70])
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: Tokens.Space.s2) {
                AddMusicFolderButton(prominent: true)
                Button {
                    router.select(.sources)
                } label: {
                    Label("Add a Source", systemImage: "antenna.radiowaves.left.and.right")
                }
            }
            .controlSize(.large)
        }
        .padding(Tokens.Space.s6)
        .frame(maxWidth: 560, alignment: .leading)
        .surfaceCard()
    }
}

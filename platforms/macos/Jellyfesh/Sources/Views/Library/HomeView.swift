// Home: counts, a welcome with Add Music Folder while the library is empty, recently added albums
// (newest first, in the core's order) and recently and most played songs.

import SwiftUI

struct HomeView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryStore.self) private var library
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    private var home: HomeModel { model.pages.home }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s7) {
                PageHeader(title: String(localized: "Home"), subtitle: statsLine)
                if home.loaded && library.isEmpty && !library.scanning {
                    WelcomeCard()
                }
                if !home.recentAlbums.isEmpty {
                    VStack(alignment: .leading, spacing: Tokens.Space.s3) {
                        SectionTitle("Recently Added")
                        ScrollView(.horizontal) {
                            LazyHStack(alignment: .top, spacing: Tokens.Space.s5) {
                                ForEach(home.recentAlbums) { album in
                                    AlbumCard(album: album, size: 160)
                                }
                            }
                            .padding(.bottom, Tokens.Space.s2)
                        }
                        .scrollIndicators(.hidden)
                    }
                }
                trackSection("Recently Played", rows: home.recentlyPlayed, prefix: "recent")
                trackSection("Most Played", rows: home.mostPlayed, prefix: "most")
            }
            .padding(.horizontal, Tokens.Space.s7)
            .padding(.vertical, Tokens.Space.s6)
            .frame(maxWidth: 1100, alignment: .leading)
        }
        .navigationTitle(Text("Home"))
        .task(id: library.revision) {
            await home.load(revision: library.revision)
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
                .glass(.mist, in: RoundedRectangle(cornerRadius: Tokens.Radius.rLg, style: .continuous))
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

/// Shown on Home while the library is empty.
struct WelcomeCard: View {
    @Environment(LibraryStore.self) private var library
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s3) {
            Text("Welcome to Jellyfesh")
                .typography(Tokens.TypeRoles.section)
                .foregroundStyle(palette[Tokens.Ink.ink90])
                .accessibilityAddTraits(.isHeader)
            Text("Add the folders that hold your music. Jellyfesh reads their tags, artwork and lyrics and keeps watching them.")
                .font(.system(size: 14))
                .foregroundStyle(palette[Tokens.Ink.ink70])
                .fixedSize(horizontal: false, vertical: true)
            Button {
                FilePanels.chooseMusicFolders { urls in
                    for url in urls { library.addFolder(url.path) }
                }
            } label: {
                Label("Add Music Folder…", systemImage: "folder.badge.plus")
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
        .padding(Tokens.Space.s6)
        .frame(maxWidth: 560, alignment: .leading)
        .glass(.mist, in: RoundedRectangle(cornerRadius: Tokens.Radius.rLg, style: .continuous))
    }
}

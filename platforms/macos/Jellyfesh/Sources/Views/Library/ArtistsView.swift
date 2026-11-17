// Artists, a page at a time in the core's order, and an artist's page: their albums and songs.

import SwiftUI

struct ArtistsView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryStore.self) private var library
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    var body: some View {
        let list = model.pages.artists
        List {
            PageHeader(title: String(localized: "Artists"), subtitle: subtitle)
                .padding(.vertical, Tokens.Space.s4)
                .trackListRow()
            ForEach(list.items) { artist in
                NavigationLink(value: Route.artist(artist)) {
                    HStack(spacing: Tokens.Space.s3) {
                        Image(systemName: "music.mic")
                            .font(.system(size: 15))
                            .foregroundStyle(palette[Tokens.Ink.ink50])
                            .frame(width: 40, height: 40)
                            .background(Circle().fill(palette[Tokens.Ink.ink06]))
                        VStack(alignment: .leading, spacing: 1) {
                            Text(artist.name.isEmpty ? String(localized: "Unknown Artist") : artist.name)
                                .typography(Tokens.TypeRoles.listTitle)
                                .foregroundStyle(palette[Tokens.Ink.ink90])
                                .lineLimit(1)
                            Text(CountText.songs(Int(artist.trackCount)))
                                .typography(Tokens.TypeRoles.listSecondary)
                                .foregroundStyle(palette[Tokens.Ink.ink70])
                        }
                        Spacer(minLength: 0)
                    }
                    .frame(height: Tokens.Player.rowHeight)
                }
                .trackListRow()
                .contextMenu {
                    Button("Play") { playback.play(.artist(id: artist.id)) }
                    Button("Shuffle") { playback.play(.artist(id: artist.id), shuffle: true) }
                    Divider()
                    Button("Play Next") { playback.enqueue(.artist(id: artist.id), next: true) }
                    Button("Play Last") { playback.enqueue(.artist(id: artist.id), next: false) }
                    AddToPlaylistMenu(collection: .artist(id: artist.id))
                }
                .onAppear { list.rowAppeared(artist) }
            }
        }
        .fieldList()
        .overlay {
            if list.items.isEmpty, list.loadedRevision != nil || list.error != nil {
                EmptyState(title: "No Artists Yet", symbol: "music.mic",
                           message: "Add a music folder to see your artists here.", error: list.error) {
                    AddMusicFolderButton()
                }
            }
        }
        .navigationTitle(Text("Artists"))
        .task(id: library.revision) {
            await list.load(revision: library.revision)
        }
    }

    private var subtitle: String? {
        guard let count = library.stats?.artists, count > 0 else { return nil }
        return CountText.artists(Int(count))
    }
}

struct ArtistDetailView: View {
    var artist: ArtistSummary

    @Environment(LibraryStore.self) private var library
    @Environment(PlaybackStore.self) private var playback
    @State private var albums: [AlbumSummary] = []
    @State private var songs: [Song] = []
    @State private var favorites = Set<TrackReference>()
    @State private var error: String?

    private let columns = [
        GridItem(.adaptive(minimum: 150, maximum: 200), spacing: Tokens.Space.s5, alignment: .top),
    ]

    var body: some View {
        let items = songs.map { TrackItem(song: $0, favorite: favorites.contains($0.reference)) }
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s7) {
                PageHeader(title: artist.name.isEmpty ? String(localized: "Unknown Artist") : artist.name,
                           subtitle: CountText.joined([
                               albums.isEmpty ? nil : CountText.albums(albums.count),
                               songs.isEmpty ? nil : CountText.songs(songs.count),
                           ])) {
                    HStack(spacing: Tokens.Space.s2) {
                        PlayShuffleButtons(collection: .artist(id: artist.id), disabled: songs.isEmpty)
                        CollectionMoreMenu(collection: .artist(id: artist.id))
                            .disabled(songs.isEmpty)
                    }
                }
                if !albums.isEmpty {
                    VStack(alignment: .leading, spacing: Tokens.Space.s3) {
                        SectionTitle("Albums")
                        LazyVGrid(columns: columns, alignment: .leading, spacing: Tokens.Space.s6) {
                            ForEach(albums) { album in
                                AlbumCard(album: album)
                            }
                        }
                    }
                }
                if !items.isEmpty {
                    VStack(alignment: .leading, spacing: Tokens.Space.s3) {
                        SectionTitle("Songs")
                        TrackStack(items: items) { index in
                            // The rows as shown, so the queue matches the page.
                            playback.play(.tracks(items.compactMap(\.track)), from: index)
                        }
                        .surface(in: RoundedRectangle(cornerRadius: surfaceRadius, style: .continuous))
                    }
                }
                if let error {
                    Text(error)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, Tokens.Space.s7)
            .padding(.vertical, Tokens.Space.s6)
        }
        .navigationTitle(Text(artist.name))
        .task(id: library.revision) {
            await load()
        }
    }

    private func load() async {
        let client = library.client
        do {
            async let albumList = client.artistAlbums(artistID: artist.id)
            async let songList = client.songs(SongQuery(offset: 0, limit: 500, sort: .album, artistID: artist.id))
            albums = try await albumList
            songs = try await songList
            favorites = (try? await client.favoriteTracks(among: songs.map(\.reference))) ?? []
            error = nil
        } catch {
            self.error = CoreClientError.describe(error)
        }
    }
}

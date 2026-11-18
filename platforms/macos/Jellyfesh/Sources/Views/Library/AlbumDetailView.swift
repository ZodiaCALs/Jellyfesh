// An album (after Apple Music), with the header every album, playlist and artist page shares
// (CollectionHeader): large artwork, the title, the artist in accent text (it opens the artist),
// genre, year and song count, Play, Shuffle and More (Play Next, Play Last, Add to Playlist…); then
// its tracks in the core's order, and a footer with the song count and length.

import SwiftUI

struct AlbumDetailView: View {
    var album: AlbumSummary

    @Environment(LibraryStore.self) private var library
    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @State private var songs: [Song] = []
    @State private var favorites = Set<TrackReference>()
    @State private var loaded = false
    @State private var error: String?
    @State private var selection = Set<TrackItem.ID>()

    var body: some View {
        let items = songs.map { TrackItem(song: $0, favorite: favorites.contains($0.reference), numbered: true) }
        List(selection: $selection) {
            header
                .padding(.vertical, Tokens.Space.s5)
                .trackListRow()
                .selectionDisabled()
            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                TrackRowView(item: item, style: .album, isCurrent: isCurrent(item), isPlaying: playback.isPlaying,
                             play: { playback.play(.album(id: album.id), from: index) })
                    .trackListRow()
                    .tag(item.id)
            }
            if loaded && !songs.isEmpty {
                Text(CountText.songsAndLength(count: songs.count,
                                              milliseconds: songs.reduce(0) { $0 + ($1.durationMs ?? 0) }))
                    .typography(Tokens.TypeRoles.listSecondary)
                    .foregroundStyle(palette[Tokens.Ink.ink50])
                    .padding(.top, Tokens.Space.s4)
                    .padding(.bottom, Tokens.Space.s6)
                    .trackListRow()
                    .selectionDisabled()
            }
            if let error {
                Text(error)
                    .foregroundStyle(.secondary)
                    .trackListRow()
                    .selectionDisabled()
            }
        }
        .fieldList()
        .trackListActions(items) { index in
            playback.play(.album(id: album.id), from: index)
        }
        .navigationTitle(Text(title))
        .task(id: library.revision) {
            await load()
        }
    }

    private var title: String {
        let name = songs.first?.album ?? album.title
        return name.isEmpty ? String(localized: "Unknown Album") : name
    }

    private var artistName: String? {
        album.albumArtist ?? songs.first?.albumArtist ?? songs.first?.artist
    }

    private var header: some View {
        let artist = artistName
        let showArtist: (() -> Void)? = artist.map { name in { self.openArtist(named: name) } }
        return CollectionHeader(
            artwork: (album.artworkHash ?? songs.first?.artworkHash).map(ArtworkSource.hash),
            placeholder: "square.stack",
            kind: String(localized: "Album"),
            title: title,
            artist: artist,
            openArtist: showArtist,
            facts: CountText.joined([
                songs.first?.genre,
                (album.year ?? songs.first?.year).map { String($0) },
                loaded ? CountText.songs(songs.count) : nil,
            ])
        ) {
            PlayShuffleButtons(collection: .album(id: album.id), disabled: songs.isEmpty)
            CollectionMoreMenu(collection: .album(id: album.id))
                .disabled(songs.isEmpty)
        }
    }

    /// Opens the artist page: the library's artist with this name, if it has one.
    private func openArtist(named name: String) {
        Task { @MainActor in
            if let artist = await library.artist(named: name) {
                router.open(.artist(artist))
            }
        }
    }

    private func isCurrent(_ item: TrackItem) -> Bool {
        item.track != nil && item.track == playback.nowPlaying?.track
    }

    private func load() async {
        let client = library.client
        do {
            songs = try await client.albumSongs(albumID: album.id)
            favorites = (try? await client.favoriteTracks(among: songs.map(\.reference))) ?? []
            error = nil
        } catch {
            self.error = CoreClientError.describe(error)
        }
        loaded = true
    }
}

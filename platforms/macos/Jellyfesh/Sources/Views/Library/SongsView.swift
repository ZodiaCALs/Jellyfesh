// Songs: Play and Shuffle for every song, then every track, sorted by the core, a page at a time.
// Playing from a row queues the rows loaded so far, from that row; more load as the list scrolls.
// And Favourites: the songs marked with the heart.

import SwiftUI

struct SongsView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryStore.self) private var library
    @Environment(PlaybackStore.self) private var playback
    @State private var selection = Set<TrackItem.ID>()
    @State private var favorites = Set<TrackReference>()

    private var songs: SongsModel { model.pages.songs }

    var body: some View {
        let list = songs.list
        let items = list.items.map { TrackItem(song: $0, favorite: favorites.contains($0.reference)) }
        List(selection: $selection) {
            PageHeader(title: String(localized: "Songs"), subtitle: subtitle) {
                PlayShuffleButtons(disabled: list.items.isEmpty) { shuffle in
                    Task { @MainActor in
                        await songs.playAll(shuffle: shuffle, on: playback)
                    }
                }
            }
            .padding(.vertical, Tokens.Space.s4)
            .trackListRow()
            .selectionDisabled()
            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                TrackRowView(item: item, isCurrent: item.track == playback.nowPlaying?.track,
                             isPlaying: playback.isPlaying)
                    .trackListRow()
                    .tag(item.id)
                    .onAppear { list.rowAppeared(index: index) }
            }
        }
        .fieldList()
        .trackListActions(items) { index in
            playback.play(tracks: items.compactMap(\.track), from: index)
        }
        .overlay {
            if list.items.isEmpty, list.loadedRevision != nil || list.error != nil {
                EmptyState(title: "No Songs Yet", symbol: "music.note",
                           message: "Add a music folder to see your songs here.", error: list.error) {
                    AddMusicFolderButton()
                }
            }
        }
        .navigationTitle(Text("Songs"))
        .toolbar {
            ToolbarItem {
                Menu {
                    Picker("Sort By", selection: Binding(
                        get: { songs.sort },
                        set: { sort in Task { @MainActor in await songs.choose(sort, revision: library.revision) } }
                    )) {
                        Text("Title").tag(SongSort.title)
                        Text("Artist").tag(SongSort.artist)
                        Text("Album").tag(SongSort.album)
                        Text("Year").tag(SongSort.year)
                        Text("Recently Added").tag(SongSort.added)
                    }
                    .pickerStyle(.inline)
                    Toggle("Descending", isOn: Binding(
                        get: { songs.descending },
                        set: { value in Task { @MainActor in await songs.setSort(songs.sort, descending: value, revision: library.revision) } }
                    ))
                } label: {
                    Label("Sort", systemImage: "arrow.up.arrow.down")
                }
                .help(Text("Sort Songs"))
            }
        }
        .task(id: library.revision) {
            await list.load(revision: library.revision)
        }
        .task(id: list.items.count) {
            // Favourite marks for the rows loaded so far.
            let refs = list.items.map(\.reference)
            guard !refs.isEmpty else { return }
            if let found = try? await library.client.favoriteTracks(among: refs) {
                favorites = found
            }
        }
    }

    private var subtitle: String? {
        guard let count = library.stats?.tracks, count > 0 else { return nil }
        return CountText.songs(Int(count))
    }
}

struct FavoritesView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryStore.self) private var library
    @Environment(PlaybackStore.self) private var playback
    @State private var selection = Set<TrackItem.ID>()

    var body: some View {
        let list = model.pages.favorites
        let items = list.items.enumerated().map { TrackItem(row: $0.element.row, id: "f\($0.offset)") }
        List(selection: $selection) {
            PageHeader(title: String(localized: "Favourites"),
                       subtitle: list.items.isEmpty ? nil : CountText.songs(list.items.count)) {
                PlayShuffleButtons(collection: .favorites, disabled: list.items.isEmpty)
            }
            .padding(.vertical, Tokens.Space.s4)
            .trackListRow()
            .selectionDisabled()
            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                TrackRowView(item: item, isCurrent: item.track == playback.nowPlaying?.track,
                             isPlaying: playback.isPlaying)
                    .trackListRow()
                    .tag(item.id)
                    .onAppear { list.rowAppeared(index: index) }
            }
        }
        .fieldList()
        .trackListActions(items) { index in
            playback.play(.favorites, from: index)
        }
        .overlay {
            if list.items.isEmpty, list.loadedRevision != nil || list.error != nil {
                EmptyState(title: "No Favourites Yet", symbol: "heart",
                           message: "Songs you mark with the heart appear here.", error: list.error)
            }
        }
        .navigationTitle(Text("Favourites"))
        .task(id: library.revision) {
            await list.load(revision: library.revision)
        }
    }
}

// Playlists: the person's playlists, new (named first) and imported ones, and a playlist's page
// with its entries in order (drag to reorder, Delete to remove) and More: Play Next, Play Last, Add
// to Playlist…, Rename…, Export Playlist… (M3U8) and Delete Playlist… (which asks first).

import SwiftUI

struct PlaylistsView: View {
    @Environment(LibraryStore.self) private var library
    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette

    var body: some View {
        List {
            PageHeader(title: String(localized: "Playlists"),
                       subtitle: library.playlists.isEmpty ? nil : String(localized: "\(library.playlists.count) playlists")) {
                HStack(spacing: Tokens.Space.s2) {
                    Button {
                        library.askForNewPlaylist()
                    } label: {
                        Label("New Playlist…", systemImage: "plus")
                    }
                    Button {
                        FilePanels.choosePlaylist { url in library.importPlaylist(url.path) }
                    } label: {
                        Label("Import Playlist…", systemImage: "square.and.arrow.down")
                    }
                }
                .buttonStyle(.bordered)
            }
            .padding(.vertical, Tokens.Space.s4)
            .trackListRow()
            ForEach(library.playlists) { playlist in
                NavigationLink(value: Route.playlist(playlist.id)) {
                    HStack(spacing: Tokens.Space.s3) {
                        ArtworkView(source: playlist.artworkHash.map(ArtworkSource.hash), size: 40,
                                    placeholder: "music.note.list")
                        VStack(alignment: .leading, spacing: 1) {
                            Text(playlist.name)
                                .typography(Tokens.TypeRoles.listTitle)
                                .foregroundStyle(palette[Tokens.Ink.ink90])
                                .lineLimit(1)
                            Text(CountText.joined([
                                CountText.songs(Int(playlist.entryCount)),
                                playlist.durationMs > 0 ? CountText.minutes(milliseconds: playlist.durationMs) : nil,
                            ]))
                            .typography(Tokens.TypeRoles.listSecondary)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                        }
                        Spacer(minLength: 0)
                    }
                    .frame(height: Tokens.Player.rowHeight)
                }
                .trackListRow()
                .contextMenu {
                    Button("Play") { playback.play(.playlist(id: playlist.id)) }
                    Button("Shuffle") { playback.play(.playlist(id: playlist.id), shuffle: true) }
                    Divider()
                    Button("Play Next") { playback.enqueue(.playlist(id: playlist.id), next: true) }
                    Button("Play Last") { playback.enqueue(.playlist(id: playlist.id), next: false) }
                    AddToPlaylistMenu(collection: .playlist(id: playlist.id), excluding: playlist.id)
                    Divider()
                    Button("Export Playlist…") {
                        FilePanels.savePlaylist(named: playlist.name) { url in
                            Task { @MainActor in await library.exportPlaylist(playlist.id, to: url.path) }
                        }
                    }
                }
            }
        }
        .fieldList()
        .overlay {
            if library.playlists.isEmpty {
                EmptyState(title: "No Playlists Yet", symbol: "music.note.list",
                           message: "Make a playlist to keep songs together.") {
                    Button("New Playlist…") {
                        library.askForNewPlaylist()
                    }
                }
            }
        }
        .navigationTitle(Text("Playlists"))
    }
}

struct PlaylistDetailView: View {
    var playlistID: Int64

    @Environment(LibraryStore.self) private var library
    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @State private var entries: PagedList<PlaylistRow>?
    @State private var selection = Set<TrackItem.ID>()
    @State private var renaming = false
    @State private var newName = ""
    @State private var confirmDelete = false

    private var info: PlaylistInfo? {
        library.playlists.first { $0.id == playlistID }
    }

    var body: some View {
        let rows = entries?.items ?? []
        let items = rows.map { TrackItem(row: $0.row, id: "e\($0.entryID)") }
        List(selection: $selection) {
            header(count: rows.count)
                .padding(.vertical, Tokens.Space.s5)
                .trackListRow()
                .selectionDisabled()
            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                TrackRowView(item: item, isCurrent: item.track == playback.nowPlaying?.track,
                             isPlaying: playback.isPlaying)
                    .trackListRow()
                    .tag(item.id)
                    .onAppear { entries?.rowAppeared(index: index) }
            }
            .onMove { from, to in
                guard let first = from.first, from.count == 1 else { return }
                library.moveEntry(from: first, to: to, in: rows, playlist: playlistID)
            }
        }
        .fieldList()
        .trackListActions(items) { index in
            playback.play(.playlist(id: playlistID), from: index)
        }
        .onDeleteCommand {
            let ids = rows.filter { selection.contains("e\($0.entryID)") }.map(\.entryID)
            library.removeEntries(ids, fromPlaylist: playlistID)
            selection = []
        }
        .overlay {
            if let list = entries, list.items.isEmpty, list.loadedRevision != nil || list.error != nil {
                EmptyState(title: "This Playlist Is Empty", symbol: "music.note.list",
                           message: "Add songs from their menu with Add to Playlist.", error: list.error)
            }
        }
        .navigationTitle(Text(info?.name ?? ""))
        .alert("Rename Playlist", isPresented: $renaming) {
            TextField("Name", text: $newName)
            Button("Rename") { library.renamePlaylist(playlistID, to: newName) }
            Button("Cancel", role: .cancel) {}
        }
        .confirmationDialog(Text("Delete “\(info?.name ?? "")”?"), isPresented: $confirmDelete,
                            titleVisibility: .visible) {
            Button("Delete Playlist", role: .destructive) {
                library.deletePlaylist(playlistID)
                router.select(.playlists)
            }
        } message: {
            Text("The songs stay in your library.")
        }
        .task(id: playlistID) {
            let list = PagedList<PlaylistRow>(pageSize: 200) { [client = library.client, playlistID] offset, limit in
                try await client.playlistEntries(id: playlistID, offset: offset, limit: limit)
            }
            entries = list
            await list.load(revision: library.revision)
        }
        .task(id: library.revision) {
            await entries?.load(revision: library.revision)
        }
    }

    private func header(count: Int) -> some View {
        CollectionHeader(
            artwork: info?.artworkHash.map(ArtworkSource.hash),
            placeholder: "music.note.list",
            kind: String(localized: "Playlist"),
            title: info?.name ?? "",
            facts: CountText.joined([
                info.map { CountText.songs(Int($0.entryCount)) },
                info.flatMap { $0.durationMs > 0 ? CountText.minutes(milliseconds: $0.durationMs) : nil },
            ])
        ) {
            PlayShuffleButtons(collection: .playlist(id: playlistID), disabled: count == 0)
            CollectionMoreMenu(collection: .playlist(id: playlistID), playlistID: playlistID) {
                Divider()
                Button("Rename…") {
                    newName = info?.name ?? ""
                    renaming = true
                }
                Button("Export Playlist…") {
                    FilePanels.savePlaylist(named: info?.name ?? String(localized: "Playlist")) { url in
                        Task { @MainActor in await library.exportPlaylist(playlistID, to: url.path) }
                    }
                }
                Divider()
                Button("Delete Playlist…", role: .destructive) { confirmDelete = true }
            }
        }
    }
}

// Playlists: the person's playlists, new and imported ones, and a playlist's page with its entries
// in order (drag to reorder, Delete to remove), Rename, Export as M3U and Delete.

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
                        library.createPlaylist(name: String(localized: "New Playlist"))
                    } label: {
                        Label("New Playlist", systemImage: "plus")
                    }
                    Button {
                        FilePanels.choosePlaylist { url in library.importPlaylist(url.path) }
                    } label: {
                        Label("Import…", systemImage: "square.and.arrow.down")
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
                    Button("Play Next") { playback.enqueue(.playlist(id: playlist.id), next: true) }
                    Button("Play Later") { playback.enqueue(.playlist(id: playlist.id), next: false) }
                    Divider()
                    Button("Export…") {
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
                EmptyState(title: "No Playlists", symbol: "music.note.list",
                           message: "Make one with New Playlist, or import an M3U file.")
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
    @Environment(\.palette) private var palette
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
                .padding(.vertical, Tokens.Space.s4)
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
        HStack(alignment: .bottom, spacing: Tokens.Space.s6) {
            ArtworkView(source: info?.artworkHash.map(ArtworkSource.hash), size: 160,
                        cornerRadius: Tokens.Artwork.radiusGrid, placeholder: "music.note.list")
                .elevation(Tokens.Elevation.shadow2)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(info?.name ?? "")
                    .font(.system(size: 28, weight: .bold))
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .lineLimit(2)
                    .accessibilityAddTraits(.isHeader)
                Text(CountText.joined([
                    info.map { CountText.songs(Int($0.entryCount)) },
                    info.flatMap { $0.durationMs > 0 ? CountText.minutes(milliseconds: $0.durationMs) : nil },
                ]))
                .typography(Tokens.TypeRoles.listSecondary)
                .foregroundStyle(palette[Tokens.Ink.ink70])
                HStack(spacing: Tokens.Space.s2) {
                    PlayShuffleButtons(collection: .playlist(id: playlistID), disabled: count == 0)
                    Menu {
                        Button("Rename…") {
                            newName = info?.name ?? ""
                            renaming = true
                        }
                        Button("Export…") {
                            FilePanels.savePlaylist(named: info?.name ?? String(localized: "Playlist")) { url in
                                Task { @MainActor in await library.exportPlaylist(playlistID, to: url.path) }
                            }
                        }
                        Divider()
                        Button("Delete…", role: .destructive) { confirmDelete = true }
                    } label: {
                        Label("More", systemImage: "ellipsis")
                    }
                    .menuStyle(.button)
                    .buttonStyle(.bordered)
                    .controlSize(.large)
                    .fixedSize()
                    .accessibilityLabel(Text("More Actions"))
                }
                .padding(.top, Tokens.Space.s2)
            }
            Spacer(minLength: 0)
        }
    }
}

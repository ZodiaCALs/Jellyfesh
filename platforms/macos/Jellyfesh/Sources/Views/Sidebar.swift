// The sidebar: library sections, Search and Sources, the person's playlists, and the scan status.
// The native sidebar material, as ADR-0007 asks. A customized look picks which places show and in
// what order (ADR-0020); a section left without places is not shown.

import SwiftUI

struct Sidebar: View {
    @Environment(Router.self) private var router
    @Environment(LibraryStore.self) private var library
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.look) private var look
    @State private var renaming: PlaylistInfo?
    @State private var newName = ""
    @State private var deleting: PlaylistInfo?

    var body: some View {
        List(selection: Binding<SidebarItem?>(
            get: { router.selection },
            set: { item in
                guard let item else { return }
                router.showNowPlaying(false, reduceMotion: true)
                router.select(item)
            }
        )) {
            let sections = SidebarItem.sections(for: look.sidebar)
            if !sections.library.isEmpty {
                Section("Library") {
                    ForEach(sections.library, id: \.self) { item in
                        Label(item.title, systemImage: item.symbol)
                            .tag(item)
                    }
                }
            }
            if !sections.discover.isEmpty {
                Section("Discover") {
                    ForEach(sections.discover, id: \.self) { item in
                        Label(item.title, systemImage: item.symbol)
                            .tag(item)
                    }
                }
            }
            Section {
                ForEach(library.playlists) { playlist in
                    Label(playlist.name, systemImage: "music.note.list")
                        .lineLimit(1)
                        .tag(SidebarItem.playlist(playlist.id))
                        .contextMenu {
                            Button("Play") { playback.play(.playlist(id: playlist.id)) }
                            Button("Shuffle") { playback.play(.playlist(id: playlist.id), shuffle: true) }
                            Divider()
                            Button("Rename…") {
                                newName = playlist.name
                                renaming = playlist
                            }
                            Button("Delete Playlist…", role: .destructive) {
                                deleting = playlist
                            }
                        }
                }
            } header: {
                HStack {
                    Text("Playlists")
                    Spacer()
                    Button {
                        library.askForNewPlaylist()
                    } label: {
                        Image(systemName: "plus")
                    }
                    .buttonStyle(.plain)
                    .help(Text("New Playlist…"))
                    .accessibilityLabel(Text("New Playlist…"))
                }
            }
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            ScanStatus()
        }
        .alert("Rename Playlist", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField("Name", text: $newName)
            Button("Rename") {
                if let playlist = renaming { library.renamePlaylist(playlist.id, to: newName) }
                renaming = nil
            }
            Button("Cancel", role: .cancel) { renaming = nil }
        }
        .confirmationDialog(
            Text("Delete “\(deleting?.name ?? "")”?"),
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            titleVisibility: .visible
        ) {
            Button("Delete Playlist", role: .destructive) {
                if let playlist = deleting {
                    if router.selection == .playlist(playlist.id) { router.select(.playlists) }
                    library.deletePlaylist(playlist.id)
                }
                deleting = nil
            }
        } message: {
            Text("The songs stay in your library.")
        }
    }
}

/// Scan progress at the foot of the sidebar while the core scans.
private struct ScanStatus: View {
    @Environment(LibraryStore.self) private var library

    var body: some View {
        if library.scanning {
            HStack(spacing: Tokens.Space.s2) {
                ProgressView()
                    .controlSize(.small)
                VStack(alignment: .leading, spacing: 0) {
                    Text("Scanning Library")
                        .lookFont(size: 11, weight: .semibold)
                    if case .progress(let seen, _, _)? = library.scan {
                        Text("Files checked: \(seen)")
                            .lookFont(size: 11)
                            .foregroundStyle(.secondary)
                            .monospacedDigit()
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, Tokens.Space.s4)
            .padding(.vertical, Tokens.Space.s2)
            .accessibilityElement(children: .combine)
        }
    }
}

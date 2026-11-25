// Downloads (ADR-0025, docs/ui.md "Downloads"): Play and Shuffle with the songs and the space used
// at the top, and why downloads wait when they do; the downloads in progress, each with its
// progress and Cancel, or after a failure the reason and Try Again; then the downloaded songs by
// album. Each album and song has Remove Download… (asked first). A downloaded song plays its file,
// offline too. Empty: "No Downloads Yet".

import SwiftUI

/// What Remove Download… asks about: one song, or an album's songs.
struct DownloadRemoval: Identifiable, Hashable {
    var name: String
    var ids: [UInt64]

    var id: [UInt64] { ids }
}

struct DownloadsView: View {
    @Environment(DownloadsStore.self) private var downloads
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette
    @Environment(\.look) private var look
    @State private var removing: DownloadRemoval?

    var body: some View {
        let state = downloads.state
        let active = downloads.inProgress
        let albums = downloads.albums
        let finished = albums.flatMap(\.entries)
        ScrollView {
            LazyVStack(alignment: .leading, spacing: look.spacing(Tokens.Space.s6)) {
                PageHeader(title: String(localized: "Downloads"), subtitle: summary(finished.count)) {
                    PlayShuffleButtons(disabled: finished.isEmpty) { shuffle in
                        play(finished, from: nil, shuffle: shuffle)
                    }
                }
                if let paused = state.paused, !active.isEmpty {
                    Label(DownloadsStore.paused(paused), systemImage: "pause.circle")
                        .lookFont(size: 13)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
                if !active.isEmpty {
                    VStack(alignment: .leading, spacing: Tokens.Space.s3) {
                        SectionTitle("In Progress")
                        LazyVStack(spacing: 0) {
                            ForEach(active) { entry in
                                DownloadProgressRow(entry: entry)
                            }
                        }
                        .surfaceCard()
                    }
                }
                ForEach(albums) { album in
                    DownloadedAlbumSection(album: album) { entry in
                        play(album.entries, from: entry, shuffle: false)
                    } remove: { removal in
                        removing = removal
                    }
                }
            }
            .pagePadding()
            .frame(maxWidth: 1100, alignment: .leading)
        }
        .overlay {
            if state.items.isEmpty {
                EmptyState(title: "No Downloads Yet", symbol: "arrow.down.circle",
                           message: "Download albums and playlists from your servers to play them offline.")
            }
        }
        .confirmationDialog(
            Text("Remove the Download of “\(removing?.name ?? "")”?"),
            isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
            titleVisibility: .visible
        ) {
            Button("Remove Download", role: .destructive) {
                if let removal = removing { downloads.remove(removal.ids) }
                removing = nil
            }
        } message: {
            Text("The file is deleted from this device. You can download it again.")
        }
        .navigationTitle(Text("Downloads"))
    }

    /// "12 songs · 1.2 GB used", once something has downloaded.
    private func summary(_ count: Int) -> String? {
        guard count > 0 else { return nil }
        let used = String(localized: "\(downloads.spaceUsed) used")
        return "\(CountText.songs(count)) · \(used)"
    }

    /// Plays downloaded songs: from one of them, or all of them (shuffled or not).
    private func play(_ entries: [DownloadEntry], from entry: DownloadEntry?, shuffle: Bool) {
        let tracks = entries.map(DownloadsStore.track)
        if shuffle {
            playback.play(.tracks(tracks), shuffle: true)
        } else {
            playback.play(tracks: tracks, from: entry.flatMap { entries.firstIndex(of: $0) } ?? 0)
        }
    }
}

/// A downloaded album: its name and artist, with Play and Remove Download… in its More menu, then
/// its songs.
private struct DownloadedAlbumSection: View {
    var album: DownloadedAlbum
    var play: (DownloadEntry?) -> Void
    var remove: (DownloadRemoval) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        let name = album.title ?? String(localized: "Unknown Album")
        VStack(alignment: .leading, spacing: Tokens.Space.s3) {
            HStack(alignment: .center, spacing: Tokens.Space.s3) {
                VStack(alignment: .leading, spacing: 2) {
                    SectionTitle(verbatim: name)
                    if let artist = album.artist, !artist.isEmpty {
                        Text(artist)
                            .typography(Tokens.TypeRoles.listSecondary)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                    }
                }
                Spacer(minLength: 0)
                DownloadMoreMenu {
                    Button("Play") { play(nil) }
                    Divider()
                    Button("Remove Download…", role: .destructive) {
                        remove(DownloadRemoval(name: name, ids: album.entries.map(\.id)))
                    }
                }
            }
            VStack(spacing: 0) {
                ForEach(album.entries) { entry in
                    DownloadedRow(entry: entry) {
                        play(entry)
                    } remove: {
                        remove(DownloadRemoval(name: entry.track.title, ids: [entry.id]))
                    }
                }
            }
            .surfaceCard()
        }
    }
}

/// A download in progress: the song, "Waiting", its progress and percentage, or why it failed with
/// Try Again; and Cancel.
private struct DownloadProgressRow: View {
    var entry: DownloadEntry

    @Environment(DownloadsStore.self) private var downloads
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Tokens.Space.s3) {
            ArtworkView(source: entry.track.artworkURL.map(ArtworkSource.url), size: 40)
            VStack(alignment: .leading, spacing: Tokens.Space.s1) {
                Text(entry.track.title)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .lineLimit(1)
                status
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if case .failed = entry.phase {
                Button("Try Again") {
                    downloads.retry(entry.id)
                }
            }
            Button("Cancel") {
                downloads.cancel(entry.id)
            }
        }
        .padding(.horizontal, Tokens.Space.s4)
        .padding(.vertical, Tokens.Space.s2)
        .frame(minHeight: Tokens.Player.rowHeight)
        .accessibilityElement(children: .contain)
    }

    @ViewBuilder private var status: some View {
        switch entry.phase {
        case .failed(let problem):
            Text(DownloadsStore.problem(problem))
                .typography(Tokens.TypeRoles.listSecondary)
                .foregroundStyle(palette[Tokens.Ink.ink70])
                .lineLimit(2)
        case .queued:
            Text("Waiting")
                .typography(Tokens.TypeRoles.listSecondary)
                .foregroundStyle(palette[Tokens.Ink.ink70])
        case .downloading, .done:
            if let progress = DownloadsStore.progress(entry) {
                HStack(spacing: Tokens.Space.s2) {
                    ProgressView(value: progress)
                        .progressViewStyle(.linear)
                        .controlSize(.small)
                    Text(progress, format: .percent.precision(.fractionLength(0)))
                        .typography(Tokens.TypeRoles.listSecondary)
                        .monospacedDigit()
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .frame(minWidth: 40, alignment: .trailing)
                }
            } else {
                ProgressView()
                    .progressViewStyle(.linear)
                    .controlSize(.small)
            }
        }
    }
}

/// A downloaded song: it plays on double-click; Remove Download… asks first.
private struct DownloadedRow: View {
    var entry: DownloadEntry
    var play: () -> Void
    var remove: () -> Void

    @Environment(\.palette) private var palette
    @State private var hovering = false

    var body: some View {
        HStack(spacing: Tokens.Space.s3) {
            ArtworkView(source: entry.track.artworkURL.map(ArtworkSource.url), size: 40)
            VStack(alignment: .leading, spacing: 1) {
                Text(entry.track.title)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .lineLimit(1)
                if let artist = entry.track.artist, !artist.isEmpty {
                    Text(artist)
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            DownloadedMark()
            DownloadMoreMenu {
                actions
            }
        }
        .padding(.horizontal, Tokens.Space.s4)
        .frame(minHeight: Tokens.Player.rowHeight)
        .background(hovering ? palette[Tokens.Line.fill] : .clear)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .onTapGesture(count: 2, perform: play)
        .contextMenu { actions }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { play() }
        .accessibilityAction(named: Text("Remove Download…"), remove)
    }

    @ViewBuilder private var actions: some View {
        Button("Play", action: play)
        Divider()
        Button("Remove Download…", role: .destructive, action: remove)
    }
}

/// A row's or album's More menu (an ellipsis).
private struct DownloadMoreMenu<Content: View>: View {
    @ViewBuilder var content: Content

    @Environment(\.palette) private var palette

    var body: some View {
        Menu {
            content
        } label: {
            Image(systemName: "ellipsis")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(palette[Tokens.Ink.ink70])
                .frame(width: 28, height: 28)
                .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityLabel(Text("More Actions"))
    }
}

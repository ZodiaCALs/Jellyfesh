// The queue: what plays now and Up Next in playback order. Drag to reorder, Delete to remove,
// double-click, Return or an entry's play button to jump; an entry's More and its right-click
// offer Play Now, Remove from Queue and the song's own menu (Go to Album, Go to Artist, Open in
// {Service}, …). Up Next pages past the slice's 50 entries as it scrolls, and Clear Queue asks
// first. While a remote session runs, Up Next is the service's ("Up Next is in
// {Service}."). And Info: About This Song, with Open in {Service} for a track that has a link.

import SwiftUI

struct QueuePanel: View {
    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        if playback.isRemote {
            ContentUnavailableView {
                Label("Up Next", systemImage: "list.bullet")
            } description: {
                Text(RemoteText.upNextElsewhere(service: playback.remoteService))
            }
        } else {
            LocalQueuePanel()
        }
    }
}

/// The engine's queue.
private struct LocalQueuePanel: View {
    @Environment(QueueStore.self) private var queue
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette
    @State private var selection = Set<UInt64>()
    @State private var confirmClear = false

    var body: some View {
        let rows = queue.upNextRows
        List(selection: $selection) {
            if let current = queue.current {
                Section {
                    QueueRowView(row: current, isCurrent: true, isPlaying: playback.isPlaying)
                        .trackListRow()
                        .selectionDisabled()
                } header: {
                    Text("Now Playing")
                        .lookFont(size: 12, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
            }
            Section {
                ForEach(rows) { row in
                    QueueRowView(row: row, isCurrent: false, isPlaying: false, isSelected: selection.contains(row.id))
                        .trackListRow()
                        .tag(row.id)
                }
                .onMove { from, to in
                    guard let first = from.first, from.count == 1 else { return }
                    playback.moveUpNext(from: first, to: to, in: queue.upNext)
                }
                if queue.hasMore {
                    HStack {
                        Spacer()
                        ProgressView().controlSize(.small)
                        Spacer()
                    }
                    .trackListRow()
                    .selectionDisabled()
                    .task(id: queue.upNext.count) { await queue.loadMore() }
                } else if rows.isEmpty && queue.current != nil {
                    NothingUpNext()
                        .padding(.vertical, Tokens.Space.s4)
                        .trackListRow()
                        .selectionDisabled()
                }
            } header: {
                HStack {
                    Text("Up Next")
                        .lookFont(size: 12, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                    Spacer()
                    if queue.current != nil || !rows.isEmpty {
                        Button("Clear Queue") {
                            confirmClear = true
                        }
                        .buttonStyle(.plain)
                        .lookFont(size: 12, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Accent.accentText])
                    }
                }
            }
        }
        .fieldList()
        .contextMenu(forSelectionType: UInt64.self) { ids in
            if ids.count == 1, let id = ids.first, let row = rows.first(where: { $0.id == id }) {
                // One entry: the same menu as its More.
                QueueRowMenu(row: row)
            } else if !ids.isEmpty {
                Button("Remove from Queue") {
                    for id in ids { playback.remove(id) }
                }
            }
        } primaryAction: { ids in
            if let id = ids.first { playback.jump(to: id) }
        }
        .onDeleteCommand {
            for id in selection { playback.remove(id) }
            selection = []
        }
        .overlay {
            if queue.current == nil && rows.isEmpty {
                EmptyState(title: "Nothing Up Next", symbol: "list.bullet",
                           message: "Use Play Next or Play Last on any song, album or playlist.")
            }
        }
        .confirmationDialog(Text("Clear the Queue?"), isPresented: $confirmClear, titleVisibility: .visible) {
            Button("Clear Queue", role: .destructive) {
                playback.clearQueue()
            }
        } message: {
            Text("Playback stops and every song is removed from the queue.")
        }
        .accessibilityLabel(Text("Queue"))
    }
}

/// Up Next with nothing after the song playing.
private struct NothingUpNext: View {
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(spacing: Tokens.Space.s1) {
            Text("Nothing Up Next")
                .lookFont(size: 13, weight: .semibold)
                .foregroundStyle(palette[Tokens.Ink.ink70])
            Text("Use Play Next or Play Last on any song, album or playlist.")
                .lookFont(size: 12)
                .foregroundStyle(palette[Tokens.Ink.ink50])
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .combine)
    }
}

/// An entry: the song playing now, or one of Up Next with its play button (which jumps to it) and
/// More while the pointer is over it or it is selected.
private struct QueueRowView: View {
    var row: QueueRow
    var isCurrent: Bool
    var isPlaying: Bool
    var isSelected = false

    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        Group {
            if isCurrent {
                TrackRowView(item: row.trackItem, style: .compact, isCurrent: true, isPlaying: isPlaying,
                             more: Optional<EmptyView>.none)
            } else {
                TrackRowView(item: row.trackItem, style: .compact, showsActions: isSelected,
                             play: { playback.jump(to: row.id) }, more: QueueRowMenu(row: row))
            }
        }
        .redacted(reason: row.resolved ? [] : .placeholder)
    }
}

/// An Up Next entry's menu, its More and its right-click alike: Play Now and Remove from Queue,
/// then the song's own menu without Play Next, Play Last and the heart.
private struct QueueRowMenu: View {
    var row: QueueRow

    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        Button("Play Now") { playback.jump(to: row.id) }
        Button("Remove from Queue") { playback.remove(row.id) }
        Divider()
        TrackActionsMenu(items: [row.trackItem], queueActions: false)
    }
}

extension QueueRow {
    /// The entry as a song's row shows it, and as its menu acts on it.
    var trackItem: TrackItem {
        var item = TrackItem(id: "q\(id)", track: entry.track, title: resolved ? title : String(localized: "Loading…"),
                             artist: subtitle, album: nil, durationMs: durationMs, artwork: artwork)
        item.keepSource(of: entry.track)
        return item
    }
}

/// About This Song (docs/ui.md): title, artist, album, year, length, format, lyrics and the file
/// (or the source it plays from), then Open in {Service} when the track has a link. For a remote
/// session: what the service reports, and where it plays.
struct InfoPanel: View {
    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        if playback.isRemote {
            RemoteInfoPanel()
        } else {
            LocalInfoPanel()
        }
    }
}

/// About This Song for a remote session's track.
private struct RemoteInfoPanel: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    var body: some View {
        let remote = playback.remote
        if let item = remote.item {
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text("About This Song")
                        .lookFont(size: 12, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                        .accessibilityAddTraits(.isHeader)
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        InfoRow(label: "Title", value: item.title)
                        InfoRow(label: "Artist", value: item.subtitle)
                        InfoRow(label: "Album", value: item.album)
                        InfoRow(label: "Length", value: playback.durationUs.map { TimeText.from(microseconds: $0) })
                        InfoRow(label: "Source", value: playback.remoteService)
                        InfoRow(label: "Device", value: remote.device?.name)
                    }
                    .padding(Tokens.Space.s4)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .surfaceCard()
                    OpenInServiceButton(link: item.link, source: remote.source)
                        .buttonStyle(.link)
                        .lookFont(size: 13)
                        .padding(.top, Tokens.Space.s1)
                }
                .padding(.vertical, Tokens.Space.s4)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollIndicators(.automatic)
        } else {
            EmptyState(title: "Nothing Playing", symbol: "info.circle", message: "Pick a song, an album or a playlist.")
        }
    }
}

/// One fact of About This Song; nothing when there is no value.
private struct InfoRow: View {
    var label: LocalizedStringKey
    var value: String?
    @Environment(\.palette) private var palette

    var body: some View {
        if let value, !value.isEmpty {
            LabeledContent {
                Text(value)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .textSelection(.enabled)
                    .multilineTextAlignment(.trailing)
            } label: {
                Text(label)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
            }
            .lookFont(size: 13)
        }
    }
}

/// About This Song and Audio for the engine's track (Get Info's rows), with the format actually
/// being decoded and where the lyrics come from.
private struct LocalInfoPanel: View {
    @Environment(AppModel.self) private var app
    @Environment(PlaybackStore.self) private var playback
    @Environment(LyricsStore.self) private var lyrics
    @Environment(LibraryStore.self) private var library
    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette
    /// The library song playing, for what Now Playing does not carry (its year).
    @State private var song: Song?
    /// The core's details of the song playing.
    @State private var details: DetailsAnswer?

    var body: some View {
        if let now = playback.nowPlaying {
            let facts = known(now)
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                    InfoSection(title: String(localized: "About This Song"),
                                lines: DetailsRows.song(details, known: facts)
                                    + [InfoLine(key: .lyrics, value: lyricsText(now))])
                    InfoSection(title: String(localized: "Audio"),
                                lines: DetailsRows.audio(details, known: facts,
                                                         decoded: now.quality ?? now.format.map(AudioQuality.init(stream:))))
                    if let attribution = details?.attribution?.nilIfEmpty {
                        Text(attribution)
                            .lookFont(size: 12)
                            .foregroundStyle(palette[Tokens.Ink.ink50])
                    }
                    if let path = now.path {
                        Button("Show in Finder") {
                            NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
                        }
                        .buttonStyle(.link)
                        .lookFont(size: 13)
                    }
                    if case .source(let song) = now.track {
                        OpenInServiceButton(link: now.link, source: song.plugin)
                            .buttonStyle(.link)
                            .lookFont(size: 13)
                    }
                }
                .padding(.vertical, Tokens.Space.s4)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollIndicators(.automatic)
            .task(id: now.uid) {
                song = nil
                details = nil
                let asked = now.uid
                app.answers.details(ItemReference(track: now.track), onAnswer: { answer in
                    if playback.nowPlaying?.uid == asked, answer.error == nil { details = answer }
                }, onFailure: { _ in })
                if case .library(let id) = now.track {
                    song = await library.song(id: id)
                }
            }
        } else {
            EmptyState(title: "Nothing Playing", symbol: "info.circle", message: "Pick a song, an album or a playlist.")
        }
    }

    /// What Now Playing and the library already say about the song.
    private func known(_ now: NowPlayingInfo) -> KnownDetails {
        var source: String?
        if case .source(let song) = now.track { source = sources.name(of: song.plugin) }
        return KnownDetails(title: now.title, artist: now.artist, album: now.album, year: (song?.year).map { String($0) },
                            durationMs: playback.durationUs.map { UInt64(clamping: max($0, 0) / 1_000) } ?? now.durationMs,
                            path: now.path, source: source)
    }

    /// "LRC · Line by line · en": where the lyrics are and how they are timed, and who made them
    /// when AI Lyrics did; "No Lyrics" without.
    private func lyricsText(_ now: NowPlayingInfo) -> String {
        guard let document = lyrics.document(for: now.uid) else { return String(localized: "No Lyrics") }
        if let engine = lyrics.madeBy(for: now.uid) {
            return CountText.joined([AiLyricsText.madeBy(engine: engine), timingText(document.timing)])
        }
        return CountText.joined([document.format, timingText(document.timing), document.language])
    }

    private func timingText(_ timing: LyricsTiming) -> String {
        switch timing {
        case .unsynced: return String(localized: "Unsynced")
        case .line: return String(localized: "Line by line")
        case .word: return String(localized: "Word by word")
        }
    }
}

/// The device button of the dock while the engine plays: where the sound goes
/// (`SetOutputDevice`; none follows the system default).
struct OutputDeviceMenu: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    var body: some View {
        Menu {
            Picker("Output", selection: Binding<String?>(
                get: { playback.devices.selected },
                set: { playback.selectOutput($0) }
            )) {
                Text("System Default").tag(String?.none)
                ForEach(playback.devices.devices) { device in
                    Text(device.name).tag(String?.some(device.id))
                }
            }
            .pickerStyle(.inline)
            Divider()
            Button("Refresh Devices") {
                playback.refreshDevices()
            }
        } label: {
            Image(systemName: "hifispeaker")
                .font(.system(size: 17))
                .foregroundStyle(palette[Tokens.Ink.ink90])
                .frame(width: 40, height: 40)
                .contentShape(Circle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityLabel(Text("Output Device"))
        .accessibilityValue(Text(currentName))
        .help(Text(currentName))
    }

    private var currentName: String {
        if let id = playback.devices.selected,
           let device = playback.devices.devices.first(where: { $0.id == id }) {
            return device.name
        }
        return String(localized: "System Default")
    }
}

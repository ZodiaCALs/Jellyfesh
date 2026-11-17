// The queue: what plays now and Up Next in playback order. Drag to reorder, Delete to remove,
// double-click or Return to jump; Up Next pages past the slice's 50 entries as it scrolls, and
// Clear Queue asks first. While a remote session runs, Up Next is the service's ("Up Next is in
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
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
            }
            Section {
                ForEach(rows) { row in
                    QueueRowView(row: row, isCurrent: false, isPlaying: false)
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
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                    Spacer()
                    if queue.current != nil || !rows.isEmpty {
                        Button("Clear Queue") {
                            confirmClear = true
                        }
                        .buttonStyle(.plain)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(palette[Tokens.Accent.accentText])
                    }
                }
            }
        }
        .fieldList()
        .contextMenu(forSelectionType: UInt64.self) { ids in
            if ids.count == 1, let id = ids.first {
                Button("Play Now") { playback.jump(to: id) }
            }
            Button("Remove from Queue") {
                for id in ids { playback.remove(id) }
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
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(palette[Tokens.Ink.ink70])
            Text("Use Play Next or Play Last on any song, album or playlist.")
                .font(.system(size: 12))
                .foregroundStyle(palette[Tokens.Ink.ink50])
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .combine)
    }
}

private struct QueueRowView: View {
    var row: QueueRow
    var isCurrent: Bool
    var isPlaying: Bool

    var body: some View {
        TrackRowView(
            item: TrackItem(id: "q\(row.id)", track: row.entry.track,
                            title: row.resolved ? row.title : String(localized: "Loading…"),
                            artist: row.subtitle, album: nil, durationMs: row.durationMs, artwork: row.artwork),
            style: .compact,
            isCurrent: isCurrent,
            isPlaying: isPlaying
        )
        .redacted(reason: row.resolved ? [] : .placeholder)
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
                        .font(.system(size: 12, weight: .semibold))
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
                    .surface(in: RoundedRectangle(cornerRadius: surfaceRadius, style: .continuous))
                    OpenInServiceButton(link: item.link, source: remote.source)
                        .buttonStyle(.link)
                        .font(.system(size: 13))
                        .padding(.top, Tokens.Space.s1)
                }
                .padding(.vertical, Tokens.Space.s4)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollIndicators(.automatic)
        } else {
            EmptyState(title: "Nothing Is Playing", symbol: "info.circle")
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
            .font(.system(size: 13))
        }
    }
}

/// About This Song for the engine's track.
private struct LocalInfoPanel: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(LyricsStore.self) private var lyrics
    @Environment(LibraryStore.self) private var library
    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette
    /// The library song playing, for what Now Playing does not carry (its year).
    @State private var song: Song?

    var body: some View {
        if let now = playback.nowPlaying {
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text("About This Song")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                        .accessibilityAddTraits(.isHeader)
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        row("Title", now.title)
                        row("Artist", now.artist)
                        row("Album", now.album)
                        row("Year", (song?.year).map { String($0) })
                        row("Length", playback.durationUs.map { TimeText.from(microseconds: $0) })
                        row("Format", now.format.map { Self.formatText($0) })
                        row("Lyrics", lyricsText(now))
                        place(now)
                    }
                    .padding(Tokens.Space.s4)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .surface(in: RoundedRectangle(cornerRadius: surfaceRadius, style: .continuous))
                    if case .source(let song) = now.track {
                        OpenInServiceButton(link: now.link, source: song.plugin)
                            .buttonStyle(.link)
                            .font(.system(size: 13))
                            .padding(.top, Tokens.Space.s1)
                    }
                }
                .padding(.vertical, Tokens.Space.s4)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollIndicators(.automatic)
            .task(id: now.uid) {
                song = nil
                if case .library(let id) = now.track {
                    song = await library.song(id: id)
                }
            }
        } else {
            EmptyState(title: "Nothing Is Playing", symbol: "info.circle")
        }
    }

    /// The file, with Show in Finder; a source's song names its source.
    @ViewBuilder private func place(_ now: NowPlayingInfo) -> some View {
        if let path = now.path {
            LabeledContent {
                VStack(alignment: .trailing, spacing: Tokens.Space.s1) {
                    Text(path)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .textSelection(.enabled)
                        .multilineTextAlignment(.trailing)
                        .lineLimit(4)
                    Button("Show in Finder") {
                        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
                    }
                    .buttonStyle(.link)
                }
            } label: {
                Text("File")
                    .foregroundStyle(palette[Tokens.Ink.ink70])
            }
            .font(.system(size: 13))
        } else if case .source(let source) = now.track {
            row("Source", sources.name(of: source.plugin))
        }
    }

    @ViewBuilder private func row(_ label: LocalizedStringKey, _ value: String?) -> some View {
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
            .font(.system(size: 13))
        }
    }

    /// "FLAC · 44.1 kHz · 2 channels · 1,411 kbps": the stream as the engine opened it.
    private static func formatText(_ format: StreamFormat) -> String {
        CountText.joined([
            format.codec,
            String(localized: "\(kilohertz(format.sampleRate)) kHz"),
            String(localized: "\(Int(format.channels)) channels"),
            format.bitrate.map { String(localized: "\(Int($0 / 1000)) kbps") },
        ])
    }

    /// "LRC · By line · en": where the lyrics are and how they are timed; "No Lyrics" without.
    private func lyricsText(_ now: NowPlayingInfo) -> String {
        guard let document = lyrics.document(for: now.uid) else { return String(localized: "No Lyrics") }
        return CountText.joined([document.format, timingText(document.timing), document.language])
    }

    /// 44.1, 48, 96: kilohertz with at most one decimal.
    private static func kilohertz(_ hertz: UInt32) -> String {
        (Double(hertz) / 1000).formatted(.number.precision(.fractionLength(0...1)))
    }

    private func timingText(_ timing: LyricsTiming) -> String {
        switch timing {
        case .unsynced: return String(localized: "Not timed")
        case .line: return String(localized: "By line")
        case .word: return String(localized: "By word")
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
        .accessibilityLabel(Text("Output device"))
        .accessibilityValue(Text(currentName))
        .help(Text(currentName))
    }

    private var currentName: String {
        if let id = playback.devices.selected,
           let device = playback.devices.devices.first(where: { $0.id == id }) {
            return device.name
        }
        return String(localized: "Speakers")
    }
}

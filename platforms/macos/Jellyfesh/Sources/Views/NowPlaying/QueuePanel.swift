// The queue: what plays now and Up Next in playback order. Drag to reorder, Delete to remove,
// double-click or Return to jump; Up Next pages past the slice's 50 entries as it scrolls.

import SwiftUI

struct QueuePanel: View {
    @Environment(QueueStore.self) private var queue
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette
    @State private var selection = Set<UInt64>()

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
                }
            } header: {
                HStack {
                    Text("Up Next")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                    Spacer()
                    if !rows.isEmpty {
                        Button("Clear") {
                            playback.clearQueue()
                        }
                        .buttonStyle(.plain)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(palette[Tokens.Accent.accentText])
                        .help(Text("Clear the queue"))
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
                EmptyState(title: "The Queue Is Empty", symbol: "list.bullet",
                           message: "Play an album, a playlist or a song and it shows here.")
            }
        }
        .accessibilityLabel(Text("Queue"))
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

/// What is playing and how: the stream's format, the file and where the lyrics came from.
struct InfoPanel: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(LyricsStore.self) private var lyrics
    @Environment(\.palette) private var palette

    var body: some View {
        if let now = playback.nowPlaying {
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s5) {
                    group("Song") {
                        row("Title", now.title)
                        row("Artist", now.artist)
                        row("Album", now.album)
                        row("Duration", playback.durationUs.map { TimeText.from(microseconds: $0) })
                    }
                    if let format = now.format {
                        group("Format") {
                            row("Codec", format.codec)
                            row("Sample Rate", String(localized: "\(Self.kilohertz(format.sampleRate)) kHz"))
                            row("Channels", String(localized: "\(Int(format.channels)) channels"))
                            row("Bit Rate", format.bitrate.map { String(localized: "\(Int($0 / 1000)) kbps") })
                            row("Gapless", format.gapless ? String(localized: "Yes") : String(localized: "No"))
                        }
                    }
                    if let document = lyrics.document(for: now.uid) {
                        group("Lyrics") {
                            row("Format", document.format)
                            row("Timing", timingText(document.timing))
                            row("Language", document.language)
                            row("Source", document.origin)
                        }
                    }
                    if let path = now.path {
                        group("File") {
                            Text(path)
                                .font(.system(size: 12))
                                .foregroundStyle(palette[Tokens.Ink.ink70])
                                .textSelection(.enabled)
                                .lineLimit(4)
                            Button("Show in Finder") {
                                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
                            }
                            .buttonStyle(.link)
                        }
                    }
                }
                .padding(.vertical, Tokens.Space.s4)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollIndicators(.automatic)
        } else {
            EmptyState(title: "Nothing Is Playing", symbol: "info.circle")
        }
    }

    private func group<Content: View>(_ title: LocalizedStringKey, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            Text(title)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(palette[Tokens.Ink.ink50])
                .accessibilityAddTraits(.isHeader)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                content()
            }
            .padding(Tokens.Space.s4)
            .frame(maxWidth: .infinity, alignment: .leading)
            .glass(.mist, in: RoundedRectangle(cornerRadius: Tokens.Radius.rLg, style: .continuous))
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

/// The Speakers chip: where the sound goes (`SetOutputDevice`; none follows the system default).
struct OutputDeviceMenu: View {
    @Environment(PlaybackStore.self) private var playback

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
            HStack(spacing: Tokens.Space.s2) {
                Image(systemName: "hifispeaker")
                Text(currentName)
                    .lineLimit(1)
            }
            .chipLabel()
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityLabel(Text("Output device"))
        .accessibilityValue(Text(currentName))
        .help(Text("Output device"))
    }

    private var currentName: String {
        if let id = playback.devices.selected,
           let device = playback.devices.devices.first(where: { $0.id == id }) {
            return device.name
        }
        return String(localized: "Speakers")
    }
}

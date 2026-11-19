// The controls of a remote session (ADR-0018, docs/ui.md "Links and remote playback"): the device
// button, which opens Devices (asking the service for its devices as it opens) with every device,
// its kind's glyph and the playing one checked; and the More menu of a remote track (Open in
// {Service}, Stop Controlling).

import SwiftUI

/// Opens Devices; choosing a device moves playback there.
struct RemoteDevicesButton: View {
    var size: CGFloat = 44
    var glyph: CGFloat = 17

    @Environment(PlaybackStore.self) private var playback
    @State private var open = false

    var body: some View {
        Button {
            open.toggle()
        } label: {
            Image(systemName: DeviceGlyph.symbol(playback.remote.device?.kind))
        }
        .buttonStyle(IconButtonStyle(size: size, glyph: glyph))
        .accessibilityLabel(Text("Devices"))
        .accessibilityValue(Text(playback.remote.device?.name ?? ""))
        .help(Text("Devices"))
        .popover(isPresented: $open, arrowEdge: .top) {
            DevicesList {
                open = false
            }
        }
        .onChange(of: open) { _, shown in
            if shown { playback.refreshRemote() }
        }
    }
}

/// Devices: the remote source's devices, the playing one checked.
private struct DevicesList: View {
    var close: () -> Void

    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        let rows = playback.remoteDeviceRows
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            Text("Devices")
                .lookFont(size: 13, weight: .semibold)
                .accessibilityAddTraits(.isHeader)
            if rows.isEmpty {
                HStack(spacing: Tokens.Space.s2) {
                    ProgressView().controlSize(.small)
                    Text("Looking for Devices…")
                        .lookFont(size: 13)
                        .foregroundStyle(.secondary)
                }
            }
            ForEach(rows) { row in
                Button {
                    if !row.checked {
                        playback.transferRemote(to: row.device.id)
                    }
                    close()
                } label: {
                    HStack(spacing: Tokens.Space.s3) {
                        Image(systemName: DeviceGlyph.symbol(row.device.kind))
                            .frame(width: 20)
                            .accessibilityHidden(true)
                        Text(row.device.name)
                            .lineLimit(1)
                        Spacer(minLength: Tokens.Space.s4)
                        if row.checked {
                            Image(systemName: "checkmark")
                                .font(.system(size: 12, weight: .semibold))
                                .accessibilityHidden(true)
                        }
                    }
                    .lookFont(size: 13)
                    .frame(minHeight: 28)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(row.checked ? [.isButton, .isSelected] : .isButton)
            }
        }
        .padding(Tokens.Space.s4)
        .frame(minWidth: 240, alignment: .leading)
    }
}

/// The glyph for a kind of device.
enum DeviceGlyph {
    static func symbol(_ kind: ServiceDeviceKind?) -> String {
        switch kind {
        case .computer?: return "desktopcomputer"
        case .smartphone?: return "iphone"
        case .speaker?: return "hifispeaker"
        case .tv?: return "tv"
        case .car?: return "car"
        case .other?, nil: return "speaker.wave.2"
        }
    }
}

/// The More menu of a remote session's track: Open in {Service} and Stop Controlling (the
/// service's player carries on by itself).
struct RemoteMoreItems: View {
    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        OpenInServiceButton(link: playback.remote.item?.link, source: playback.remote.source)
        Button("Stop Controlling") {
            playback.endRemote()
        }
    }
}

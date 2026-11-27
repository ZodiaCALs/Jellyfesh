// The player bar in windows 900 pt and wider (docs/ui.md, "The player bar and the mini player";
// Luminous): a floating Veil capsule 80 pt high with r-2xl corners, at most 1280 pt wide, the pages
// fading out just above it. In the web's order:
// * the artwork and two lines, the second being the line sung now while the lyrics bar shows
//   (LyricsBarView); they open Now Playing;
// * shuffle, previous, play, next and repeat (play is the only accent fill) above the seek bar with
//   its times;
// * the quality badge and its menu (QualityBadge, ADR-0026 §3);
// * Lyrics, Queue, the device button while a remote source plays, and the volume;
// * the mini player button (MiniPlayerStore) and More: the song's menu, Desktop Lyrics
//   (NowPlayingMoreMenu).
// The columns are laid out as the web's grid lays them out (PlayerBarColumns): each keeps to its
// own width, so nothing in the bar overlaps at any width (v0.0.9); the title, the artist and the
// translation end with "…", the sung line fades at the column's edge. A narrower bar drops the
// volume's line and keeps its mute button, then drops Lyrics and Queue, then the mute button, as
// the web's does (PlayerBarLayout). While a remote session runs the bar shows its track with
// "Playing on {Device}" (ADR-0018). Liquid Glass on macOS 26, like the capsule (FloatingGlass).

import SwiftUI

/// What the player bar has room for, by its own width (beside the sidebar it is the window's width
/// less 280 pt), and how its columns share that width. The web's container queries and grid.
enum PlayerBarLayout {
    /// The volume line shows instead of the mute button from here (the default window's bar).
    static let volumeLineMinWidth: CGFloat = 900
    /// Lyrics and Queue show from here; narrower, Now Playing still has them (⌥⌘L, ⌥⌘U).
    static let paneButtonsMinWidth: CGFloat = 760
    /// The mute button shows from here; narrower, ⌘↑ and ⌘↓ still change the volume.
    static let muteButtonMinWidth: CGFloat = 640

    /// The space before the artwork and after the last tool, and between the columns.
    static let leadingInset: CGFloat = Tokens.Space.s2
    static let trailingInset: CGFloat = Tokens.Space.s4
    static let columnSpacing: CGFloat = Tokens.Space.s4
    /// The artwork and lines' least width (the web's `minmax(112px, 1fr)`).
    static let linesMinWidth: CGFloat = 112
    /// The transport's narrowest (its five buttons) and widest, with the scrubber under it (the
    /// web's `minmax(232px, 340px)`; the buttons here are 220 pt).
    static let transportWidths: ClosedRange<CGFloat> = 220...340

    static func showsVolumeLine(barWidth: CGFloat) -> Bool { barWidth >= volumeLineMinWidth }
    static func showsPaneButtons(barWidth: CGFloat) -> Bool { barWidth >= paneButtonsMinWidth }
    static func showsMuteButton(barWidth: CGFloat) -> Bool { barWidth >= muteButtonMinWidth }

    /// The widths of the artwork-and-lines column and the transport column in a row `width` wide
    /// whose tools take `tools`: the transport grows to its widest while the lines keep their least,
    /// then the lines take the rest. The three columns and the two gaps never add up to more than
    /// `width` while it holds the tools and the narrowest transport.
    static func columns(width: CGFloat, tools: CGFloat) -> (lines: CGFloat, transport: CGFloat) {
        let free = max(0, width - tools - 2 * columnSpacing)
        let transport = min(max(free - linesMinWidth, transportWidths.lowerBound), transportWidths.upperBound, free)
        return (lines: free - transport, transport: transport)
    }
}

/// The bar's three columns as the web's grid lays them out (`minmax(112px, 1fr) minmax(232px,
/// 340px) auto`): the tools at their own width at the end, then the transport and the artwork and
/// lines as `PlayerBarLayout.columns` shares the rest. Each column is offered its width and no
/// more, so none runs into the next.
struct PlayerBarColumns: Layout {
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let sizes = subviews.map { $0.sizeThatFits(.unspecified) }
        let ideal = sizes.reduce(PlayerBarLayout.columnSpacing * CGFloat(max(sizes.count - 1, 0))) { $0 + $1.width }
        return CGSize(width: proposal.width ?? ideal, height: proposal.height ?? (sizes.map(\.height).max() ?? 0))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        guard subviews.count == 3 else {
            // Not the bar's three columns: side by side at their own widths.
            var x = bounds.minX
            for subview in subviews {
                let size = subview.sizeThatFits(.unspecified)
                subview.place(at: CGPoint(x: x, y: bounds.midY), anchor: .leading, proposal: ProposedViewSize(size))
                x += size.width + PlayerBarLayout.columnSpacing
            }
            return
        }
        let tools = subviews[2].sizeThatFits(.unspecified)
        let columns = PlayerBarLayout.columns(width: bounds.width, tools: tools.width)
        subviews[0].place(at: CGPoint(x: bounds.minX, y: bounds.midY), anchor: .leading,
                          proposal: ProposedViewSize(width: columns.lines, height: bounds.height))
        subviews[1].place(at: CGPoint(x: bounds.minX + columns.lines + PlayerBarLayout.columnSpacing, y: bounds.midY),
                          anchor: .leading, proposal: ProposedViewSize(width: columns.transport, height: bounds.height))
        subviews[2].place(at: CGPoint(x: bounds.maxX, y: bounds.midY), anchor: .trailing,
                          proposal: ProposedViewSize(tools))
    }
}

struct PlayerBar: View {
    @Environment(PlaybackStore.self) private var playback
    @State private var width: CGFloat = 1000

    var body: some View {
        if let item = playback.playing {
            FloatingGlass(cornerRadius: LuminousGeometry.barRadius) {
                content(item)
            }
            .elevation(Tokens.Elevation.shadow2)
            .frame(maxWidth: 1280)
            .background {
                GeometryReader { geometry in
                    Color.clear
                        .onAppear { width = geometry.size.width }
                        .onChange(of: geometry.size.width) { _, value in width = value }
                }
            }
            .padding(.horizontal, LuminousGeometry.playerMargin)
            .padding(.bottom, LuminousGeometry.playerMargin)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .accessibilityElement(children: .contain)
            .accessibilityLabel(Text("Player"))
        }
    }

    private func content(_ item: PlayingItem) -> some View {
        PlayerBarColumns() {
            PlayerBarNowPlaying(item: item)
                .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)

            VStack(spacing: 0) {
                PlayerBarTransport()
                SeekBar()
            }
            .frame(minWidth: 0, maxWidth: .infinity)

            PlayerBarTools(item: item, volumeLine: PlayerBarLayout.showsVolumeLine(barWidth: width),
                           paneButtons: PlayerBarLayout.showsPaneButtons(barWidth: width),
                           muteButton: PlayerBarLayout.showsMuteButton(barWidth: width))
        }
        .padding(.leading, PlayerBarLayout.leadingInset)
        .padding(.trailing, PlayerBarLayout.trailingInset)
        .frame(height: Tokens.Player.barHeight)
    }
}

/// The artwork and the two lines, one button that opens Now Playing; a soft fill on hover. The
/// lines take what the column leaves beside the artwork, and no more.
private struct PlayerBarNowPlaying: View {
    var item: PlayingItem

    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @Environment(\.look) private var look
    @State private var hovering = false

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: look.radius(Tokens.Radius.rXl), style: .continuous)
        Button {
            router.showNowPlaying(true, reduceMotion: !effects.animates)
        } label: {
            HStack(spacing: Tokens.Space.s3) {
                ArtworkView(source: item.artwork, size: 56, cornerRadius: Tokens.Radius.rLg)
                    .elevation(Tokens.Elevation.shadow1)
                PlayerBarLines(item: item)
                    .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
            }
            .padding(.leading, 6)
            .padding(.trailing, Tokens.Space.s2)
            .frame(height: 68)
            .background(shape.fill(hovering ? palette[Tokens.Line.fillHover] : .clear))
            .contentShape(shape)
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .accessibilityLabel(Text(CapsulePlayer.label(for: item)))
        .accessibilityHint(Text("Opens Now Playing"))
        .help(Text("Show Now Playing"))
    }
}

/// Shuffle, previous, play, next and repeat.
private struct PlayerBarTransport: View {
    @Environment(PlaybackStore.self) private var playback

    var body: some View {
        HStack(spacing: Tokens.Space.s1) {
            Button {
                playback.toggleShuffle()
            } label: {
                Image(systemName: "shuffle")
            }
            .buttonStyle(ToggleButtonStyle(isOn: playback.shuffle, size: 40, glyph: 14))
            .accessibilityLabel(Text("Shuffle"))
            .accessibilityValue(playback.shuffle ? Text("On") : Text("Off"))
            .help(Text("Shuffle"))

            Button {
                playback.previous()
            } label: {
                Image(systemName: "backward.end")
            }
            .buttonStyle(IconButtonStyle(size: 40, glyph: 16, prominent: true))
            .accessibilityLabel(Text("Previous"))
            .help(Text("Previous"))

            PlayPauseButton(size: 44, glyph: 16, prominent: true)

            Button {
                playback.next()
            } label: {
                Image(systemName: "forward.end")
            }
            .buttonStyle(IconButtonStyle(size: 40, glyph: 16, prominent: true))
            .accessibilityLabel(Text("Next"))
            .help(Text("Next"))

            Button {
                playback.cycleRepeat()
            } label: {
                Image(systemName: playback.repeatMode == .one ? "repeat.1" : "repeat")
            }
            .buttonStyle(ToggleButtonStyle(isOn: playback.repeatMode != .off, size: 40, glyph: 14))
            .accessibilityLabel(Text("Repeat"))
            .help(Text("Repeat"))
        }
    }
}

/// The quality badge, Lyrics and Queue (when there is room), the device button while a remote
/// source plays, the volume (its line, or its mute button, when there is room), the mini player
/// button and More, at their own width.
private struct PlayerBarTools: View {
    var item: PlayingItem
    var volumeLine: Bool
    var paneButtons: Bool
    var muteButton: Bool

    @Environment(PlaybackStore.self) private var playback
    @Environment(SettingsStore.self) private var settings
    @Environment(Router.self) private var router
    @Environment(MiniPlayerStore.self) private var miniPlayer
    @Environment(\.effects) private var effects

    var body: some View {
        HStack(spacing: 2) {
            if let menu = playback.qualityMenu(chosen: settings.current.streamingQuality) {
                QualityBadge(content: menu) { level in
                    settings.update { $0.streamingQuality = level }
                }
                .padding(.trailing, Tokens.Space.s1)
            }

            if paneButtons {
                Button {
                    router.showNowPlaying(true, panel: .lyrics, reduceMotion: !effects.animates)
                } label: {
                    Image(systemName: "quote.bubble")
                }
                .buttonStyle(IconButtonStyle(size: 40, glyph: 15))
                .accessibilityLabel(Text("Lyrics"))
                .help(Text("Lyrics"))

                Button {
                    router.showNowPlaying(true, panel: .queue, reduceMotion: !effects.animates)
                } label: {
                    Image(systemName: "list.bullet")
                }
                .buttonStyle(IconButtonStyle(size: 40, glyph: 15))
                .accessibilityLabel(Text("Queue"))
                .help(Text("Queue"))
            }

            if item.isRemote {
                RemoteDevicesButton(size: 40, glyph: 15)
            }

            if playback.showsVolume {
                if volumeLine {
                    VolumeSlider()
                        .frame(width: 150)
                        .padding(.horizontal, Tokens.Space.s1)
                } else if muteButton && playback.canMute {
                    VolumeMuteButton()
                }
            }

            Button {
                miniPlayer.toggle()
            } label: {
                Image(systemName: "pip")
            }
            .buttonStyle(ToggleButtonStyle(isOn: miniPlayer.shown, size: 40, glyph: 15))
            .accessibilityLabel(miniPlayer.shown ? Text("Close Mini Player") : Text("Open Mini Player"))
            .help(miniPlayer.shown ? Text("Close Mini Player") : Text("Open Mini Player"))

            NowPlayingMoreMenu()
        }
        .fixedSize()
    }
}

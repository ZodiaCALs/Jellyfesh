// The player bar in windows 900 pt and wider (docs/ui.md, "The player bar and the mini player";
// Luminous): a floating Veil capsule 80 pt high with r-2xl corners, at most 1280 pt wide, the pages
// fading out just above it. In the web's order:
// * the artwork and two lines, the second being the line sung now while the lyrics bar shows
//   (LyricsBarView); they open Now Playing;
// * shuffle, previous, play, next and repeat (play is the only accent fill) above the seek bar with
//   its times;
// * Lyrics, Queue, the device button while a remote source plays, and the volume;
// * More: the song's menu, Desktop Lyrics (NowPlayingMoreMenu).
// A narrower bar keeps the volume's mute button and drops its line, as the web's does
// (PlayerBarLayout). While a remote session runs the bar shows its track with "Playing on {Device}"
// (ADR-0018). Liquid Glass on macOS 26, like the capsule (FloatingGlass).

import SwiftUI

/// What the player bar has room for, by its own width.
enum PlayerBarLayout {
    /// The bar is this wide or wider: the volume line shows beside the mute button (in windows of
    /// about 1100 pt, beside the sidebar; the web keeps the button alone under about 1040 px).
    static let volumeLineMinWidth: CGFloat = 820

    static func showsVolumeLine(barWidth: CGFloat) -> Bool { barWidth >= volumeLineMinWidth }
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
        HStack(spacing: Tokens.Space.s4) {
            PlayerBarNowPlaying(item: item)
                .frame(minWidth: 120, maxWidth: .infinity, alignment: .leading)

            VStack(spacing: 0) {
                PlayerBarTransport()
                SeekBar()
            }
            .frame(minWidth: 220, maxWidth: 560)

            PlayerBarTools(item: item, volumeLine: PlayerBarLayout.showsVolumeLine(barWidth: width))
                .frame(maxWidth: .infinity, alignment: .trailing)
        }
        .padding(.leading, Tokens.Space.s2)
        .padding(.trailing, Tokens.Space.s4)
        .frame(height: Tokens.Player.barHeight)
    }
}

/// The artwork and the two lines, one button that opens Now Playing; a soft fill on hover.
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
                Spacer(minLength: 0)
            }
            .padding(.leading, 6)
            .padding(.trailing, Tokens.Space.s2)
            .frame(height: 68)
            .background(shape.fill(hovering ? palette[Tokens.Line.fillHover] : .clear))
            .contentShape(shape)
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .accessibilityLabel(Text(MiniPlayer.label(for: item)))
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

/// Lyrics, Queue, the device button while a remote source plays, the volume (its line when there
/// is room), and More.
private struct PlayerBarTools: View {
    var item: PlayingItem
    var volumeLine: Bool

    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @Environment(\.effects) private var effects

    var body: some View {
        HStack(spacing: 2) {
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

            if item.isRemote {
                RemoteDevicesButton(size: 40, glyph: 15)
            }

            if playback.showsVolume {
                if volumeLine {
                    VolumeSlider()
                        .frame(width: 150)
                        .padding(.horizontal, Tokens.Space.s1)
                } else if playback.canMute {
                    VolumeMuteButton()
                }
            }

            NowPlayingMoreMenu()
        }
        .fixedSize()
    }
}

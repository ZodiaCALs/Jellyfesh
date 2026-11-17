// The player bar for content columns 900 pt and wider (docs/design-controls.md, "v0.0.4"; Luminous,
// docs/ui.md): a floating Veil capsule 80 pt high with r-2xl corners, the content fading out just
// above it. Start: the artwork and two lines (they open Now Playing). Middle: shuffle, previous,
// play, next and repeat (play is the only accent fill) above the seek bar with its times. End:
// Lyrics, Queue, the Devices button while a remote session runs, and the volume. While a remote
// session runs the bar shows its track with "Playing on {Device}" (ADR-0018). Liquid Glass on
// macOS 26, like the mini player (FloatingGlass).

import SwiftUI

struct PlayerBar: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    var body: some View {
        if let item = playback.playing {
            FloatingGlass(cornerRadius: LuminousGeometry.barRadius) {
                content(item)
            }
            .elevation(Tokens.Elevation.shadow2)
            .padding(.horizontal, LuminousGeometry.playerMargin)
            .padding(.bottom, LuminousGeometry.playerMargin)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .accessibilityElement(children: .contain)
            .accessibilityLabel(Text("Player"))
        }
    }

    private func content(_ item: PlayingItem) -> some View {
        HStack(spacing: Tokens.Space.s4) {
            Button {
                router.showNowPlaying(true, reduceMotion: !effects.animates)
            } label: {
                HStack(spacing: Tokens.Space.s3) {
                    ArtworkView(source: item.artwork, size: 56, cornerRadius: Tokens.Radius.rLg)
                        .elevation(Tokens.Elevation.shadow1)
                    NowPlayingLines(item: item)
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityLabel(Text(MiniPlayer.label(for: item)))
            .accessibilityHint(Text("Opens Now Playing"))
            .help(Text("Show Now Playing"))

            VStack(spacing: 0) {
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
                SeekBar()
            }
            .frame(maxWidth: 560)

            HStack(spacing: Tokens.Space.s1) {
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
                    VolumeSlider()
                        .frame(maxWidth: 180)
                }
            }
            .frame(maxWidth: .infinity, alignment: .trailing)
        }
        .padding(.leading, Tokens.Space.s3)
        .padding(.trailing, Tokens.Space.s5)
        .frame(height: Tokens.Player.barHeight)
        .frame(maxWidth: 1240)
    }
}

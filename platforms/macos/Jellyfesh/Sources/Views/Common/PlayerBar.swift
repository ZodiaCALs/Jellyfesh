// The player bar for content columns 900 pt and wider (docs/design-controls.md, "v0.0.4"), after
// Media Player and Apple Music: a floating Veil bar 80 pt high with r-xl corners. Start: the
// artwork and two lines (they open Now Playing). Middle: the transport capsule (shuffle, previous,
// play, next, repeat; play is the only accent fill) above the seek bar with its times. End: Lyrics,
// Queue and the volume. It bends CMP-02 (one control) on purpose, for desktop use. Liquid Glass on
// macOS 26, like the mini player (FloatingGlass).

import SwiftUI

struct PlayerBar: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    var body: some View {
        if let now = playback.nowPlaying {
            FloatingGlass(cornerRadius: Tokens.Radius.rXl) {
                content(now)
            }
            .elevation(Tokens.Elevation.shadow2)
            .padding(.horizontal, Tokens.Space.s5)
            .padding(.bottom, Tokens.Space.s4)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .accessibilityElement(children: .contain)
            .accessibilityLabel(Text("Player"))
        }
    }

    private func content(_ now: NowPlayingInfo) -> some View {
        HStack(spacing: Tokens.Space.s4) {
            Button {
                router.showNowPlaying(true, reduceMotion: !effects.animates)
            } label: {
                HStack(spacing: Tokens.Space.s3) {
                    ArtworkView(source: now.artwork.flatMap { ArtworkSource(hash: nil, location: $0) },
                                size: 48, cornerRadius: Tokens.Radius.rMd)
                    NowPlayingLines(now: now)
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityLabel(Text(MiniPlayer.label(for: now)))
            .accessibilityHint(Text("Opens Now Playing"))
            .help(Text("Show Now Playing"))

            VStack(spacing: 0) {
                HStack(spacing: Tokens.Space.s1) {
                    Button {
                        playback.toggleShuffle()
                    } label: {
                        Image(systemName: "shuffle")
                    }
                    .buttonStyle(ToggleButtonStyle(isOn: playback.queue.shuffle, size: 40, glyph: 14))
                    .accessibilityLabel(Text("Shuffle"))
                    .accessibilityValue(playback.queue.shuffle ? Text("On") : Text("Off"))
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
                        Image(systemName: playback.queue.repeatMode == .one ? "repeat.1" : "repeat")
                    }
                    .buttonStyle(ToggleButtonStyle(isOn: playback.queue.repeatMode != .off, size: 40, glyph: 14))
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

                VolumeSlider()
                    .frame(maxWidth: 180)
            }
            .frame(maxWidth: .infinity, alignment: .trailing)
        }
        .padding(.horizontal, Tokens.Space.s3)
        .frame(height: Tokens.Player.barHeight)
        .frame(maxWidth: 1240)
    }
}

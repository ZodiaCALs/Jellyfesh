// The mini player for content columns under 900 pt (docs/design-controls.md, "v0.0.4"): a floating
// Veil capsule with `player.mini-radius` corners, the artwork, two lines of type and one control,
// play and pause (Misty CMP-02: art, two lines, one progress indicator, one control; never
// accent-filled, COL-04). Its progress is a straight 2 pt line along the bottom edge, inset by the
// corners, redrawn only when it grows by a pixel. Clicking it opens Now Playing.

import SwiftUI

struct MiniPlayer: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    var body: some View {
        if let now = playback.nowPlaying {
            FloatingGlass(cornerRadius: SeekGeometry.miniRadius) {
                HStack(spacing: Tokens.Space.s3) {
                    Button {
                        router.showNowPlaying(true, reduceMotion: !effects.animates)
                    } label: {
                        HStack(spacing: Tokens.Space.s3) {
                            ArtworkView(source: now.artwork.flatMap { ArtworkSource(hash: nil, location: $0) },
                                        size: 44, cornerRadius: Tokens.Radius.rMd)
                            NowPlayingLines(now: now)
                            Spacer(minLength: 0)
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(Text(MiniPlayer.label(for: now)))
                    .accessibilityHint(Text("Opens Now Playing"))
                    .help(Text("Show Now Playing"))

                    PlayPauseButton(size: 44, glyph: 16, prominent: false)
                }
                .padding(10)
                .frame(height: Tokens.Player.miniHeight)
                .frame(maxWidth: 560)
                .overlay(alignment: .bottom) {
                    MiniProgressLine()
                        .padding(.horizontal, SeekGeometry.miniRadius)
                        .padding(.bottom, 1)
                }
            }
            .elevation(Tokens.Elevation.shadow2)
            .padding(.horizontal, Tokens.Space.s5)
            .padding(.bottom, Tokens.Space.s4)
            .transition(.move(edge: .bottom).combined(with: .opacity))
        }
    }

    static func label(for now: NowPlayingInfo) -> String {
        if let artist = now.artist {
            return String(localized: "Now Playing: \(now.title), \(artist)")
        }
        return String(localized: "Now Playing: \(now.title)")
    }
}

/// The title and the artist under it (the mini player and the player bar).
struct NowPlayingLines: View {
    var now: NowPlayingInfo
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(now.title)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(palette[Tokens.Ink.ink90])
                .lineLimit(1)
            if let artist = now.artist {
                Text(artist)
                    .font(.system(size: 12))
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .lineLimit(1)
            }
        }
    }
}

/// Play or pause, morphing into each other (an SF Symbol replace, dur-2 Spring). On the mini player
/// a plain glyph button (never accent-filled); on Now Playing and the player bar the accent circle.
struct PlayPauseButton: View {
    var size: CGFloat
    var glyph: CGFloat
    /// The accent circle (Now Playing, the player bar) instead of a plain glyph.
    var prominent: Bool

    @Environment(PlaybackStore.self) private var playback
    @Environment(\.effects) private var effects

    var body: some View {
        let playing = playback.isPlaying
        Group {
            if prominent {
                Button {
                    playback.togglePlayPause()
                } label: {
                    glyphImage(playing)
                }
                .buttonStyle(PlayButtonStyle(size: size))
            } else {
                Button {
                    playback.togglePlayPause()
                } label: {
                    glyphImage(playing)
                }
                .buttonStyle(IconButtonStyle(size: size, glyph: glyph, prominent: true))
            }
        }
        .animation(Motion.unless(!effects.animates, Motion.spring(Tokens.Motion.dur2Ms)), value: playing)
        .accessibilityLabel(playing ? Text("Pause") : Text("Play"))
        .help(playing ? Text("Pause") : Text("Play"))
    }

    private func glyphImage(_ playing: Bool) -> some View {
        Image(systemName: playing ? "pause.fill" : "play.fill")
            .contentTransition(.symbolEffect(.replace))
    }
}

/// The mini player's progress: a straight `player.mini-progress` line, the rest faint and the
/// played part in ink-90, one step per device pixel.
struct MiniProgressLine: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @Environment(\.displayScale) private var displayScale
    @State private var width: CGFloat = 0

    var body: some View {
        let step = barStepUs(durationUs: playback.durationUs, lengthPoints: width, scale: displayScale)
        TimelineView(ProgressSchedule(start: playback.scheduleStart(), stepUs: step, seconds: false,
                                      active: effects.windowVisible)) { _ in
            let fraction = playback.progressFraction()
            ZStack(alignment: .leading) {
                Capsule().fill(palette[Tokens.Ink.ink10])
                Capsule()
                    .fill(palette[Tokens.Ink.ink90])
                    .frame(width: width * CGFloat(min(max(fraction, 0), 1)))
            }
        }
        .frame(height: SeekGeometry.miniProgress)
        .background {
            GeometryReader { geometry in
                Color.clear
                    .onAppear { width = geometry.size.width }
                    .onChange(of: geometry.size.width) { _, value in width = value }
            }
        }
        .accessibilityHidden(true)
    }
}

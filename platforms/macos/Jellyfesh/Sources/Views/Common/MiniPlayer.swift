// The capsule player for windows under 900 pt (docs/ui.md, "The player bar and the mini player"):
// a floating Veil capsule with `player.mini-radius` corners, the artwork, two lines of type, play
// and next (never accent-filled, COL-04). Its progress is a straight 2 pt line along the bottom
// edge, inset by the corners, redrawn only when it grows by a pixel. Clicking it opens Now Playing.
// While a remote session runs it shows the session's track with "Playing on {Device}" (ADR-0018).
// The look's Floating style makes it a smaller capsule (ADR-0020).

import SwiftUI

struct MiniPlayer: View {
    /// A smaller capsule, for the look's Floating style.
    var floating = false

    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    var body: some View {
        if let item = playback.playing {
            FloatingGlass(cornerRadius: radius) {
                HStack(spacing: Tokens.Space.s3) {
                    Button {
                        router.showNowPlaying(true, reduceMotion: !effects.animates)
                    } label: {
                        HStack(spacing: Tokens.Space.s3) {
                            ArtworkView(source: item.artwork, size: 44, cornerRadius: Tokens.Radius.rMd)
                            NowPlayingLines(item: item)
                            Spacer(minLength: 0)
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(Text(MiniPlayer.label(for: item)))
                    .accessibilityHint(Text("Opens Now Playing"))
                    .help(Text("Show Now Playing"))

                    HStack(spacing: 0) {
                        PlayPauseButton(size: 44, glyph: 16, prominent: false)
                        Button {
                            playback.next()
                        } label: {
                            Image(systemName: "forward.fill")
                        }
                        .buttonStyle(IconButtonStyle(size: 44, glyph: 15, prominent: true))
                        .accessibilityLabel(Text("Next"))
                        .help(Text("Next"))
                    }
                }
                .padding(10)
                .padding(.horizontal, floating ? Tokens.Space.s2 : 0)
                .frame(height: Tokens.Player.miniHeight)
                .frame(maxWidth: floating ? 400 : 560)
                .overlay(alignment: .bottom) {
                    MiniProgressLine()
                        .padding(.horizontal, radius)
                        .padding(.bottom, 1)
                }
            }
            .elevation(Tokens.Elevation.shadow2)
            .padding(.horizontal, LuminousGeometry.playerMargin)
            .padding(.bottom, LuminousGeometry.playerMargin)
            .transition(.move(edge: .bottom).combined(with: .opacity))
        }
    }

    /// The capsule's corners: `player.mini-radius`, or round ends when floating.
    private var radius: CGFloat {
        floating ? Tokens.Player.miniHeight / 2 : SeekGeometry.miniRadius
    }

    static func label(for item: PlayingItem) -> String {
        if let second = NowPlayingLines.secondLine(item) {
            return String(localized: "Now Playing: \(item.title), \(second)")
        }
        return String(localized: "Now Playing: \(item.title)")
    }
}

/// The title and the line under it (the mini player and the player bar): the artist, or for a
/// remote session "Playing on {Device}", with its last error in a quieter line below.
struct NowPlayingLines: View {
    var item: PlayingItem
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(item.title)
                .lookFont(size: 14, weight: .semibold)
                .foregroundStyle(palette[Tokens.Ink.ink90])
                .lineLimit(1)
            if let second = Self.secondLine(item) {
                Text(second)
                    .lookFont(size: 12)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .lineLimit(1)
            }
            if let error = item.remote?.error {
                Text(error)
                    .lookFont(size: 11)
                    .foregroundStyle(palette[Tokens.Ink.ink50])
                    .lineLimit(1)
                    .help(error)
            }
        }
    }

    /// The artist; "Playing on {Device}" (or the service) for a remote session.
    static func secondLine(_ item: PlayingItem) -> String? {
        if let remote = item.remote {
            return RemoteText.playingOn(remote)
        }
        return item.artist?.nilIfEmpty
    }
}

/// The words of a remote session (docs/ui.md, "Links and remote playback").
enum RemoteText {
    /// "Playing on {Device}"; the service's name while the device is not known yet.
    static func playingOn(_ remote: PlayingItem.Remote) -> String {
        if let device = remote.device {
            return String(localized: "Playing on \(device)")
        }
        return remote.service
    }

    static func lyricsHidden(service: String) -> String {
        String(localized: "Lyrics aren't shown while \(service) plays.")
    }

    static func upNextElsewhere(service: String) -> String {
        String(localized: "Up Next is in \(service).")
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

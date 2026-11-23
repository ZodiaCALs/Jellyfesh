// The mini player (ADR-0026 §4, docs/ui.md "The player bar and the mini player"), in its own small
// window above other windows (Platform/MiniPlayerWindow.swift): the cover, the title and the
// artist, the line being sung when Show Lyrics Line is on and the song has timed lyrics, previous,
// play and next, and a progress line along the bottom edge. Close is its own button in the top
// corner, brighter while the pointer is over the window. With nothing playing it says "Nothing
// Playing". While a remote session runs it shows the session's track (ADR-0018). Nothing
// time-driven is drawn while the window cannot be seen.

import SwiftUI

struct MiniPlayerView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(ThemeStore.self) private var theme
    @Environment(LookStore.self) private var lookStore
    @Environment(SystemState.self) private var system
    @Environment(MiniPlayerStore.self) private var miniPlayer
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @State private var hovering = false
    /// The window can be seen: nothing time-driven is drawn while it cannot.
    @State private var visible = true

    static let radius: CGFloat = Tokens.Player.miniRadius

    var body: some View {
        let look = LookStyle(state: lookStore.state)
        let palette = theme.palette(for: colorScheme)
        let effects = makeEffects(look: look)
        ZStack(alignment: .topTrailing) {
            MiniPlayerContent()
                .padding(14)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .overlay(alignment: .bottom) {
                    MiniProgressLine()
                        .padding(.horizontal, look.radius(Self.radius))
                        .padding(.bottom, 1)
                }
            Button {
                miniPlayer.close()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .frame(width: 22, height: 22)
                    .background(Circle().fill(palette[Tokens.Line.fill]))
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .padding(Tokens.Space.s2)
            .opacity(hovering ? 1 : 0.55)
            .accessibilityLabel(Text("Close Mini Player"))
            .help(Text("Close Mini Player"))
        }
        .frame(width: MiniPlayerWindow.size.width, height: MiniPlayerWindow.size.height)
        .background { windowGlass(palette: palette, effects: effects, look: look) }
        .animation(reduceMotion ? nil : .easeOut(duration: 0.15), value: hovering)
        .background {
            ZStack {
                WindowVisibilityReader(onChange: { visible = $0 })
                PointerHoverReader(onChange: { hovering = $0 })
            }
        }
        .environment(\.palette, palette)
        .environment(\.effects, effects)
        .environment(\.look, look)
        .tint(palette[Tokens.Accent.accent])
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Mini Player"))
    }

    /// The system's material with the Veil tint, or solid Cloud when transparency is reduced; one
    /// edge.
    @ViewBuilder private func windowGlass(palette: Palette, effects: Effects, look: LookStyle) -> some View {
        let radius = look.radius(Self.radius)
        let shape = RoundedRectangle(cornerRadius: radius, style: .continuous)
        ZStack {
            if effects.solidGlass {
                shape.fill(palette[Tokens.Glass.cloud])
            } else {
                BehindWindowMaterial(cornerRadius: radius)
                shape.fill(GlassLevel.veil.tintColor(palette, transparency: effects.transparency))
            }
            shape.strokeBorder(palette[effects.solidGlass ? Tokens.Line.lineSoft : Tokens.Glass.glassEdge], lineWidth: 1)
        }
    }

    private func makeEffects(look: LookStyle) -> Effects {
        let current = settings.current
        let transparency = look.transparency(setting: Double(current.transparency))
        return Effects(
            reduceMotion: reduceMotion,
            solidGlass: reduceTransparency || system.lowPower || current.reduceEffects || transparency <= 0.001,
            reduceEffects: current.reduceEffects,
            windowVisible: visible,
            transparency: transparency
        )
    }
}

/// The cover, the lines and the transport.
private struct MiniPlayerContent: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(MiniPlayerStore.self) private var miniPlayer
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s3) {
            ArtworkView(source: playback.playing?.artwork, size: 84, cornerRadius: Tokens.Radius.rLg)
            VStack(alignment: .leading, spacing: 1) {
                if let item = playback.playing {
                    Text(item.title)
                        .lookFont(size: 14, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .lineLimit(1)
                    if let second = NowPlayingLines.secondLine(item) {
                        Text(second)
                            .lookFont(size: 12)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                            .lineLimit(1)
                    }
                    if miniPlayer.showsLyricsLine {
                        MiniPlayerLyricsLine()
                            .frame(height: 18, alignment: .leading)
                    }
                } else {
                    Text("Nothing Playing")
                        .lookFont(size: 14, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
                Spacer(minLength: 0)
                HStack(spacing: Tokens.Space.s1) {
                    Button {
                        playback.previous()
                    } label: {
                        Image(systemName: "backward.fill")
                    }
                    .buttonStyle(IconButtonStyle(size: 32, glyph: 13, prominent: true))
                    .accessibilityLabel(Text("Previous"))
                    .help(Text("Previous"))

                    PlayPauseButton(size: 34, glyph: 13, prominent: true)

                    Button {
                        playback.next()
                    } label: {
                        Image(systemName: "forward.fill")
                    }
                    .buttonStyle(IconButtonStyle(size: 32, glyph: 13, prominent: true))
                    .accessibilityLabel(Text("Next"))
                    .help(Text("Next"))
                }
                .disabled(!playback.hasTrack)
            }
            .frame(height: 84)
            .padding(.trailing, Tokens.Space.s5)
        }
    }
}

/// The line being sung, filling word by word (the lyrics bar's line); nothing without timed
/// lyrics, before the first line, or while another device plays.
private struct MiniPlayerLyricsLine: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(LyricsStore.self) private var lyrics
    @Environment(\.effects) private var effects

    var body: some View {
        let uid = playback.nowPlaying?.uid
        if let item = playback.playing, !item.isRemote, let document = lyrics.document(for: uid),
           let timeline = lyrics.timeline(for: uid) {
            TimelineView(LyricsSchedule(timeline: timeline, start: playback.scheduleStart(),
                                        active: effects.windowVisible)) { _ in
                let bar = timeline.bar(atMs: lyricMilliseconds(playback.positionUs()))
                if let index = bar.line, index < document.lines.count {
                    LyricsBarLine(line: document.lines[index], index: index, bar: bar, timeline: timeline,
                                  fontSize: 13, weight: .semibold)
                }
            }
        }
    }
}

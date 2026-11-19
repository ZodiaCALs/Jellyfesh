// Desktop Lyrics (ADR-0022 §6, docs/ui.md "Lyrics, word by word"): the line being sung, filling
// word by word in the highlight colour, and the next line under it, in light ink with a soft
// shadow so they read over any desktop. Before the first line the song's title stands in for it;
// without timed lyrics (or while another device plays) the title and the artist show, and with
// nothing playing, "Nothing Playing". Unlocked, pointing at the window shows a backdrop with Lock
// and Close; locked, clicks pass through it, and View › Unlock Desktop Lyrics unlocks it. The
// window is Platform/DesktopLyricsWindow.swift.

import SwiftUI

struct DesktopLyricsView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(ThemeStore.self) private var theme
    @Environment(LookStore.self) private var lookStore
    @Environment(SystemState.self) private var system
    @Environment(DesktopLyricsStore.self) private var desktop
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @State private var hovering = false
    /// The window can be seen: nothing time-driven is drawn while it cannot.
    @State private var visible = true

    var body: some View {
        let look = LookStyle(state: lookStore.state)
        let chrome = hovering && !desktop.locked
        ZStack(alignment: .topTrailing) {
            // Unlocked, the faintest fill keeps the whole window under the pointer (the system
            // passes clicks through pixels nothing is drawn in).
            RoundedRectangle(cornerRadius: look.radius(Tokens.Radius.r2xl), style: .continuous)
                .fill(Color.black.opacity(chrome ? 0.45 : desktop.locked ? 0 : 0.01))
            DesktopLyricsLines(scale: NowPlayingMotion.lyricsScale(settings.current.lyricsSize))
                .padding(.horizontal, Tokens.Space.s8)
                .padding(.vertical, Tokens.Space.s3)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            if chrome {
                DesktopLyricsControls()
                    .padding(Tokens.Space.s2)
                    .transition(.opacity)
            }
        }
        .frame(width: DesktopLyricsWindow.size.width, height: DesktopLyricsWindow.size.height)
        .onChange(of: desktop.locked) { _, _ in
            // A locked window gets no pointer events, so it cannot hear the pointer leave.
            hovering = false
        }
        .animation(reduceMotion ? nil : .easeOut(duration: 0.2), value: chrome)
        .background {
            ZStack {
                WindowVisibilityReader(onChange: { visible = $0 })
                PointerHoverReader(onChange: { hovering = $0 })
            }
        }
        .environment(\.colorScheme, .dark)
        .environment(\.palette, theme.palette(dark: true))
        .environment(\.effects, effects)
        .environment(\.look, look)
    }

    private var effects: Effects {
        let current = settings.current
        let transparency = Double(current.transparency)
        return Effects(
            reduceMotion: reduceMotion,
            solidGlass: reduceTransparency || system.lowPower || current.reduceEffects || transparency <= 0.001,
            reduceEffects: current.reduceEffects,
            windowVisible: visible,
            transparency: transparency
        )
    }
}

/// The line being sung and the next one, centred.
private struct DesktopLyricsLines: View {
    /// The Lyrics Size setting's scale.
    var scale: CGFloat

    @Environment(PlaybackStore.self) private var playback
    @Environment(LyricsStore.self) private var lyrics
    @Environment(\.effects) private var effects
    @Environment(\.palette) private var palette
    @Environment(\.look) private var look

    private var mainSize: CGFloat { (30 * scale).rounded() }
    private var nextSize: CGFloat { (20 * scale).rounded() }

    var body: some View {
        Group {
            if let item = playback.playing {
                let uid = playback.nowPlaying?.uid
                if !item.isRemote, let document = lyrics.document(for: uid), !document.lines.isEmpty,
                   let timeline = lyrics.timeline(for: uid) {
                    TimelineView(LyricsSchedule(timeline: timeline, start: playback.scheduleStart(),
                                                active: effects.windowVisible)) { _ in
                        let bar = timeline.bar(atMs: lyricMilliseconds(playback.positionUs()))
                        VStack(spacing: Tokens.Space.s1) {
                            if let index = bar.line, index < document.lines.count {
                                LyricsBarLine(line: document.lines[index], index: index, bar: bar, timeline: timeline,
                                              fontSize: mainSize,
                                              weight: look.lyricsWeight(token: Tokens.Lyrics.line.weight),
                                              centred: true)
                            } else {
                                first(item.title)
                            }
                            if let next = bar.next, next < document.lines.count {
                                second(document.lines[next].text)
                            }
                        }
                    }
                } else {
                    VStack(spacing: Tokens.Space.s1) {
                        first(item.title)
                        if let artist = NowPlayingLines.secondLine(item) {
                            second(artist)
                        }
                    }
                }
            } else {
                first(String(localized: "Nothing Playing"))
            }
        }
        .shadow(color: .black.opacity(0.55), radius: 3, y: 1)
        .accessibilityElement(children: .combine)
    }

    private func first(_ text: String) -> some View {
        Text(text)
            .font(look.font(size: mainSize, weight: look.lyricsWeight(token: Tokens.Lyrics.line.weight)))
            .foregroundStyle(look.sungInk(palette))
            .lineLimit(1)
            .minimumScaleFactor(0.6)
            .frame(maxWidth: .infinity)
    }

    private func second(_ text: String) -> some View {
        Text(text)
            .font(look.font(size: nextSize, weight: .semibold))
            .foregroundStyle(palette.ink(Tokens.Lyrics.inkUnsung))
            .lineLimit(1)
            .minimumScaleFactor(0.7)
            .frame(maxWidth: .infinity)
    }
}

/// Lock and Close, while the pointer is over the unlocked window.
private struct DesktopLyricsControls: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(DesktopLyricsStore.self) private var desktop

    var body: some View {
        HStack(spacing: Tokens.Space.s1) {
            control("lock", label: "Lock Desktop Lyrics") {
                desktop.setLocked(true)
            }
            control("xmark", label: "Close Desktop Lyrics") {
                settings.setDesktopLyrics(false)
            }
        }
    }

    private func control(_ symbol: String, label: LocalizedStringKey, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 26, height: 26)
                .background(Circle().fill(Color.white.opacity(0.18)))
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(Text(label))
        .help(Text(label))
    }
}

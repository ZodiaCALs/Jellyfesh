// Lyrics (docs/lyrics-model.md, "Display behaviour"; docs/design-controls.md, "v0.0.4"): one size
// for every line, scaled by the Lyrics size setting and aligned to the start or the centre. The
// active line is ink-90 at full size; the others are ink-50 (sung ones ink-30) at 96 %, scaled from
// their start edge. In word-timed lyrics the sung words fill linearly over their duration. The
// view wakes only when a line or word changes (LyricsSchedule) and draws per frame only while a
// word is being sung. The active line scrolls to 38 % of the panel in 380 ms and the lines below
// follow 30 ms apart (the cascade); manual scrolling pauses that for 4 s. Under reduced motion
// there is no cascade and the view jumps; under the system's Reduce Motion there is no sweep.

import AppKit
import SwiftUI

struct LyricsPanel: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(LyricsStore.self) private var lyrics
    @Environment(SettingsStore.self) private var settings

    var body: some View {
        let uid = playback.nowPlaying?.uid
        let shown = settings.current
        let look = LyricsLook(scale: NowPlayingMotion.lyricsScale(shown.lyricsSize),
                              centred: shown.lyricsAlign == .center)
        if let document = lyrics.document(for: uid), !document.lines.isEmpty {
            if let timeline = lyrics.timeline(for: uid) {
                SyncedLyrics(document: document, timeline: timeline, translation: shown.showTranslation,
                             romanization: shown.showRomanization, look: look)
            } else {
                UnsyncedLyrics(document: document, translation: shown.showTranslation,
                               romanization: shown.showRomanization, look: look)
            }
        } else {
            EmptyState(title: "No Lyrics", symbol: "quote.bubble",
                       message: "Lyrics show here when the song has them: in its tags, or in an .lrc or .ttml file beside it.")
        }
    }
}

/// The Lyrics size and alignment settings.
private struct LyricsLook: Equatable {
    /// `lyrics.scale-small`, 1 or `lyrics.scale-large`: line, translation and romanization sizes.
    var scale: CGFloat
    /// Lines centred instead of at the start edge.
    var centred: Bool

    func size(_ role: Typography) -> CGFloat { (role.size * scale).rounded() }
}

/// Timed lyrics that follow playback.
private struct SyncedLyrics: View {
    var document: LyricsDocument
    var timeline: LyricsTimeline
    var translation: Bool
    var romanization: Bool
    var look: LyricsLook

    @Environment(PlaybackStore.self) private var playback
    @Environment(\.effects) private var effects
    @State private var userScrolledAt: Date?

    var body: some View {
        let start = playback.scheduleStart()
        TimelineView(LyricsSchedule(timeline: timeline, start: start, active: effects.windowVisible)) { context in
            let ms = lyricMilliseconds(playback.positionUs())
            let cursor = timeline.position(atMs: ms)
            let last = LyricsLayout.lastStarted(lineStarts: timeline.lineStarts, atMs: ms)
            LyricsScroller(
                document: document,
                timeline: timeline,
                cursor: cursor,
                scrollLine: LyricsLayout.scrollLine(active: cursor.line, lastStarted: last),
                translation: translation,
                romanization: romanization,
                look: look,
                following: following(at: context.date),
                onUserScroll: { userScrolledAt = Date() }
            )
        }
    }

    /// Follow mode, paused for 4 s after the person scrolls.
    private func following(at date: Date) -> Bool {
        guard let scrolled = userScrolledAt else { return true }
        return date.timeIntervalSince(scrolled) > 4
    }
}

/// The lines in a scroll view that keeps the current one at 38 % of its height.
private struct LyricsScroller: View {
    var document: LyricsDocument
    var timeline: LyricsTimeline
    var cursor: LyricsPosition
    var scrollLine: Int?
    var translation: Bool
    var romanization: Bool
    var look: LyricsLook
    var following: Bool
    var onUserScroll: () -> Void

    @Environment(PlaybackStore.self) private var playback
    @Environment(\.effects) private var effects
    @State private var width: CGFloat = 600
    @State private var height: CGFloat = 600
    @State private var cascade = LyricsCascade()

    var body: some View {
        let size = (LyricsLayout.lineSize(forWidth: width) * look.scale).rounded()
        let anchor = Tokens.Lyrics.anchorPosition
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: look.centred ? .center : .leading, spacing: Tokens.Lyrics.lineGap) {
                    ForEach(Array(document.lines.enumerated()), id: \.offset) { index, line in
                        let role = NowPlayingMotion.role(index: index, active: cursor.line)
                        LyricLineView(
                            line: line,
                            aux: LyricsLayout.auxLines(in: document, for: line, translation: translation,
                                                       romanization: romanization),
                            role: role,
                            sweep: sweep(for: index, line: line),
                            fontSize: size,
                            look: look,
                            language: document.language,
                            timeline: timeline
                        )
                        .equatable()
                        .scaleEffect(NowPlayingMotion.lineScale(role), anchor: scaleAnchor(for: line))
                        .animation(effects.animates ? Motion.lyricScroll : nil, value: role)
                        .modifier(CascadeOffset(
                            cascade: cascade,
                            delayMs: effects.animates
                                ? NowPlayingMotion.cascadeDelayMs(index: index, active: cascade.active) : 0
                        ))
                        .id(index)
                        .onTapGesture { seek(to: line) }
                        .accessibilityAction { seek(to: line) }
                    }
                }
                .padding(.top, height * anchor)
                .padding(.bottom, height * (1 - anchor))
                .frame(maxWidth: .infinity, alignment: look.centred ? .center : .leading)
            }
            .scrollIndicators(.never)
            .mask {
                // Lines fade towards the panel's edges instead of being cut.
                LinearGradient(stops: [
                    .init(color: .clear, location: 0),
                    .init(color: .black, location: 0.12),
                    .init(color: .black, location: 0.88),
                    .init(color: .clear, location: 1),
                ], startPoint: .top, endPoint: .bottom)
            }
            .modifier(UserScrollDetector(onScroll: onUserScroll))
            .onChange(of: scrollLine, initial: true) { previous, line in
                guard following, let line else { return }
                if effects.animates {
                    if let previous, line > previous, line - previous <= 2 {
                        // The lines below follow the scroll one after another.
                        cascade = cascade.next(active: line, distance: advance(from: previous, to: line, size: size))
                    }
                    withAnimation(Motion.lyricScroll) {
                        proxy.scrollTo(line, anchor: UnitPoint(x: 0, y: anchor))
                    }
                } else {
                    proxy.scrollTo(line, anchor: UnitPoint(x: 0, y: anchor))
                }
            }
            .onChange(of: following) { _, now in
                guard now, let line = scrollLine else { return }
                withAnimation(Motion.unless(!effects.animates, Motion.lyricScroll)) {
                    proxy.scrollTo(line, anchor: UnitPoint(x: 0, y: anchor))
                }
            }
        }
        .background {
            GeometryReader { geometry in
                Color.clear
                    .onAppear {
                        width = geometry.size.width
                        height = geometry.size.height
                    }
                    .onChange(of: geometry.size) { _, size in
                        width = size.width
                        height = size.height
                    }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Lyrics"))
    }

    /// Tapping a timed line plays from it.
    private func seek(to line: LyricsLine) {
        if let start = line.startMs {
            playback.seek(toUs: Int64(start) * 1_000)
        }
    }

    /// Inactive lines shrink towards their start edge, or their centre when centred.
    private func scaleAnchor(for line: LyricsLine) -> UnitPoint {
        if look.centred { return .center }
        return line.direction == .rightToLeft ? .trailing : .leading
    }

    /// About how far the lines scroll when the active line moves from `from` to `to`: the lines
    /// between, one row each with their translation and romanization, and the gaps.
    private func advance(from: Int, to: Int, size: CGFloat) -> CGFloat {
        var distance: CGFloat = 0
        for index in from..<min(to, document.lines.count) {
            let line = document.lines[index]
            distance += size * CGFloat(LyricsLayout.lineHeight(for: line.text, language: document.language))
            for extra in LyricsLayout.auxLines(in: document, for: line, translation: translation,
                                               romanization: romanization) {
                let role = extra.kind == .translation ? Tokens.Lyrics.translation : Tokens.Lyrics.romanization
                distance += look.size(role) * CGFloat(role.lineHeight) + Tokens.Space.s1
            }
            distance += Tokens.Lyrics.lineGap
        }
        return distance
    }

    /// The sweep of the active word-timed line, when it runs.
    private func sweep(for index: Int, line: LyricsLine) -> LyricLineView.Sweep? {
        guard cursor.line == index, !line.words.isEmpty, line.direction != .rightToLeft else { return nil }
        // Under Reduce Motion the whole active line lights at once.
        guard effects.sweepsLyrics else { return nil }
        let wordInProgress = cursor.word != nil && cursor.wordProgress < 1
        return LyricLineView.Sweep(
            word: cursor.word,
            progress: cursor.wordProgress,
            live: wordInProgress && playback.isPlaying && !playback.buffering && effects.windowVisible
        )
    }
}

/// A line change's scroll, which the lines below the new active line follow one after another.
private struct LyricsCascade: Equatable {
    var serial = 0
    var active: Int?
    var distance: CGFloat = 0

    func next(active: Int, distance: CGFloat) -> LyricsCascade {
        LyricsCascade(serial: serial + 1, active: active, distance: distance)
    }
}

/// A line's part in the cascade: it starts `delayMs` after the scroll by lagging behind it and
/// catching up (`NowPlayingMotion.cascadeKeyframes`), on SwiftUI's keyframe animator. Lines with no
/// delay keep a fixed trigger, so nothing runs for them.
private struct CascadeOffset: ViewModifier {
    var cascade: LyricsCascade
    var delayMs: Int

    func body(content: Content) -> some View {
        let frames = NowPlayingMotion.cascadeKeyframes(distance: cascade.distance, delayMs: delayMs,
                                                       durationMs: Tokens.Motion.dur3Ms,
                                                       curve: Tokens.Motion.easeFluid)
        let k = frames.count == 5 ? frames : Array(repeating: (value: CGFloat(0), seconds: 0.001), count: 5)
        return content.keyframeAnimator(initialValue: CGFloat(0), trigger: frames.isEmpty ? -1 : cascade.serial) { view, y in
            view.offset(y: y)
        } keyframes: { _ in
            KeyframeTrack(\.self) {
                LinearKeyframe(k[0].value, duration: k[0].seconds)
                LinearKeyframe(k[1].value, duration: k[1].seconds)
                LinearKeyframe(k[2].value, duration: k[2].seconds)
                LinearKeyframe(k[3].value, duration: k[3].seconds)
                LinearKeyframe(k[4].value, duration: k[4].seconds)
            }
        }
    }
}

/// One lyric line with its translation and romanization lines.
private struct LyricLineView: View, Equatable {
    /// The word cursor of the active line: where the sweep is, and whether it moves now.
    struct Sweep: Equatable {
        var word: Int?
        var progress: Float
        /// A word is being sung: redraw every frame until it ends.
        var live: Bool
    }

    var line: LyricsLine
    var aux: [LyricsLayout.AuxLine]
    var role: LyricLineRole
    var sweep: Sweep?
    var fontSize: CGFloat
    var look: LyricsLook
    var language: String?
    var timeline: LyricsTimeline

    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    static func == (a: LyricLineView, b: LyricLineView) -> Bool {
        a.line == b.line && a.aux == b.aux && a.role == b.role && a.sweep == b.sweep
            && a.fontSize == b.fontSize && a.look == b.look && a.language == b.language
    }

    private var isActive: Bool { role == .active }

    /// Start, end or centre, for the stack, the frame and the text.
    private var alignment: HorizontalAlignment {
        if look.centred { return .center }
        return line.direction == .rightToLeft ? .trailing : .leading
    }

    private var textAlignment: TextAlignment {
        if look.centred { return .center }
        return line.direction == .rightToLeft ? .trailing : .leading
    }

    /// The main line's ink: ink-90 active, ink-50 to come, ink-30 sung.
    private var lineInk: Color {
        switch role {
        case .active: return palette.ink(Tokens.Lyrics.inkActive)
        case .upcoming: return palette.ink(Tokens.Lyrics.inkInactive)
        case .sung: return palette[Tokens.Ink.ink30]
        }
    }

    /// Translation, romanization and background vocals: ink-70 under the active line.
    private var auxInk: Color {
        switch role {
        case .active: return palette.ink(Tokens.Lyrics.inkUnsung)
        case .upcoming: return palette.ink(Tokens.Lyrics.inkInactive)
        case .sung: return palette[Tokens.Ink.ink30]
        }
    }

    var body: some View {
        let lineHeight = LyricsLayout.lineHeight(for: line.text, language: language)
        VStack(alignment: alignment, spacing: Tokens.Space.s1) {
            mainLine(lineHeight: lineHeight)
            if !line.background.isEmpty {
                Text(Self.joined(line.background))
                    .font(.system(size: look.size(Tokens.Lyrics.translation),
                                  weight: Font.Weight(cssWeight: Tokens.Lyrics.translation.weight)))
                    .foregroundStyle(auxInk)
                    .multilineTextAlignment(textAlignment)
            }
            ForEach(aux, id: \.self) { extra in
                let role = extra.kind == .translation ? Tokens.Lyrics.translation : Tokens.Lyrics.romanization
                Text(extra.text)
                    .typography(role, size: look.size(role))
                    .foregroundStyle(auxInk)
                    .multilineTextAlignment(textAlignment)
            }
        }
        .frame(maxWidth: .infinity, alignment: Alignment(horizontal: alignment, vertical: .center))
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isActive ? .isSelected : [])
    }

    @ViewBuilder private func mainLine(lineHeight: Double) -> some View {
        let type = Typography(size: fontSize, lineHeight: lineHeight, weight: Tokens.Lyrics.line.weight)
        if let sweep, !line.words.isEmpty {
            TimelineView(.animation(minimumInterval: nil, paused: !sweep.live)) { _ in
                SweptWords(words: line.words, current: liveCursor(sweep), fontSize: fontSize,
                           lineHeight: lineHeight, centred: look.centred)
            }
        } else {
            Text(line.text)
                .typography(type)
                .foregroundStyle(lineInk)
                .multilineTextAlignment(textAlignment)
                .environment(\.layoutDirection, line.direction == .rightToLeft ? .rightToLeft : .leftToRight)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// The word cursor now: per frame while a word is sung, else the one the line was drawn with.
    private func liveCursor(_ sweep: Sweep) -> (word: Int?, progress: Float) {
        guard sweep.live else { return (sweep.word, sweep.progress) }
        let cursor = timeline.position(atMs: lyricMilliseconds(playback.positionUs()))
        return (cursor.word, cursor.wordProgress)
    }

    private static func joined(_ words: [LyricsWord]) -> String {
        var out = ""
        for word in words {
            out += word.text.trimmingCharacters(in: .whitespaces)
            if !word.joinsNext { out += " " }
        }
        return out.trimmingCharacters(in: .whitespaces)
    }
}

/// The words of the active line: sung words in ink-90, the current word filling linearly, the
/// rest in ink-70 (`lyrics.ink-sung`, `ink-unsung`).
private struct SweptWords: View {
    var words: [LyricsWord]
    var current: (word: Int?, progress: Float)
    var fontSize: CGFloat
    var lineHeight: Double
    var centred: Bool

    @Environment(\.palette) private var palette

    var body: some View {
        let sung = palette.ink(Tokens.Lyrics.inkSung)
        let unsung = palette.ink(Tokens.Lyrics.inkUnsung)
        WordFlow(spaces: words.map { !$0.joinsNext }, spaceWidth: fontSize * 0.28,
                 lineSpacing: fontSize * CGFloat(max(lineHeight - 1.2, 0)), centred: centred) {
            ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                SweepWord(text: word.text.trimmingCharacters(in: .whitespaces),
                          fill: LyricsLayout.fill(wordIndex: index, current: current.word, progress: current.progress),
                          sung: sung, unsung: unsung)
            }
        }
        .font(.system(size: fontSize, weight: Font.Weight(cssWeight: Tokens.Lyrics.line.weight)))
    }
}

/// One word, filled from the leading edge by `fill`.
private struct SweepWord: View {
    var text: String
    var fill: Double
    var sung: Color
    var unsung: Color

    var body: some View {
        if fill >= 1 {
            Text(text).foregroundStyle(sung)
        } else if fill <= 0 {
            Text(text).foregroundStyle(unsung)
        } else {
            Text(text)
                .foregroundStyle(unsung)
                .overlay(alignment: .leading) {
                    Text(text)
                        .foregroundStyle(sung)
                        .mask(alignment: .leading) {
                            GeometryReader { geometry in
                                Rectangle().frame(width: geometry.size.width * fill)
                            }
                        }
                }
        }
    }
}

/// Words left to right, wrapping at the offered width; a space after each word unless it joins
/// the next (CJK, syllables). Rows start at the leading edge, or are centred.
private struct WordFlow: Layout {
    var spaces: [Bool]
    var spaceWidth: CGFloat
    var lineSpacing: CGFloat
    var centred = false

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = arrange(subviews: subviews, width: proposal.width ?? .infinity)
        let width = rows.map(\.width).max() ?? 0
        let height = rows.reduce(0) { $0 + $1.height } + lineSpacing * CGFloat(max(rows.count - 1, 0))
        return CGSize(width: proposal.width ?? width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let rows = arrange(subviews: subviews, width: bounds.width)
        var y = bounds.minY
        for row in rows {
            let start = centred ? max(0, (bounds.width - row.width) / 2) : 0
            for item in row.items {
                subviews[item.index].place(at: CGPoint(x: bounds.minX + start + item.x, y: y),
                                           anchor: .topLeading, proposal: .unspecified)
            }
            y += row.height + lineSpacing
        }
    }

    private struct Row {
        var items: [(index: Int, x: CGFloat)] = []
        var width: CGFloat = 0
        var height: CGFloat = 0
    }

    private func arrange(subviews: Subviews, width: CGFloat) -> [Row] {
        var rows: [Row] = []
        var row = Row()
        var x: CGFloat = 0
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            if x > 0, x + size.width > width {
                rows.append(row)
                row = Row()
                x = 0
            }
            row.items.append((index, x))
            row.width = x + size.width
            row.height = max(row.height, size.height)
            x += size.width
            if index < spaces.count, spaces[index] { x += spaceWidth }
        }
        if !row.items.isEmpty { rows.append(row) }
        return rows
    }
}

/// Lyrics without timing: static, scrollable text.
private struct UnsyncedLyrics: View {
    var document: LyricsDocument
    var translation: Bool
    var romanization: Bool
    var look: LyricsLook

    @Environment(\.palette) private var palette

    var body: some View {
        let alignment: HorizontalAlignment = look.centred ? .center : .leading
        ScrollView {
            VStack(alignment: alignment, spacing: Tokens.Space.s4) {
                ForEach(Array(document.lines.enumerated()), id: \.offset) { _, line in
                    VStack(alignment: alignment, spacing: Tokens.Space.s1) {
                        Text(line.text)
                            .font(.system(size: (22 * look.scale).rounded(), weight: .semibold))
                            .foregroundStyle(palette[Tokens.Ink.ink90])
                        ForEach(LyricsLayout.auxLines(in: document, for: line, translation: translation,
                                                      romanization: romanization), id: \.self) { extra in
                            let role = extra.kind == .translation ? Tokens.Lyrics.translation : Tokens.Lyrics.romanization
                            Text(extra.text)
                                .typography(role, size: look.size(role))
                                .foregroundStyle(palette[Tokens.Ink.ink70])
                        }
                    }
                    .multilineTextAlignment(look.centred ? .center : .leading)
                    .frame(maxWidth: .infinity, alignment: Alignment(horizontal: alignment, vertical: .center))
                }
            }
            .padding(.vertical, Tokens.Space.s6)
            .textSelection(.enabled)
        }
        .scrollIndicators(.automatic)
        .accessibilityLabel(Text("Lyrics"))
    }
}

/// Pauses lyric follow mode when the person scrolls the panel with a wheel or trackpad.
private struct UserScrollDetector: ViewModifier {
    var onScroll: () -> Void
    @State private var monitor: Any?

    func body(content: Content) -> some View {
        content
            .onHover { inside in
                if inside { install() } else { remove() }
            }
            .onDisappear { remove() }
    }

    private func install() {
        guard monitor == nil else { return }
        let onScroll = self.onScroll
        monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { event in
            // Momentum after the fingers lift is not a new scroll.
            if event.momentumPhase.isEmpty {
                onScroll()
            }
            return event
        }
    }

    private func remove() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
}

// The lyrics bar (ADR-0022 §6, docs/ui.md "Lyrics, word by word"): with Show Lyrics Bar on and
// timed lyrics, the player bar shows the line being sung under the title, filling word by word in
// the highlight colour (and its translation when Show Translations is on); Desktop Lyrics shows
// the same line and the next one. What shows when comes from the core's timing helper
// (`LyricsTimeline.bar`, a port replayed against tests/contract/lyrics-bar.json). The view wakes
// when a line or word changes and draws every frame only while a word is being sung.

import SwiftUI

/// The lines under the title in the player bar: the lyric line being sung (and its translation)
/// while the lyrics bar shows, else the artist.
struct PlayerBarLines: View {
    var item: PlayingItem

    @Environment(PlaybackStore.self) private var playback
    @Environment(LyricsStore.self) private var lyrics
    @Environment(SettingsStore.self) private var settings
    @Environment(\.effects) private var effects
    @Environment(\.palette) private var palette

    var body: some View {
        let uid = playback.nowPlaying?.uid
        if settings.current.showLyricsBar, !item.isRemote, let document = lyrics.document(for: uid),
           let timeline = lyrics.timeline(for: uid) {
            TimelineView(LyricsSchedule(timeline: timeline, start: playback.scheduleStart(),
                                        active: effects.windowVisible)) { _ in
                let bar = timeline.bar(atMs: lyricMilliseconds(playback.positionUs()))
                if let index = bar.line, index < document.lines.count {
                    let line = document.lines[index]
                    VStack(alignment: .leading, spacing: 1) {
                        Text(item.title)
                            .lookFont(size: 14, weight: .semibold)
                            .foregroundStyle(palette[Tokens.Ink.ink90])
                            .lineLimit(1)
                        LyricsBarLine(line: line, index: index, bar: bar, timeline: timeline, fontSize: 13,
                                      weight: .semibold)
                        if settings.current.showTranslation,
                           let translation = LyricsBarText.translation(in: document, line: line) {
                            Text(translation)
                                .lookFont(size: 11)
                                .foregroundStyle(palette[Tokens.Ink.ink70])
                                .lineLimit(1)
                        }
                    }
                } else {
                    // Before the first line.
                    NowPlayingLines(item: item)
                }
            }
        } else {
            NowPlayingLines(item: item)
        }
    }
}

/// One lyric line in one row, its words filling from the start edge as they are sung (the
/// highlight colour over the unsung ink). A line without word timing, a right-to-left line, and
/// any line under Reduce Motion show whole in the sung colour. A line too long for the row fades
/// out at its end; a centred one that fits stays centred.
struct LyricsBarLine: View {
    var line: LyricsLine
    /// Its place in the document, to match the bar computed per frame.
    var index: Int
    var bar: LyricsBarState
    var timeline: LyricsTimeline
    var fontSize: CGFloat
    var weight: Font.Weight
    var centred = false

    @Environment(PlaybackStore.self) private var playback
    @Environment(\.effects) private var effects
    @Environment(\.palette) private var palette
    @Environment(\.look) private var look

    var body: some View {
        let sung = look.sungInk(palette)
        Group {
            if line.words.isEmpty || line.direction == .rightToLeft || !effects.sweepsLyrics {
                fitted(Text(line.text)
                    .foregroundStyle(sung)
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false))
            } else {
                let unsung = palette.ink(Tokens.Lyrics.inkUnsung)
                let live = LyricsBarText.isSweeping(bar) && playback.isPlaying && !playback.buffering
                    && effects.windowVisible
                TimelineView(.animation(minimumInterval: nil, paused: !live)) { _ in
                    fitted(FilledWords(words: line.words, fills: live ? liveFills() : bar.words, sung: sung,
                                       unsung: unsung))
                }
            }
        }
        .font(look.font(size: fontSize, weight: weight))
        .lyricsGlow(look, palette: palette, size: look.size(fontSize))
    }

    /// The words' progress now, every frame while one is sung.
    private func liveFills() -> [Float] {
        let now = timeline.bar(atMs: lyricMilliseconds(playback.positionUs()))
        return now.line == index ? now.words : bar.words
    }

    /// Centred while the line fits, else from its start edge with the end fading.
    @ViewBuilder private func fitted<Content: View>(_ content: Content) -> some View {
        if centred {
            ViewThatFits(in: .horizontal) {
                content.frame(maxWidth: .infinity)
                faded(content)
            }
        } else {
            faded(content)
        }
    }

    private func faded<Content: View>(_ content: Content) -> some View {
        let rtl = line.direction == .rightToLeft
        return content
            .frame(maxWidth: .infinity, alignment: rtl ? .trailing : .leading)
            .mask { LyricsBarFade(fromTrailing: rtl) }
    }
}

/// The words of a line side by side, each filled by its progress; a space after each word unless
/// it joins the next (CJK, syllables). The line keeps its whole width: the row cuts and fades it.
struct FilledWords: View {
    var words: [LyricsWord]
    var fills: [Float]
    var sung: Color
    var unsung: Color

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                FilledWord(text: LyricsBarText.wordText(word),
                           fill: index < fills.count ? Double(fills[index]) : 0,
                           sung: sung, unsung: unsung)
            }
        }
        .lineLimit(1)
        .fixedSize(horizontal: true, vertical: false)
    }
}

/// One word, filled from the leading edge.
private struct FilledWord: View {
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

/// The end of a line that does not fit fades instead of being cut: the trailing edge, or the
/// leading one for a right-to-left line.
private struct LyricsBarFade: View {
    var fromTrailing: Bool

    var body: some View {
        LinearGradient(stops: [.init(color: .black, location: 0), .init(color: .black, location: 0.9),
                               .init(color: .clear, location: 1)],
                       startPoint: fromTrailing ? .trailing : .leading,
                       endPoint: fromTrailing ? .leading : .trailing)
    }
}

/// What the lyrics bar draws, as rules apart from the views.
enum LyricsBarText {
    /// A word as drawn: its text and the space after it, unless it joins the next.
    static func wordText(_ word: LyricsWord) -> String {
        let text = word.text.trimmingCharacters(in: .whitespaces)
        return word.joinsNext ? text : text + " "
    }

    /// A word is part sung: the bar moves every frame.
    static func isSweeping(_ bar: LyricsBarState) -> Bool {
        bar.words.contains { $0 > 0 && $0 < 1 }
    }

    /// The line's translation, when the lyrics carry one (the one Now Playing shows).
    static func translation(in document: LyricsDocument, line: LyricsLine) -> String? {
        LyricsLayout.auxLines(in: document, for: line, translation: true, romanization: false).first?.text
    }
}

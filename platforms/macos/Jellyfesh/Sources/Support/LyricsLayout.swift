// What the lyrics view shows and where it scrolls (docs/lyrics-model.md, "Display behaviour"),
// kept apart from the view so tests can check it. The same decisions as the Linux app's
// logic/lyrics.rs.

import CoreGraphics
import Foundation

enum LyricsLayout {
    /// A line under a lyric line: romanization first (it reads the original aloud), then the
    /// translation, each only when the Lyrics settings show it and the text is not blank.
    struct AuxLine: Hashable {
        var kind: LyricsAuxKind
        var text: String
        var language: String?
    }

    static func auxLines(in document: LyricsDocument, for line: LyricsLine, translation: Bool,
                         romanization: Bool) -> [AuxLine] {
        var out: [AuxLine] = []
        let wanted: [(LyricsAuxKind, Bool)] = [(.romanization, romanization), (.translation, translation)]
        for (kind, shown) in wanted where shown {
            guard let track = document.tracks.first(where: { $0.kind == kind }),
                  let text = track.lines[line.id]?.trimmingCharacters(in: .whitespacesAndNewlines),
                  !text.isEmpty
            else { continue }
            out.append(AuxLine(kind: kind, text: text, language: track.language))
        }
        return out
    }

    /// The last line that started at or before `tMs`. A gap between lines keeps it, so the view
    /// stays where it was.
    static func lastStarted(lineStarts: [UInt32], atMs tMs: UInt32) -> Int? {
        var low = 0
        var high = lineStarts.count
        while low < high {
            let mid = low + (high - low) / 2
            if lineStarts[mid] <= tMs {
                low = mid + 1
            } else {
                high = mid
            }
        }
        return low > 0 ? low - 1 : nil
    }

    /// The line the view keeps at the anchor: the active one, else the last one that started.
    static func scrollLine(active: Int?, lastStarted: Int?) -> Int? {
        active ?? lastStarted
    }

    /// The lyric line size for a lyrics column `width` points wide (`lyrics.line`): Luminous's 40
    /// once the column is 640 pt wide, fluid down to 28 in narrow windows, in whole points so text
    /// does not reflow on every resize step.
    static func lineSize(forWidth width: CGFloat) -> CGFloat {
        let fluid = Tokens.Lyrics.lineFluid
        guard width.isFinite else { return fluid.max }
        return min(max(width / 16, fluid.min), fluid.max).rounded()
    }

    /// The line height multiple for a line: generous for Han, Kana and Hangul (Misty TYP-08).
    static func lineHeight(for text: String, language: String?) -> Double {
        isCJK(text: text, language: language) ? Tokens.Lyrics.lineCjkLineHeight : Tokens.Lyrics.line.lineHeight
    }

    /// True for Chinese, Japanese and Korean text, by its language tag or its characters.
    static func isCJK(text: String, language: String?) -> Bool {
        if let language {
            let primary = language.split(separator: "-").first.map { $0.lowercased() } ?? ""
            if ["zh", "ja", "ko", "yue"].contains(primary) { return true }
        }
        return text.unicodeScalars.contains { scalar in
            switch scalar.value {
            case 0x3040...0x30FF, // Hiragana, Katakana
                 0x3400...0x4DBF, // CJK Extension A
                 0x4E00...0x9FFF, // CJK Unified Ideographs
                 0xAC00...0xD7AF, // Hangul syllables
                 0xF900...0xFAFF: // CJK Compatibility Ideographs
                return true
            default:
                return false
            }
        }
    }

    /// How much of `word` is sung at `progress` (0 to 1) for the sweep: words already sung are
    /// full, words not reached are empty.
    static func fill(wordIndex: Int, current: Int?, progress: Float) -> Double {
        guard let current else { return 0 }
        if wordIndex < current { return 1 }
        if wordIndex > current { return 0 }
        return Double(min(max(progress, 0), 1))
    }
}

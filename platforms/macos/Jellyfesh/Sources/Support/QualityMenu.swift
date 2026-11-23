// The quality badge and its menu (ADR-0026 §3, docs/ui.md "The player bar and the mini player"):
// the words the app puts on what the core decides. The badge shows the playing quality's label
// (the core's `QualityLabel`, as in Get Info), else the chosen level's name. The menu shows the
// format line ("FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s"), then the levels the core offers for
// what plays with the chosen one checked (Auto first), or why there are none: "Files play as they
// are." or "Quality is set in {Service}.". The rules are pure, for tests.

import Foundation

/// Streaming Quality (ADR-0026 §1): the level asked of sources. Auto is Lossless, or High on a
/// metered connection (the core decides).
enum StreamingQualityOption: Hashable, Sendable, CaseIterable {
    case auto, lossless, high, normal, low

    var title: String {
        switch self {
        case .auto: return String(localized: "Auto")
        case .lossless: return String(localized: "Lossless")
        case .high: return String(localized: "High")
        case .normal: return String(localized: "Normal")
        case .low: return String(localized: "Low")
        }
    }
}

/// What the badge and its menu show.
struct QualityMenuContent: Equatable {
    /// The badge's words.
    var badge: String
    /// "FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s"; nil before the engine has opened the song.
    var formatLine: String?
    /// The levels to choose from, Auto first; empty when the source offers none.
    var levels: [StreamingQualityOption]
    var chosen: StreamingQualityOption
    /// Why there are no levels.
    var note: String?

    /// `offered`: the levels the core offers for what plays (`NowPlaying.qualities`); `service`:
    /// the source that sets the quality itself (a remote source), if any.
    init(playing format: AudioQuality?, offered: [StreamingQualityOption], chosen: StreamingQualityOption,
         service: String?) {
        badge = format?.label.map(InfoWords.quality) ?? chosen.title
        formatLine = format.flatMap(Self.formatLine)
        let levels = offered.filter { $0 != .auto }
        self.levels = levels.isEmpty ? [] : [.auto] + levels
        self.chosen = chosen
        if !levels.isEmpty {
            note = nil
        } else if let service, !service.isEmpty {
            note = String(localized: "Quality is set in \(service).")
        } else {
            note = String(localized: "Files play as they are.")
        }
    }

    /// The format line: the codec, then the bit depth and the sample rate for lossless audio, the
    /// bitrate for the rest.
    static func formatLine(_ format: AudioQuality) -> String? {
        var parts: [String] = []
        if let codec = format.codec?.trimmingCharacters(in: .whitespaces), !codec.isEmpty {
            parts.append(DetailsRows.codecName(codec))
        }
        let lossless = format.lossless ?? (format.bitDepth != nil)
        if lossless {
            if let bits = format.bitDepth, bits > 0 {
                parts.append(String(localized: "\(Int(bits))-bit"))
            }
            if let hertz = format.sampleRateHz, hertz > 0 {
                let kilohertz = (Double(hertz) / 1000).formatted(.number.precision(.fractionLength(0...1)))
                parts.append(String(localized: "\(kilohertz) kHz"))
            }
        } else if let rate = format.bitrateKbps, rate > 0 {
            parts.append(String(localized: "\(Int(rate)) kbit/s"))
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }
}

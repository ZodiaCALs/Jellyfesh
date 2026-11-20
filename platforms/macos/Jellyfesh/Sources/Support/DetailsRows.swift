// What Get Info and Now Playing's Info pane list (docs/ui.md "Details", ADR-0022): for a song,
// About This Song (Title, Artist, Album, Version, Year, Genre, Composer, Lyricist, Label, Length)
// and Audio (Quality, Format, Bitrate, Size, ReplayGain, File or Source); for an album or an
// artist, the facts the core gives. Rows without a value are left out. What the row or the player
// already knows shows until the core's details come; the Info pane prefers the format actually
// being decoded. The rules are here; the views word the rows (`InfoWords`).

import Foundation

/// One row of Get Info or the Info pane.
struct InfoLine: Identifiable, Equatable {
    enum Key: Hashable {
        case title, artist, album, version, year, genre, composer, lyricist, label, length
        case quality, format, bitrate, size, replayGain, file, source
        /// Now Playing's Info pane: where the lyrics come from and how they are timed.
        case lyrics
        /// Another fact of an album or an artist.
        case fact(DetailFactKey)
    }

    var key: Key
    var value: String

    var id: Key { key }
}

extension AudioQuality {
    /// The engine's facts about the stream, while the decoded format is not known yet.
    init(stream: StreamFormat) {
        self.init(codec: stream.codec, sampleRateHz: stream.sampleRate, bitDepth: stream.bitsPerSample,
                  channels: UInt32(stream.channels), bitrateKbps: stream.bitrate.map { $0 / 1000 }, lossless: nil,
                  label: nil)
    }
}

extension ItemReference {
    /// What Details names a song by: a library song, a file, or a source's song.
    init(track: TrackReference) {
        switch track {
        case .library(let id): self = .track(id: id)
        case .file(let path): self = .file(path: path)
        case .source(let song): self = .source(plugin: song.plugin, kind: .track, id: song.id)
        }
    }
}

/// What a row or the player knows about an item before the core answers.
struct KnownDetails: Equatable {
    var title: String?
    var artist: String?
    var album: String?
    var year: String?
    var durationMs: UInt64?
    /// A file's place, for File.
    var path: String?
    /// The source's name, for a source's item (Source).
    var source: String?
}

enum DetailsRows {
    /// About This Song.
    static func song(_ answer: DetailsAnswer?, known: KnownDetails) -> [InfoLine] {
        let facts = answer?.facts ?? []
        func fact(_ key: DetailFactKey) -> String? { facts.first { $0.key == key }?.value }
        let length = (answer?.technical?.durationMs ?? known.durationMs).flatMap { $0 > 0 ? $0 : nil }
        return lines([
            (.title, answer?.title ?? known.title),
            (.artist, answer?.subtitle ?? known.artist),
            (.album, known.album),
            (.version, fact(.version)),
            (.year, fact(.released) ?? known.year),
            (.genre, fact(.genre)),
            (.composer, fact(.composer)),
            (.lyricist, fact(.lyricist)),
            (.label, fact(.label)),
            (.length, length.map { TimeText.from(milliseconds: $0) }),
        ])
    }

    /// Audio. `decoded` is what plays now (Now Playing's), preferred over the format the details
    /// read from the file or the stream.
    static func audio(_ answer: DetailsAnswer?, known: KnownDetails, decoded: AudioQuality? = nil) -> [InfoLine] {
        let technical = answer?.technical
        let format = decoded ?? technical?.format
        let place: (InfoLine.Key, String?) = known.source != nil && technical?.path == nil && known.path == nil
            ? (.source, known.source)
            : (.file, technical?.path ?? known.path)
        return lines([
            (.quality, format?.label.map(InfoWords.quality)),
            (.format, format.flatMap(formatLine)),
            (.bitrate, format?.bitrateKbps.flatMap { $0 > 0 ? String(localized: "\(Int($0)) kbps") : nil }),
            (.size, technical?.fileSizeBytes.flatMap { $0 > 0 ? ByteCountFormatter.string(fromByteCount: Int64(clamping: $0), countStyle: .file) : nil }),
            (.replayGain, replayGainLine(technical?.replayGain)),
            place,
        ])
    }

    /// An album's or an artist's facts, in the core's order.
    static func facts(_ answer: DetailsAnswer?) -> [InfoLine] {
        lines((answer?.facts ?? []).map { fact -> (InfoLine.Key, String?) in (.fact(fact.key), fact.value) })
    }

    /// "FLAC · 24-bit · 96 kHz · Stereo".
    static func formatLine(_ format: AudioQuality) -> String? {
        var parts: [String] = []
        if let codec = format.codec?.trimmingCharacters(in: .whitespaces), !codec.isEmpty {
            parts.append(codecName(codec))
        }
        if let bits = format.bitDepth, bits > 0 {
            parts.append(String(localized: "\(Int(bits))-bit"))
        }
        if let hertz = format.sampleRateHz, hertz > 0 {
            let kilohertz = (Double(hertz) / 1000).formatted(.number.precision(.fractionLength(0...1)))
            parts.append(String(localized: "\(kilohertz) kHz"))
        }
        if let channels = format.channels, channels > 0 {
            switch channels {
            case 1: parts.append(String(localized: "Mono"))
            case 2: parts.append(String(localized: "Stereo"))
            default: parts.append(String(localized: "\(Int(channels)) channels"))
            }
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    /// "Track −6.20 dB · Album −5.80 dB".
    static func replayGainLine(_ gain: ReplayGainInfo?) -> String? {
        guard let gain else { return nil }
        var parts: [String] = []
        if let track = gain.trackGainDb, track.isFinite {
            parts.append(String(localized: "Track \(decibels(track))"))
        }
        if let album = gain.albumGainDb, album.isFinite {
            parts.append(String(localized: "Album \(decibels(album))"))
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    /// "FLAC", "MP3", "AAC", "Opus": short codec names in capitals.
    static func codecName(_ codec: String) -> String {
        switch codec.lowercased() {
        case "opus": return "Opus"
        case "vorbis": return "Vorbis"
        default: return codec.count <= 4 ? codec.uppercased() : codec
        }
    }

    private static func decibels(_ value: Float) -> String {
        let sign = value > 0 ? "+" : value < 0 ? "−" : ""
        return sign + abs(Double(value)).formatted(.number.precision(.fractionLength(2))) + " dB"
    }

    private static func lines(_ pairs: [(InfoLine.Key, String?)]) -> [InfoLine] {
        pairs.compactMap { key, value in
            guard let text = value?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else { return nil }
            return InfoLine(key: key, value: text)
        }
    }
}

/// The words of Get Info's rows.
enum InfoWords {
    static func label(_ key: InfoLine.Key) -> String {
        switch key {
        case .title: return String(localized: "Title")
        case .artist: return String(localized: "Artist")
        case .album: return String(localized: "Album")
        case .version: return String(localized: "Version")
        case .year: return String(localized: "Year")
        case .genre: return String(localized: "Genre")
        case .composer: return String(localized: "Composer")
        case .lyricist: return String(localized: "Lyricist")
        case .label: return String(localized: "Label")
        case .length: return String(localized: "Length")
        case .quality: return String(localized: "Quality")
        case .format: return String(localized: "Format")
        case .bitrate: return String(localized: "Bitrate")
        case .size: return String(localized: "Size")
        case .replayGain: return String(localized: "ReplayGain")
        case .file: return String(localized: "File")
        case .source: return String(localized: "Source")
        case .lyrics: return String(localized: "Lyrics")
        case .fact(let fact): return label(fact)
        }
    }

    static func label(_ fact: DetailFactKey) -> String {
        switch fact {
        case .version: return String(localized: "Version")
        case .genre: return String(localized: "Genre")
        case .language: return String(localized: "Language")
        case .released: return String(localized: "Released")
        case .label: return String(localized: "Label")
        case .composer: return String(localized: "Composer")
        case .lyricist: return String(localized: "Lyricist")
        case .arranger: return String(localized: "Arranger")
        case .producer: return String(localized: "Producer")
        case .popularity: return String(localized: "Popularity")
        case .quality: return String(localized: "Quality")
        case .isrc: return String(localized: "ISRC")
        case .disc: return String(localized: "Disc")
        case .track: return String(localized: "Track")
        case .other(let label): return label
        }
    }

    /// The core's quality label: "Hi-Res Lossless", "Lossless", "High", "Standard".
    static func quality(_ label: AudioQualityLabel) -> String {
        switch label {
        case .hiResLossless: return String(localized: "Hi-Res Lossless")
        case .lossless: return String(localized: "Lossless")
        case .high: return String(localized: "High")
        case .standard: return String(localized: "Standard")
        }
    }
}

/// Get Info for an item: what to ask the core about, and what the view already knows.
struct InfoRequest: Identifiable, Equatable {
    enum Kind: Equatable {
        case song, album, artist
    }

    let id = UUID()
    var item: ItemReference
    var kind: Kind
    var known: KnownDetails

    /// A song row's Get Info: a library song, a file, or a source's song (naming the source).
    static func song(_ row: TrackItem, sourceName: String?) -> InfoRequest? {
        let item: ItemReference
        var path = row.path
        switch row.track {
        case .library(let id)?:
            item = .track(id: id)
        case .file(let file)?:
            item = .file(path: file)
            path = path ?? file
        case .source(let song)?:
            item = .source(plugin: song.plugin, kind: .track, id: song.id)
        case nil:
            return nil
        }
        let isSource: Bool
        if case .source = item { isSource = true } else { isSource = false }
        return InfoRequest(item: item, kind: .song,
                           known: KnownDetails(title: row.title, artist: row.artist, album: row.album, year: nil,
                                               durationMs: row.durationMs, path: path,
                                               source: isSource ? sourceName : nil))
    }

    /// A source's album or artist (its page's More menu).
    static func container(_ result: SourceResult, sourceName: String?) -> InfoRequest? {
        guard let item = result.itemReference else { return nil }
        let kind: Kind
        switch result.kind {
        case .album: kind = .album
        case .artist: kind = .artist
        case .track: kind = .song
        case .playlist, .folder: return nil
        }
        return InfoRequest(item: item, kind: kind,
                           known: KnownDetails(title: result.title, artist: result.subtitle, album: result.album,
                                               year: nil, durationMs: result.durationMs, path: nil,
                                               source: sourceName))
    }

    static func == (a: InfoRequest, b: InfoRequest) -> Bool {
        a.id == b.id
    }
}

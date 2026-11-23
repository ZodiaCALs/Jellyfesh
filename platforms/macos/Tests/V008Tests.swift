// v0.0.8's app rules on top of the core's contract: the network the core hears about, and the
// quality badge's words.

import XCTest
@testable import Jellyfesh

final class NetworkStateTests: XCTestCase {
    func testAnExpensiveOrConstrainedPathIsMetered() {
        XCTAssertEqual(NetworkState.from(satisfied: true, expensive: false, constrained: false),
                       NetworkState(online: true, metered: false))
        XCTAssertEqual(NetworkState.from(satisfied: true, expensive: true, constrained: false),
                       NetworkState(online: true, metered: true), "a phone's hotspot")
        XCTAssertEqual(NetworkState.from(satisfied: true, expensive: false, constrained: true),
                       NetworkState(online: true, metered: true), "Low Data Mode")
    }

    func testWithoutAPathNothingIsMetered() {
        XCTAssertEqual(NetworkState.from(satisfied: false, expensive: true, constrained: true),
                       NetworkState(online: false, metered: false))
    }
}

final class QualityMenuTests: XCTestCase {
    private let flac = AudioQuality(codec: "flac", sampleRateHz: 96_000, bitDepth: 24, channels: 2,
                                    bitrateKbps: 2_300, lossless: true, label: .hiResLossless)
    private let aac = AudioQuality(codec: "aac", sampleRateHz: 44_100, bitDepth: nil, channels: 2,
                                   bitrateKbps: 256, lossless: false, label: .high)

    func testTheFormatLineIsTheDepthAndRateOrTheBitrate() {
        XCTAssertEqual(QualityMenuContent.formatLine(flac), "FLAC · 24-bit · 96 kHz")
        XCTAssertEqual(QualityMenuContent.formatLine(aac), "AAC · 256 kbit/s")
        XCTAssertNil(QualityMenuContent.formatLine(AudioQuality(codec: nil, sampleRateHz: nil, bitDepth: nil,
                                                                channels: nil, bitrateKbps: nil, lossless: nil,
                                                                label: nil)))
    }

    func testAStreamOffersItsLevelsAutoFirst() {
        let menu = QualityMenuContent(playing: flac, offered: [.auto, .lossless, .high, .normal, .low], chosen: .high,
                                      note: nil)
        XCTAssertEqual(menu.badge, "Hi-Res Lossless", "the core's label for what plays")
        XCTAssertEqual(menu.levels, [.auto, .lossless, .high, .normal, .low])
        XCTAssertEqual(menu.chosen, .high)
        XCTAssertNil(menu.note)
    }

    func testWithoutLevelsTheMenuSaysWhy() {
        let file = QualityMenuContent(playing: aac, offered: [], chosen: .auto, note: .filesPlayAsTheyAre)
        XCTAssertTrue(file.levels.isEmpty)
        XCTAssertEqual(file.note, "Files play as they are.")
        let remote = QualityMenuContent(playing: nil, offered: [], chosen: .lossless, note: .setInService("Spotify"))
        XCTAssertEqual(remote.note, "Quality is set in Spotify.")
        XCTAssertEqual(remote.badge, "Lossless", "the chosen level before anything is decoded")
        XCTAssertNil(remote.formatLine)
    }
}

@MainActor
final class QualityMenuStoreTests: XCTestCase {
    func testTheBadgeFollowsWhatPlays() {
        let store = PlaybackStore(client: FakeCoreClient(), tracker: CommandTracker())
        XCTAssertNil(store.qualityMenu(chosen: .auto), "no badge with nothing playing")
        var now = NowPlayingInfo(uid: 1, track: .file(path: "/Music/a.flac"), path: "/Music/a.flac", title: "A",
                                 artist: nil, album: nil, durationMs: 1_000, artwork: nil, format: nil)
        now.qualityNote = .filesPlayAsTheyAre
        store.apply(nowPlaying: now)
        XCTAssertEqual(store.qualityMenu(chosen: .auto)?.note, "Files play as they are.")
        XCTAssertEqual(store.qualityMenu(chosen: .auto)?.levels, [])

        now.qualities = [.auto, .lossless, .high]
        now.qualityNote = nil
        store.apply(nowPlaying: now)
        let menu = store.qualityMenu(chosen: .high)
        XCTAssertEqual(menu?.levels, [.auto, .lossless, .high])
        XCTAssertEqual(menu?.chosen, .high)
        XCTAssertEqual(menu?.badge, "High", "the chosen level until the engine has opened the song")
    }
}

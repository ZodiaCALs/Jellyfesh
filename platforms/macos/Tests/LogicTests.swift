import XCTest
@testable import Jellyfesh

final class JSONValueTests: XCTestCase {
    func testObjectsKeepTheirOrder() throws {
        let value = try JSONValue.parse(#"{"zeta": 1, "alpha": [true, null, "x"], "mid": {"b": 2, "a": 1}}"#)
        XCTAssertEqual(value.members?.map(\.key), ["zeta", "alpha", "mid"])
        XCTAssertEqual(value["mid"]?.members?.map(\.key), ["b", "a"])
        XCTAssertEqual(value["alpha"], .array([.bool(true), .null, .string("x")]))
        XCTAssertEqual(value["zeta"]?.doubleValue, 1)
    }

    func testStringsAndNumbers() throws {
        let value = try JSONValue.parse(#"["a\"b\\c\n", "é🎵", -1.5e2, 0, 12345678901]"#)
        XCTAssertEqual(value.arrayValue?[0].stringValue, "a\"b\\c\n")
        XCTAssertEqual(value.arrayValue?[1].stringValue, "é🎵")
        XCTAssertEqual(value.arrayValue?[2].doubleValue, -150)
        XCTAssertEqual(value.arrayValue?[4].doubleValue, 12_345_678_901)
    }

    func testWritesCompactJSONThatReadsBack() throws {
        let value = JSONValue.object([
            JSONMember(key: "url", value: .string("https://music.example/\"q\"")),
            JSONMember(key: "port", value: .number(4533)),
            JSONMember(key: "ratio", value: .number(0.25)),
            JSONMember(key: "on", value: .bool(false)),
            JSONMember(key: "none", value: .null),
        ])
        XCTAssertEqual(value.jsonText,
                       #"{"url":"https://music.example/\"q\"","port":4533,"ratio":0.25,"on":false,"none":null}"#)
        XCTAssertEqual(try JSONValue.parse(value.jsonText), value)
    }

    func testBrokenTextIsAnError() {
        XCTAssertThrowsError(try JSONValue.parse("not json"))
        XCTAssertThrowsError(try JSONValue.parse(#"{"a": 1"#))
        XCTAssertThrowsError(try JSONValue.parse(#"{"a": 1} trailing"#))
        XCTAssertThrowsError(try JSONValue.parse(String(repeating: "[", count: 100) + String(repeating: "]", count: 100)))
    }

    func testSettingAKeyKeepsTheOthers() {
        let value = JSONValue.object([JSONMember(key: "a", value: .number(1)), JSONMember(key: "b", value: .number(2))])
        let changed = value.setting("a", to: .string("x")).setting("c", to: .bool(true))
        XCTAssertEqual(changed.members?.map(\.key), ["a", "b", "c"])
        XCTAssertEqual(changed["a"], .string("x"))
    }
}

final class SchemaFormTests: XCTestCase {
    private let schema = """
    {
        "type": "object",
        "required": ["server_url"],
        "properties": {
            "server_url": { "type": "string", "format": "uri", "title": "Server" },
            "api_key": { "type": "string", "writeOnly": true },
            "transcode": { "type": "boolean", "default": false },
            "max_bitrate": { "type": "integer", "minimum": 64, "maximum": 320, "multipleOf": 32 },
            "quality": { "type": "string", "enum": ["low", "high"], "enumNames": ["Low", "High"] },
            "region": { "oneOf": [ { "const": "eu", "title": "Europe" }, { "const": "us" } ] },
            "volume": { "type": "number", "minimum": 0, "maximum": 1 },
            "nested": { "type": "object" }
        }
    }
    """

    func testReadsFieldsInTheSchemasOrder() throws {
        let fields = try SchemaForm.fields(schemaJSON: schema)
        XCTAssertEqual(fields.map(\.key),
                       ["server_url", "api_key", "transcode", "max_bitrate", "quality", "region", "volume"])
        XCTAssertEqual(fields[0].title, "Server")
        XCTAssertTrue(fields[0].required)
        XCTAssertEqual(fields[0].kind, .text(secret: false, url: true))
        XCTAssertEqual(fields[1].title, "Api key")
        XCTAssertEqual(fields[1].kind, .text(secret: true, url: false))
        XCTAssertEqual(fields[2].kind, .toggle)
        XCTAssertEqual(fields[3].kind, .integer(min: 64, max: 320, step: 32))
        XCTAssertEqual(fields[4].kind, .choice([
            SchemaField.Choice(value: .string("low"), label: "Low"),
            SchemaField.Choice(value: .string("high"), label: "High"),
        ]))
        XCTAssertEqual(fields[5].kind, .choice([
            SchemaField.Choice(value: .string("eu"), label: "Europe"),
            SchemaField.Choice(value: .string("us"), label: "us"),
        ]))
        if case .number = fields[6].kind {} else { XCTFail("volume is a number") }
    }

    func testEmptyAndBrokenSchemas() throws {
        XCTAssertTrue(try SchemaForm.fields(schemaJSON: "").isEmpty)
        XCTAssertTrue(try SchemaForm.fields(schemaJSON: "{}").isEmpty)
        XCTAssertThrowsError(try SchemaForm.fields(schemaJSON: "not json"))
    }

    func testValuesAndEdits() throws {
        let fields = try SchemaForm.fields(schemaJSON: schema)
        let settings = #"{"server_url":"https://music.example","transcode":null}"#
        XCTAssertEqual(SchemaForm.current(settingsJSON: settings, field: fields[0]), .string("https://music.example"))
        XCTAssertEqual(SchemaForm.current(settingsJSON: settings, field: fields[2]), .bool(false),
                       "null falls back to the default")
        XCTAssertNil(SchemaForm.current(settingsJSON: "", field: fields[3]))
        let edited = try JSONValue.parse(SchemaForm.withValue(settingsJSON: settings, key: "transcode", value: .bool(true)))
        XCTAssertEqual(edited["transcode"], .bool(true))
        XCTAssertEqual(edited["server_url"], .string("https://music.example"))
        XCTAssertEqual(SchemaForm.withValue(settingsJSON: "garbage", key: "a", value: .number(1)), #"{"a":1}"#)
    }

    func testHumanizedNames() {
        XCTAssertEqual(SchemaForm.humanize("server_url"), "Server url")
        XCTAssertEqual(SchemaForm.humanize("max-bitrate"), "Max bitrate")
        XCTAssertEqual(SchemaForm.humanize(""), "")
    }
}

final class LyricsLayoutTests: XCTestCase {
    private func document() -> LyricsDocument {
        LyricsDocument(
            language: "en",
            timing: .line,
            lines: [
                LyricsLine(id: 0, startMs: 0, endMs: 1000, agent: nil, text: "one", words: [], background: [],
                           direction: .leftToRight),
                LyricsLine(id: 1, startMs: 1000, endMs: 2000, agent: nil, text: "two", words: [], background: [],
                           direction: .leftToRight),
            ],
            tracks: [
                LyricsAuxTrack(kind: .translation, language: "zh-Hans", lines: [0: "一", 1: " "]),
                LyricsAuxTrack(kind: .romanization, language: "en-Latn", lines: [0: "wan"]),
            ],
            format: "LRC",
            origin: nil
        )
    }

    func testAuxLinesFollowTheSettings() {
        let doc = document()
        let first = doc.lines[0]
        XCTAssertEqual(LyricsLayout.auxLines(in: doc, for: first, translation: true, romanization: true).map(\.text),
                       ["wan", "一"], "romanization first, then the translation")
        XCTAssertEqual(LyricsLayout.auxLines(in: doc, for: first, translation: true, romanization: false).map(\.text),
                       ["一"])
        XCTAssertTrue(LyricsLayout.auxLines(in: doc, for: first, translation: false, romanization: false).isEmpty)
        XCTAssertTrue(LyricsLayout.auxLines(in: doc, for: doc.lines[1], translation: true, romanization: true).isEmpty,
                      "blank translations are not shown")
    }

    func testTheLastLineStartedHoldsThroughGaps() {
        XCTAssertNil(LyricsLayout.lastStarted(lineStarts: [1000, 2000, 3000], atMs: 999))
        XCTAssertEqual(LyricsLayout.lastStarted(lineStarts: [1000, 2000, 3000], atMs: 2000), 1)
        XCTAssertEqual(LyricsLayout.lastStarted(lineStarts: [1000, 2000, 3000], atMs: 9000), 2)
        XCTAssertEqual(LyricsLayout.scrollLine(active: nil, lastStarted: 2), 2)
        XCTAssertEqual(LyricsLayout.scrollLine(active: 1, lastStarted: 2), 1)
    }

    func testLineSizeIsFluidWithinTheTokens() {
        // Luminous: 40 pt from a 640 pt column, down to 28 in narrow windows.
        XCTAssertEqual(LyricsLayout.lineSize(forWidth: 300), 28)
        XCTAssertEqual(LyricsLayout.lineSize(forWidth: 560), 35)
        XCTAssertEqual(LyricsLayout.lineSize(forWidth: 640), 40)
        XCTAssertEqual(LyricsLayout.lineSize(forWidth: 2000), 40)
        XCTAssertEqual(LyricsLayout.lineSize(forWidth: .infinity), 40)
    }

    func testCJKGetsTheTallerLineHeight() {
        XCTAssertEqual(LyricsLayout.lineHeight(for: "清晨把港湾折进光里", language: nil), 1.6)
        XCTAssertEqual(LyricsLayout.lineHeight(for: "Morning", language: "ja"), 1.6)
        XCTAssertEqual(LyricsLayout.lineHeight(for: "Morning folds the harbour", language: "en"), 1.18)
    }
}

final class ScheduleTests: XCTestCase {
    private let start = Date(timeIntervalSinceReferenceDate: 1_000)

    private func dates<S: Sequence>(_ entries: S, count: Int) -> [TimeInterval] where S.Element == Date {
        Array(entries.prefix(count)).map { $0.timeIntervalSince(start) }
    }

    func testProgressWakesAtEachSecondAndStep() {
        let snapshot = PlaybackSnapshotForSchedule(positionUs: 1_500_000, anchorPositionUs: 1_500_000, running: true,
                                                   durationUs: 10_000_000)
        let schedule = ProgressSchedule(start: snapshot, stepUs: 250_000, seconds: true, active: true)
        let times = dates(schedule.entries(from: start, mode: .normal), count: 4)
        XCTAssertEqual(times[0], 0, accuracy: 1e-9)
        XCTAssertEqual(times[1], 0.251, accuracy: 1e-6)
        XCTAssertEqual(times[2], 0.501, accuracy: 1e-6)
        XCTAssertEqual(times[3], 0.751, accuracy: 1e-6)
    }

    func testPausedOrHiddenDrawsOnce() {
        let paused = PlaybackSnapshotForSchedule(positionUs: 0, anchorPositionUs: 0, running: false, durationUs: nil)
        XCTAssertEqual(Array(ProgressSchedule(start: paused, stepUs: nil, seconds: true, active: true)
            .entries(from: start, mode: .normal)).count, 1)
        let hidden = PlaybackSnapshotForSchedule(positionUs: 0, anchorPositionUs: 0, running: true, durationUs: nil)
        XCTAssertEqual(Array(ProgressSchedule(start: hidden, stepUs: nil, seconds: true, active: false)
            .entries(from: start, mode: .normal)).count, 1)
    }

    func testProgressStopsAtTheEndOfTheTrack() {
        let snapshot = PlaybackSnapshotForSchedule(positionUs: 8_200_000, anchorPositionUs: 8_200_000, running: true,
                                                   durationUs: 10_000_000)
        let all = Array(ProgressSchedule(start: snapshot, stepUs: nil, seconds: true, active: true)
            .entries(from: start, mode: .normal))
        XCTAssertEqual(all.count, 3, "now, at 9 s and at 10 s")
    }

    func testAWakeUpIsNeverBeforeTheChange() {
        // The anchor moved 20 ms ahead of the drawn position: the drawn position slews towards it.
        let behind = PlaybackSnapshotForSchedule(positionUs: 1_480_000, anchorPositionUs: 1_500_000, running: true,
                                                 durationUs: nil)
        XCTAssertEqual(ScheduleClock.date(for: 2_000_000, start: behind, at: start).timeIntervalSince(start),
                       0.521, accuracy: 1e-6, "the drawn position's estimate is the later one")
        let ahead = PlaybackSnapshotForSchedule(positionUs: 1_520_000, anchorPositionUs: 1_500_000, running: true,
                                                durationUs: nil)
        XCTAssertEqual(ScheduleClock.date(for: 2_000_000, start: ahead, at: start).timeIntervalSince(start),
                       0.501, accuracy: 1e-6, "the anchor's estimate is the later one")
        XCTAssertEqual(ScheduleClock.date(for: 0, start: ahead, at: start).timeIntervalSince(start), 0.001,
                       accuracy: 1e-9, "never at or before the start")
    }

    func testBarStepsArePixelsAndAtMostTenASecond() {
        XCTAssertEqual(barStepUs(durationUs: 240_000_000, lengthPoints: 300, scale: 2), 400_000)
        XCTAssertEqual(barStepUs(durationUs: 10_000_000, lengthPoints: 600, scale: 2), 100_000)
        XCTAssertNil(barStepUs(durationUs: nil, lengthPoints: 300, scale: 2))
        XCTAssertNil(barStepUs(durationUs: 1_000_000, lengthPoints: 0, scale: 2))
    }

    func testLyricsWakeAtEveryChange() {
        // Two lines, the first with two words: 0–500, 500–1000; the second from 2000.
        let timeline = LyricsTimeline(
            lineStarts: [0, 2000],
            lineEnds: [1500, UInt32.max],
            segmentStarts: [0, 1500, 2000],
            segmentLines: [0, UInt32.max, 1],
            wordOffsets: [0, 2, 2],
            wordStarts: [0, 500],
            wordEnds: [500, 1000]
        )
        let snapshot = PlaybackSnapshotForSchedule(positionUs: 100_000, anchorPositionUs: 100_000, running: true,
                                                   durationUs: nil)
        let times = dates(LyricsSchedule(timeline: timeline, start: snapshot, active: true)
            .entries(from: start, mode: .normal), count: 5)
        XCTAssertEqual(times.count, 5)
        XCTAssertEqual(times[1], 0.401, accuracy: 1e-6, "the second word starts at 500 ms")
        XCTAssertEqual(times[2], 0.901, accuracy: 1e-6, "it ends at 1000 ms")
        XCTAssertEqual(times[3], 1.401, accuracy: 1e-6, "the first line ends at 1500 ms")
        XCTAssertEqual(times[4], 1.901, accuracy: 1e-6, "the second line starts at 2000 ms")
    }
}

final class EventBatchTests: XCTestCase {
    func testOnlyTheLastOfEachSliceIsKeptInOrder() {
        let first = Fixture.playback(generation: 1)
        let last = Fixture.playback(generation: 2)
        let batch: [CoreEvent] = [
            .playback(first),
            .error("one"),
            .libraryChanged(revision: 1),
            .playback(last),
            .scan(.progress(filesSeen: 1, filesChanged: 0, current: "a")),
            .scan(.progress(filesSeen: 2, filesChanged: 0, current: "b")),
            .libraryChanged(revision: 2),
            .error("two"),
        ]
        XCTAssertEqual(EventBatch.coalesced(batch), [
            .error("one"),
            .playback(last),
            .scan(.progress(filesSeen: 2, filesChanged: 0, current: "b")),
            .libraryChanged(revision: 2),
            .error("two"),
        ])
    }

    func testOnlyTheLastRemoteSliceIsKept() {
        let started = Fixture.remote(version: 1)
        let ended = Fixture.ended(version: 2, error: "Sign in to Spotify again.")
        XCTAssertEqual(EventBatch.coalesced([.remote(started), .error("x"), .remote(ended)]),
                       [.error("x"), .remote(ended)])
    }

    func testDiscreteEventsAreAllKept() {
        let batch: [CoreEvent] = [
            .commandFailed(id: 1, message: "a"),
            .commandFailed(id: 2, message: "b"),
            .playlistCreated(request: 3, id: 4),
            .scan(.finished(ScanSummary(added: 1, updated: 0, removed: 0, unchanged: 0, errors: []))),
        ]
        XCTAssertEqual(EventBatch.coalesced(batch), batch)
    }
}

final class DockIconTests: XCTestCase {
    func testAutomaticFollowsTheSystemAppearance() {
        XCTAssertNil(DockIcon.assetName(for: "automatic", systemDark: false), "Ice is the bundle icon")
        XCTAssertEqual(DockIcon.assetName(for: "automatic", systemDark: true), "DockIcon-steel")
    }

    func testAChoiceReplacesTheDockIcon() {
        XCTAssertNil(DockIcon.assetName(for: "ice", systemDark: true))
        XCTAssertEqual(DockIcon.assetName(for: "navy-tide", systemDark: false), "DockIcon-navy-tide")
        XCTAssertNil(DockIcon.assetName(for: "made-up", systemDark: false), "unknown names keep the bundle icon")
    }
}

final class FormattingTests: XCTestCase {
    func testClockText() {
        XCTAssertEqual(TimeText.from(microseconds: 84_000_000), "1:24")
        XCTAssertEqual(TimeText.from(microseconds: -5), "0:00")
        XCTAssertEqual(TimeText.from(milliseconds: 3_725_000), "1:02:05")
    }

    func testDecibels() {
        XCTAssertEqual(DecibelText.format(3), "+3.0 dB")
        XCTAssertEqual(DecibelText.format(-1.54), "−1.5 dB")
        XCTAssertEqual(DecibelText.format(0), "0.0 dB")
    }

    func testThumbnailSizes() {
        XCTAssertEqual(ArtworkLoader.thumbnailSize(for: 40), 64)
        XCTAssertEqual(ArtworkLoader.thumbnailSize(for: 88), 128)
        XCTAssertEqual(ArtworkLoader.thumbnailSize(for: 4000), 1024)
    }
}

final class AboutInfoTests: XCTestCase {
    func testTheWebsiteAndItsLicencesPage() {
        let links = AboutInfo.links(website: " https://example.github.io/jellyfesh ")
        XCTAssertEqual(links?.website.absoluteString, "https://example.github.io/jellyfesh/")
        XCTAssertEqual(links?.licences.absoluteString, "https://example.github.io/jellyfesh/THIRD_PARTY_NOTICES.html")
        XCTAssertEqual(AboutInfo.links(website: "https://jellyfesh.example/")?.website.absoluteString,
                       "https://jellyfesh.example/", "a slash is not doubled")
    }

    func testNoWebsiteMeansNoLinks() {
        XCTAssertNil(AboutInfo.links(website: nil))
        XCTAssertNil(AboutInfo.links(website: ""))
        XCTAssertNil(AboutInfo.links(website: "ftp://example.com/"), "only web addresses")
        XCTAssertNil(AboutInfo.links(website: "$(JF_WEBSITE)"), "a setting the build did not fill in")
    }

    func testTheCoresVersionShowsOnlyWhenItDiffers() {
        XCTAssertEqual(AboutInfo.version(app: "0.0.6", core: "0.0.6"), "0.0.6")
        XCTAssertEqual(AboutInfo.version(app: "0.0.6", core: nil), "0.0.6")
        let both = AboutInfo.version(app: "0.0.6", core: "0.0.7")
        XCTAssertTrue(both.contains("0.0.6") && both.contains("0.0.7"), both)
        XCTAssertEqual(AboutInfo.version(app: "", core: "0.0.7"), "0.0.7")
    }

    func testTheVersionComesFromTheBuild() {
        XCTAssertFalse(AboutInfo.appVersion().isEmpty, "MARKETING_VERSION, never typed in")
    }
}

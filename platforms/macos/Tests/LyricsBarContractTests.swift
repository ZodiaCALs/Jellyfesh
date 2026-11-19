// The lyrics bar port (Support/LyricsBar.swift) replays tests/contract/lyrics-bar.json, recorded
// from jf_core::lyrics_bar by core/jf-ffi/tests/lyrics_bar_contract.rs: line and next exactly,
// each word's progress and the line's progress within 1e-6 (the file rounds them to millionths).

import Foundation
import XCTest
@testable import Jellyfesh

final class LyricsBarContractTests: XCTestCase {
    /// tests/contract/lyrics-bar.json, found from this file's location in the repository.
    private func contract() throws -> [String: Any] {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { root.deleteLastPathComponent() } // the file, Tests, macos, platforms
        let data = try Data(contentsOf: root.appendingPathComponent("tests/contract/lyrics-bar.json"))
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    func testTheLyricsBarContract() throws {
        let file = try contract()
        XCTAssertEqual(file["version"] as? Int, 1)
        var checked = 0
        for case let testCase as [String: Any] in try XCTUnwrap(file["cases"] as? [Any]) {
            let name = testCase["name"] as? String ?? "?"
            let arrays = try XCTUnwrap(testCase["timeline"] as? [String: Any])
            func uints(_ key: String) -> [UInt32] {
                ((arrays[key] as? [NSNumber]) ?? []).map { $0.uint32Value }
            }
            let timeline = LyricsTimeline(
                lineStarts: uints("line_starts"),
                lineEnds: uints("line_ends"),
                segmentStarts: uints("segment_starts"),
                segmentLines: uints("segment_lines"),
                wordOffsets: uints("word_offsets"),
                wordStarts: uints("word_starts"),
                wordEnds: uints("word_ends")
            )
            for case let query as [String: Any] in try XCTUnwrap(testCase["queries"] as? [Any]) {
                let t = try XCTUnwrap(query["t"] as? NSNumber).uint32Value
                let bar = timeline.bar(atMs: t)
                XCTAssertEqual(bar.line, (query["line"] as? NSNumber)?.intValue, "\(name): line at \(t)")
                XCTAssertEqual(bar.next, (query["next"] as? NSNumber)?.intValue, "\(name): next at \(t)")
                let words = ((query["words"] as? [NSNumber]) ?? []).map(\.doubleValue)
                XCTAssertEqual(bar.words.count, words.count, "\(name): words at \(t)")
                for (got, want) in zip(bar.words, words) {
                    XCTAssertEqual(Double(got), want, accuracy: 1e-6, "\(name): a word at \(t)")
                }
                let progress = try XCTUnwrap(query["line_progress"] as? NSNumber).doubleValue
                XCTAssertEqual(Double(bar.lineProgress), progress, accuracy: 1e-6, "\(name): line progress at \(t)")
                checked += 1
            }
        }
        XCTAssertGreaterThan(checked, 60)
    }

    func testNoLyricsShowNothing() {
        let empty = LyricsTimeline(lineStarts: [], lineEnds: [], segmentStarts: [], segmentLines: [], wordOffsets: [],
                                   wordStarts: [], wordEnds: [])
        XCTAssertEqual(empty.bar(atMs: 1_000), .empty)
    }
}

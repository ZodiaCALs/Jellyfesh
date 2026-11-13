// The Swift ports replay the contracts recorded from the Rust originals:
// tests/contract/lyrics-cursor.json and display-clock.json. Add this file to a test target that
// compiles (or imports) the generated JellyfeshFFI bindings and ../Sources (docs/ffi.md).
// Not run yet: there is no Swift toolchain where these were written.

import Foundation
import XCTest
#if canImport(JellyfeshFFI)
import JellyfeshFFI
#endif

final class DrawingContractTests: XCTestCase {
    /// tests/contract/, found from this file's location in the repository.
    private func contract(_ name: String) throws -> [String: Any] {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() } // Tests, swift, shared, platforms, repo
        let data = try Data(contentsOf: root.appendingPathComponent("tests/contract/\(name)"))
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    func testLyricsCursorContract() throws {
        let file = try contract("lyrics-cursor.json")
        XCTAssertEqual(file["version"] as? Int, 1)
        var checked = 0
        for case let testCase as [String: Any] in try XCTUnwrap(file["cases"] as? [Any]) {
            let name = testCase["name"] as? String ?? "?"
            let tl = try XCTUnwrap(testCase["timeline"] as? [String: Any])
            func uints(_ key: String) -> [UInt32] {
                ((tl[key] as? [NSNumber]) ?? []).map { $0.uint32Value }
            }
            let timeline = LyricTimeline(
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
                let cursor = timeline.cursor(t)
                XCTAssertEqual(cursor.line, (query["line"] as? NSNumber)?.intValue, "\(name): line at \(t)")
                XCTAssertEqual(cursor.word, (query["word"] as? NSNumber)?.intValue, "\(name): word at \(t)")
                let progress = try XCTUnwrap(query["word_progress"] as? NSNumber).doubleValue
                XCTAssertEqual(Double(cursor.wordProgress), progress, accuracy: 1e-6, "\(name): word progress at \(t)")
                XCTAssertEqual(
                    timeline.nextChange(t),
                    (query["next_change"] as? NSNumber)?.uint32Value,
                    "\(name): next change after \(t)"
                )
                checked += 1
            }
        }
        XCTAssertGreaterThan(checked, 100)
    }

    func testDisplayClockContract() throws {
        let file = try contract("display-clock.json")
        XCTAssertEqual(file["version"] as? Int, 1)
        var checks = 0
        for case let testCase as [String: Any] in try XCTUnwrap(file["cases"] as? [Any]) {
            let name = testCase["name"] as? String ?? "?"
            var clock = DisplayClock()
            for (i, step) in try XCTUnwrap(testCase["steps"] as? [[String: Any]]).enumerated() {
                let at = "\(name), step \(i)"
                let nowNs = (step["now_ns"] as? NSNumber)?.uint64Value ?? 0
                switch step["op"] as? String {
                case "update":
                    clock.update(try anchor(XCTUnwrap(step["anchor"] as? [String: Any])), nowNs: nowNs)
                case "position":
                    XCTAssertEqual(clock.positionUs(nowNs: nowNs), (step["expect"] as? NSNumber)?.int64Value, at)
                case "wait_until":
                    let target = try XCTUnwrap(step["target_us"] as? NSNumber).int64Value
                    XCTAssertEqual(
                        clock.waitUntilUs(targetUs: target, nowNs: nowNs),
                        (step["expect_us"] as? NSNumber)?.uint64Value,
                        at
                    )
                case "running":
                    XCTAssertEqual(clock.isRunning, step["expect"] as? Bool, at)
                default:
                    XCTFail("unknown op in \(at)")
                }
                checks += 1
            }
        }
        for row in try XCTUnwrap(file["next_second"] as? [[NSNumber]]) {
            XCTAssertEqual(nextSecond(row[0].int64Value), row[1].int64Value, "nextSecond(\(row[0]))")
            checks += 1
        }
        for row in try XCTUnwrap(file["next_step"] as? [[Any]]) {
            let position = try XCTUnwrap(row[0] as? NSNumber).int64Value
            let step = try XCTUnwrap(row[1] as? NSNumber).int64Value
            XCTAssertEqual(nextStep(position, step: step), (row[2] as? NSNumber)?.int64Value, "nextStep(\(position), \(step))")
            checks += 1
        }
        XCTAssertGreaterThan(checks, 1000)
    }

    private func anchor(_ json: [String: Any]) throws -> PlaybackAnchor {
        PlaybackAnchor(
            trackGeneration: try XCTUnwrap(json["track_generation"] as? NSNumber).uint64Value,
            positionUs: try XCTUnwrap(json["position_us"] as? NSNumber).int64Value,
            anchorMonoNs: try XCTUnwrap(json["anchor_mono_ns"] as? NSNumber).uint64Value,
            rate: try XCTUnwrap(json["rate"] as? NSNumber).floatValue,
            durationUs: (json["duration_us"] as? NSNumber)?.int64Value
        )
    }
}

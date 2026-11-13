// The glass-alpha port replays tests/contract/glass-alpha.json, recorded from jf_palette. Add this
// file to a test target that compiles (or imports) ../Sources (docs/ffi.md). Not run yet: there is
// no Swift toolchain where it was written.

import Foundation
import XCTest
#if canImport(JellyfeshFFI)
import JellyfeshFFI
#endif

final class GlassContractTests: XCTestCase {
    /// tests/contract/, found from this file's location in the repository.
    private func contract(_ name: String) throws -> [String: Any] {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() } // Tests, swift, shared, platforms, repo
        let data = try Data(contentsOf: root.appendingPathComponent("tests/contract/\(name)"))
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    func testGlassAlphaContract() throws {
        let file = try contract("glass-alpha.json")
        XCTAssertEqual(file["version"] as? Int, 1)
        let tolerance = try XCTUnwrap(file["tolerance"] as? NSNumber).doubleValue
        var checked = 0
        for case let values as [NSNumber] in try XCTUnwrap(file["cases"] as? [Any]) {
            XCTAssertEqual(values.count, 3)
            let alpha = values[0].floatValue
            let transparency = values[1].floatValue
            let actual = Double(glassAlpha(alpha, transparency: transparency))
            XCTAssertEqual(actual, values[2].doubleValue, accuracy: tolerance, "glass alpha (\(alpha), \(transparency))")
            checked += 1
        }
        XCTAssertGreaterThanOrEqual(checked, 50)
    }

    func testGlassAlphaKeepsTheTokensAtTheDefaultAndClampsItsInputs() {
        for a: Float in [0, 0.3, 0.42, 0.62, 1] {
            XCTAssertEqual(glassAlpha(a, transparency: defaultTransparency), a)
        }
        XCTAssertEqual(glassAlpha(0.42, transparency: -1), 1)
        XCTAssertEqual(glassAlpha(0.42, transparency: .nan), 0.42)
        XCTAssertEqual(glassAlpha(.nan, transparency: 0.75), 0.75)
        XCTAssertEqual(glassAlpha(1.5, transparency: 0.5), 1)
    }
}

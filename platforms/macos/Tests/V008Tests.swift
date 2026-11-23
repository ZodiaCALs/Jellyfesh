// v0.0.8's app rules on top of the core's contract: the network the core hears about.

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

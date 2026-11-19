// Desktop Lyrics following its setting and keeping its lock, on a stand-in for the panel, and the
// panel's first place on the screen.

import SwiftUI
import XCTest
@testable import Jellyfesh

/// Desktop Lyrics' panel, as calls.
@MainActor
private final class FakeDesktopLyricsHost: DesktopLyricsHost {
    private(set) var calls: [String] = []

    func show() { calls.append("show") }
    func hide() { calls.append("hide") }
    func setClickThrough(_ clickThrough: Bool) { calls.append(clickThrough ? "click-through" : "clickable") }
}

@MainActor
final class DesktopLyricsStoreTests: XCTestCase {
    private var suites: [String] = []

    override func tearDown() {
        for name in suites {
            UserDefaults.standard.removePersistentDomain(forName: name)
        }
        suites = []
        super.tearDown()
    }

    private func freshDefaults() -> UserDefaults {
        let name = "org.jellyfesh.tests.desktoplyrics.\(UUID().uuidString)"
        suites.append(name)
        return UserDefaults(suiteName: name)!
    }

    func testTheWindowFollowsTheSetting() {
        let host = FakeDesktopLyricsHost()
        let store = DesktopLyricsStore(defaults: freshDefaults())
        store.attach(host)
        XCTAssertEqual(host.calls, ["clickable"])
        store.apply(enabled: false)
        XCTAssertFalse(store.shown)
        store.apply(enabled: true)
        store.apply(enabled: true)
        XCTAssertTrue(store.shown)
        store.apply(enabled: false)
        XCTAssertEqual(host.calls, ["clickable", "show", "hide"], "each change once")
    }

    func testAWindowAttachedLateShowsWhenTheSettingIsOn() {
        let store = DesktopLyricsStore(defaults: freshDefaults())
        store.apply(enabled: true)
        let host = FakeDesktopLyricsHost()
        store.attach(host)
        XCTAssertEqual(host.calls, ["clickable", "show"])
    }

    func testTheLockLetsClicksThroughAndIsKept() {
        let defaults = freshDefaults()
        let host = FakeDesktopLyricsHost()
        let store = DesktopLyricsStore(defaults: defaults)
        XCTAssertFalse(store.locked)
        store.attach(host)
        store.apply(enabled: true)
        store.setLocked(true)
        store.setLocked(true)
        XCTAssertTrue(store.locked)
        XCTAssertEqual(host.calls, ["clickable", "show", "click-through"])
        XCTAssertTrue(defaults.bool(forKey: DesktopLyricsStore.lockedKey))

        let next = DesktopLyricsStore(defaults: defaults)
        XCTAssertTrue(next.locked, "the lock is kept between launches")
        let nextHost = FakeDesktopLyricsHost()
        next.attach(nextHost)
        XCTAssertEqual(nextHost.calls, ["click-through"])
        next.setLocked(false)
        XCTAssertEqual(nextHost.calls, ["click-through", "clickable"])
        XCTAssertFalse(defaults.bool(forKey: DesktopLyricsStore.lockedKey))
    }

    func testTheWindowFirstOpensCentredAboveTheDock() {
        let frame = DesktopLyricsWindow.defaultFrame(in: CGRect(x: 0, y: 80, width: 1440, height: 820))
        XCTAssertEqual(frame.midX, 720, accuracy: 1e-9)
        XCTAssertEqual(frame.minY, 144, accuracy: 1e-9)
        XCTAssertEqual(frame.size, DesktopLyricsWindow.size)
        let narrow = DesktopLyricsWindow.defaultFrame(in: CGRect(x: 100, y: 0, width: 600, height: 400))
        XCTAssertEqual(narrow.width, 600, accuracy: 1e-9)
        XCTAssertEqual(narrow.minX, 100, accuracy: 1e-9)
    }
}

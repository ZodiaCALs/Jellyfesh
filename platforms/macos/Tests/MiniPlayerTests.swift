// The mini player window (ADR-0026 §4): opening and closing it once each, on a stand-in for the
// panel; Keep on Top; opening when the main window closes while music plays; its first place.

import XCTest
@testable import Jellyfesh

/// The mini player's panel, as calls.
@MainActor
private final class FakeMiniPlayerHost: MiniPlayerHost {
    private(set) var calls: [String] = []

    func show() { calls.append("show") }
    func hide() { calls.append("hide") }
    func setKeepOnTop(_ keepOnTop: Bool) { calls.append(keepOnTop ? "on top" : "normal") }
}

@MainActor
final class MiniPlayerStoreTests: XCTestCase {
    func testTheButtonAndTheMenuOpenAndCloseItOnce() {
        let host = FakeMiniPlayerHost()
        let store = MiniPlayerStore()
        store.attach(host)
        XCTAssertEqual(host.calls, ["on top"], "Keep on Top is on by default")
        store.open()
        store.open()
        XCTAssertTrue(store.shown)
        store.toggle()
        XCTAssertFalse(store.shown)
        store.close()
        store.toggle()
        XCTAssertEqual(host.calls, ["on top", "show", "hide", "show"])
    }

    func testAWindowAttachedLateShowsWhatWasAsked() {
        let store = MiniPlayerStore()
        store.open()
        let host = FakeMiniPlayerHost()
        store.attach(host)
        XCTAssertEqual(host.calls, ["on top", "show"])
    }

    func testTheSettingsReachTheWindow() {
        let host = FakeMiniPlayerHost()
        let store = MiniPlayerStore()
        store.attach(host)
        store.apply(keepOnTop: false, opensWhenWindowCloses: true, showsLyricsLine: false)
        store.apply(keepOnTop: false, opensWhenWindowCloses: true, showsLyricsLine: false)
        XCTAssertEqual(host.calls, ["on top", "normal"], "each change once")
        XCTAssertFalse(store.showsLyricsLine)
    }

    func testClosingTheWindowOpensItOnlyWhenAskedAndMusicPlays() {
        let store = MiniPlayerStore()
        store.mainWindowClosed(playing: true)
        XCTAssertFalse(store.shown, "Open When the Window Is Closed is off by default")
        store.apply(keepOnTop: true, opensWhenWindowCloses: true, showsLyricsLine: true)
        store.mainWindowClosed(playing: false)
        XCTAssertFalse(store.shown, "not while nothing plays")
        store.mainWindowClosed(playing: true)
        XCTAssertTrue(store.shown)
    }

    func testTheWindowFirstOpensAtTheTopRight() {
        let visible = CGRect(x: 0, y: 40, width: 1440, height: 860)
        let frame = MiniPlayerWindow.defaultFrame(in: visible)
        XCTAssertEqual(frame.size, MiniPlayerWindow.size)
        XCTAssertEqual(frame.maxX, visible.maxX - 24)
        XCTAssertEqual(frame.maxY, visible.maxY - 24)
    }
}

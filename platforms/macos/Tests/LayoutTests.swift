// v0.0.8: one layout, Jellyfesh Web's (ADR-0023): Settings and About as pages of the main window.

import XCTest
@testable import Jellyfesh

@MainActor
final class SettingsPageTests: XCTestCase {
    func testSettingsIsAPageOfTheMainWindow() {
        let model = AppModel(client: FakeCoreClient())
        model.router.select(.albums)
        model.router.showNowPlaying(true, reduceMotion: true)
        model.openSettings(.about)
        XCTAssertEqual(model.router.selection, .settings)
        XCTAssertEqual(model.settingsTab, .about, "About Jellyfesh opens Settings › About")
        XCTAssertFalse(model.router.nowPlayingShown, "Now Playing closes over it")

        model.router.select(.home)
        model.openSettings()
        XCTAssertEqual(model.router.selection, .settings)
        XCTAssertEqual(model.settingsTab, .about, "Settings… opens the section shown last")
    }

    func testTheSectionsAreInTheSpecsOrder() {
        XCTAssertEqual(SettingsTab.allCases, [.appearance, .playback, .library, .sources, .extensions, .about])
    }

    func testAboutListsTheSharedKeysWithCommand() {
        XCTAssertEqual(AboutInfo.shortcuts.map(\.keys),
                       ["Space", "⌘ →", "⌘ ←", "⇧ →", "⇧ ←", "⌘ ↑", "⌘ ↓", "⌘ F", "⌘ ,", "Esc"])
        XCTAssertEqual(AboutInfo.shortcuts.map(\.action),
                       ["Play or Pause", "Next Song", "Previous Song", "Skip Forward 5 Seconds",
                        "Skip Back 5 Seconds", "Volume Up", "Volume Down", "Search", "Settings", "Back"])
    }
}

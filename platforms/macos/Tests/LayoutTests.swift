// v0.0.8: one layout, Jellyfesh Web's (ADR-0023): the web's breakpoints, the sidebar panel and the
// phone tabs, and Settings and About as pages of the main window.

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

final class ShellLayoutTests: XCTestCase {
    func testTheWebsBreakpoints() {
        XCTAssertTrue(ShellLayout(width: 760).showsTabs, "up to 760 pt: the phone tabs")
        XCTAssertFalse(ShellLayout(width: 760).showsSidebar)
        XCTAssertTrue(ShellLayout(width: 761).showsSidebar, "from 761 pt: the sidebar panel")
        XCTAssertFalse(ShellLayout(width: 761).showsTabs)
        XCTAssertFalse(ShellLayout(width: 899).showsPlayerBar, "the capsule under 900 pt")
        XCTAssertTrue(ShellLayout(width: 900).showsPlayerBar, "the player bar from 900 pt of window")
        XCTAssertLessThan(ShellLayout.minimumWidth, ShellLayout.narrowMax, "narrow windows can be had")
    }

    func testThePagesFadeAboveWhatFloatsAtTheBottom() {
        XCTAssertEqual(ShellLayout.bottomZone(playerHeight: nil, tabs: false), 0)
        XCTAssertEqual(ShellLayout.bottomZone(playerHeight: 80, tabs: false), 96)
        XCTAssertEqual(ShellLayout.bottomZone(playerHeight: nil, tabs: true), 92, "the tabs and their margin")
        XCTAssertEqual(ShellLayout.bottomZone(playerHeight: 64, tabs: true), 172)
    }

    func testTheWordmarkShowsTheChosenIcon() {
        XCTAssertEqual(AppIconChoice.wordmarkThumbnail(for: "automatic", dark: false), "IconThumb-ice")
        XCTAssertEqual(AppIconChoice.wordmarkThumbnail(for: "automatic", dark: true), "IconThumb-steel")
        XCTAssertEqual(AppIconChoice.wordmarkThumbnail(for: "navy-tide", dark: true), "IconThumb-navy-tide")
        XCTAssertEqual(AppIconChoice.wordmarkThumbnail(for: "no-such-icon", dark: false), "IconThumb-ice")
    }
}

@MainActor
final class NavigationTests: XCTestCase {
    func testTheLibraryTabReturnsToTheLibrarysPlaceShownLast() {
        let router = Router()
        XCTAssertEqual(router.tab, .home)
        router.selectTab(.library)
        XCTAssertEqual(router.selection, .albums, "Albums first")
        router.select(.favorites)
        XCTAssertEqual(router.tab, .library)
        router.selectTab(.search)
        XCTAssertEqual(router.tab, .search)
        router.selectTab(.library)
        XCTAssertEqual(router.selection, .favorites)
        router.selectTab(.settings)
        XCTAssertEqual(router.selection, .settings)
    }

    func testAPlaylistMadeHereOpensOnPlaylists() {
        let router = Router()
        router.select(.songs)
        router.openPlaylist(7)
        XCTAssertEqual(router.selection, .playlists)
        XCTAssertEqual(router.path, [.playlist(7)])
        router.openPlaylist(8)
        XCTAssertEqual(router.path, [.playlist(8)], "one playlist page at a time")
    }

    func testEveryPlaceHasItsWordsAndGlyph() {
        XCTAssertEqual(SidebarItem.libraryPlaces, [.albums, .artists, .songs, .playlists, .favorites, .downloads])
        XCTAssertFalse(SidebarItem.home.isLibraryPlace)
        XCTAssertTrue(SidebarItem.downloads.isLibraryPlace)
        for item in SidebarItem.allCases {
            XCTAssertFalse(item.symbol.isEmpty)
        }
    }
}

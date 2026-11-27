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
        XCTAssertEqual(SettingsTab.allCases, [.appearance, .playback, .library, .downloads, .sources, .extensions, .about])
    }

    func testAboutListsTheSharedKeysWithCommand() {
        XCTAssertEqual(AboutInfo.shortcuts.map(\.keys),
                       ["Space", "⌘ →", "⌘ ←", "⇧ →", "⇧ ←", "⌘ ↑", "⌘ ↓", "⌘ F", "⌥⌘ M", "⌘ ,", "Esc"])
        XCTAssertEqual(AboutInfo.shortcuts.map(\.action),
                       ["Play or Pause", "Next Song", "Previous Song", "Skip Forward 5 Seconds",
                        "Skip Back 5 Seconds", "Volume Up", "Volume Down", "Search", "Open Mini Player",
                        "Settings", "Back"])
    }
}

@MainActor
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

    func testThePlayerFloatsOverEveryPageButNowPlaying() {
        XCTAssertTrue(ShellLayout.showsPlayer(hasTrack: true, nowPlayingShown: false))
        XCTAssertFalse(ShellLayout.showsPlayer(hasTrack: true, nowPlayingShown: true), "Now Playing has its own dock")
        XCTAssertFalse(ShellLayout.showsPlayer(hasTrack: false, nowPlayingShown: false))
    }

    /// The bar beside the panel in the narrowest window that shows it (900 pt).
    private static let narrowestBar = SeekGeometry.playerBarMinWidth - ShellLayout.sidebarWidth - ShellLayout.margin
        - 2 * LuminousGeometry.playerMargin

    func testANarrowBarDropsTheVolumeLineThenLyricsAndQueueThenTheMuteButton() {
        let narrowest = Self.narrowestBar
        XCTAssertFalse(PlayerBarLayout.showsVolumeLine(barWidth: narrowest), "a 900 pt window beside the panel")
        XCTAssertFalse(PlayerBarLayout.showsPaneButtons(barWidth: narrowest), "Now Playing still has them")
        XCTAssertFalse(PlayerBarLayout.showsMuteButton(barWidth: narrowest), "under 640 pt, as on the web")
        XCTAssertTrue(PlayerBarLayout.showsMuteButton(barWidth: 640))
        XCTAssertFalse(PlayerBarLayout.showsPaneButtons(barWidth: 759))
        XCTAssertTrue(PlayerBarLayout.showsPaneButtons(barWidth: 760), "from 760 pt, as on the web")
        let defaultWindow = 1180 - ShellLayout.sidebarWidth - ShellLayout.margin - 2 * LuminousGeometry.playerMargin
        XCTAssertTrue(PlayerBarLayout.showsVolumeLine(barWidth: defaultWindow), "the default window has it all")
        XCTAssertLessThan(NowPlayingDock.compactBelow, SeekGeometry.playerBarMinWidth)
    }

    func testTheBarsColumnsKeepToTheirOwnWidthsAtEveryWidth() {
        // The artwork-and-lines button with no room left for its lines: 6 pt, the 56 pt artwork,
        // 12 pt, 8 pt.
        let artworkAlone: CGFloat = 6 + 56 + 12 + 8
        for barWidth in stride(from: Self.narrowestBar, through: 1280, by: 2) {
            let row = barWidth - PlayerBarLayout.leadingInset - PlayerBarLayout.trailingInset
            let tools = Self.widestTools(barWidth: barWidth)
            let columns = PlayerBarLayout.columns(width: row, tools: tools)
            XCTAssertEqual(columns.lines + columns.transport + tools + 2 * PlayerBarLayout.columnSpacing, row,
                           accuracy: 0.01, "nothing runs past the bar at \(barWidth) pt")
            XCTAssertGreaterThanOrEqual(columns.transport, PlayerBarLayout.transportWidths.lowerBound,
                                        "the five buttons fit at \(barWidth) pt")
            XCTAssertGreaterThanOrEqual(columns.lines, artworkAlone, "the artwork fits at \(barWidth) pt")
        }
    }

    func testTheTransportGrowsToItsWidestOnceTheLinesHaveTheirLeast() {
        let wide = PlayerBarLayout.columns(width: 1256, tools: 400)
        XCTAssertEqual(wide.transport, 340, "the web's widest")
        XCTAssertEqual(wide.lines, 1256 - 400 - 32 - 340, "the lines take the rest")
        let narrow = PlayerBarLayout.columns(width: 700, tools: 300)
        XCTAssertEqual(narrow.lines, PlayerBarLayout.linesMinWidth)
        XCTAssertEqual(narrow.transport, 700 - 300 - 32 - PlayerBarLayout.linesMinWidth)
        let cramped = PlayerBarLayout.columns(width: 600, tools: 300)
        XCTAssertEqual(cramped.transport, 220, "never narrower than its buttons while they fit")
        XCTAssertEqual(cramped.lines, 600 - 300 - 32 - 220)
    }

    /// The tools at their widest at a bar width, as PlayerBarTools draws them 2 pt apart: the
    /// quality badge's widest (132 pt) with its 4 pt margin, the device button (a remote source has
    /// it, and no mute button), Lyrics and Queue, the volume line (150 pt and 4 pt each side), the
    /// mini player button and More, each button 40 pt.
    private static func widestTools(barWidth: CGFloat) -> CGFloat {
        var widths: [CGFloat] = [132 + 4, 40]
        if PlayerBarLayout.showsPaneButtons(barWidth: barWidth) { widths += [40, 40] }
        if PlayerBarLayout.showsVolumeLine(barWidth: barWidth) { widths.append(150 + 8) }
        widths += [40, 40]
        return widths.reduce(0, +) + 2 * CGFloat(widths.count - 1)
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

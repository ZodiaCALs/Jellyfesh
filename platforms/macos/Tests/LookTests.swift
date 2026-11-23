// The look's rules (Design/Look.swift): Luminous draws as v0.0.6, a customized look scales and
// colours within its ranges, and the sidebar's and Home's places keep their order, once each.

import SwiftUI
import XCTest
@testable import Jellyfesh

final class LookStyleTests: XCTestCase {
    private let palette = Palette(colors: ["ink.ink-90": 0x1020_30FF, "accent.accent-text": 0x4050_60FF],
                                  isDark: false)

    /// Colours compare as what they draw.
    private func resolved(_ color: Color) -> Color.Resolved {
        color.resolve(in: EnvironmentValues())
    }

    private func customized(_ change: (inout LookState) -> Void) -> LookStyle {
        var state = LookState.luminous
        state.customized = true
        change(&state)
        return LookStyle(state: state)
    }

    func testLuminousDrawsAsBefore() {
        let look = LookStyle.luminous
        XCTAssertFalse(look.customized)
        XCTAssertEqual(look.cornerScale, 1)
        XCTAssertEqual(look.radius(24), 24)
        XCTAssertEqual(look.glass, 1)
        XCTAssertEqual(look.transparency(setting: 0.5), 0.5, accuracy: 1e-9)
        XCTAssertEqual(look.aurora, Tokens.Field.auroraStrength, accuracy: 1e-9)
        XCTAssertEqual(look.fontDesign, .default)
        XCTAssertEqual(look.textScale, 1)
        XCTAssertEqual(look.size(13), 13)
        XCTAssertEqual(look.densityScale, 1)
        XCTAssertEqual(look.lyricsWeight(token: 700), .bold)
        XCTAssertEqual(resolved(look.sungInk(palette)), resolved(Color(rgba: 0x1020_30FF)))
        XCTAssertEqual(look.glowRadius(forSize: 40), 0)
        XCTAssertEqual(look.sidebar, LookPlace.allCases)
        XCTAssertEqual(look.home, [.summary, .recentlyAdded, .recentlyPlayed, .mostPlayed])
    }

    func testAnUncustomizedLookKeepsLuminousWhateverItsValues() {
        // The core resolves every value; while not customized the shell still draws Luminous.
        var state = LookState.luminous
        state.cornerScale = 2
        state.glass = 0
        state.aurora = 0.1
        state.typeface = .serif
        state.textScale = 1.3
        state.density = .roomy
        state.lyricsWeight = .regular
        state.lyricsGlow = 1
        state.lyricsHighlight = .accent
        state.sidebar = [.songs]
        state.home = [.favourites]
        let look = LookStyle(state: state)
        XCTAssertEqual(look.cornerScale, 1)
        XCTAssertEqual(look.glass, 1)
        XCTAssertEqual(look.fontDesign, .default)
        XCTAssertEqual(look.textScale, 1)
        XCTAssertEqual(look.densityScale, 1)
        XCTAssertEqual(look.sidebar, LookPlace.allCases)
        XCTAssertEqual(look.home, [.summary, .recentlyAdded, .recentlyPlayed, .mostPlayed])
        XCTAssertEqual(look.aurora, Tokens.Field.auroraStrength, accuracy: 1e-9)
        XCTAssertEqual(look.lyricsWeight(token: 700), .bold)
        XCTAssertEqual(look.glowRadius(forSize: 40), 0)
        XCTAssertEqual(resolved(look.sungInk(palette)), resolved(Color(rgba: 0x1020_30FF)))
    }

    func testACustomizedLookScalesWithinItsRanges() {
        let look = customized {
            $0.cornerScale = 1.5
            $0.glass = 0.4
            $0.aurora = 0.2
            $0.textScale = 1.2
            $0.density = .compact
        }
        XCTAssertEqual(look.radius(24), 36, accuracy: 1e-9)
        XCTAssertEqual(look.transparency(setting: 0.5), 0.2, accuracy: 1e-6)
        XCTAssertEqual(look.aurora, 0.2, accuracy: 1e-6)
        XCTAssertEqual(look.size(10), 12, accuracy: 1e-5)
        XCTAssertEqual(look.spacing(10), 8, accuracy: 1e-9)

        let wild = customized {
            $0.cornerScale = 9
            $0.glass = -1
            $0.textScale = .nan
            $0.aurora = .infinity
        }
        XCTAssertEqual(wild.cornerScale, 2)
        XCTAssertEqual(wild.glass, 0)
        XCTAssertEqual(wild.transparency(setting: 1), 0, "no glass is solid whatever the setting")
        XCTAssertEqual(wild.textScale, 1)
        XCTAssertEqual(wild.aurora, Tokens.Field.auroraStrength, accuracy: 1e-6)
        XCTAssertEqual(customized { $0.density = .roomy }.densityScale, 1.2, accuracy: 1e-9)
    }

    func testTheTypefaceIsThePlatformsOwnFace() {
        XCTAssertEqual(customized { $0.typeface = .system }.fontDesign, .default)
        XCTAssertEqual(customized { $0.typeface = .rounded }.fontDesign, .rounded)
        XCTAssertEqual(customized { $0.typeface = .serif }.fontDesign, .serif)
        XCTAssertEqual(customized { $0.typeface = .mono }.fontDesign, .monospaced)
    }

    func testTheLyricsStyle() {
        XCTAssertEqual(customized { $0.lyricsWeight = .medium }.lyricsWeight(token: 700), .medium)
        XCTAssertEqual(customized { $0.lyricsWeight = .heavy }.lyricsWeight(token: 700), .heavy)
        XCTAssertEqual(resolved(customized { $0.lyricsHighlight = .artwork }.sungInk(palette)),
                       resolved(Color(rgba: 0x1020_30FF)))
        XCTAssertEqual(resolved(customized { $0.lyricsHighlight = .accent }.sungInk(palette)),
                       resolved(Color(rgba: 0x4050_60FF)))
        XCTAssertEqual(resolved(customized { $0.lyricsHighlight = .custom(rgb: 0xFF8800) }.sungInk(palette)),
                       resolved(Color(rgba: 0xFF88_00FF)))
        XCTAssertEqual(customized { $0.lyricsGlow = 1 }.glowRadius(forSize: 40), 14, accuracy: 1e-5)
        XCTAssertEqual(customized { $0.lyricsGlow = 0 }.glowRadius(forSize: 40), 0)
    }

    func testPlacesShowOnceInTheirOrder() {
        let look = customized {
            $0.sidebar = [.songs, .home, .songs, .sources]
            $0.home = [.mostPlayed, .summary, .mostPlayed]
        }
        XCTAssertEqual(look.sidebar, [.songs, .home, .sources])
        XCTAssertEqual(look.home, [.mostPlayed, .summary])
        // Without one of the library's places the sidebar would be empty of music: Luminous's.
        XCTAssertEqual(customized { $0.sidebar = [.search] }.sidebar, LookPlace.allCases)
    }

    func testThePanelListsTheLooksPlacesInItsOrder() {
        XCTAssertEqual(SidebarItem.panelEntries(for: .luminous), [
            .place(.home), .place(.albums), .place(.artists), .place(.songs), .place(.playlists),
            .place(.favorites), .place(.downloads), .separator, .place(.search), .place(.sources),
        ])
        // A customized look lists its places without the line; Downloads follows its library places.
        XCTAssertEqual(SidebarItem.panelEntries(for: customized { $0.sidebar = [.sources, .songs, .home] }),
                       [.place(.sources), .place(.songs), .place(.home), .place(.downloads)])
        XCTAssertEqual(SidebarItem.panelEntries(for: customized { $0.sidebar = [.albums, .search] }),
                       [.place(.albums), .place(.downloads), .place(.search)])
    }

    func testThePhoneTabsFollowTheLook() {
        XCTAssertEqual(PhoneTab.tabs(for: .luminous), [.home, .library, .search, .sources, .settings])
        XCTAssertEqual(PhoneTab.tabs(for: customized { $0.sidebar = [.search, .songs, .albums, .home] }),
                       [.search, .library, .home, .settings], "the library's places are one tab; Settings last")
    }

    func testTheLayoutStaysLuminousUntilCustomized() {
        var state = LookState.luminous
        state.nowPlaying = .lyrics
        state.miniPlayer = .floating
        XCTAssertEqual(LookStyle(state: state).nowPlaying, .stage)
        XCTAssertEqual(LookStyle(state: state).miniPlayer, .bar)
        XCTAssertEqual(customized { $0.nowPlaying = .lyrics }.nowPlaying, .lyrics)
        XCTAssertEqual(customized { $0.miniPlayer = .compact }.miniPlayer, .compact)
    }

    func testThePlayerAtTheBottom() {
        XCTAssertEqual(LuminousGeometry.bottomPlayer(style: .bar, wide: true), .bar)
        XCTAssertEqual(LuminousGeometry.bottomPlayer(style: .bar, wide: false), .mini)
        XCTAssertEqual(LuminousGeometry.bottomPlayer(style: .compact, wide: true), .mini)
        XCTAssertEqual(LuminousGeometry.bottomPlayer(style: .floating, wide: true), .floating)
        XCTAssertEqual(LuminousGeometry.bottomPlayer(style: .floating, wide: false), .floating)
    }

    func testNowPlayingsLayouts() {
        for panel in NowPlayingPanel.allCases {
            XCTAssertEqual(LuminousGeometry.nowPlayingArrangement(.stage, panel: panel), .stage)
            XCTAssertEqual(LuminousGeometry.nowPlayingArrangement(.lyrics, panel: panel), .widePane)
        }
        XCTAssertEqual(LuminousGeometry.nowPlayingArrangement(.artwork, panel: .lyrics), .cover)
        XCTAssertEqual(LuminousGeometry.nowPlayingArrangement(.artwork, panel: .queue), .stage)
        XCTAssertEqual(LuminousGeometry.nowPlayingArrangement(.artwork, panel: .info), .stage)
        XCTAssertEqual(LuminousGeometry.coverArtworkSide(width: 1600, height: 1000), 520)
        XCTAssertEqual(LuminousGeometry.coverArtworkSide(width: 1180, height: 760), 340)
        XCTAssertEqual(LuminousGeometry.coverArtworkSide(width: 300, height: 300), 180, "never smaller than 180")
    }
}

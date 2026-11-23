// The window's layout for its width (ADR-0023): Jellyfesh Web's breakpoints and measures. From
// 761 pt wide the floating sidebar panel sits beside the pages; up to 760 pt the phone tabs float at
// the bottom instead. The full player bar shows from 900 pt, the capsule below that (both measured
// on the window, as the web measures its viewport). The rules are pure, for tests.

import CoreGraphics

struct ShellLayout: Equatable {
    /// The window's width in points.
    var width: CGFloat

    /// The widest window that takes the phone layout.
    static let narrowMax: CGFloat = 760
    /// The narrowest the window gets: the pages, the capsule and Now Playing still fit.
    static let minimumWidth: CGFloat = 600
    static let minimumHeight: CGFloat = 600

    /// The sidebar panel: 236 pt of glass, 12 pt from the window's edges (the web's 248 px column).
    static let sidebarWidth: CGFloat = 236
    static let margin: CGFloat = 12

    /// The phone tabs: a capsule 68 pt high, 14 pt from the window's sides and 24 pt above its
    /// bottom; the capsule player floats above it.
    static let tabsHeight: CGFloat = 68
    static let tabsSideMargin: CGFloat = 14
    static let tabsBottomMargin: CGFloat = 24

    /// The floating sidebar panel beside the pages.
    var showsSidebar: Bool { width > Self.narrowMax }
    /// The phone tabs under the pages.
    var showsTabs: Bool { !showsSidebar }
    /// The full player bar rather than the capsule.
    var showsPlayerBar: Bool { SeekGeometry.usesPlayerBar(width: width) }

    /// The player floats over every page while something plays, and is not drawn while Now Playing
    /// is open: Now Playing has its own dock (ADR-0023 §2).
    static func showsPlayer(hasTrack: Bool, nowPlayingShown: Bool) -> Bool {
        hasTrack && !nowPlayingShown
    }

    /// What floats at the bottom of the pages, margins included: the player (`playerHeight`, nil
    /// while it is hidden) and, in the phone layout, the tabs. The pages fade out just above it.
    static func bottomZone(playerHeight: CGFloat?, tabs: Bool) -> CGFloat {
        let player = playerHeight.map { $0 + LuminousGeometry.playerMargin } ?? 0
        return tabs ? player + tabsHeight + tabsBottomMargin : player
    }
}

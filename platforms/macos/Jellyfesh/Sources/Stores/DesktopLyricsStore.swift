// Desktop Lyrics (ADR-0022 §6, docs/ui.md "Lyrics, word by word"): the line being sung and the
// next one in a window of their own above other apps. The core keeps whether it is on (Settings ›
// Appearance › Lyrics › Desktop Lyrics); the lock, which lets clicks pass through the window, is
// the person's on this Mac. The window is AppKit's (Platform/DesktopLyricsWindow.swift), behind
// `DesktopLyricsHost` so tests use a stand-in.

import Foundation
import Observation

/// Where Desktop Lyrics shows: the floating panel, or a stand-in in tests.
@MainActor
protocol DesktopLyricsHost: AnyObject {
    /// Brings the window up, making it the first time, without making Jellyfesh the active app.
    func show()
    /// Takes the window off the screen.
    func hide()
    /// While true, every click passes through the window to what is below it.
    func setClickThrough(_ clickThrough: Bool)
}

@MainActor
@Observable
final class DesktopLyricsStore {
    /// The window is up: the Desktop Lyrics setting is on.
    private(set) var shown = false
    /// Clicks pass through the window to the apps below it.
    private(set) var locked: Bool

    /// The lock's key in the defaults.
    static let lockedKey = "DesktopLyricsLocked"

    @ObservationIgnored private var host: DesktopLyricsHost?
    @ObservationIgnored private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        locked = defaults.bool(forKey: Self.lockedKey)
    }

    /// The window to show the lyrics in. Until one is attached the store only keeps the state.
    func attach(_ host: DesktopLyricsHost) {
        self.host = host
        host.setClickThrough(locked)
        if shown {
            host.show()
        }
    }

    /// Follows the Desktop Lyrics setting.
    func apply(enabled: Bool) {
        guard enabled != shown else { return }
        shown = enabled
        if enabled {
            host?.show()
        } else {
            host?.hide()
        }
    }

    /// Locks the window (clicks pass through it) or unlocks it; kept between launches.
    func setLocked(_ locked: Bool) {
        guard locked != self.locked else { return }
        self.locked = locked
        defaults.set(locked, forKey: Self.lockedKey)
        host?.setClickThrough(locked)
    }
}

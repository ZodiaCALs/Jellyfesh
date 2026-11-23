// The mini player (ADR-0026 §4, docs/ui.md "The player bar and the mini player"): a small window
// above other windows with the cover, the title and the artist, previous, play and next, a
// progress line and the lyrics bar's line. The bar's mini player button and View › Open Mini
// Player (⌥⌘M; ⌘M minimises) open it, and its own button closes it. Settings › Appearance › Mini
// Player keeps it on top (on by default), opens it when the main window closes while music plays
// (off), and shows the lyrics line (on); those settings come from the core. The window is AppKit's
// (Platform/MiniPlayerWindow.swift), behind `MiniPlayerHost` so tests use a stand-in.

import Foundation
import Observation

/// Where the mini player shows: the floating panel, or a stand-in in tests.
@MainActor
protocol MiniPlayerHost: AnyObject {
    /// Brings the window up, making it the first time, without making Jellyfesh the active app.
    func show()
    /// Takes the window off the screen.
    func hide()
    /// Above other apps' windows (Keep on Top), or among them as a normal window.
    func setKeepOnTop(_ keepOnTop: Bool)
}

@MainActor
@Observable
final class MiniPlayerStore {
    /// The window is up.
    private(set) var shown = false
    /// Settings › Appearance › Mini Player, as the core keeps them (these defaults until it says).
    private(set) var keepOnTop = true
    private(set) var opensWhenWindowCloses = false
    private(set) var showsLyricsLine = true

    @ObservationIgnored private var host: MiniPlayerHost?

    /// The window to show the mini player in. Until one is attached the store only keeps the state.
    func attach(_ host: MiniPlayerHost) {
        self.host = host
        host.setKeepOnTop(keepOnTop)
        if shown {
            host.show()
        }
    }

    /// Opens the mini player (the bar's button, View › Open Mini Player).
    func open() {
        guard !shown else { return }
        shown = true
        host?.show()
    }

    /// Closes it (its own button, View › Close Mini Player).
    func close() {
        guard shown else { return }
        shown = false
        host?.hide()
    }

    func toggle() {
        if shown { close() } else { open() }
    }

    /// Follows the Mini Player settings.
    func apply(keepOnTop: Bool, opensWhenWindowCloses: Bool, showsLyricsLine: Bool) {
        if keepOnTop != self.keepOnTop {
            self.keepOnTop = keepOnTop
            host?.setKeepOnTop(keepOnTop)
        }
        self.opensWhenWindowCloses = opensWhenWindowCloses
        self.showsLyricsLine = showsLyricsLine
    }

    /// The main window closed: Open When the Window Is Closed opens the mini player while music
    /// plays.
    func mainWindowClosed(playing: Bool) {
        if opensWhenWindowCloses && playing {
            open()
        }
    }
}

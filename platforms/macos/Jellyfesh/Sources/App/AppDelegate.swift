// Application events SwiftUI does not cover: launch and quit (the core saves the session), files
// opened from Finder (audio plays, `.jfpkg` installs) and the Dock menu. About Jellyfesh is Settings ›
// About, in the main window (AppCommands).

import AppKit

// AppKit calls every method here, the Dock menu's actions too, on the main thread.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var model: AppModel { AppModel.shared }

    func applicationDidFinishLaunching(_ notification: Notification) {
        model.start()
    }

    func applicationWillTerminate(_ notification: Notification) {
        model.shutdown()
    }

    /// Music keeps playing with the window closed; the Dock icon brings it back.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    func application(_ application: NSApplication, open urls: [URL]) {
        model.open(urls)
    }

    func applicationDockMenu(_ sender: NSApplication) -> NSMenu? {
        let menu = NSMenu()
        let playback = model.playback
        if let now = playback.playing {
            let title = NSMenuItem(title: now.title, action: nil, keyEquivalent: "")
            title.isEnabled = false
            menu.addItem(title)
            if let artist = NowPlayingLines.secondLine(now) {
                let line = NSMenuItem(title: artist, action: nil, keyEquivalent: "")
                line.isEnabled = false
                menu.addItem(line)
            }
            menu.addItem(.separator())
        }
        let play = NSMenuItem(
            title: playback.isPlaying ? String(localized: "Pause") : String(localized: "Play"),
            action: #selector(togglePlayPause(_:)), keyEquivalent: ""
        )
        let next = NSMenuItem(title: String(localized: "Next"), action: #selector(nextTrack(_:)), keyEquivalent: "")
        let previous = NSMenuItem(title: String(localized: "Previous"), action: #selector(previousTrack(_:)),
                                  keyEquivalent: "")
        for item in [play, next, previous] {
            item.target = self
            item.isEnabled = playback.hasTrack || item === play
            menu.addItem(item)
        }
        return menu
    }

    @objc private func togglePlayPause(_ sender: Any?) {
        model.playback.togglePlayPause()
    }

    @objc private func nextTrack(_ sender: Any?) {
        model.playback.next()
    }

    @objc private func previousTrack(_ sender: Any?) {
        model.playback.previous()
    }
}

// Application events SwiftUI does not cover: launch and quit (the core saves the session), files
// opened from Finder (audio plays, `.jfpkg` installs), the Dock menu and the About panel.

import AppKit

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
        if playback.hasTrack, let now = playback.nowPlaying {
            let title = NSMenuItem(title: now.title, action: nil, keyEquivalent: "")
            title.isEnabled = false
            menu.addItem(title)
            if let artist = now.artist {
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

    /// The standard About panel with the design credits (docs/design-tokens.md, NOTICE).
    @MainActor
    static func showAbout() {
        let credits = NSMutableAttributedString()
        let lines = [
            String(localized: "An elegant, lightweight music player."),
            String(localized: "Designed with the Misty design language."),
            String(localized: "Design tokens derived from the Misty Design Language 1.1 tokens, © 2026 ZodiaCALs, Apache License 2.0."),
            String(localized: "Core version \(FFICoreClient.coreVersion)"),
        ]
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = .center
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: NSFont.smallSystemFontSize),
            .foregroundColor: NSColor.secondaryLabelColor,
            .paragraphStyle: paragraph,
        ]
        credits.append(NSAttributedString(string: lines.joined(separator: "\n"), attributes: attributes))
        NSApplication.shared.orderFrontStandardAboutPanel(options: [
            .applicationName: "Jellyfesh",
            .credits: credits,
        ])
        NSApplication.shared.activate()
    }
}

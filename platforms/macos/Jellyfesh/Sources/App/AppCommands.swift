// The menu bar: File (playlists, folders, plugins), View (sections, search, Now Playing, Desktop
// Lyrics) and Controls (transport, volume, shuffle and repeat), with the keys every Jellyfesh shares
// (docs/ui.md, "Keyboard"). Space (play and pause), Shift-Right and Shift-Left (skip 5 seconds)
// and Esc (back) go through SharedKeyMonitor instead of key equivalents, so text fields keep them;
// the skip items also answer Option-Command-arrows, as before.

import SwiftUI

struct AppCommands: Commands {
    let model: AppModel

    var body: some Commands {
        CommandGroup(replacing: .appInfo) {
            AboutMenuItem()
        }

        CommandGroup(replacing: .newItem) {
            FileMenuItems(model: model)
        }

        CommandGroup(after: .sidebar) {
            ViewMenuItems(model: model)
        }

        CommandMenu("Controls") {
            ControlsMenuItems(model: model)
        }
    }
}

/// About Jellyfesh: opens the About window (AboutView).
private struct AboutMenuItem: View {
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("About Jellyfesh") {
            openWindow(id: AboutView.windowID)
        }
    }
}

private struct FileMenuItems: View {
    let model: AppModel

    var body: some View {
        Button("New Playlist…") {
            // The window asks for the name, then opens the playlist the core makes.
            model.router.showNowPlaying(false, reduceMotion: true)
            model.library.askForNewPlaylist()
        }
        .keyboardShortcut("n")
        .disabled(model.startupError != nil)

        Divider()

        Button("Add Music Folder…") {
            FilePanels.chooseMusicFolders { urls in
                for url in urls { model.library.addFolder(url) }
            }
        }
        .keyboardShortcut("o", modifiers: [.command, .shift])
        .disabled(model.startupError != nil)

        Button("Import Playlist…") {
            FilePanels.choosePlaylist { url in
                model.library.importPlaylist(url.path)
            }
        }
        .disabled(model.startupError != nil)

        // Not in builds that may not install plugins (the App Store's).
        if model.sources.canInstallPlugins {
            Button("Install from File…") {
                FilePanels.choosePlugin { url in
                    model.sources.install(path: url.path)
                }
            }
            .disabled(model.startupError != nil)
        }
    }
}

private struct ViewMenuItems: View {
    let model: AppModel

    private struct Shortcut: Identifiable {
        var item: SidebarItem
        var key: KeyEquivalent
        var id: SidebarItem { item }
    }

    private let shortcuts: [Shortcut] = [
        Shortcut(item: .home, key: "1"), Shortcut(item: .albums, key: "2"), Shortcut(item: .artists, key: "3"),
        Shortcut(item: .songs, key: "4"), Shortcut(item: .playlists, key: "5"), Shortcut(item: .favorites, key: "6"),
        Shortcut(item: .sources, key: "7"),
    ]

    var body: some View {
        ForEach(shortcuts) { shortcut in
            Button(shortcut.item.title) {
                model.router.showNowPlaying(false, reduceMotion: true)
                model.router.select(shortcut.item)
            }
            .keyboardShortcut(shortcut.key)
        }

        Divider()

        Button("Search") {
            model.router.focusSearch()
        }
        .keyboardShortcut("f")

        Button(model.router.nowPlayingShown ? LocalizedStringKey("Hide Now Playing") : LocalizedStringKey("Show Now Playing")) {
            model.router.showNowPlaying(!model.router.nowPlayingShown, reduceMotion: false)
        }
        .keyboardShortcut("l")
        .disabled(!model.playback.hasTrack && !model.router.nowPlayingShown)

        Button("Lyrics") {
            model.router.showNowPlaying(true, panel: .lyrics, reduceMotion: false)
        }
        .keyboardShortcut("l", modifiers: [.command, .option])
        .disabled(!model.playback.hasTrack)

        Button("Up Next") {
            model.router.showNowPlaying(true, panel: .queue, reduceMotion: false)
        }
        .keyboardShortcut("u", modifiers: [.command, .option])
        .disabled(!model.playback.hasTrack)

        Divider()

        DesktopLyricsMenuItems(settings: model.settings, desktop: model.desktopLyrics)
            .disabled(model.startupError != nil)
    }
}

private struct ControlsMenuItems: View {
    let model: AppModel

    private var playback: PlaybackStore { model.playback }

    var body: some View {
        Button(playback.isPlaying ? LocalizedStringKey("Pause") : LocalizedStringKey("Play")) {
            playback.togglePlayPause()
        }
        .disabled(!playback.hasTrack && playback.queue.length == 0)

        Button("Stop") {
            playback.stop()
        }
        .keyboardShortcut(".")
        .disabled(!playback.hasTrack)

        Divider()

        Button("Next") {
            playback.next()
        }
        .keyboardShortcut(.rightArrow)
        .disabled(!playback.hasTrack)

        Button("Previous") {
            playback.previous()
        }
        .keyboardShortcut(.leftArrow)
        .disabled(!playback.hasTrack)

        Button("Skip Forward 5 Seconds") {
            playback.seek(byUs: 5_000_000)
        }
        .keyboardShortcut(.rightArrow, modifiers: [.command, .option])
        .disabled(!playback.hasTrack)

        Button("Skip Back 5 Seconds") {
            playback.seek(byUs: -5_000_000)
        }
        .keyboardShortcut(.leftArrow, modifiers: [.command, .option])
        .disabled(!playback.hasTrack)

        Divider()

        Button("Increase Volume") {
            playback.nudgeVolume(by: 0.05)
        }
        .keyboardShortcut(.upArrow)
        .disabled(!playback.showsVolume)

        Button("Decrease Volume") {
            playback.nudgeVolume(by: -0.05)
        }
        .keyboardShortcut(.downArrow)
        .disabled(!playback.showsVolume)

        Button(playback.muted ? LocalizedStringKey("Unmute") : LocalizedStringKey("Mute")) {
            playback.toggleMute()
        }
        .keyboardShortcut(.downArrow, modifiers: [.command, .option])
        .disabled(!playback.canMute)

        Divider()

        // The remote session's shuffle and repeat while one runs (the core routes the commands).
        Toggle("Shuffle", isOn: Binding(
            get: { playback.shuffle },
            set: { playback.setShuffle($0) }
        ))

        Picker("Repeat", selection: Binding(
            get: { playback.repeatMode },
            set: { playback.setRepeat($0) }
        )) {
            Text("Off").tag(RepeatOption.off)
            Text("All").tag(RepeatOption.all)
            Text("One").tag(RepeatOption.one)
        }

        Divider()

        Button("Go to Current Song") {
            model.router.showNowPlaying(true, reduceMotion: false)
        }
        .keyboardShortcut("j")
        .disabled(!playback.hasTrack)
    }
}

/// Desktop Lyrics on or off, and while it shows, its lock (a locked window lets clicks through,
/// so it is unlocked from here). In the View menu and Now Playing's More menu.
struct DesktopLyricsMenuItems: View {
    let settings: SettingsStore
    let desktop: DesktopLyricsStore

    var body: some View {
        Toggle("Desktop Lyrics", isOn: Binding(
            get: { settings.current.desktopLyrics },
            set: { settings.setDesktopLyrics($0) }
        ))
        if desktop.shown {
            Button(desktop.locked ? LocalizedStringKey("Unlock Desktop Lyrics") : LocalizedStringKey("Lock Desktop Lyrics")) {
                desktop.setLocked(!desktop.locked)
            }
        }
    }
}

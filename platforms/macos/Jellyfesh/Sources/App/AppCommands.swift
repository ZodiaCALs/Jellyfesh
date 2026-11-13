// The menu bar: File (playlists, folders, plugins), View (sections, search, Now Playing) and
// Controls (transport, volume, shuffle and repeat), with the shortcuts other Mac players use.
// Space plays and pauses through SpaceKeyMonitor, so text fields keep it.

import SwiftUI

struct AppCommands: Commands {
    let model: AppModel

    var body: some Commands {
        CommandGroup(replacing: .appInfo) {
            Button("About Jellyfesh") {
                AppDelegate.showAbout()
            }
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

private struct FileMenuItems: View {
    let model: AppModel

    var body: some View {
        Button("New Playlist") {
            model.library.createPlaylist(name: String(localized: "New Playlist"))
            model.router.showNowPlaying(false, reduceMotion: true)
            model.router.select(.playlists)
        }
        .keyboardShortcut("n")
        .disabled(model.startupError != nil)

        Divider()

        Button("Add Music Folder…") {
            FilePanels.chooseMusicFolders { urls in
                for url in urls { model.library.addFolder(url.path) }
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

        Button("Install Plugin…") {
            FilePanels.choosePlugin { url in
                model.sources.install(path: url.path)
            }
        }
        .disabled(model.startupError != nil)
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

        Button("Decrease Volume") {
            playback.nudgeVolume(by: -0.05)
        }
        .keyboardShortcut(.downArrow)

        Button(playback.muted ? LocalizedStringKey("Unmute") : LocalizedStringKey("Mute")) {
            playback.toggleMute()
        }
        .keyboardShortcut(.downArrow, modifiers: [.command, .option])

        Divider()

        Toggle("Shuffle", isOn: Binding(
            get: { playback.queue.shuffle },
            set: { playback.setShuffle($0) }
        ))

        Picker("Repeat", selection: Binding(
            get: { playback.queue.repeatMode },
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

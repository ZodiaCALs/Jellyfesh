// The app: one main window (the library, Settings, Now Playing and the player) and the menu bar
// commands. Settings and About are pages of the main window (ADR-0023): the app menu's Settings…
// (⌘,) and About Jellyfesh open them there.

import SwiftUI

@main
struct JellyfeshApp: App {
    /// The main window's scene id.
    static let mainWindowID = "main"

    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var model = AppModel.shared

    var body: some Scene {
        Window("Jellyfesh", id: Self.mainWindowID) {
            RootView()
                .stores(model)
        }
        .defaultSize(width: 1180, height: 760)
        .windowResizability(.contentMinSize)
        // The field fills the window under the system's title-bar controls (ADR-0023).
        .windowStyle(.hiddenTitleBar)
        .commands {
            AppCommands(model: model)
        }
    }
}

extension View {
    /// Every store, for the views of one window.
    func stores(_ model: AppModel) -> some View {
        environment(model)
            .environment(model.router)
            .environment(model.playback)
            .environment(model.library)
            .environment(model.settings)
            .environment(model.theme)
            .environment(model.lyrics)
            .environment(model.queue)
            .environment(model.search)
            .environment(model.sources)
            .environment(model.appleMusic)
            .environment(model.toasts)
            .environment(model.system)
            .environment(model.pro)
            .environment(model.updates)
            .environment(model.aiLyrics)
            .environment(model.look)
            .environment(model.desktopLyrics)
            .environment(model.miniPlayer)
            .environment(model.sourceLibrary)
            .environment(\.artworkLoader, model.artwork)
    }
}

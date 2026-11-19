// The app: one main window (library, Now Playing, the mini player), the Settings window, the About
// window and the menu bar commands.

import SwiftUI

@main
struct JellyfeshApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var model = AppModel.shared

    var body: some Scene {
        Window("Jellyfesh", id: "main") {
            RootView()
                .stores(model)
        }
        .defaultSize(width: 1180, height: 760)
        .windowResizability(.contentMinSize)
        .windowToolbarStyle(.unified(showsTitle: false))
        .commands {
            AppCommands(model: model)
        }

        Settings {
            SettingsView()
                .stores(model)
        }

        // About Jellyfesh in the app menu opens it; it is not listed in the Window menu.
        Window("About Jellyfesh", id: AboutView.windowID) {
            AboutView()
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .defaultPosition(.center)
        .commandsRemoved()
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
            .environment(\.artworkLoader, model.artwork)
    }
}

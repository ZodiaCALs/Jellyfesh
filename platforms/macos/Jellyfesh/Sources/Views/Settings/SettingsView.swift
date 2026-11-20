// The Settings window: Appearance (with the lyrics settings), Playback, Library, Sources and
// Extensions.
// Native forms (the window chrome is the platform's, Misty PLT-10); every change goes to the core
// as a settings command, and the app applies the appearance ones.

import SwiftUI

struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @Environment(ThemeStore.self) private var theme
    @Environment(SettingsStore.self) private var settings
    @Environment(SystemState.self) private var system
    @Environment(LookStore.self) private var look
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency

    var body: some View {
        @Bindable var model = model
        TabView(selection: $model.settingsTab) {
            AppearanceSettings()
                .tabItem { Label("Appearance", systemImage: "paintpalette") }
                .tag(SettingsTab.appearance)
            PlaybackSettings()
                .tabItem { Label("Playback", systemImage: "play.circle") }
                .tag(SettingsTab.playback)
            LibrarySettings()
                .tabItem { Label("Library", systemImage: "folder") }
                .tag(SettingsTab.library)
            SourcesSettings()
                .tabItem { Label("Sources", systemImage: "antenna.radiowaves.left.and.right") }
                .tag(SettingsTab.sources)
            ExtensionsSettings()
                .tabItem { Label("Extensions", systemImage: "puzzlepiece.extension") }
                .tag(SettingsTab.extensions)
        }
        .proSheet(in: .settings)
        .frame(width: 640, height: 560)
        .environment(\.palette, theme.palette(for: colorScheme))
        .environment(\.effects, Effects(
            reduceMotion: reduceMotion,
            solidGlass: reduceTransparency || system.lowPower || settings.current.reduceEffects
                || settings.current.transparency <= 0.001,
            reduceEffects: settings.current.reduceEffects,
            windowVisible: true,
            transparency: Double(settings.current.transparency)
        ))
        .environment(\.look, LookStyle(state: look.state))
    }
}

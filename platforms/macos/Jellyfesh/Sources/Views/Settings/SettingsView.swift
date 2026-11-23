// Settings, a page in the main window (ADR-0023, docs/ui.md "Settings"): the title, the sections
// Appearance, Playback, Library, Sources, Extensions and About as Jellyfesh Web's pill tabs, and
// the chosen section's groups in a column as wide as the web's (`layout.reading`). The groups are
// the native grouped forms over the field; the player bar floats over them like every page. ⌘,
// and the app menu's Settings… open the page, About Jellyfesh its About section. Every change goes
// to the core as a settings command, and the app applies the appearance ones.

import SwiftUI

struct SettingsPage: View {
    @Environment(AppModel.self) private var model
    @Environment(\.look) private var look

    /// Settings' column, as wide as the web's (`layout.reading`).
    static let readingWidth: CGFloat = Tokens.Layout.reading
    /// How far a grouped form insets its groups: the groups line up with the title.
    static let formInset: CGFloat = 20

    var body: some View {
        @Bindable var model = model
        let margin = look.spacing(Tokens.Space.s7)
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: look.spacing(Tokens.Space.s5)) {
                PageHeader(title: String(localized: "Settings"))
                PillTabs(
                    selection: $model.settingsTab,
                    options: SettingsTab.allCases.map { PillTabs<SettingsTab>.Option(value: $0, title: $0.title) },
                    label: "Settings"
                )
            }
            .frame(maxWidth: Self.readingWidth, alignment: .leading)
            .padding(.horizontal, margin)
            .padding(.top, look.spacing(Tokens.Space.s6))

            section(model.settingsTab)
                .scrollContentBackground(.hidden)
                .frame(maxWidth: Self.readingWidth + 2 * Self.formInset, maxHeight: .infinity, alignment: .top)
                .padding(.horizontal, max(0, margin - Self.formInset))
                .id(model.settingsTab)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .navigationTitle(Text("Settings"))
    }

    @ViewBuilder private func section(_ tab: SettingsTab) -> some View {
        switch tab {
        case .appearance: AppearanceSettings()
        case .playback: PlaybackSettings()
        case .library: LibrarySettings()
        case .sources: SourcesSettings()
        case .extensions: ExtensionsSettings()
        case .about: AboutSettings()
        }
    }
}

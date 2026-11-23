// Settings › About (docs/ui.md, "About"; the app menu's About Jellyfesh opens it): Keyboard
// Shortcuts, the keys every Jellyfesh shares with ⌘ for Ctrl; then the icon and name, the version
// from the build, what Jellyfesh is, the website, the third-party licences, the Licence Agreement
// and the Privacy Policy where the build knows where they are published (AboutInfo); the update
// check for direct downloads (ADR-0021 §5); and the design credits.

import AppKit
import SwiftUI

struct AboutSettings: View {
    @Environment(UpdatesStore.self) private var updates
    @Environment(\.palette) private var palette

    private let links = AboutInfo.links(website: AboutInfo.buildWebsite())
    private let version = AboutInfo.version(app: AboutInfo.appVersion(), core: FFICoreClient.coreVersion)

    var body: some View {
        SettingsCardsColumn {
            SettingsCard(title: "Keyboard Shortcuts") {
                ForEach(Array(AboutInfo.shortcuts.enumerated()), id: \.element.id) { index, shortcut in
                    if index > 0 {
                        SettingsDivider()
                    }
                    HStack(spacing: Tokens.Space.s4) {
                        KeyCaps(keys: shortcut.keys)
                            .frame(minWidth: 72, alignment: .leading)
                        Text(LocalizedStringKey(shortcut.action))
                            .lookFont(size: 14)
                            .foregroundStyle(palette[Tokens.Ink.ink90])
                        Spacer(minLength: 0)
                    }
                    .padding(.vertical, 10)
                    .accessibilityElement(children: .combine)
                }
            }

            SettingsCard(title: "About Jellyfesh") {
                HStack(spacing: Tokens.Space.s4) {
                    Image(nsImage: NSApplication.shared.applicationIconImage)
                        .resizable()
                        .frame(width: 64, height: 64)
                        .accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(verbatim: "Jellyfesh")
                            .lookFont(size: 17, weight: .bold)
                            .foregroundStyle(palette[Tokens.Ink.ink90])
                            .accessibilityAddTraits(.isHeader)
                        Text("Version \(version)")
                            .lookFont(size: 12)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                            .textSelection(.enabled)
                        Text("An elegant, lightweight music player.")
                            .lookFont(size: 13)
                            .foregroundStyle(palette[Tokens.Ink.ink90])
                    }
                }
                .padding(.vertical, Tokens.Space.s3)
                if let links {
                    SettingsDivider()
                    HStack(spacing: Tokens.Space.s4) {
                        Link("Website", destination: links.website)
                        Link("Third-Party Licences", destination: links.licences)
                        Link("Licence Agreement", destination: links.licenceAgreement)
                        Link("Privacy Policy", destination: links.privacyPolicy)
                    }
                    .lookFont(size: 13)
                    .padding(.vertical, Tokens.Space.s3)
                }
                if updates.state.supported {
                    SettingsDivider()
                    UpdateCheck()
                }
            }

            VStack(alignment: .leading, spacing: 2) {
                Text("Designed with the Misty design language.")
                Text("Design tokens derived from the Misty Design Language 1.1 tokens, © 2026 ZodiaCALs, Apache License 2.0.")
                if let copyright = AboutInfo.copyright() {
                    Text(copyright)
                }
            }
            .lookFont(size: 11)
            .foregroundStyle(palette[Tokens.Ink.ink70])
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, Tokens.Space.s6)
        }
    }
}

/// Keys as on a keyboard's caps (`⌘ →`).
private struct KeyCaps: View {
    var keys: String
    @Environment(\.palette) private var palette

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: Tokens.Radius.rSm, style: .continuous)
        Text(verbatim: keys)
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(palette[Tokens.Ink.ink90])
            .padding(.horizontal, Tokens.Space.s2)
            .frame(minHeight: 24)
            .background(shape.fill(palette[Tokens.Line.fill]))
            .overlay(shape.strokeBorder(palette[Tokens.Line.lineSoft], lineWidth: 1))
    }
}

/// Check for Updates (direct downloads): a newer release with View Release, the check, and whether
/// it runs by itself once a day.
private struct UpdateCheck: View {
    @Environment(UpdatesStore.self) private var updates
    @Environment(SettingsStore.self) private var settings
    @Environment(\.openURL) private var openURL

    var body: some View {
        let state = updates.state
        if let release = state.available {
            SettingsRow(title: Text(verbatim: UpdatesStore.availableTitle(release))) {
                if let url = UpdatesStore.releaseURL(release) {
                    Button("View Release") {
                        openURL(url)
                    }
                }
            }
            SettingsDivider()
        }
        SettingsRow(title: Text("Check for Updates"), help: checkHelp(state)) {
            HStack(spacing: Tokens.Space.s2) {
                if state.checking {
                    ProgressView()
                        .controlSize(.small)
                }
                Button("Check for Updates") {
                    updates.check()
                }
                .disabled(state.checking)
            }
        }
        SettingsDivider()
        SettingsToggleRow("Check for Updates Automatically", isOn: Binding(
            get: { settings.current.checkForUpdates },
            set: { settings.setCheckForUpdates($0) }
        ))
    }

    /// "Checking…" while it runs; afterwards why it failed, if it did.
    private func checkHelp(_ state: UpdateState) -> Text? {
        if state.checking { return Text("Checking…") }
        if let error = updates.failure ?? state.error { return Text(verbatim: error) }
        return nil
    }
}

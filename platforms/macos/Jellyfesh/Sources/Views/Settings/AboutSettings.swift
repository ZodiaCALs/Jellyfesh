// Settings › About (docs/ui.md, "About"; the app menu's About Jellyfesh opens it): Keyboard
// Shortcuts, the keys every Jellyfesh shares with ⌘ for Ctrl; then the icon and name, the version
// from the build, what Jellyfesh is, the website, the third-party licences, the Licence Agreement
// and the Privacy Policy where the build knows where they are published (AboutInfo); the update
// check for direct downloads (ADR-0021 §5); and the design credits.

import AppKit
import SwiftUI

struct AboutSettings: View {
    @Environment(UpdatesStore.self) private var updates

    private let links = AboutInfo.links(website: AboutInfo.buildWebsite())
    private let version = AboutInfo.version(app: AboutInfo.appVersion(), core: FFICoreClient.coreVersion)

    var body: some View {
        Form {
            Section {
                ForEach(AboutInfo.shortcuts) { shortcut in
                    LabeledContent {
                        KeyCaps(keys: shortcut.keys)
                    } label: {
                        Text(LocalizedStringKey(shortcut.action))
                    }
                    .accessibilityElement(children: .combine)
                }
            } header: {
                Text("Keyboard Shortcuts")
            }

            Section {
                HStack(spacing: Tokens.Space.s4) {
                    Image(nsImage: NSApplication.shared.applicationIconImage)
                        .resizable()
                        .frame(width: 64, height: 64)
                        .accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(verbatim: "Jellyfesh")
                            .font(.system(size: 17, weight: .bold))
                            .accessibilityAddTraits(.isHeader)
                        Text("Version \(version)")
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                        Text("An elegant, lightweight music player.")
                            .font(.system(size: 13))
                    }
                }
                .padding(.vertical, Tokens.Space.s1)
                if let links {
                    HStack(spacing: Tokens.Space.s4) {
                        Link("Website", destination: links.website)
                        Link("Third-Party Licences", destination: links.licences)
                        Link("Licence Agreement", destination: links.licenceAgreement)
                        Link("Privacy Policy", destination: links.privacyPolicy)
                    }
                }
            } header: {
                Text("About Jellyfesh")
            }

            if updates.state.supported {
                Section {
                    UpdateCheck()
                }
            }

            Section {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Designed with the Misty design language.")
                    Text("Design tokens derived from the Misty Design Language 1.1 tokens, © 2026 ZodiaCALs, Apache License 2.0.")
                    if let copyright = AboutInfo.copyright() {
                        Text(copyright)
                    }
                }
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            }
        }
        .formStyle(.grouped)
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
            LabeledContent {
                if let url = UpdatesStore.releaseURL(release) {
                    Button("View Release") {
                        openURL(url)
                    }
                }
            } label: {
                Text(UpdatesStore.availableTitle(release))
                    .font(.system(size: 13, weight: .semibold))
            }
        }
        HStack(spacing: Tokens.Space.s2) {
            Button("Check for Updates") {
                updates.check()
            }
            .disabled(state.checking)
            if state.checking {
                ProgressView()
                    .controlSize(.small)
                Text("Checking…")
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
        if let error = updates.failure ?? state.error, !state.checking {
            Text(error)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        Toggle("Check for Updates Automatically", isOn: Binding(
            get: { settings.current.checkForUpdates },
            set: { settings.setCheckForUpdates($0) }
        ))
    }
}

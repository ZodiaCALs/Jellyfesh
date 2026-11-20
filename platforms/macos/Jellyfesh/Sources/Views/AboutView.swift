// About Jellyfesh, from the app menu (docs/ui.md: on macOS About lives there): the icon and name,
// the version from the build, what Jellyfesh is, the website, the third-party licences, the Licence
// Agreement and the Privacy Policy where the build knows where they are published (AboutInfo), the
// update check for direct downloads (ADR-0021 §5), and the design credits.

import AppKit
import SwiftUI

struct AboutView: View {
    /// The About window's scene id (JellyfeshApp).
    static let windowID = "about"

    @Environment(UpdatesStore.self) private var updates

    private let links = AboutInfo.links(website: AboutInfo.buildWebsite())
    private let version = AboutInfo.version(app: AboutInfo.appVersion(), core: FFICoreClient.coreVersion)

    var body: some View {
        VStack(spacing: Tokens.Space.s3) {
            Image(nsImage: NSApplication.shared.applicationIconImage)
                .resizable()
                .frame(width: 96, height: 96)
                .accessibilityHidden(true)
            VStack(spacing: Tokens.Space.s1) {
                Text(verbatim: "Jellyfesh")
                    .font(.system(size: 20, weight: .bold))
                    .accessibilityAddTraits(.isHeader)
                Text("Version \(version)")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
            }
            Text("An elegant, lightweight music player.")
                .font(.system(size: 13))
                .multilineTextAlignment(.center)
            if let links {
                VStack(spacing: Tokens.Space.s1) {
                    HStack(spacing: Tokens.Space.s4) {
                        Link("Website", destination: links.website)
                        Link("Third-Party Licences", destination: links.licences)
                    }
                    HStack(spacing: Tokens.Space.s4) {
                        Link("Licence Agreement", destination: links.licenceAgreement)
                        Link("Privacy Policy", destination: links.privacyPolicy)
                    }
                }
                .font(.system(size: 13))
            }
            if updates.state.supported {
                UpdateCheck()
                    .padding(.top, Tokens.Space.s1)
            }
            VStack(spacing: 2) {
                Text("Designed with the Misty design language.")
                Text("Design tokens derived from the Misty Design Language 1.1 tokens, © 2026 ZodiaCALs, Apache License 2.0.")
                if let copyright = AboutInfo.copyright() {
                    Text(copyright)
                }
            }
            .font(.system(size: 11))
            .foregroundStyle(.secondary)
            .multilineTextAlignment(.center)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, Tokens.Space.s2)
        }
        .padding(.horizontal, Tokens.Space.s6)
        .padding(.top, Tokens.Space.s6)
        .padding(.bottom, Tokens.Space.s6)
        .frame(width: 340)
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
        VStack(spacing: Tokens.Space.s2) {
            if let release = state.available {
                HStack(spacing: Tokens.Space.s2) {
                    Text(UpdatesStore.availableTitle(release))
                        .font(.system(size: 13, weight: .semibold))
                    if let url = UpdatesStore.releaseURL(release) {
                        Button("View Release") {
                            openURL(url)
                        }
                    }
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
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
            }
            if let error = updates.failure ?? state.error, !state.checking {
                Text(error)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Toggle("Check for Updates Automatically", isOn: Binding(
                get: { settings.current.checkForUpdates },
                set: { settings.setCheckForUpdates($0) }
            ))
            .toggleStyle(.checkbox)
            .font(.system(size: 12))
        }
    }
}

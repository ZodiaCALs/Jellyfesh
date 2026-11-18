// About Jellyfesh, from the app menu (docs/ui.md: on macOS About lives there): the icon and name,
// the version from the build, what Jellyfesh is, the website and the third-party licences where the
// build knows where they are published (AboutInfo), and the design credits.

import AppKit
import SwiftUI

struct AboutView: View {
    /// The About window's scene id (JellyfeshApp).
    static let windowID = "about"

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
                HStack(spacing: Tokens.Space.s4) {
                    Link("Website", destination: links.website)
                    Link("Third-Party Licences", destination: links.licences)
                }
                .font(.system(size: 13))
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

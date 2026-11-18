// What About Jellyfesh shows (docs/ui.md, "About"): the version from the build, never typed in, and
// the website with the third-party licences it publishes, where the build knows the website. The
// same rules as the Android app's (AboutInfo in platforms/android/logic).

import Foundation

enum AboutInfo {
    /// The page the website publishes the third-party licences on (tools/release/build_site.py).
    static let noticesPage = "THIRD_PARTY_NOTICES.html"

    /// The website and its licences page.
    struct Links: Equatable {
        var website: URL
        var licences: URL
    }

    /// The links for the address the build gave (`JFWebsite`), with a slash added at its end; nil
    /// without an address, or for one that is not a web address.
    static func links(website: String?) -> Links? {
        let site = (website ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let lowered = site.lowercased()
        guard lowered.hasPrefix("https://") || lowered.hasPrefix("http://") else { return nil }
        let base = site.hasSuffix("/") ? site : site + "/"
        guard let home = URL(string: base), home.host != nil, let licences = URL(string: base + noticesPage)
        else { return nil }
        return Links(website: home, licences: licences)
    }

    /// The version line: the app's version, with the core's after it only when they differ (they
    /// are released together, so normally one number shows).
    static func version(app: String, core: String?) -> String {
        let app = app.trimmingCharacters(in: .whitespacesAndNewlines)
        let core = (core ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        guard !core.isEmpty, core != app else { return app }
        guard !app.isEmpty else { return core }
        return String(localized: "\(app) (core \(core))")
    }

    /// The website the build was given: `JF_WEBSITE` in project.yml, as the Info.plist key
    /// `JFWebsite` (scripts/build.sh sets it).
    static func buildWebsite(in bundle: Bundle = .main) -> String? {
        bundle.object(forInfoDictionaryKey: "JFWebsite") as? String
    }

    /// The app's version from the build (`CFBundleShortVersionString`, from MARKETING_VERSION).
    static func appVersion(in bundle: Bundle = .main) -> String {
        bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    }

    /// The copyright line of the build's Info.plist.
    static func copyright(in bundle: Bundle = .main) -> String? {
        guard let line = bundle.object(forInfoDictionaryKey: "NSHumanReadableCopyright") as? String,
              !line.isEmpty
        else { return nil }
        return line
    }
}

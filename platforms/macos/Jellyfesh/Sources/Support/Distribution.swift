// How this build reached the person (ADR-0021 §4): the build flag JF_DISTRIBUTION (project.yml,
// scripts/build.sh) as the Info.plist key JFDistribution: `source` (built from source, the
// default), `direct` (the release page's download) or `app-store`. The core takes it in its
// config (`CoreConfig.distribution`) and decides from it where Pro is sold, whether licence keys,
// updates and installing plugins from files are offered; the app uses it only to know whether
// StoreKit sells Pro here.

import Foundation

extension AppDistribution {
    /// The distribution an Info.plist value names; anything else is a build from source.
    static func from(infoValue: String?) -> AppDistribution {
        switch infoValue?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "direct"?: return .direct
        case "app-store"?, "appstore"?: return .appStore
        default: return .source
        }
    }

    /// This build's distribution.
    static func current(in bundle: Bundle = .main) -> AppDistribution {
        from(infoValue: bundle.object(forInfoDictionaryKey: "JFDistribution") as? String)
    }

    /// The Mac App Store build: Pro is bought with StoreKit, and music folders are kept as
    /// security-scoped bookmarks (the sandbox).
    var isAppStore: Bool { self == .appStore }
}

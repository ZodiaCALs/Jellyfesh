// Updates for direct downloads (ADR-0021 §5, docs/ui.md "About"): the `Update` slice and Check for
// Updates. The core checks GitHub's releases; nothing is installed. Store builds have no update
// check (the slice is not `supported` there).

import Foundation
import Observation

@MainActor
@Observable
final class UpdatesStore {
    let client: CoreClient
    private let tracker: CommandTracker

    private(set) var state: UpdateState = .unsupported
    /// Why the last Check for Updates failed as a command.
    private(set) var failure: String?

    init(client: CoreClient, tracker: CommandTracker) {
        self.client = client
        self.tracker = tracker
    }

    func apply(_ state: UpdateState) {
        self.state = state
    }

    /// Check for Updates.
    func check() {
        guard state.supported else { return }
        failure = nil
        let id = client.send(.checkForUpdates)
        tracker.watch(id) { [weak self] message in
            self?.failure = message
        }
    }

    /// "Jellyfesh 0.0.8 Is Available".
    static func availableTitle(_ release: ReleaseInfo) -> String {
        String(localized: "Jellyfesh \(release.version) Is Available")
    }

    /// The release page to open with View Release: `https` only.
    static func releaseURL(_ release: ReleaseInfo) -> URL? {
        guard let url = URL(string: release.url), url.scheme?.lowercased() == "https", url.host != nil else {
            return nil
        }
        return url
    }
}

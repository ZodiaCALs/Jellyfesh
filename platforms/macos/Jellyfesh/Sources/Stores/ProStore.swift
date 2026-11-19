// Jellyfesh Pro (ADR-0021, docs/ui.md "The Pro sheet"): the `Pro` slice, the Pro sheet that any
// Pro action opens while Pro is inactive, and licence keys. Where Pro comes from is the core's
// decision; the app reports what the App Store says (StoreKitPurchases) and sends the keys typed.

import Foundation
import Observation

@MainActor
@Observable
final class ProStore {
    let client: CoreClient
    private let tracker: CommandTracker

    private(set) var state: ProState = .inactive
    /// The Pro sheet is open.
    var sheetShown = false
    /// Enter Licence Key… is open.
    var licenceEntryShown = false
    /// A key was sent and the core has not answered yet.
    private(set) var activating = false
    /// Why the last licence command failed, when it failed as a command (the core words a refused
    /// key as `licenceProblem` instead).
    private(set) var failure: String?

    @ObservationIgnored private var sentKeyVersion: UInt64?

    init(client: CoreClient, tracker: CommandTracker) {
        self.client = client
        self.tracker = tracker
    }

    func apply(_ state: ProState) {
        self.state = state
        if let sent = sentKeyVersion, state.version > sent {
            // The core answered the key: active, or with the reason it was refused.
            activating = false
            sentKeyVersion = nil
            if state.active, state.licenceProblem == nil { licenceEntryShown = false }
        }
    }

    var isActive: Bool { state.active }

    /// For a Pro action: true when Pro is active and the action can go ahead; otherwise opens the
    /// Pro sheet and returns false.
    @discardableResult
    func require() -> Bool {
        guard !state.active else { return true }
        sheetShown = true
        return false
    }

    /// Activate: sends the key as typed (the core ignores case, spaces and dashes).
    func activate(key: String) {
        let trimmed = key.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        failure = nil
        activating = true
        sentKeyVersion = state.version
        let id = client.send(.activateLicence(key: trimmed))
        tracker.watch(id) { [weak self] message in
            self?.activating = false
            self?.sentKeyVersion = nil
            self?.failure = message
        }
    }

    /// Remove Licence (asked first by the view).
    func removeLicence() {
        failure = nil
        let id = client.send(.removeLicence)
        tracker.watch(id) { [weak self] message in
            self?.failure = message
        }
    }

    /// Clears what the last key's answer said, for a new key.
    func startLicenceEntry() {
        failure = nil
        licenceEntryShown = true
    }
}

/// Pro's words (docs/ui.md "AI Lyrics, Customize and Pro", "Settings › Extensions").
enum ProText {
    /// The line under Jellyfesh Pro in Settings: where Pro comes from, or what it holds.
    static func status(_ state: ProState) -> String {
        switch state.origin {
        case .preview: return String(localized: "Pro features are free during the public beta.")
        case .store: return String(localized: "Jellyfesh Pro · Purchased")
        case .licence(let suffix, _): return String(localized: "Jellyfesh Pro · Licence …\(suffix)")
        case .none: return String(localized: "AI Lyrics and Customize are part of Jellyfesh Pro.")
        }
    }

    /// Why a key was not accepted.
    static func problem(_ problem: LicenceProblem) -> String {
        switch problem {
        case .malformed: return String(localized: "That doesn't look like a Jellyfesh licence key.")
        case .unknownKey, .badSignature: return String(localized: "This licence key isn't valid.")
        case .wrongProduct: return String(localized: "This key is for another product.")
        case .revoked: return String(localized: "This licence key has been withdrawn.")
        }
    }

    /// Whether the licence can be removed: a key gave Pro.
    static func hasLicence(_ state: ProState) -> Bool {
        if case .licence = state.origin { return true }
        return false
    }
}

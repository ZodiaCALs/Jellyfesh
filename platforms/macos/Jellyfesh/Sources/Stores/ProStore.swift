// Jellyfesh Pro (ADR-0021, docs/ui.md "The Pro sheet"): the `Pro` slice, the Pro sheet that any
// Pro action opens while Pro is inactive, licence keys, and in the Mac App Store build Buy and
// Restore Purchases. Where Pro comes from is the core's decision: the app reports what the App
// Store says (StoreKitPurchases, behind ProPurchasing) and sends the keys typed.

import Foundation
import Observation

@MainActor
@Observable
final class ProStore {
    let client: CoreClient
    private let tracker: CommandTracker

    /// The window whose sheets show the Pro sheet and Enter Licence Key.
    enum SheetWindow {
        case main, settings
    }

    private(set) var state: ProState = .inactive
    /// The Pro sheet is open.
    var sheetShown = false
    /// Where the Pro sheet and Enter Licence Key show.
    private(set) var sheetWindow: SheetWindow = .main
    /// Enter Licence Key… is open.
    var licenceEntryShown = false
    /// A key was sent and the core has not answered yet.
    private(set) var activating = false
    /// Why the last licence command failed, when it failed as a command (the core words a refused
    /// key as `licenceProblem` instead).
    private(set) var failure: String?
    /// Buy's price from the App Store, once it is known (App Store builds).
    private(set) var price: String?
    /// A purchase or a restore is under way.
    private(set) var buying = false
    /// Why the last purchase or restore did not go through.
    private(set) var storeFailure: String?

    @ObservationIgnored private var sentKeyVersion: UInt64?
    @ObservationIgnored private var purchasing: ProPurchasing?

    init(client: CoreClient, tracker: CommandTracker) {
        self.client = client
        self.tracker = tracker
    }

    func apply(_ state: ProState) {
        self.state = state
        if state.active {
            // Nothing is left to sell.
            sheetShown = false
        }
        if let sent = sentKeyVersion, state.version > sent {
            // The core answered the key: active, or with the reason it was refused.
            activating = false
            sentKeyVersion = nil
            if state.active, state.licenceProblem == nil { licenceEntryShown = false }
        }
    }

    var isActive: Bool { state.active }

    /// For a Pro action: true when Pro is active and the action can go ahead; otherwise opens the
    /// Pro sheet in `window` and returns false.
    @discardableResult
    func require(in window: SheetWindow = .main) -> Bool {
        guard !state.active else { return true }
        sheetWindow = window
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

    /// Opens Enter Licence Key, clearing what the last key's answer said: in `window`, or where
    /// the Pro sheet is when it is asked from there.
    func startLicenceEntry(in window: SheetWindow? = nil) {
        failure = nil
        if let window {
            sheetWindow = window
        }
        licenceEntryShown = true
    }

    // MARK: The App Store

    /// Buy and Restore Purchases show: this build sells Pro in the App Store and can reach it.
    var sellsInAppStore: Bool { purchasing != nil && state.stores.contains(.appStore) }

    /// Starts the App Store side (App Store builds): reports what the person owns, then every
    /// change, and loads Buy's price.
    func attach(_ purchasing: ProPurchasing) async {
        self.purchasing = purchasing
        purchasing.listen { [weak self] entitlement in
            self?.report(entitlement)
        }
        report(await purchasing.currentEntitlement())
        price = await purchasing.price()
    }

    /// Buy Jellyfesh Pro. The sheet closes once the purchase completes; the core then turns Pro on.
    func buy() {
        guard let purchasing, !buying else { return }
        buying = true
        storeFailure = nil
        Task {
            do {
                if let entitlement = try await purchasing.purchase() {
                    report(entitlement)
                    sheetShown = false
                }
            } catch {
                storeFailure = error.localizedDescription
            }
            buying = false
        }
    }

    /// Restore Purchases: the App Store brings the person's purchases, and what they own is reported.
    func restorePurchases() {
        guard let purchasing, !buying else { return }
        buying = true
        storeFailure = nil
        Task {
            do {
                try await purchasing.sync()
                report(await purchasing.currentEntitlement())
            } catch {
                storeFailure = error.localizedDescription
            }
            buying = false
        }
    }

    private func report(_ entitlement: ProEntitlement) {
        Task { [client] in
            try? await client.setStoreEntitlement(entitlement)
        }
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

// Jellyfesh Pro in the Mac App Store build (ADR-0021 §4) through StoreKit 2: the product
// `org.jellyfesh.pro` and its price, Buy, Restore Purchases (AppStore.sync), what the person owns at
// start (Transaction.currentEntitlements) and every change after (Transaction.updates). Only
// transactions StoreKit verified count, and each is reported to the core, which decides whether Pro
// is active. Other builds sell Pro elsewhere and never call the App Store. ProStore works through
// `ProPurchasing`, so tests use a stand-in.

import Foundation
import StoreKit

/// The App Store's side of Jellyfesh Pro, or a stand-in in tests.
@MainActor
protocol ProPurchasing: AnyObject {
    /// The price Buy shows, nil when the App Store has no such product.
    func price() async -> String?
    /// Buys Pro: the entitlement once the purchase completes, nil when the person cancels or it
    /// waits for approval (Ask to Buy reports it later through `listen`).
    func purchase() async throws -> ProEntitlement?
    /// What the person owns now: a verified entitlement, or an inactive one.
    func currentEntitlement() async -> ProEntitlement
    /// Restore Purchases: has the App Store bring the person's purchases to this Mac (it may ask
    /// them to sign in).
    func sync() async throws
    /// Calls back with every verified change from now on.
    func listen(_ onChange: @escaping @MainActor (ProEntitlement) -> Void)
}

/// Why a purchase did not go through, in the person's words.
enum PurchaseFailure: LocalizedError, Equatable {
    case unavailable
    case unverified

    var errorDescription: String? {
        switch self {
        case .unavailable: return String(localized: "Jellyfesh Pro isn't available from the App Store right now.")
        case .unverified: return String(localized: "The App Store couldn't confirm this purchase.")
        }
    }
}

@MainActor
final class StoreKitPurchases: ProPurchasing {
    /// The product in App Store Connect.
    static let productID = "org.jellyfesh.pro"

    private var product: Product?
    /// Listens to Transaction.updates for as long as the app runs.
    private var updates: Task<Void, Never>?

    func price() async -> String? {
        await loadProduct()?.displayPrice
    }

    func purchase() async throws -> ProEntitlement? {
        guard let product = await loadProduct() else { throw PurchaseFailure.unavailable }
        switch try await product.purchase() {
        case .success(let result):
            guard case .verified(let transaction) = result else { throw PurchaseFailure.unverified }
            await transaction.finish()
            return Self.entitlement(transaction)
        case .userCancelled, .pending:
            return nil
        @unknown default:
            return nil
        }
    }

    func currentEntitlement() async -> ProEntitlement {
        for await result in StoreKit.Transaction.currentEntitlements {
            if case .verified(let transaction) = result, transaction.productID == Self.productID {
                return Self.entitlement(transaction)
            }
        }
        return ProEntitlement(store: .appStore, product: Self.productID, active: false, transaction: nil)
    }

    func sync() async throws {
        try await AppStore.sync()
    }

    func listen(_ onChange: @escaping @MainActor (ProEntitlement) -> Void) {
        updates?.cancel()
        updates = Task {
            for await result in StoreKit.Transaction.updates {
                guard case .verified(let transaction) = result, transaction.productID == Self.productID else { continue }
                await transaction.finish()
                onChange(Self.entitlement(transaction))
            }
        }
    }

    private func loadProduct() async -> Product? {
        if let product { return product }
        product = try? await Product.products(for: [Self.productID]).first
        return product
    }

    /// A verified transaction as the core takes it: active until it is revoked (a refund).
    private static func entitlement(_ transaction: StoreKit.Transaction) -> ProEntitlement {
        ProEntitlement(store: .appStore, product: transaction.productID, active: transaction.revocationDate == nil,
                       transaction: String(transaction.id))
    }
}

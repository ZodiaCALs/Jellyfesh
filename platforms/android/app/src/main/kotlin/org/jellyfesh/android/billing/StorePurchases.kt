package org.jellyfesh.android.billing

import android.app.Activity
import kotlinx.coroutines.flow.StateFlow
import org.jellyfesh.logic.core.Store

/**
 * Jellyfesh Pro in the store this build comes from (ADR-0021 §4): Google Play Billing in the
 * Google Play build, nothing elsewhere. What the store says goes to the core
 * (`set_store_entitlement`), which decides whether Pro is on; the Pro group and the Pro sheet
 * offer Buy and Restore Purchases only where the core lists this store in `ProSnapshot.stores`.
 */
interface StorePurchases {
    val store: Store

    /** The store's price for Jellyfesh Pro, formatted by the store; null until it has said. */
    val price: StateFlow<String?>

    /** Starts the store's purchase flow over [activity]. */
    fun buy(activity: Activity)

    /** Asks the store again for what the person owns (Restore Purchases, and on each return to the app). */
    fun restore()
}

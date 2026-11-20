package org.jellyfesh.android.billing

import android.app.Activity
import kotlinx.coroutines.flow.SharedFlow
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

    /** What the person is told about a purchase they asked for (shown as a short message). */
    val notices: SharedFlow<StoreNotice>

    /** Starts the store's purchase flow over [activity]. */
    fun buy(activity: Activity)

    /** Restore Purchases: asks the store what the person owns, and says so when it has nothing. */
    fun restore()

    /** Asks the store again quietly (whenever the app returns to the foreground). */
    fun refresh()
}

/** Messages about a purchase the person asked for. */
enum class StoreNotice {
    /** The payment is pending: Pro turns on once the store completes it. */
    Pending,

    /** Restore Purchases found no Jellyfesh Pro on this account. */
    NothingToRestore,

    /** The store could not do it now. */
    Failed,
}

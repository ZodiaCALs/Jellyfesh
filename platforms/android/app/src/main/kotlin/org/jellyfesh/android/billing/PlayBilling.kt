package org.jellyfesh.android.billing

import android.app.Activity
import android.content.Context
import com.android.billingclient.api.AcknowledgePurchaseParams
import com.android.billingclient.api.BillingClient
import com.android.billingclient.api.BillingClientStateListener
import com.android.billingclient.api.BillingFlowParams
import com.android.billingclient.api.BillingResult
import com.android.billingclient.api.PendingPurchasesParams
import com.android.billingclient.api.ProductDetails
import com.android.billingclient.api.Purchase
import com.android.billingclient.api.PurchasesUpdatedListener
import com.android.billingclient.api.QueryProductDetailsParams
import com.android.billingclient.api.QueryPurchasesParams
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import org.jellyfesh.logic.core.Store
import org.jellyfesh.logic.core.StoreEntitlement
import org.jellyfesh.logic.store.PlayPurchase
import org.jellyfesh.logic.store.PlayPurchases
import org.jellyfesh.logic.store.PlayState
import org.jellyfesh.logic.store.PurchaseSignature

/**
 * Jellyfesh Pro through Google Play Billing (Play Billing Library 9; the Google Play build only):
 * the one-time product `pro`. On start and whenever the app returns to the foreground it asks
 * Play what the person owns (`queryPurchasesAsync`), checks each purchase's signature with the
 * app's licence key ([licenceKey], BuildConfig.PLAY_LICENSE_KEY), reports the result to the core
 * ([report]: `set_store_entitlement`), which decides whether Pro is on, and then acknowledges
 * new purchases. Pending payments grant nothing until Play says they completed. Calls are made on
 * the main thread, where Play answers them.
 */
class PlayBilling(
    context: Context,
    private val licenceKey: String,
    private val report: (StoreEntitlement) -> Unit,
) : StorePurchases {
    override val store: Store = Store.GooglePlay

    private val _price = MutableStateFlow<String?>(null)
    override val price: StateFlow<String?> = _price.asStateFlow()

    private val _notices = MutableSharedFlow<StoreNotice>(extraBufferCapacity = 4)
    override val notices: SharedFlow<StoreNotice> = _notices.asSharedFlow()

    /** The last entitlement reported, so an unchanged answer is not sent again. */
    private var reported: StoreEntitlement? = null

    private val client: BillingClient = BillingClient.newBuilder(context)
        .setListener(PurchasesUpdatedListener { result, purchases -> onPurchasesUpdated(result, purchases) })
        .enablePendingPurchases(PendingPurchasesParams.newBuilder().enableOneTimeProducts().build())
        .enableAutoServiceReconnection()
        .build()

    private val productParams: QueryProductDetailsParams
        get() = QueryProductDetailsParams.newBuilder()
            .setProductList(
                listOf(
                    QueryProductDetailsParams.Product.newBuilder()
                        .setProductId(PlayPurchases.PRODUCT)
                        .setProductType(BillingClient.ProductType.INAPP)
                        .build(),
                ),
            )
            .build()

    /** Connects to Play once the core runs; then what the person owns and the price. */
    fun start() {
        client.startConnection(object : BillingClientStateListener {
            override fun onBillingSetupFinished(billingResult: BillingResult) {
                if (billingResult.responseCode == BillingClient.BillingResponseCode.OK) {
                    queryPurchases(userAsked = false)
                    queryPrice()
                }
            }

            // The client reconnects by itself (enableAutoServiceReconnection).
            override fun onBillingServiceDisconnected() = Unit
        })
    }

    override fun refresh() {
        if (client.isReady) queryPurchases(userAsked = false)
    }

    override fun restore() = queryPurchases(userAsked = true)

    override fun buy(activity: Activity) {
        // Fresh product details for each purchase: kept ones can make the flow fail.
        client.queryProductDetailsAsync(productParams) { result, details ->
            val product = details.productDetailsList.firstOrNull { it.productId == PlayPurchases.PRODUCT }
            if (result.responseCode != BillingClient.BillingResponseCode.OK || product == null) {
                _notices.tryEmit(StoreNotice.Failed)
                return@queryProductDetailsAsync
            }
            val offer = offer(product)
            _price.value = offer?.formattedPrice
            val item = BillingFlowParams.ProductDetailsParams.newBuilder().setProductDetails(product)
            offer?.offerToken?.let(item::setOfferToken)
            val flow = BillingFlowParams.newBuilder().setProductDetailsParamsList(listOf(item.build())).build()
            activity.runOnUiThread {
                when (client.launchBillingFlow(activity, flow).responseCode) {
                    BillingClient.BillingResponseCode.OK, BillingClient.BillingResponseCode.USER_CANCELED -> Unit
                    // Bought before (on this account): Restore Purchases finds it.
                    BillingClient.BillingResponseCode.ITEM_ALREADY_OWNED -> queryPurchases(userAsked = true)
                    else -> _notices.tryEmit(StoreNotice.Failed)
                }
            }
        }
    }

    private fun onPurchasesUpdated(result: BillingResult, purchases: List<Purchase>?) {
        when (result.responseCode) {
            BillingClient.BillingResponseCode.OK -> handle(purchases.orEmpty(), complete = false, userAsked = true)
            BillingClient.BillingResponseCode.USER_CANCELED -> Unit
            BillingClient.BillingResponseCode.ITEM_ALREADY_OWNED -> queryPurchases(userAsked = true)
            else -> _notices.tryEmit(StoreNotice.Failed)
        }
    }

    private fun queryPurchases(userAsked: Boolean) {
        val params = QueryPurchasesParams.newBuilder().setProductType(BillingClient.ProductType.INAPP).build()
        client.queryPurchasesAsync(params) { result, purchases ->
            if (result.responseCode == BillingClient.BillingResponseCode.OK) {
                handle(purchases, complete = true, userAsked = userAsked)
            } else if (userAsked) {
                _notices.tryEmit(StoreNotice.Failed)
            }
        }
    }

    private fun queryPrice() {
        client.queryProductDetailsAsync(productParams) { result, details ->
            if (result.responseCode == BillingClient.BillingResponseCode.OK) {
                _price.value = details.productDetailsList.firstOrNull { it.productId == PlayPurchases.PRODUCT }?.let(::offer)?.formattedPrice
            }
        }
    }

    /** The offer to buy: the first of a product with several, or its only one. */
    private fun offer(product: ProductDetails): ProductDetails.OneTimePurchaseOfferDetails? =
        product.oneTimePurchaseOfferDetailsList?.firstOrNull() ?: product.oneTimePurchaseOfferDetails

    private fun handle(purchases: List<Purchase>, complete: Boolean, userAsked: Boolean) {
        val read = purchases.map { purchase ->
            PlayPurchase(
                products = purchase.products,
                state = when (purchase.purchaseState) {
                    Purchase.PurchaseState.PURCHASED -> PlayState.Purchased
                    Purchase.PurchaseState.PENDING -> PlayState.Pending
                    else -> PlayState.Other
                },
                acknowledged = purchase.isAcknowledged,
                orderId = purchase.orderId,
                token = purchase.purchaseToken,
                verified = PurchaseSignature.verify(licenceKey, purchase.originalJson, purchase.signature),
            )
        }
        val entitlement = PlayPurchases.entitlement(read, complete)
        if (entitlement != null && entitlement != reported) {
            reported = entitlement
            report(entitlement)
        }
        // Acknowledged after the core has it; one that fails is tried again on the next query.
        PlayPurchases.toAcknowledge(read).forEach { token ->
            client.acknowledgePurchase(AcknowledgePurchaseParams.newBuilder().setPurchaseToken(token).build()) { }
        }
        when {
            !userAsked -> Unit
            PlayPurchases.pending(read) -> _notices.tryEmit(StoreNotice.Pending)
            complete && entitlement?.active != true -> _notices.tryEmit(StoreNotice.NothingToRestore)
        }
    }
}

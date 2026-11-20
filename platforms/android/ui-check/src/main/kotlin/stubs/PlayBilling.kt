// Stand-ins for the Google Play Billing Library 9 classes the app uses (Google's Maven only).
// Signatures follow developer.android.com/reference/com/android/billingclient/api (checked
// 2026-11-19): Java getters appear here as Kotlin properties, static builders as @JvmStatic
// companions and the response-code annotation interfaces as objects of constants.
@file:Suppress("unused", "UNUSED_PARAMETER")

package com.android.billingclient.api

import android.app.Activity
import android.content.Context

class BillingResult {
    val responseCode: Int get() = error("stub")
    val debugMessage: String get() = error("stub")
}

abstract class BillingClient {
    object BillingResponseCode {
        const val NETWORK_ERROR = 12
        const val SERVICE_TIMEOUT = -3
        const val FEATURE_NOT_SUPPORTED = -2
        const val SERVICE_DISCONNECTED = -1
        const val OK = 0
        const val USER_CANCELED = 1
        const val SERVICE_UNAVAILABLE = 2
        const val BILLING_UNAVAILABLE = 3
        const val ITEM_UNAVAILABLE = 4
        const val DEVELOPER_ERROR = 5
        const val ERROR = 6
        const val ITEM_ALREADY_OWNED = 7
        const val ITEM_NOT_OWNED = 8
    }

    object ProductType {
        const val INAPP = "inapp"
        const val SUBS = "subs"
    }

    class Builder {
        fun setListener(listener: PurchasesUpdatedListener): Builder = error("stub")
        fun enablePendingPurchases(params: PendingPurchasesParams): Builder = error("stub")
        fun enableAutoServiceReconnection(): Builder = error("stub")
        fun build(): BillingClient = error("stub")
    }

    abstract val isReady: Boolean

    abstract fun startConnection(listener: BillingClientStateListener)

    abstract fun endConnection()

    abstract fun queryProductDetailsAsync(params: QueryProductDetailsParams, listener: ProductDetailsResponseListener)

    abstract fun queryPurchasesAsync(queryPurchasesParams: QueryPurchasesParams, listener: PurchasesResponseListener)

    abstract fun acknowledgePurchase(params: AcknowledgePurchaseParams, listener: AcknowledgePurchaseResponseListener)

    abstract fun launchBillingFlow(activity: Activity, params: BillingFlowParams): BillingResult

    companion object {
        @JvmStatic
        fun newBuilder(context: Context): Builder = error("stub")
    }
}

fun interface PurchasesUpdatedListener {
    fun onPurchasesUpdated(billingResult: BillingResult, purchases: MutableList<Purchase>?)
}

interface BillingClientStateListener {
    fun onBillingSetupFinished(billingResult: BillingResult)
    fun onBillingServiceDisconnected()
}

fun interface ProductDetailsResponseListener {
    fun onProductDetailsResponse(billingResult: BillingResult, productDetailsResult: QueryProductDetailsResult)
}

fun interface PurchasesResponseListener {
    fun onQueryPurchasesResponse(billingResult: BillingResult, purchases: MutableList<Purchase>)
}

fun interface AcknowledgePurchaseResponseListener {
    fun onAcknowledgePurchaseResponse(billingResult: BillingResult)
}

class PendingPurchasesParams {
    class Builder {
        fun enableOneTimeProducts(): Builder = error("stub")
        fun build(): PendingPurchasesParams = error("stub")
    }

    companion object {
        @JvmStatic
        fun newBuilder(): Builder = error("stub")
    }
}

class QueryProductDetailsParams {
    class Product {
        class Builder {
            fun setProductId(productId: String): Builder = error("stub")
            fun setProductType(productType: String): Builder = error("stub")
            fun build(): Product = error("stub")
        }

        companion object {
            @JvmStatic
            fun newBuilder(): Builder = error("stub")
        }
    }

    class Builder {
        fun setProductList(productList: List<Product>): Builder = error("stub")
        fun build(): QueryProductDetailsParams = error("stub")
    }

    companion object {
        @JvmStatic
        fun newBuilder(): Builder = error("stub")
    }
}

class QueryProductDetailsResult {
    val productDetailsList: MutableList<ProductDetails> get() = error("stub")
    val unfetchedProductList: MutableList<UnfetchedProduct> get() = error("stub")
}

class UnfetchedProduct {
    val productId: String get() = error("stub")
}

class ProductDetails {
    val productId: String get() = error("stub")
    val oneTimePurchaseOfferDetails: OneTimePurchaseOfferDetails? get() = error("stub")
    val oneTimePurchaseOfferDetailsList: MutableList<OneTimePurchaseOfferDetails>? get() = error("stub")

    class OneTimePurchaseOfferDetails {
        val formattedPrice: String get() = error("stub")
        val offerToken: String get() = error("stub")
    }
}

class QueryPurchasesParams {
    class Builder {
        fun setProductType(productType: String): Builder = error("stub")
        fun build(): QueryPurchasesParams = error("stub")
    }

    companion object {
        @JvmStatic
        fun newBuilder(): Builder = error("stub")
    }
}

class Purchase {
    object PurchaseState {
        const val UNSPECIFIED_STATE = 0
        const val PURCHASED = 1
        const val PENDING = 2
    }

    val products: MutableList<String> get() = error("stub")
    val purchaseState: Int get() = error("stub")
    val isAcknowledged: Boolean get() = error("stub")
    val orderId: String? get() = error("stub")
    val originalJson: String get() = error("stub")
    val signature: String get() = error("stub")
    val purchaseToken: String get() = error("stub")
}

class AcknowledgePurchaseParams {
    class Builder {
        fun setPurchaseToken(purchaseToken: String): Builder = error("stub")
        fun build(): AcknowledgePurchaseParams = error("stub")
    }

    companion object {
        @JvmStatic
        fun newBuilder(): Builder = error("stub")
    }
}

class BillingFlowParams {
    class ProductDetailsParams {
        class Builder {
            fun setProductDetails(productDetails: ProductDetails): Builder = error("stub")
            fun setOfferToken(offerToken: String): Builder = error("stub")
            fun build(): ProductDetailsParams = error("stub")
        }

        companion object {
            @JvmStatic
            fun newBuilder(): Builder = error("stub")
        }
    }

    class Builder {
        fun setProductDetailsParamsList(productDetailsParamsList: List<ProductDetailsParams>): Builder = error("stub")
        fun build(): BillingFlowParams = error("stub")
    }

    companion object {
        @JvmStatic
        fun newBuilder(): Builder = error("stub")
    }
}

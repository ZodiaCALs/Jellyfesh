package org.jellyfesh.logic.store

import java.security.KeyFactory
import java.security.PublicKey
import java.security.Signature
import java.security.spec.X509EncodedKeySpec
import java.util.Base64
import org.jellyfesh.logic.core.Store
import org.jellyfesh.logic.core.StoreEntitlement

/**
 * Google Play's signature over a purchase (`Purchase.getOriginalJson()`, signed with the
 * RSASSA-PKCS1-v1_5 scheme), checked with the app's licence key: the base64 RSA public key from
 * Play Console (Monetization setup › Licensing), built in as BuildConfig.PLAY_LICENSE_KEY. A build
 * without the key verifies nothing, so no purchase grants Pro there.
 */
object PurchaseSignature {
    /** Play has signed with SHA-1; SHA-256 is accepted as well. */
    private val ALGORITHMS = listOf("SHA1withRSA", "SHA256withRSA")

    fun publicKey(base64: String): PublicKey? {
        val bytes = runCatching { Base64.getDecoder().decode(base64.filterNot(Char::isWhitespace)) }.getOrNull() ?: return null
        if (bytes.isEmpty()) return null
        return runCatching { KeyFactory.getInstance("RSA").generatePublic(X509EncodedKeySpec(bytes)) }.getOrNull()
    }

    fun verify(publicKeyBase64: String, signedData: String, signatureBase64: String): Boolean {
        if (signedData.isEmpty() || signatureBase64.isBlank()) return false
        val key = publicKey(publicKeyBase64) ?: return false
        val signature = runCatching { Base64.getDecoder().decode(signatureBase64.trim()) }.getOrNull() ?: return false
        return ALGORITHMS.any { algorithm ->
            runCatching {
                Signature.getInstance(algorithm).run {
                    initVerify(key)
                    update(signedData.toByteArray(Charsets.UTF_8))
                    verify(signature)
                }
            }.getOrDefault(false)
        }
    }
}

/** A purchase's state as Google Play reports it. */
enum class PlayState { Purchased, Pending, Other }

/** What the app read from one Google Play purchase ([verified]: its signature checked out). */
data class PlayPurchase(
    val products: List<String>,
    val state: PlayState,
    val acknowledged: Boolean,
    val orderId: String?,
    val token: String,
    val verified: Boolean,
)

/**
 * Jellyfesh Pro on Google Play (ADR-0021 §4; docs/store-release.md): the one-time product `pro`.
 * The app reports what Play says to the core with `set_store_entitlement`, at start and on every
 * change, and acknowledges a purchase once the core has it (Play refunds purchases left
 * unacknowledged for three days). A pending payment grants nothing until Play says it completed.
 */
object PlayPurchases {
    const val PRODUCT = "pro"

    /** The purchases that grant Pro: Jellyfesh Pro's, purchased (not pending) and signed by Google Play. */
    fun granting(purchases: List<PlayPurchase>): List<PlayPurchase> =
        purchases.filter { PRODUCT in it.products && it.state == PlayState.Purchased && it.verified }

    /**
     * What to report: owned, from a granting purchase; or, after a full query of what the person
     * owns ([complete]), not owned when none grants it. An update that carries no granting
     * purchase says nothing about what was owned before.
     */
    fun entitlement(purchases: List<PlayPurchase>, complete: Boolean): StoreEntitlement? {
        val owned = granting(purchases).firstOrNull()
        return when {
            owned != null -> StoreEntitlement(Store.GooglePlay, PRODUCT, active = true, transaction = owned.orderId)
            complete -> StoreEntitlement(Store.GooglePlay, PRODUCT, active = false, transaction = null)
            else -> null
        }
    }

    /** The purchase tokens to acknowledge once the core has the entitlement. */
    fun toAcknowledge(purchases: List<PlayPurchase>): List<String> = granting(purchases).filterNot { it.acknowledged }.map { it.token }

    /** Jellyfesh Pro waits for a payment to complete (the person is told so). */
    fun pending(purchases: List<PlayPurchase>): Boolean = purchases.any { PRODUCT in it.products && it.state == PlayState.Pending }
}

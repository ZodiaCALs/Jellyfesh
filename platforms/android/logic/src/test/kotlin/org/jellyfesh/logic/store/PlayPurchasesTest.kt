package org.jellyfesh.logic.store

import java.security.KeyPairGenerator
import java.security.Signature
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.Store
import org.jellyfesh.logic.core.StoreEntitlement

class PurchaseSignatureTest {
    private val pair = KeyPairGenerator.getInstance("RSA").apply { initialize(2048) }.generateKeyPair()
    private val other = KeyPairGenerator.getInstance("RSA").apply { initialize(2048) }.generateKeyPair()
    private val licenceKey = Base64.getEncoder().encodeToString(pair.public.encoded)
    private val json = """{"orderId":"GPA.1234-5678-9012-34567","productId":"pro","purchaseState":0,"purchaseToken":"token"}"""

    private fun sign(data: String, algorithm: String = "SHA1withRSA", key: java.security.PrivateKey = pair.private): String =
        Base64.getEncoder().encodeToString(Signature.getInstance(algorithm).run { initSign(key); update(data.toByteArray()); sign() })

    @Test
    fun aPurchaseSignedWithTheAppsKeyVerifies() {
        assertTrue(PurchaseSignature.verify(licenceKey, json, sign(json)))
        assertTrue(PurchaseSignature.verify(licenceKey, json, sign(json, "SHA256withRSA")))
        // The key as Play Console shows it may be wrapped.
        assertTrue(PurchaseSignature.verify(licenceKey.chunked(64).joinToString("\n"), json, sign(json)))
    }

    @Test
    fun anythingElseDoesNot() {
        assertFalse(PurchaseSignature.verify(licenceKey, json.replace("pro", "pr0"), sign(json)))
        assertFalse(PurchaseSignature.verify(licenceKey, json, sign(json, key = other.private)))
        assertFalse(PurchaseSignature.verify(licenceKey, json, "not base64!"))
        assertFalse(PurchaseSignature.verify(licenceKey, json, ""))
        // A build without the licence key grants nothing.
        assertFalse(PurchaseSignature.verify("", json, sign(json)))
        assertFalse(PurchaseSignature.verify("bm90IGEga2V5", json, sign(json)))
    }
}

class PlayPurchasesTest {
    private fun purchase(
        state: PlayState = PlayState.Purchased,
        acknowledged: Boolean = false,
        verified: Boolean = true,
        products: List<String> = listOf("pro"),
        token: String = "t1",
    ) = PlayPurchase(products, state, acknowledged, orderId = if (state == PlayState.Purchased) "GPA.1" else null, token = token, verified = verified)

    @Test
    fun aVerifiedPurchaseOfProGrantsItAndIsAcknowledged() {
        val bought = purchase()
        assertEquals(StoreEntitlement(Store.GooglePlay, "pro", true, "GPA.1"), PlayPurchases.entitlement(listOf(bought), complete = false))
        assertEquals(listOf("t1"), PlayPurchases.toAcknowledge(listOf(bought)))
        assertEquals(emptyList(), PlayPurchases.toAcknowledge(listOf(bought.copy(acknowledged = true))))
    }

    @Test
    fun pendingUnverifiedOrOtherPurchasesGrantNothing() {
        val pending = purchase(state = PlayState.Pending)
        val forged = purchase(verified = false)
        val other = purchase(products = listOf("tip"))
        for (p in listOf(pending, forged, other)) {
            assertNull(PlayPurchases.entitlement(listOf(p), complete = false))
            assertEquals(StoreEntitlement(Store.GooglePlay, "pro", false, null), PlayPurchases.entitlement(listOf(p), complete = true))
            assertEquals(emptyList(), PlayPurchases.toAcknowledge(listOf(p)))
        }
        assertTrue(PlayPurchases.pending(listOf(pending)))
        assertFalse(PlayPurchases.pending(listOf(forged)))
    }

    @Test
    fun onlyAFullQueryReportsProAsNotOwned() {
        assertNull(PlayPurchases.entitlement(emptyList(), complete = false))
        assertEquals(StoreEntitlement(Store.GooglePlay, "pro", false, null), PlayPurchases.entitlement(emptyList(), complete = true))
    }
}

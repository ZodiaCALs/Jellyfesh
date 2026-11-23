package org.jellyfesh.logic.sources

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginKind
import org.jellyfesh.logic.core.PluginSummary

class LocalNetworkTest {
    private val jellyfin = PluginSummary(
        id = "org.jellyfesh.jellyfin",
        name = "Jellyfin",
        version = "1.3.0",
        author = "Jellyfesh",
        badge = PluginBadge.Official,
        vendorPlayback = false,
        capabilities = listOf("auth", "search", "browse", "resolve_stream"),
        authMethods = listOf(AuthMethod.PasswordExchange),
        enabled = true,
        account = "listener",
        needsDisclosure = false,
        settingsSchemaJson = "{}",
        settingsJson = "{}",
        icon = null,
        localNetwork = true,
    )
    private val spotify = jellyfin.copy(id = "org.jellyfesh.spotify", name = "Spotify", localNetwork = false)

    @Test
    fun onlyAndroid17AndLaterEnforceIt() {
        assertFalse(LocalNetwork.enforced(36))
        assertTrue(LocalNetwork.enforced(37))
        assertTrue(LocalNetwork.enforced(38))
        assertEquals("android.permission.ACCESS_LOCAL_NETWORK", LocalNetwork.PERMISSION)
    }

    @Test
    fun theGateAsksOnlyForALocalAddressOnAndroid17WithoutThePermission() {
        assertTrue(LocalNetwork.needsPermission(local = true, sdkInt = 37, granted = false))
        assertFalse(LocalNetwork.needsPermission(local = true, sdkInt = 37, granted = true))
        assertFalse(LocalNetwork.needsPermission(local = false, sdkInt = 37, granted = false))
        // Android 16 grants it with INTERNET: never ask there.
        assertFalse(LocalNetwork.needsPermission(local = true, sdkInt = 36, granted = false))
    }

    @Test
    fun localSourcesAreTheEnabledSourcesTheCoreCallsLocal() {
        val off = jellyfin.copy(id = "off", enabled = false)
        val extension = jellyfin.copy(id = "ai", kind = PluginKind.Extension)
        assertEquals(listOf(jellyfin), LocalNetwork.localSources(listOf(spotify, off, extension, jellyfin)))
        // Until the bindings carry the core's flag, nothing is local and nothing is asked.
        assertFalse(PluginSummary("x", "X", "1", "a", PluginBadge.Official, false, emptyList(), emptyList(), true, null, false, "{}", "{}", null).localNetwork)
    }

    @Test
    fun accessFollowsAndroidsAnswerAndWhetherItWillAskAgain() {
        fun access(sdk: Int = 37, anyLocal: Boolean = true, granted: Boolean = false, asked: Boolean = false, rationale: Boolean = false) =
            LocalNetwork.access(sdk, anyLocal, granted, asked, rationale)
        assertEquals(LocalNetworkAccess.NotNeeded, access(sdk = 36))
        assertEquals(LocalNetworkAccess.NotNeeded, access(anyLocal = false))
        assertEquals(LocalNetworkAccess.Granted, access(granted = true, asked = true))
        assertEquals(LocalNetworkAccess.NotAsked, access())
        assertEquals(LocalNetworkAccess.Denied, access(asked = true, rationale = true))
        // Revoked in Android's settings, or the Nearby devices group denied before: Android wants the reason.
        assertEquals(LocalNetworkAccess.Denied, access(asked = false, rationale = true))
        assertEquals(LocalNetworkAccess.Blocked, access(asked = true, rationale = false))
    }

    @Test
    fun afterTheDialogADenialEitherAllowsAnotherAskOrLeavesTheSettingsPage() {
        assertEquals(LocalNetworkAccess.Granted, LocalNetwork.afterAnswer(granted = true, rationale = false))
        assertEquals(LocalNetworkAccess.Denied, LocalNetwork.afterAnswer(granted = false, rationale = true))
        assertEquals(LocalNetworkAccess.Blocked, LocalNetwork.afterAnswer(granted = false, rationale = false))
    }

    @Test
    fun theFullerReasonFollowsADenialAndSettingsOnlyOnceAndroidStopsAsking() {
        assertEquals(LocalNetworkReason.Short, LocalNetwork.reason(LocalNetworkAccess.NotAsked))
        assertEquals(LocalNetworkReason.Full, LocalNetwork.reason(LocalNetworkAccess.Denied))
        assertEquals(LocalNetworkReason.Full, LocalNetwork.reason(LocalNetworkAccess.Blocked))
        assertEquals(LocalNetworkStep.Ask, LocalNetwork.step(LocalNetworkAccess.NotAsked))
        assertEquals(LocalNetworkStep.Ask, LocalNetwork.step(LocalNetworkAccess.Denied))
        assertEquals(LocalNetworkStep.OpenSettings, LocalNetwork.step(LocalNetworkAccess.Blocked))
        assertNull(LocalNetwork.step(LocalNetworkAccess.Granted))
        assertNull(LocalNetwork.step(LocalNetworkAccess.NotNeeded))
    }

    @Test
    fun itAsksByItselfOncePerInstallAndNeverAfterADenial() {
        assertTrue(LocalNetwork.asksAtStart(LocalNetworkAccess.NotAsked, introduced = false))
        assertFalse(LocalNetwork.asksAtStart(LocalNetworkAccess.NotAsked, introduced = true))
        assertFalse(LocalNetwork.asksAtStart(LocalNetworkAccess.Denied, introduced = false))
        assertFalse(LocalNetwork.asksAtStart(LocalNetworkAccess.Granted, introduced = false))
    }

    @Test
    fun theNoticeNamesTheLocalSourcesUntilAccessIsGranted() {
        assertEquals(LocalNetworkNotice(listOf("Jellyfin"), LocalNetworkStep.Ask), LocalNetwork.notice(LocalNetworkAccess.Denied, listOf(jellyfin)))
        assertEquals(LocalNetworkNotice(listOf("Jellyfin"), LocalNetworkStep.OpenSettings), LocalNetwork.notice(LocalNetworkAccess.Blocked, listOf(jellyfin)))
        assertNull(LocalNetwork.notice(LocalNetworkAccess.Granted, listOf(jellyfin)))
        assertNull(LocalNetwork.notice(LocalNetworkAccess.Denied, emptyList()))
    }
}

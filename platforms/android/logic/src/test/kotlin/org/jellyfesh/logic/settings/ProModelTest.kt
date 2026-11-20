package org.jellyfesh.logic.settings

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.AiLyricsSnapshot
import org.jellyfesh.logic.core.AudioConsent
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.LicenceError
import org.jellyfesh.logic.core.LookSnapshot
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginKind
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.ProSnapshot
import org.jellyfesh.logic.core.ProSource
import org.jellyfesh.logic.core.Store

class ProModelTest {
    private val inactive = ProSnapshot.Inactive.copy(version = 3)
    private val googlePlay = inactive.copy(stores = listOf(Store.GooglePlay), licenceKeys = false)

    @Test
    fun theProGroupSaysWherePreComesFrom() {
        assertEquals(ProLine.Offer, ProModel.line(inactive))
        assertEquals(ProLine.Preview, ProModel.line(inactive.copy(active = true, source = ProSource.Preview, preview = true)))
        assertEquals(ProLine.Purchased, ProModel.line(inactive.copy(active = true, source = ProSource.Purchased(Store.GooglePlay))))
        assertEquals(ProLine.Licence("ABCD"), ProModel.line(inactive.copy(active = true, source = ProSource.Licence("ABCD", 1L))))
    }

    @Test
    fun storeBuildsSellThroughTheStoreAndGooglePlayTakesNoKeys() {
        // Google Play: Buy and Restore Purchases, no Enter Licence Key….
        assertTrue(ProModel.sellsIn(googlePlay, Store.GooglePlay))
        assertFalse(ProModel.takesLicenceKeys(googlePlay))
        // The direct download: keys only.
        assertFalse(ProModel.sellsIn(inactive, null))
        assertFalse(ProModel.sellsIn(inactive, Store.GooglePlay))
        assertTrue(ProModel.takesLicenceKeys(inactive))
        // Nothing to buy or enter once Pro is on.
        val on = googlePlay.copy(active = true, source = ProSource.Purchased(Store.GooglePlay))
        assertFalse(ProModel.sellsIn(on, Store.GooglePlay))
        assertFalse(ProModel.takesLicenceKeys(on.copy(licenceKeys = true)))
        assertTrue(ProModel.allows(on))
        assertFalse(ProModel.allows(googlePlay))
    }

    @Test
    fun onlyAKeyCanBeRemoved() {
        assertTrue(ProModel.canRemoveLicence(inactive.copy(active = true, source = ProSource.Licence("ABCD", 1L))))
        assertFalse(ProModel.canRemoveLicence(inactive.copy(active = true, source = ProSource.Purchased(Store.GooglePlay))))
        assertFalse(ProModel.canRemoveLicence(inactive))
    }

    @Test
    fun aKeyIsSentAsTypedAndTheNextSliceAnswersIt() {
        assertNull(LicenceEntries.send(inactive, "   "))
        val (entry, command) = LicenceEntries.send(inactive, "  jfp1-abcd-efgh ")!!
        assertEquals(Command.ActivateLicence("jfp1-abcd-efgh"), command)
        val sent = LicenceEntries.sent(entry, 41)
        assertEquals(LicenceAnswer.Idle, LicenceEntries.answer(LicenceEntry(), inactive))
        assertEquals(LicenceAnswer.Waiting, LicenceEntries.answer(sent, inactive))
        assertEquals(
            LicenceAnswer.Accepted,
            LicenceEntries.answer(sent, inactive.copy(version = 4, active = true, source = ProSource.Licence("EFGH", 2L))),
        )
        assertEquals(
            LicenceAnswer.Refused(LicenceError.Malformed),
            LicenceEntries.answer(sent, inactive.copy(version = 4, licenceError = LicenceError.Malformed)),
        )
        // A newer slice that says nothing about the key: still waiting.
        assertEquals(LicenceAnswer.Waiting, LicenceEntries.answer(sent, inactive.copy(version = 4)))
    }

    @Test
    fun aFailedCommandIsTheKeysOnlyWhenItIsItsCommand() {
        val sent = LicenceEntries.sent(LicenceEntries.send(inactive, "JFP1-X")!!.first, 41)
        assertEquals(sent, LicenceEntries.failed(sent, 40, "not yet"))
        val failed = LicenceEntries.failed(sent, 41, "Licence keys are not taken in this build.")
        assertEquals(LicenceAnswer.Failed("Licence keys are not taken in this build."), LicenceEntries.answer(failed, inactive))
    }
}

class ExtensionsTest {
    private fun plugin(id: String, capabilities: List<String>, kind: PluginKind = PluginKind.Extension, requiresPro: Boolean = true) = PluginSummary(
        id = id,
        name = id,
        version = "0.0.7",
        author = "Jellyfesh",
        badge = PluginBadge.Official,
        vendorPlayback = false,
        capabilities = capabilities,
        authMethods = emptyList(),
        enabled = true,
        account = null,
        needsDisclosure = false,
        settingsSchemaJson = "{}",
        settingsJson = "{}",
        icon = null,
        kind = kind,
        requiresPro = requiresPro,
    )

    private val subsonic = plugin("org.jellyfesh.subsonic", listOf("auth", "search", "browse"), PluginKind.Source, requiresPro = false)
    private val aiLyrics = plugin("org.jellyfesh.ai-lyrics", listOf("auth", "make_lyrics"))
    private val customize = plugin("org.jellyfesh.customize", listOf("appearance"))
    private val plugins = listOf(subsonic, aiLyrics, customize)

    @Test
    fun extensionsAreThePluginsThatOfferNoMusic() {
        assertEquals(listOf(aiLyrics, customize), Extensions.list(plugins))
        assertTrue(Extensions.makesLyrics(aiLyrics))
        assertTrue(Extensions.changesTheLook(customize))
        assertFalse(Extensions.changesTheLook(aiLyrics))
    }

    @Test
    fun customizeAndAiLyricsAreTheSlicesOrTheFirstInstalled() {
        assertEquals(customize, Extensions.customize(plugins, LookSnapshot.Default))
        val other = plugin("org.example.look", listOf("appearance"))
        assertEquals(other, Extensions.customize(plugins + other, LookSnapshot.Default.copy(plugin = "org.example.look")))
        assertNull(Extensions.customize(listOf(subsonic), LookSnapshot.Default))
        assertEquals(aiLyrics, Extensions.aiLyrics(plugins, AiLyricsSnapshot.Empty))
        assertEquals(aiLyrics, Extensions.aiLyrics(plugins, AiLyricsSnapshot.Empty.copy(plugin = "org.jellyfesh.ai-lyrics")))
    }

    @Test
    fun theProBadgeShowsWhileProIsInactive() {
        assertTrue(Extensions.showsProBadge(customize, ProSnapshot.Inactive))
        assertFalse(Extensions.showsProBadge(customize, ProSnapshot.Inactive.copy(active = true, source = ProSource.Preview)))
        assertFalse(Extensions.showsProBadge(subsonic, ProSnapshot.Inactive))
    }

    @Test
    fun sendingAudioListsThisPluginsOrigins() {
        val ai = AiLyricsSnapshot.Empty.copy(
            consents = listOf(AudioConsent("org.jellyfesh.ai-lyrics", "https://api.openai.com"), AudioConsent("org.example.other", "https://x.example")),
        )
        assertEquals(listOf("https://api.openai.com"), Extensions.consents(ai, "org.jellyfesh.ai-lyrics").map { it.origin })
    }
}

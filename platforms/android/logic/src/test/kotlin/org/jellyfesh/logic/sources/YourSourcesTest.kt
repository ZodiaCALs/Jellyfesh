package org.jellyfesh.logic.sources

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.nav.Screen

class YourSourcesTest {
    private val server = PluginSummary(
        id = "org.jellyfesh.subsonic",
        name = "Subsonic",
        version = "1.2.0",
        author = "Jellyfesh",
        badge = PluginBadge.Official,
        vendorPlayback = false,
        capabilities = listOf("auth", "search", "browse", "resolve_stream"),
        authMethods = listOf(AuthMethod.PasswordExchange),
        enabled = true,
        account = null,
        needsDisclosure = false,
        settingsSchemaJson = "{}",
        settingsJson = "{}",
        icon = null,
    )

    @Test
    fun choosingASourceThatIsOnAndBrowsesOpensItsCatalogue() {
        assertEquals(Screen.SourceBrowse("org.jellyfesh.subsonic", null, "Subsonic"), YourSources.opens(server))
        assertTrue(YourSources.canBrowse(server))
    }

    @Test
    fun aSourceThatIsOffOrCannotBrowseOpensItsPage() {
        assertEquals(Screen.Source("org.jellyfesh.subsonic"), YourSources.opens(server.copy(enabled = false)))
        val lyricsOnly = server.copy(id = "lyrics", capabilities = listOf("lyrics"))
        assertFalse(YourSources.canBrowse(lyricsOnly))
        assertEquals(Screen.Source("lyrics"), YourSources.opens(lyricsOnly))
    }

    @Test
    fun theLineSaysOffSignedInOrNotSignedInAndNothingWithoutSignIn() {
        assertEquals(SourceState.NotSignedIn, YourSources.state(server))
        assertEquals(SourceState.SignedIn, YourSources.state(server.copy(account = "listener")))
        assertEquals(SourceState.Off, YourSources.state(server.copy(enabled = false, account = "listener")))
        assertEquals(SourceState.Ready, YourSources.state(server.copy(authMethods = emptyList())))
    }
}

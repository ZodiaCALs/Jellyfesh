package org.jellyfesh.logic.state

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.jellyfesh.logic.core.Command

/** What the core is told about the network (ADR-0025, ADR-0026 §1). */
class NetworkReportTest {
    private val wifi = NetworkFacts(connected = true, internet = true, notMetered = true)
    private val mobile = NetworkFacts(connected = true, internet = true, notMetered = false)

    @Test
    fun meteredUnlessAndroidSaysOtherwise() {
        assertEquals(Command.SetNetworkState(online = true, metered = false), NetworkReport.command(wifi))
        assertEquals(Command.SetNetworkState(online = true, metered = true), NetworkReport.command(mobile))
    }

    @Test
    fun offlineWithoutANetworkThatReachesTheInternet() {
        assertEquals(Command.SetNetworkState(online = false, metered = false), NetworkReport.command(NetworkFacts.None))
        // A captive or local-only network (no internet capability) is not online.
        assertEquals(Command.SetNetworkState(online = false, metered = false), NetworkReport.command(mobile.copy(internet = false)))
    }

    @Test
    fun onlyChangesAreSent() {
        assertEquals(Command.SetNetworkState(online = true, metered = true), NetworkReport.changed(null, mobile))
        assertNull(NetworkReport.changed(Command.SetNetworkState(online = true, metered = true), mobile))
        assertEquals(Command.SetNetworkState(online = true, metered = false), NetworkReport.changed(Command.SetNetworkState(online = true, metered = true), wifi))
    }
}

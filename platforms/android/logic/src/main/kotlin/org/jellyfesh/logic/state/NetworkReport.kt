package org.jellyfesh.logic.state

import org.jellyfesh.logic.core.Command

/** The default network as Android describes it, reduced to what the core is told. */
data class NetworkFacts(
    /** A default network exists. */
    val connected: Boolean,
    /** It reaches the internet (`NET_CAPABILITY_INTERNET`). */
    val internet: Boolean,
    /** `NET_CAPABILITY_NOT_METERED`, or Android 11's `TEMPORARILY_NOT_METERED` (an unmetered 5G plan). */
    val notMetered: Boolean,
) {
    companion object {
        val None = NetworkFacts(connected = false, internet = false, notMetered = false)
    }
}

/**
 * The network the core is told (`Command.SetNetworkState`; ADR-0025, ADR-0026 §1): online when the
 * default network reaches the internet, metered unless Android says it is not. Data Saver only
 * restricts metered networks, so it needs nothing more. The app sends it at start and whenever it
 * changes; the core decides Auto quality (High while metered) and whether downloads wait.
 */
object NetworkReport {
    fun command(facts: NetworkFacts): Command.SetNetworkState {
        val online = facts.connected && facts.internet
        return Command.SetNetworkState(online = online, metered = online && !facts.notMetered)
    }

    /** What to send for [facts] when it differs from what was sent last ([last]); null otherwise. */
    fun changed(last: Command.SetNetworkState?, facts: NetworkFacts): Command.SetNetworkState? = command(facts).takeIf { it != last }
}

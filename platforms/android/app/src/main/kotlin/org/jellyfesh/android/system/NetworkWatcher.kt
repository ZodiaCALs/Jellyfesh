package org.jellyfesh.android.system

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Build
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.state.NetworkFacts
import org.jellyfesh.logic.state.NetworkReport

/**
 * Tells the core about the default network ([NetworkReport]), at start and whenever Android reports
 * a change through ConnectivityManager's default network callback, so Auto quality and downloads
 * follow it. Only changes are sent. Needs `ACCESS_NETWORK_STATE`.
 */
class NetworkWatcher(context: Context, private val send: (Command) -> Unit) {
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)
    private var last: Command.SetNetworkState? = null
    private val callback = object : ConnectivityManager.NetworkCallback() {
        // Android delivers the capabilities right after a network becomes the default (API 26+).
        override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) = report(facts(capabilities))

        override fun onLost(network: Network) = report(NetworkFacts.None)
    }

    fun start() {
        report(current())
        runCatching { connectivity.registerDefaultNetworkCallback(callback) }
    }

    fun stop() {
        runCatching { connectivity.unregisterNetworkCallback(callback) }
    }

    private fun current(): NetworkFacts {
        val network = connectivity.activeNetwork ?: return NetworkFacts.None
        val capabilities = connectivity.getNetworkCapabilities(network) ?: return NetworkFacts.None.copy(connected = true)
        return facts(capabilities)
    }

    private fun facts(capabilities: NetworkCapabilities) = NetworkFacts(
        connected = true,
        internet = capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET),
        notMetered = capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED) ||
            (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_TEMPORARILY_NOT_METERED)),
    )

    // The callback runs on ConnectivityManager's thread, the first report on the caller's.
    @Synchronized
    private fun report(facts: NetworkFacts) {
        NetworkReport.changed(last, facts)?.let {
            last = it
            send(it)
        }
    }
}

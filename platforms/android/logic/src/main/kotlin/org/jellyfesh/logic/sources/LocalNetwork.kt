package org.jellyfesh.logic.sources

import org.jellyfesh.logic.core.PluginKind
import org.jellyfesh.logic.core.PluginSummary

/** Where Android's Local Network permission stands for Jellyfesh ([LocalNetwork.access]). */
enum class LocalNetworkAccess {
    /** Android 16 or earlier, or no enabled source on the home network: nothing to ask. */
    NotNeeded,

    /** Granted (on its own, or with another permission of the Nearby devices group). */
    Granted,

    /** Jellyfesh has never asked on this install. */
    NotAsked,

    /** Denied or revoked, and Android shows its dialog again when asked. */
    Denied,

    /** Denied, and Android no longer shows its dialog: only the app's settings page turns it on. */
    Blocked,
}

/** What the Local Network sheet says before Android's dialog ([LocalNetwork.reason]). */
enum class LocalNetworkReason {
    /** The first time: the source is on the home network and Android asks first. */
    Short,

    /** After a denial: what stops working without it, and that nothing else is reached. */
    Full,
}

/** What a notice's button does ([LocalNetwork.step]). */
enum class LocalNetworkStep {
    /** The Local Network sheet, then Android's dialog. */
    Ask,

    /** The app's page in Android's settings, where Nearby devices is turned on. */
    OpenSettings,
}

/**
 * The notice shown while sources on the home network cannot be reached ([LocalNetwork.notice]):
 * the sources' names and what its button does.
 */
data class LocalNetworkNotice(val names: List<String>, val step: LocalNetworkStep)

/**
 * Android 17's Local Network permission (`ACCESS_LOCAL_NETWORK`, in the Nearby devices group).
 * An app that targets API 37 reaches servers on the home network only once it is granted:
 * without it TCP connections time out and UDP fails with EPERM
 * (developer.android.com/privacy-and-security/local-network-permission).
 *
 * The core says which addresses are local ([PluginSummary.localNetwork], and the address typed in
 * Add a Source); this decides only when to ask, what to say and what the person can do after a
 * denial. The answer is advice: Jellyfesh connects whatever it is, because a server reached
 * through a VPN needs no permission.
 *
 * After a denial Jellyfesh asks again only when the person chooses to, with the fuller reason.
 * Android resets its denial count for this permission ("permission request reset counter"), so
 * its dialog shows again; `shouldShowRequestPermissionRationale` says whether it will.
 */
object LocalNetwork {
    /** The permission, by its constant value (API 37). */
    const val PERMISSION = "android.permission.ACCESS_LOCAL_NETWORK"

    /** Android 17, the first version that enforces it for apps targeting it. */
    const val FIRST_SDK = 37

    /** Whether this device enforces the permission. Earlier versions grant it with INTERNET. */
    fun enforced(sdkInt: Int): Boolean = sdkInt >= FIRST_SDK

    /**
     * The one gate the core's word feeds: whether connecting to an address the core calls local
     * ([local]) needs the permission first, on a device at [sdkInt] where it is [granted] or not.
     */
    fun needsPermission(local: Boolean, sdkInt: Int, granted: Boolean): Boolean = local && enforced(sdkInt) && !granted

    /** The enabled music sources whose addresses the core calls local, in their order. */
    fun localSources(plugins: List<PluginSummary>): List<PluginSummary> =
        plugins.filter { it.kind == PluginKind.Source && it.enabled && it.localNetwork }

    /**
     * Where the permission stands: [anyLocal] whether a source needs it, [granted] Android's
     * answer now, [asked] whether Jellyfesh asked on this install, [rationale] what
     * `shouldShowRequestPermissionRationale` says (Android shows its dialog again).
     */
    fun access(sdkInt: Int, anyLocal: Boolean, granted: Boolean, asked: Boolean, rationale: Boolean): LocalNetworkAccess = when {
        !enforced(sdkInt) || !anyLocal -> LocalNetworkAccess.NotNeeded
        granted -> LocalNetworkAccess.Granted
        rationale -> LocalNetworkAccess.Denied
        !asked -> LocalNetworkAccess.NotAsked
        else -> LocalNetworkAccess.Blocked
    }

    /** Where it stands after Android's dialog answered ([rationale] read right after). */
    fun afterAnswer(granted: Boolean, rationale: Boolean): LocalNetworkAccess = when {
        granted -> LocalNetworkAccess.Granted
        rationale -> LocalNetworkAccess.Denied
        else -> LocalNetworkAccess.Blocked
    }

    /** What the sheet says before Android's dialog: the fuller reason once the person said no. */
    fun reason(access: LocalNetworkAccess): LocalNetworkReason =
        if (access == LocalNetworkAccess.Denied || access == LocalNetworkAccess.Blocked) LocalNetworkReason.Full else LocalNetworkReason.Short

    /** What asking does now: the sheet and Android's dialog, or the settings page once Android stops asking. */
    fun step(access: LocalNetworkAccess): LocalNetworkStep? = when (access) {
        LocalNetworkAccess.NotNeeded, LocalNetworkAccess.Granted -> null
        LocalNetworkAccess.NotAsked, LocalNetworkAccess.Denied -> LocalNetworkStep.Ask
        LocalNetworkAccess.Blocked -> LocalNetworkStep.OpenSettings
    }

    /**
     * Whether Jellyfesh asks by itself when it opens: once per install ([introduced] after the
     * first time), and only before Android's dialog was ever shown. Later it waits for the person.
     */
    fun asksAtStart(access: LocalNetworkAccess, introduced: Boolean): Boolean = access == LocalNetworkAccess.NotAsked && !introduced

    /**
     * The notice on the Sources page, Settings › Sources and a local source's pages while access
     * is missing: the names of [sources] (the local ones) and its button. Null once granted, or
     * when nothing needs it.
     */
    fun notice(access: LocalNetworkAccess, sources: List<PluginSummary>): LocalNetworkNotice? {
        val step = step(access) ?: return null
        if (sources.isEmpty()) return null
        return LocalNetworkNotice(sources.map { it.name }, step)
    }
}

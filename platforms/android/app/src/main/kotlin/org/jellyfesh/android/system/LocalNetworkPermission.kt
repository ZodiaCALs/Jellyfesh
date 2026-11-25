package org.jellyfesh.android.system

import android.app.Activity
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import org.jellyfesh.logic.sources.LocalNetwork
import org.jellyfesh.logic.sources.LocalNetworkAccess

/**
 * Android 17's Local Network permission as the device reports it (the rules are
 * [LocalNetwork]'s): whether it is granted, whether Jellyfesh asked on this install, whether
 * Android shows its dialog again, and the app's settings page for when it does not.
 *
 * The two remembered facts live in the app's own preferences: they describe this install's
 * permission prompts, which the core knows nothing about.
 */
class LocalNetworkPermission(private val context: Context) {
    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** Whether this device enforces the permission (Android 17 and later). */
    val enforced: Boolean get() = LocalNetwork.enforced(Build.VERSION.SDK_INT)

    /** Granted, or not needed on this Android version. */
    fun granted(): Boolean =
        !enforced || context.checkSelfPermission(LocalNetwork.PERMISSION) == PackageManager.PERMISSION_GRANTED

    /** Whether Jellyfesh has shown Android's dialog on this install. */
    var asked: Boolean
        get() = prefs.getBoolean(KEY_ASKED, false)
        set(value) = prefs.edit().putBoolean(KEY_ASKED, value).apply()

    /** Whether Jellyfesh has explained it once by itself, when it opened. */
    var introduced: Boolean
        get() = prefs.getBoolean(KEY_INTRODUCED, false)
        set(value) = prefs.edit().putBoolean(KEY_INTRODUCED, value).apply()

    /** Whether Android shows its dialog again, with a reason first (`shouldShowRequestPermissionRationale`). */
    fun rationale(activity: Activity?): Boolean =
        enforced && activity?.shouldShowRequestPermissionRationale(LocalNetwork.PERMISSION) == true

    /** Where the permission stands while [anyLocal] sources need it. */
    fun access(activity: Activity?, anyLocal: Boolean): LocalNetworkAccess =
        LocalNetwork.access(Build.VERSION.SDK_INT, anyLocal, granted(), asked, rationale(activity))

    /** Opens Jellyfesh's page in Android's settings, where Permissions › Nearby devices turns it on. */
    fun openSettings(activity: Activity?) = openAppSettings(context, activity)

    private companion object {
        const val PREFS = "permissions"
        const val KEY_ASKED = "local_network_asked"
        const val KEY_INTRODUCED = "local_network_introduced"
    }
}

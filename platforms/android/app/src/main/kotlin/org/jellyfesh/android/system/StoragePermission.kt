package org.jellyfesh.android.system

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import org.jellyfesh.logic.library.StorageAccess

/**
 * Android 8 and 9's storage permission for downloads as the device reports it (the rules are
 * [StorageAccess]'s): whether it is granted, whether Jellyfesh asked on this install, and whether
 * Android shows its dialog again. Whether it asked lives in the app's own preferences.
 */
class StoragePermission(private val context: Context) {
    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** Granted, or not needed on this Android version. */
    fun granted(): Boolean =
        !StorageAccess.required(Build.VERSION.SDK_INT) || context.checkSelfPermission(StorageAccess.PERMISSION) == PackageManager.PERMISSION_GRANTED

    /** Whether Jellyfesh has shown Android's dialog on this install. */
    var asked: Boolean
        get() = prefs.getBoolean(KEY_ASKED, false)
        set(value) = prefs.edit().putBoolean(KEY_ASKED, value).apply()

    /** Whether Android shows its dialog again (`shouldShowRequestPermissionRationale`). */
    fun rationale(activity: Activity?): Boolean =
        StorageAccess.required(Build.VERSION.SDK_INT) && activity?.shouldShowRequestPermissionRationale(StorageAccess.PERMISSION) == true

    private companion object {
        const val PREFS = "permissions"
        const val KEY_ASKED = "storage_asked"
    }
}

/** Opens Jellyfesh's page in Android's settings, where its permissions are turned on. */
fun openAppSettings(context: Context, activity: Activity?) {
    val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null))
    if (activity == null) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        (activity ?: context).startActivity(intent)
    } catch (_: ActivityNotFoundException) {
        // A device without the page: the notice stays, which is all that can be done.
    }
}

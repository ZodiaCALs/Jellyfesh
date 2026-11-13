package org.jellyfesh.android.system

import android.app.ActivityManager
import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import org.jellyfesh.logic.settings.AppIcon
import org.jellyfesh.logic.settings.AppIcons

/**
 * Tells the core when output devices come and go (ADR-0009: Android's AudioDeviceCallback,
 * forwarded as `RefreshDevices`; the core never polls).
 */
class DeviceWatcher(context: Context, private val onChange: () -> Unit) {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val handler = Handler(Looper.getMainLooper())
    private val callback = object : AudioDeviceCallback() {
        override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>) = onChange()
        override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) = onChange()
    }

    fun start() = audio.registerAudioDeviceCallback(callback, handler)

    fun stop() = audio.unregisterAudioDeviceCallback(callback)
}

/**
 * The launcher icon (ADR-0017): one `activity-alias` per icon, exactly one enabled. Switching
 * enables the new alias before disabling the others, without killing the app; some launchers
 * take a few seconds to refresh, which the picker says next to it.
 */
class IconSwitcher(private val context: Context) {
    private val packages: PackageManager = context.packageManager

    /** The icon whose alias is enabled now. */
    fun current(): AppIcon = AppIcon.entries.firstOrNull { isEnabled(it) } ?: AppIcon.Ice

    /** Makes the launcher show the icon for [setting] (`automatic` or an icon id). */
    fun apply(setting: String) {
        val target = AppIcons.launcherIcon(setting)
        if (current() == target && isEnabled(target)) return
        set(target, PackageManager.COMPONENT_ENABLED_STATE_ENABLED)
        AppIcon.entries.filter { it != target }.forEach { set(it, PackageManager.COMPONENT_ENABLED_STATE_DISABLED) }
    }

    private fun component(icon: AppIcon) = ComponentName(context.packageName, AppIcons.aliasClass(NAMESPACE, icon))

    private fun isEnabled(icon: AppIcon): Boolean = when (packages.getComponentEnabledSetting(component(icon))) {
        PackageManager.COMPONENT_ENABLED_STATE_ENABLED -> true
        PackageManager.COMPONENT_ENABLED_STATE_DEFAULT -> icon == AppIcon.Ice // the manifest default
        else -> false
    }

    private fun set(icon: AppIcon, state: Int) {
        packages.setComponentEnabledSetting(component(icon), state, PackageManager.DONT_KILL_APP)
    }

    private companion object {
        /** The manifest's namespace (the aliases' class names), independent of the applicationId. */
        const val NAMESPACE = "org.jellyfesh.android"
    }
}

/** How much glass the device can afford (ADR-0005). */
enum class GlassTier {
    /** API 33+: Cloudy backdrop blur, progressive where asked. */
    Progressive,

    /** API 31-32: uniform blur; progressive edges become a gradient scrim. */
    Uniform,

    /** API < 31: no blur; Misty Cloud at the dialog glass alpha (86 %), so the field shows faintly. */
    Translucent,

    /** Low-RAM, battery saver or reduce effects: solid Misty Cloud, no blur (MAT-08). */
    Solid,
}

/** The device-side inputs of motion and glass decisions. */
object DeviceEffects {
    fun glassTier(context: Context, reduceEffects: Boolean): GlassTier {
        val activity = context.getSystemService(ActivityManager::class.java)
        val power = context.getSystemService(PowerManager::class.java)
        return when {
            reduceEffects -> GlassTier.Solid
            activity?.isLowRamDevice == true -> GlassTier.Solid
            power?.isPowerSaveMode == true -> GlassTier.Solid
            Build.VERSION.SDK_INT < Build.VERSION_CODES.S -> GlassTier.Translucent
            Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU -> GlassTier.Uniform
            else -> GlassTier.Progressive
        }
    }

    /** The system's "remove animations" (animator duration scale 0). */
    fun systemReducesMotion(context: Context): Boolean =
        Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
}

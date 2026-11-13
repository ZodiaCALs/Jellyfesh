package org.jellyfesh.android.plugins

import android.content.Context
import java.io.File

/**
 * The official Subsonic and Jellyfin plugins that ship inside every APK (the .jfpkg files in
 * assets/plugins, packed by the app's build from dist/plugins). The core installs plugins from paths, so the
 * packages are copied into the app's files once per install or update of the app; their paths go
 * to the core's config, which installs them on first start and updates them with the app. The same
 * packages are also offered as separate downloads, which update one between releases.
 */
object BundledPlugins {
    private const val ASSETS = "plugins"
    private const val STAMP = ".app-version"

    /** The bundled packages' paths, copied out of the APK when the app is new or was updated. */
    fun prepare(context: Context): List<String> {
        val names = runCatching { context.assets.list(ASSETS) }.getOrNull().orEmpty().filter { it.endsWith(".jfpkg") }.sorted()
        val dir = File(context.filesDir, "bundled-plugins")
        if (names.isEmpty()) {
            dir.deleteRecursively()
            return emptyList()
        }
        val version = appVersion(context)
        val stamp = File(dir, STAMP)
        val files = names.map { File(dir, it) }
        val current = runCatching { stamp.readText() }.getOrNull()
        if (current != version || files.any { !it.isFile }) {
            dir.deleteRecursively()
            dir.mkdirs()
            for (name in names) {
                context.assets.open("$ASSETS/$name").use { input -> File(dir, name).outputStream().use { input.copyTo(it) } }
            }
            stamp.writeText(version)
        }
        return files.map { it.path }
    }

    /** Changes with every install or update of the APK. */
    @Suppress("DEPRECATION") // getPackageInfo(String, Int) works on every API level we support.
    private fun appVersion(context: Context): String =
        runCatching { context.packageManager.getPackageInfo(context.packageName, 0).lastUpdateTime.toString() }.getOrDefault("unknown")
}

package org.jellyfesh.android.ffi

import org.jellyfesh.ffi.CoreConfig
import org.jellyfesh.ffi.Distribution
import org.jellyfesh.ffi.OutputKind

/**
 * The core's configuration on Android: the system's default audio output, the official plugins
 * that ship inside the APK ([bundledPlugins]: paths of their `.jfpkg` files, installed on first
 * start and updated with the app), how this build was distributed ([distribution]) and whether
 * Resume on Launch may start playback as the core starts ([resumeOnStart]: false on Android, where
 * the process may start without its window; the app sends Play itself, ResumeOnLaunch).
 *
 * `CoreConfig.bundledPlugins` is being added to the core and the FFI while this is written
 * (Rust `Config::bundled_plugins`). It is named here and nowhere else, so a different name after
 * the first CI build is fixed in this one file. Until the generated bindings have the field, the
 * builds compile src/ffi-config-compat instead (settings.gradle.kts decides).
 */
internal fun androidCoreConfig(
    dataDir: String,
    cacheDir: String,
    watchFolders: Boolean,
    bundledPlugins: List<String>,
    distribution: Distribution,
    resumeOnStart: Boolean,
): CoreConfig =
    CoreConfig(
        dataDir = dataDir,
        cacheDir = cacheDir,
        output = OutputKind.Device(name = null),
        watchFolders = watchFolders,
        bundledPlugins = bundledPlugins,
        distribution = distribution,
        resumeOnStart = resumeOnStart,
    )

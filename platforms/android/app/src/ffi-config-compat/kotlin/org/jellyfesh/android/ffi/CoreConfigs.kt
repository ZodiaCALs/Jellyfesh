package org.jellyfesh.android.ffi

import org.jellyfesh.ffi.CoreConfig
import org.jellyfesh.ffi.OutputKind

// Stand-in for src/ffi-config/.../CoreConfigs.kt while the generated bindings have no
// `CoreConfig.bundledPlugins` yet (the core and FFI are gaining it): the core then starts without
// the bundled plugins. settings.gradle.kts picks one of the two for :app, :ffi-check and :ui-check.
@Suppress("UNUSED_PARAMETER")
internal fun androidCoreConfig(
    dataDir: String,
    cacheDir: String,
    watchFolders: Boolean,
    bundledPlugins: List<String>,
    distribution: org.jellyfesh.ffi.Distribution,
    resumeOnStart: Boolean,
): CoreConfig =
    CoreConfig(
        dataDir = dataDir,
        cacheDir = cacheDir,
        output = OutputKind.Device(name = null),
        watchFolders = watchFolders,
    )

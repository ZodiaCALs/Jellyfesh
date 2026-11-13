// Hand-written companion to the generated bindings; `cargo xtask bindings --lang kotlin` copies
// it next to them. Source: core/jf-ffi/kotlin/.

package org.jellyfesh.ffi

/**
 * Hands the app's `Context` to the core's audio output, which needs it on Android before the
 * first playback (`ndk-context`, read by the AAudio output). JNA loads the library without
 * telling Rust about the Java VM, so the app does it once, for example in
 * `Application.onCreate`:
 *
 * ```
 * AndroidContext.initialize(applicationContext)
 * ```
 *
 * Pass the application context, not an activity: the core keeps a global reference to it for the
 * life of the process. Calling it again does nothing. On other platforms it is harmless.
 */
object AndroidContext {
    /**
     * Returns true when the context is set (now or by an earlier call).
     *
     * @param context an `android.content.Context`; typed `Any` so this file also compiles for the
     *   JVM tests.
     */
    @JvmStatic
    @Synchronized
    fun initialize(context: Any): Boolean {
        loadLibrary()
        return nativeInitialize(context)
    }

    @JvmStatic
    private external fun nativeInitialize(context: Any): Boolean

    private var loaded = false

    /**
     * JNI only finds functions in libraries loaded through `System`, so load the same library
     * JNA uses: the `uniffi.component.jellyfesh.libraryOverride` path when that is set (tests),
     * else `jf_ffi` from the app's native libraries.
     */
    private fun loadLibrary() {
        if (loaded) return
        val override = System.getProperty("uniffi.component.jellyfesh.libraryOverride")
        if (override != null && java.io.File(override).isAbsolute) {
            System.load(override)
        } else {
            System.loadLibrary(override ?: "jf_ffi")
        }
        loaded = true
    }
}

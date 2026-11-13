package org.jellyfesh.android

import android.app.Application
import android.content.ComponentCallbacks2
import kotlinx.coroutines.flow.MutableSharedFlow

/** Owns the [AppGraph]; the core starts with the process (off the main thread). */
class JellyfeshApp : Application() {
    lateinit var graph: AppGraph
        private set

    /** OAuth redirects (`jellyfesh://auth?...`) for the sign-in sheet that is waiting for one. */
    val authRedirects = MutableSharedFlow<String>(extraBufferCapacity = 4)

    override fun onCreate() {
        super.onCreate()
        graph = AppGraph(this)
        graph.start()
    }

    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        // In the background the decoded artwork is the first thing to give back.
        if (level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND) graph.images.trim()
    }
}

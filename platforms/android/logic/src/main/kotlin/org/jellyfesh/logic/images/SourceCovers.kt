package org.jellyfesh.logic.images

import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.withTimeoutOrNull
import org.jellyfesh.logic.core.ArtworkReady

/**
 * Source pictures through the core (docs/core-api.md, "Covers"; ADR-0022 §5): the app never
 * fetches them. Asking `source_artwork(url, size)` gives the cached file, or null while the core
 * fetches it, and an `Artwork` event follows with the file (or none: the picture cannot be had, so
 * it is not asked for again). This keeps the waits for those events and the pictures that cannot
 * be had. Thread-safe: the core's event thread answers while loads wait on other threads.
 */
class SourceCovers(private val maxMissing: Int = 1_000) {
    private val waits = ConcurrentHashMap<String, CompletableDeferred<String?>>()
    private val missing: MutableSet<String> = java.util.Collections.newSetFromMap(
        object : LinkedHashMap<String, Boolean>(64, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Boolean>?): Boolean = size > maxMissing
        },
    )

    /**
     * The cached file of the picture at [url] for [sizePx] (one of [ArtworkSizes.BUCKETS]): [ask]
     * the core, a blocking query (run it off the main thread), then wait for its word at most
     * [timeoutMs] and ask once more; null when the picture cannot be had or does not come.
     */
    suspend fun path(url: String, sizePx: Int, timeoutMs: Long = 30_000, ask: (url: String, sizePx: Int) -> String?): String? {
        val key = key(url, sizePx)
        if (synchronized(missing) { key in missing }) return null
        // Waiting before asking, so an answer that comes at once is not missed.
        val wait = waits.computeIfAbsent(key) { CompletableDeferred() }
        ask(url, sizePx)?.let { cached ->
            waits.remove(key, wait)
            return cached
        }
        val answer = withTimeoutOrNull(timeoutMs) { wait.await() }
        if (answer != null) return answer
        if (synchronized(missing) { key in missing }) return null
        // No word in time (the event may have gone to an earlier waiter): the cache may have it now.
        return ask(url, sizePx)
    }

    /** The core's word on a picture ([ArtworkReady]): the waits for it end. */
    fun onReady(ready: ArtworkReady) {
        val key = key(ready.url, ready.sizePx)
        if (ready.path == null) synchronized(missing) { missing += key }
        waits.remove(key)?.complete(ready.path)
    }

    /** Whether loads still wait for an answer (for tests). */
    val waiting: Int get() = waits.size

    private fun key(url: String, sizePx: Int): String = "$sizePx@$url"
}

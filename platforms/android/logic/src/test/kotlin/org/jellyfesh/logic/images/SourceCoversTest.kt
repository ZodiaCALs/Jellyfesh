package org.jellyfesh.logic.images

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlinx.coroutines.async
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.jellyfesh.logic.core.ArtworkReady

class SourceCoversTest {
    private val url = "https://img.example/a.jpg"

    @Test
    fun aCachedPictureComesAtOnce() = runTest {
        val covers = SourceCovers()
        assertEquals("/cache/a-256.jpg", covers.path(url, 256) { _, _ -> "/cache/a-256.jpg" })
        assertEquals(0, covers.waiting)
    }

    @Test
    fun aPictureBeingFetchedComesWithTheCoresWord() = runTest {
        val covers = SourceCovers()
        val asked = mutableListOf<Int>()
        val load = async { covers.path(url, 256) { _, size -> asked += size; null } }
        runCurrent()
        assertEquals(1, covers.waiting)
        // An answer for another size is not this one's.
        covers.onReady(ArtworkReady(url, 512, "/cache/a-512.jpg"))
        runCurrent()
        assertEquals(1, covers.waiting)
        covers.onReady(ArtworkReady(url, 256, "/cache/a-256.jpg"))
        assertEquals("/cache/a-256.jpg", load.await())
        assertEquals(listOf(256), asked, "asked once")
    }

    @Test
    fun aPictureThatCannotBeHadIsNotAskedForAgain() = runTest {
        val covers = SourceCovers()
        var asks = 0
        val load = async { covers.path(url, 128) { _, _ -> asks++; null } }
        runCurrent()
        covers.onReady(ArtworkReady(url, 128, null))
        assertNull(load.await())
        assertNull(covers.path(url, 128) { _, _ -> asks++; null })
        assertEquals(1, asks)
    }

    @Test
    fun withoutAWordItAsksOnceMoreAfterTheWait() = runTest {
        val covers = SourceCovers()
        var cached: String? = null
        val load = async { covers.path(url, 64, timeoutMs = 1_000) { _, _ -> cached } }
        runCurrent()
        cached = "/cache/a-64.jpg"
        advanceTimeBy(1_001)
        assertEquals("/cache/a-64.jpg", load.await())
    }
}

package org.jellyfesh.logic.images

/**
 * The sizes the core caches thumbnails at (`artwork_thumbnail(hash, size)`), and how a decoded
 * image is scaled down while it is read. Pure; the app's loader uses it off the main thread.
 */
object ArtworkSizes {
    /** The core's thumbnail sizes, in pixels. */
    val BUCKETS: List<Int> = listOf(64, 128, 256, 512, 1024)

    /** The smallest thumbnail size that covers [px] device pixels (the largest when none does). */
    fun bucket(px: Int): Int = BUCKETS.firstOrNull { it >= px } ?: BUCKETS.last()

    /**
     * BitmapFactory's `inSampleSize` for an image of [width] by [height] shown at about [targetPx]:
     * the largest power of two that keeps the short edge at or above the target.
     */
    fun sampleSize(width: Int, height: Int, targetPx: Int): Int {
        if (width <= 0 || height <= 0 || targetPx <= 0) return 1
        val shortEdge = minOf(width, height)
        var sample = 1
        while (shortEdge / (sample * 2) >= targetPx) sample *= 2
        return sample
    }

    /** Bytes for an ARGB_8888 image, the unit of the memory cache's budget. */
    fun bytes(width: Int, height: Int): Int = width * height * 4

    /**
     * The decoded-image cache budget: an eighth of the heap, between 16 and 64 MB (the Linux app
     * keeps 64 MB on desktops).
     */
    fun cacheBudget(maxHeapBytes: Long): Int =
        (maxHeapBytes / 8).coerceIn(16L * 1024 * 1024, 64L * 1024 * 1024).toInt()
}

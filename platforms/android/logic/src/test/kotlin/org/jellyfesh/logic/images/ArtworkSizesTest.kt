package org.jellyfesh.logic.images

import kotlin.test.Test
import kotlin.test.assertEquals

class ArtworkSizesTest {
    @Test
    fun bucketsCoverTheShownSize() {
        assertEquals(64, ArtworkSizes.bucket(1))
        assertEquals(64, ArtworkSizes.bucket(64))
        assertEquals(128, ArtworkSizes.bucket(65))
        assertEquals(512, ArtworkSizes.bucket(400))
        assertEquals(1024, ArtworkSizes.bucket(5_000))
    }

    @Test
    fun sampleSizeKeepsTheShortEdgeAboveTheTarget() {
        assertEquals(1, ArtworkSizes.sampleSize(1_000, 1_000, 600))
        assertEquals(2, ArtworkSizes.sampleSize(1_200, 1_600, 600))
        assertEquals(4, ArtworkSizes.sampleSize(3_000, 3_000, 600))
        assertEquals(1, ArtworkSizes.sampleSize(0, 100, 64))
        assertEquals(1, ArtworkSizes.sampleSize(100, 100, 0))
    }

    @Test
    fun cacheBudgetIsAnEighthOfTheHeapWithinBounds() {
        val mb = 1024L * 1024
        assertEquals((16 * mb).toInt(), ArtworkSizes.cacheBudget(64 * mb))
        assertEquals((32 * mb).toInt(), ArtworkSizes.cacheBudget(256 * mb))
        assertEquals((64 * mb).toInt(), ArtworkSizes.cacheBudget(4_096 * mb))
        assertEquals(400, ArtworkSizes.bytes(10, 10))
    }
}

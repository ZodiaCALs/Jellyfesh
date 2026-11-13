package org.jellyfesh.logic.format

import kotlin.test.Test
import kotlin.test.assertEquals

class TimeFormatTest {
    @Test
    fun clockUnderAnHour() {
        assertEquals("0:00", TimeFormat.clock(0))
        assertEquals("0:00", TimeFormat.clock(999))
        assertEquals("1:24", TimeFormat.clock(84_000))
        assertEquals("12:05", TimeFormat.clock(725_000))
        assertEquals("59:59", TimeFormat.clock(3_599_999))
    }

    @Test
    fun clockFromAnHour() {
        assertEquals("1:00:00", TimeFormat.clock(3_600_000))
        assertEquals("1:02:03", TimeFormat.clock(3_723_000))
    }

    @Test
    fun negativeIsZero() {
        assertEquals("0:00", TimeFormat.clock(-5_000))
        assertEquals("0:00", TimeFormat.clockUs(-1))
    }

    @Test
    fun remainingRoundsUpSoBothLabelsAddUp() {
        // 1:24 played of 3:52 leaves 2:28.
        assertEquals("−2:28", TimeFormat.remaining(84_000, 232_000))
        // 1:24.4 played: the left label shows 1:24, so the right one shows 2:28 (2:27.6 rounded up).
        assertEquals("−2:28", TimeFormat.remaining(84_400, 232_000))
        assertEquals("−0:00", TimeFormat.remaining(300_000, 232_000))
    }

    @Test
    fun hoursAndMinutes() {
        assertEquals(0L to 4L, TimeFormat.hoursMinutes(232_000))
        assertEquals(1L to 1L, TimeFormat.hoursMinutes(3_660_000))
        assertEquals(0L to 0L, TimeFormat.hoursMinutes(-1))
    }
}

package org.jellyfesh.logic.format

/**
 * Times as people read them: `m:ss` under an hour, `h:mm:ss` from an hour.
 *
 * Digits are ASCII on purpose: the views draw them with tabular figures, and Misty asks for the
 * same numerals in every language for times.
 */
object TimeFormat {
    /** `1:24`, `12:05`, `1:02:03`. Negative input shows as zero. */
    fun clock(ms: Long): String {
        val totalSeconds = (ms.coerceAtLeast(0)) / 1_000
        val hours = totalSeconds / 3_600
        val minutes = (totalSeconds % 3_600) / 60
        val seconds = totalSeconds % 60
        return if (hours > 0) {
            "$hours:${two(minutes)}:${two(seconds)}"
        } else {
            "$minutes:${two(seconds)}"
        }
    }

    /** [clock] for microseconds, the unit of playback positions. */
    fun clockUs(us: Long): String = clock(us / 1_000)

    /**
     * The time left, with a true minus sign: `−2:28`. The remaining time is rounded up to whole
     * seconds so the two labels of a seek bar always add up to the duration.
     */
    fun remaining(positionMs: Long, durationMs: Long): String {
        val left = (durationMs - positionMs).coerceAtLeast(0)
        val roundedUp = (left + 999) / 1_000 * 1_000
        return "−" + clock(roundedUp)
    }

    /**
     * A total such as an album's length, rounded to minutes: `(hours, minutes)`. The app turns
     * it into words with its plural resources.
     */
    fun hoursMinutes(ms: Long): Pair<Long, Long> {
        val totalMinutes = (ms.coerceAtLeast(0) + 30_000) / 60_000
        return totalMinutes / 60 to totalMinutes % 60
    }

    private fun two(value: Long): String = if (value < 10) "0$value" else value.toString()
}

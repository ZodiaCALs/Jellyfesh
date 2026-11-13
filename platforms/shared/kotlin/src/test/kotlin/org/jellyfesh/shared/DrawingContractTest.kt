package org.jellyfesh.shared

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.double
import kotlinx.serialization.json.float
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.jellyfesh.ffi.LyricTimeline
import org.jellyfesh.ffi.PlaybackAnchor
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/** The ports replay the contracts recorded from the Rust originals. */
class DrawingContractTest {
    @Test
    fun lyricsCursorContract() {
        val contract = Contract.read("lyrics-cursor.json")
        assertEquals(1, contract.field("version").jsonPrimitive.int)
        var checked = 0
        for (case in contract.field("cases").jsonArray) {
            case as JsonObject
            val name = case.field("name").text()
            val tl = case.field("timeline").jsonObject
            val timeline = LyricTimeline(
                lineStarts = tl.field("line_starts").uints(),
                lineEnds = tl.field("line_ends").uints(),
                segmentStarts = tl.field("segment_starts").uints(),
                segmentLines = tl.field("segment_lines").uints(),
                wordOffsets = tl.field("word_offsets").uints(),
                wordStarts = tl.field("word_starts").uints(),
                wordEnds = tl.field("word_ends").uints(),
            )
            for (query in case.field("queries").jsonArray) {
                query as JsonObject
                val t = query.field("t").jsonPrimitive.content.toUInt()
                val cursor = timeline.cursor(t)
                assertEquals(query.field("line").intOrNull(), cursor.line, "$name: line at $t")
                assertEquals(query.field("word").intOrNull(), cursor.word, "$name: word at $t")
                assertEquals(
                    query.field("word_progress").jsonPrimitive.double,
                    cursor.wordProgress.toDouble(),
                    1e-6,
                    "$name: word progress at $t",
                )
                assertEquals(
                    query.field("next_change").longOrNull()?.toUInt(),
                    timeline.nextChange(t),
                    "$name: next change after $t",
                )
                checked++
            }
        }
        assertTrue(checked > 100, "only $checked queries")
    }

    @Test
    fun displayClockContract() {
        val contract = Contract.read("display-clock.json")
        assertEquals(1, contract.field("version").jsonPrimitive.int)
        var checked = 0
        for (case in contract.field("cases").jsonArray) {
            case as JsonObject
            val name = case.field("name").text()
            val clock = DisplayClock()
            for ((i, step) in case.field("steps").jsonArray.withIndex()) {
                step as JsonObject
                val at = "$name, step $i"
                when (step.field("op").text()) {
                    "update" -> clock.update(anchor(step.field("anchor").jsonObject), step.field("now_ns").ulong())
                    "position" -> assertEquals(
                        step.field("expect").long(),
                        clock.positionUs(step.field("now_ns").ulong()),
                        at,
                    )
                    "wait_until" -> assertEquals(
                        step.field("expect_us").longOrNull(),
                        clock.waitUntil(step.field("target_us").long(), step.field("now_ns").ulong())
                            ?.inWholeMicroseconds,
                        at,
                    )
                    "running" -> assertEquals(step.field("expect").jsonPrimitive.boolean, clock.isRunning, at)
                    else -> error("unknown op in $at")
                }
                checked++
            }
        }
        for (row in contract.field("next_second").jsonArray) {
            val (position, expected) = row.jsonArray.map { it.long() }
            assertEquals(expected, nextSecond(position), "nextSecond($position)")
            checked++
        }
        for (row in contract.field("next_step").jsonArray) {
            val values = row.jsonArray
            val (position, step) = values.take(2).map { it.long() }
            assertEquals(values[2].longOrNull(), nextStep(position, step), "nextStep($position, $step)")
            checked++
        }
        assertTrue(checked > 1000, "only $checked checks")
    }

    private fun anchor(json: JsonObject) = PlaybackAnchor(
        trackGeneration = json.field("track_generation").ulong(),
        positionUs = json.field("position_us").long(),
        anchorMonoNs = json.field("anchor_mono_ns").ulong(),
        rate = json.field("rate").jsonPrimitive.float,
        durationUs = json.field("duration_us").longOrNull(),
    )
}

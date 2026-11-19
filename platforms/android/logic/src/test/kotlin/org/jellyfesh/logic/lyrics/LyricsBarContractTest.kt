package org.jellyfesh.logic.lyrics

import java.io.File
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.double
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long

/**
 * The lyrics bar port against tests/contract/lyrics-bar.json, recorded from the core's
 * `lyrics_bar`: the line and the next line exactly, each word's progress and the line's progress
 * within 1e-6 (the file rounds them to millionths).
 */
class LyricsBarContractTest {
    private val contract: JsonObject by lazy {
        var dir: File? = File("").absoluteFile
        while (dir != null && !dir.resolve("tests/contract/lyrics-bar.json").isFile) dir = dir.parentFile
        val file = requireNotNull(dir) { "tests/contract/lyrics-bar.json not found" }.resolve("tests/contract/lyrics-bar.json")
        Json.parseToJsonElement(file.readText()).jsonObject
    }

    private fun longs(o: JsonObject, name: String): LongArray =
        (o[name] as JsonArray).map { it.jsonPrimitive.long }.toLongArray()

    private fun timeline(o: JsonObject) = TimelineArrays(
        lineStarts = longs(o, "line_starts"),
        lineEnds = longs(o, "line_ends"),
        segmentStarts = longs(o, "segment_starts"),
        segmentLines = longs(o, "segment_lines"),
        wordOffsets = longs(o, "word_offsets"),
        wordStarts = longs(o, "word_starts"),
        wordEnds = longs(o, "word_ends"),
    )

    @Test
    fun replaysTheContract() {
        val cases = contract["cases"]!!.jsonArray
        assertTrue(cases.isNotEmpty())
        var checked = 0
        for (case in cases) {
            val c = case.jsonObject
            val name = c["name"]!!.jsonPrimitive.content
            val t = timeline(c["timeline"]!!.jsonObject)
            for (q in c["queries"]!!.jsonArray) {
                val query = q.jsonObject
                val at = query["t"]!!.jsonPrimitive.long
                val bar = LyricsBars.bar(t, at)
                val where = "$name at $at"
                assertEquals(query["line"].let { if (it == null || it is JsonNull) null else it.jsonPrimitive.int }, bar.line, "$where: line")
                assertEquals(query["next"].let { if (it == null || it is JsonNull) null else it.jsonPrimitive.int }, bar.next, "$where: next")
                val words = query["words"]!!.jsonArray.map { it.jsonPrimitive.double }
                assertEquals(words.size, bar.words.size, "$where: words")
                words.zip(bar.words).forEachIndexed { i, (want, got) ->
                    assertTrue(abs(want - got) <= 1e-6, "$where: word $i is $got, not $want")
                }
                val progress = query["line_progress"]!!.jsonPrimitive.double
                assertTrue(abs(progress - bar.lineProgress) <= 1e-6, "$where: line progress is ${bar.lineProgress}, not $progress")
                checked++
            }
        }
        assertTrue(checked > 20, "only $checked queries")
    }
}

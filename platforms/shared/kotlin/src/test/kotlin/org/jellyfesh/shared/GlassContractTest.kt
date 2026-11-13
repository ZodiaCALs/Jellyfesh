package org.jellyfesh.shared

import kotlin.math.abs
import kotlinx.serialization.json.double
import kotlinx.serialization.json.float
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonPrimitive
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/** The glass-alpha port replays tests/contract/glass-alpha.json, recorded from jf_palette. */
class GlassContractTest {
    @Test
    fun glassAlphaContract() {
        val contract = Contract.read("glass-alpha.json")
        assertEquals(1, contract.field("version").jsonPrimitive.int)
        val tolerance = contract.field("tolerance").jsonPrimitive.double
        var checked = 0
        for (case in contract.field("cases").jsonArray) {
            val (alpha, transparency, expected) = case.jsonArray.map { it.jsonPrimitive }
            val actual = glassAlpha(alpha.float, transparency.float)
            assertTrue(
                abs(actual - expected.double) <= tolerance,
                "glass alpha (${alpha.float}, ${transparency.float}) = $actual, expected ${expected.double}",
            )
            checked++
        }
        assertTrue(checked >= 50, "only $checked cases")
    }

    @Test
    fun glassAlphaKeepsTheTokensAtTheDefaultAndClampsItsInputs() {
        for (a in listOf(0f, 0.3f, 0.42f, 0.62f, 1f)) assertEquals(a, glassAlpha(a, DEFAULT_TRANSPARENCY))
        assertEquals(1f, glassAlpha(0.42f, -1f))
        assertEquals(0.42f, glassAlpha(0.42f, Float.NaN))
        assertEquals(0.75f, glassAlpha(Float.NaN, 0.75f))
        assertEquals(1f, glassAlpha(1.5f, 0.5f))
    }
}

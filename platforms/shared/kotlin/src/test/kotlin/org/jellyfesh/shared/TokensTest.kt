package org.jellyfesh.shared

import org.jellyfesh.tokens.Tokens
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/** The tokens `cargo xtask tokens --target kotlin` writes compile and carry the token file's values. */
class TokensTest {
    @Test
    fun generatedTokens() {
        assertEquals(250f, Tokens.defaultHue)
        assertEquals(16f, Tokens.Space.s4)
        assertEquals(80, Tokens.Motion.dur0Ms)
        assertEquals("ink.ink-90", Tokens.Ink.ink90.name)
        assertEquals(listOf("clear", "mist", "veil"), Tokens.Glass.grades.map { it.name })
        assertEquals("glass.tint-mist", Tokens.Glass.tintMist.name)
        assertEquals("elevation.shadow-1.0", Tokens.Elevation.shadow1.light[0].color.name)
        assertEquals(700, Tokens.TypeRoles.nowPlayingTitle.weight)
    }
}

package org.jellyfesh.logic

import java.io.File
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.jellyfesh.logic.core.HueSource
import org.jellyfesh.logic.core.LyricsSize
import org.jellyfesh.logic.player.CubicBezier
import org.jellyfesh.logic.player.LyricLineRole
import org.jellyfesh.logic.player.NowPlayingMotion
import org.jellyfesh.logic.seek.SeekBarState
import org.jellyfesh.logic.seek.SeekGeometry
import org.jellyfesh.logic.seek.SeekKey
import org.jellyfesh.logic.settings.SettingsEdits
import org.jellyfesh.logic.theme.AccentChoices
import org.jellyfesh.logic.theme.AuroraColour
import org.jellyfesh.logic.theme.Luminous
import org.jellyfesh.logic.theme.Oklch

/**
 * The v0.0.4 design pass (docs/design-controls.md): the straight seek bar's geometry, the player
 * bar's breakpoint, Now Playing's motion numbers and the accent swatches.
 */
class DesignTest {
    private val tokens: JsonObject by lazy {
        var dir: File? = File("").absoluteFile
        while (dir != null && !dir.resolve("design/tokens.json").isFile) dir = dir.parentFile
        Json.parseToJsonElement(requireNotNull(dir) { "design/tokens.json not found" }.resolve("design/tokens.json").readText()).jsonObject
    }

    private fun token(group: String, key: String): Double {
        val value = tokens.getValue(group).jsonObject.getValue(key).jsonObject.getValue("\$value")
        return if (value is JsonObject) value.getValue("value").jsonPrimitive.double else value.jsonPrimitive.double
    }

    @Test
    fun geometryConstantsAreTheTokens() {
        assertEquals(token("player", "scrubber-track"), SeekGeometry.TRACK.toDouble())
        assertEquals(token("player", "scrubber-track-hover"), SeekGeometry.TRACK_HOVER.toDouble())
        assertEquals(token("player", "scrubber-track-drag"), SeekGeometry.TRACK_DRAG.toDouble())
        assertEquals(token("player", "scrubber-track-touch"), SeekGeometry.TRACK_TOUCH.toDouble())
        assertEquals(token("player", "scrubber-track-touch-drag"), SeekGeometry.TRACK_TOUCH_DRAG.toDouble())
        assertEquals(token("player", "scrubber-thumb"), SeekGeometry.THUMB.toDouble())
        assertEquals(token("player", "mini-progress"), SeekGeometry.MINI_PROGRESS.toDouble())
        assertEquals(token("player", "mini-radius"), SeekGeometry.MINI_RADIUS.toDouble())
        assertEquals(token("player", "bar-height"), SeekGeometry.BAR_HEIGHT.toDouble())
        assertEquals(token("player", "artwork-paused-scale"), NowPlayingMotion.ARTWORK_PAUSED_SCALE.toDouble(), 1e-6)
        assertEquals(token("lyrics", "scale-inactive"), NowPlayingMotion.INACTIVE_LINE_SCALE.toDouble(), 1e-6)
        assertEquals(token("lyrics", "scale-small"), NowPlayingMotion.SMALL_LYRICS.toDouble(), 1e-6)
        assertEquals(token("lyrics", "scale-large"), NowPlayingMotion.LARGE_LYRICS.toDouble(), 1e-6)
        assertEquals(token("lyrics", "stagger"), NowPlayingMotion.STAGGER_MS.toDouble())
    }

    @Test
    fun touchThickensWithoutAThumbAndAMouseShowsOne() {
        assertEquals(6f, SeekGeometry.thickness(SeekBarState.Rest, touch = true))
        assertEquals(12f, SeekGeometry.thickness(SeekBarState.Drag, touch = true))
        assertFalse(SeekGeometry.showsThumb(SeekBarState.Drag, touch = true))
        assertEquals(4f, SeekGeometry.thickness(SeekBarState.Rest, touch = false))
        assertEquals(6f, SeekGeometry.thickness(SeekBarState.Hover, touch = false))
        assertEquals(8f, SeekGeometry.thickness(SeekBarState.Drag, touch = false))
        assertTrue(SeekGeometry.showsThumb(SeekBarState.Hover, touch = false))
        assertFalse(SeekGeometry.showsThumb(SeekBarState.Rest, touch = false))
    }

    @Test
    fun theTrackIsInsetAndMirroredRightToLeft() {
        // 212 dp on touch: 6 dp at each end for the 12 dp drag line, a 200 dp line.
        assertEquals(200f, SeekGeometry.trackLength(212f, touch = true))
        assertEquals(0.25f, SeekGeometry.fractionAt(56f, 212f, touch = true, rtl = false))
        assertEquals(0.75f, SeekGeometry.fractionAt(56f, 212f, touch = true, rtl = true))
        assertEquals(1f, SeekGeometry.fractionAt(999f, 212f, touch = true, rtl = false))
        assertEquals(0f, SeekGeometry.fractionAt(Float.NaN, 212f, touch = true, rtl = false))
        assertEquals(56f, SeekGeometry.thumbCentre(0.25f, 212f, touch = true, rtl = false))
        assertEquals(156f, SeekGeometry.thumbCentre(0.25f, 212f, touch = true, rtl = true))
        // Steps are device pixels of the line: 212 dp at 2.5x is 530 px, 500 px of line.
        assertEquals(500, SeekGeometry.steps(530f, touch = true, density = 2.5f))
    }

    @Test
    fun keysStepFiveAndThirtySecondsWithinTheTrack() {
        assertEquals(55_000L, SeekGeometry.keyTarget(SeekKey.Back, 60_000, 225_000))
        assertEquals(90_000L, SeekGeometry.keyTarget(SeekKey.PageForward, 60_000, 225_000))
        assertEquals(0L, SeekGeometry.keyTarget(SeekKey.PageBack, 10_000, 225_000))
        assertEquals(225_000L, SeekGeometry.keyTarget(SeekKey.End, 60_000, 225_000))
        assertEquals(0L, SeekGeometry.keyTarget(SeekKey.Start, 60_000, 225_000))
    }

    @Test
    fun theMiniLineRunsBetweenTheCornersAndTheBarTakesOverAt900() {
        assertEquals(328f, SeekGeometry.miniTrackLength(360f))
        assertEquals(82f, SeekGeometry.miniProgressLength(0.25f, 360f))
        assertEquals(984, SeekGeometry.miniSteps(1080f, 3f))
        assertFalse(SeekGeometry.usesPlayerBar(899f))
        assertTrue(SeekGeometry.usesPlayerBar(900f))
    }

    @Test
    fun nowPlayingMotionFollowsTheTokens() {
        assertEquals(0.88f, NowPlayingMotion.artworkScale(playing = false, reduceMotion = false))
        assertEquals(1f, NowPlayingMotion.artworkScale(playing = false, reduceMotion = true))
        assertEquals(1f, NowPlayingMotion.artworkScale(playing = true, reduceMotion = false))
        assertEquals(0.8f, NowPlayingMotion.lyricsScale(LyricsSize.Small))
        assertEquals(1.2f, NowPlayingMotion.lyricsScale(LyricsSize.Large))
        assertEquals(LyricLineRole.Sung, NowPlayingMotion.role(1, 3))
        assertEquals(LyricLineRole.Active, NowPlayingMotion.role(3, 3))
        assertEquals(LyricLineRole.Upcoming, NowPlayingMotion.role(4, 3))
        assertEquals(LyricLineRole.Upcoming, NowPlayingMotion.role(0, null))
        // Luminous: inactive lines at 92 %, the cascade 40 ms apart.
        assertEquals(0.92f, NowPlayingMotion.lineScale(LyricLineRole.Upcoming))
        assertEquals(1f, NowPlayingMotion.lineScale(LyricLineRole.Active))
        assertEquals(0, NowPlayingMotion.cascadeDelayMs(3, 3))
        assertEquals(80, NowPlayingMotion.cascadeDelayMs(5, 3))
        assertEquals(320, NowPlayingMotion.cascadeDelayMs(50, 3))
    }

    @Test
    fun theCascadeLagTurnsOneScrollIntoALaterStart() {
        val fluid = CubicBezier(0.22f, 1f, 0.36f, 1f)
        val frames = NowPlayingMotion.cascadeLag(60f, delayMs = 90, durationMs = 380, ease = fluid::ease)
        assertEquals(13, frames.size)
        assertEquals(0f, frames.first().offset)
        assertEquals(0f, frames.last().offset)
        assertTrue(frames.all { it.offset in 0f..60f })
        val total = 380f + 90f
        frames.dropLast(1).forEach { f ->
            val t = f.progress * total
            val drawn = 60f * fluid.ease(t / 380f) - f.offset
            assertEquals(60f * fluid.ease((t - 90f) / 380f), drawn, 1e-4f)
        }
    }

    private fun fluid(group: String, key: String, field: String): Double =
        tokens.getValue(group).jsonObject.getValue(key).jsonObject.getValue("\$extensions").jsonObject
            .getValue("org.jellyfesh").jsonObject.getValue("fluid").jsonObject.getValue(field).jsonPrimitive.double

    @Test
    fun theLuminousFieldHasFourGlowsAtTheTokensStrength() {
        val strength = token("field", "aurora-strength")
        assertEquals(Luminous.REFERENCE_STRENGTH.toDouble(), strength, 1e-6)
        val glows = Luminous.glows(strength.toFloat())
        assertEquals(4, glows.size)
        assertEquals(listOf(0.80f, 0.72f, 0.66f, 0.46f), glows.map { it.alpha })
        assertEquals(listOf(AuroraColour.Aurora1, AuroraColour.Aurora2, AuroraColour.Aurora3, AuroraColour.Aurora1), glows.map { it.colour })
        // Larger than Misty's three (60 % by 55 % at most): the first spans 62 % by 58 %.
        assertEquals(0.62f, glows.first().rx)
        // The strength scales every glow and never goes past opaque.
        assertEquals(0.40f, Luminous.glows(0.375f).first().alpha, 1e-6f)
        assertTrue(Luminous.glows(1f).all { it.alpha <= 1f })
        assertTrue(Luminous.glows(0f).all { it.alpha == 0f })
        assertTrue(Luminous.glows(Float.NaN).all { it.alpha == 0f })
    }

    @Test
    fun lyricsAndTitlesScaleForPhones() {
        val lyrics = tokens.getValue("lyrics").jsonObject.getValue("line").jsonObject.getValue("\$value").jsonObject
        val max = lyrics.getValue("fontSize").jsonObject.getValue("value").jsonPrimitive.double.toFloat()
        val min = fluid("lyrics", "line", "min").toFloat()
        assertEquals(40f, max)
        assertEquals(min, Luminous.lyricLineSp(360f, min, max), "a phone gets the token's minimum")
        assertEquals(max, Luminous.lyricLineSp(1280f, min, max))
        assertEquals((min + max) / 2f, Luminous.lyricLineSp(780f, min, max), 1e-4f)
        // Page titles: text-h1 (48) where the window allows, the fluid minimum (38.4) on phones.
        val h1 = token("type", "text-h1").toFloat() * 16f
        val h1Min = fluid("type", "text-h1", "min").toFloat() * 16f
        val vw = fluid("type", "text-h1", "vw").toFloat()
        assertEquals(48f, h1, 1e-4f)
        assertEquals(h1Min, Luminous.pageTitleSp(390f, h1Min, vw, h1), 1e-4f)
        assertEquals(48f, Luminous.pageTitleSp(1280f, h1Min, vw, h1), 1e-4f)
        // The Now Playing title: 25 on the reference 390 dp phone, the full 32 from 600 dp.
        assertEquals(25f, Luminous.nowPlayingTitleSp(390f, 32f), 1e-4f)
        assertEquals(32f, Luminous.nowPlayingTitleSp(800f, 32f))
    }

    @Test
    fun theOklchPortReproducesTheTokenColours() {
        var checked = 0
        for (group in listOf("accent", "semantic", "field")) {
            for ((_, token) in tokens.getValue(group).jsonObject) {
                val value = (token as? JsonObject)?.get("\$value") as? JsonObject ?: continue
                val c = value["components"]?.jsonArray ?: continue
                val argb = Oklch.toArgb(c[0].jsonPrimitive.double, c[1].jsonPrimitive.double, c[2].jsonPrimitive.double)
                assertEquals(value.getValue("hex").jsonPrimitive.content, "#%06x".format(argb and 0xFFFFFF))
                checked++
            }
        }
        assertTrue(checked >= 10, "only $checked colours")
        assertEquals(0xFF0059A7.toInt(), AccentChoices.accent(250f, dark = false))
    }

    @Test
    fun swatchesMatchFixedHuesOnly() {
        assertEquals(listOf(250f, 210f, 180f, 150f, 120f, 60f, 30f, 0f, 330f, 290f), AccentChoices.SWATCH_HUES)
        assertEquals(0, AccentChoices.swatchIndex(HueSource.Fixed(250f)))
        assertEquals(7, AccentChoices.swatchIndex(HueSource.Fixed(359.8f)))
        assertEquals(8, AccentChoices.swatchIndex(HueSource.Fixed(-30f)))
        assertEquals(-1, AccentChoices.swatchIndex(HueSource.Fixed(212.5f)))
        assertEquals(-1, AccentChoices.swatchIndex(HueSource.Artwork))
        assertEquals(13, AccentChoices.hueTrack(dark = true).size)
    }

    @Test
    fun transparencyIsWholePercentOfTheCoresRange() {
        assertEquals(0.25f, SettingsEdits.transparency(25.4f))
        assertEquals(1f, SettingsEdits.transparency(140f))
        assertEquals(0f, SettingsEdits.transparency(-3f))
        assertEquals(0.5f, SettingsEdits.transparency(Float.NaN))
        assertEquals(50, SettingsEdits.transparencyPercent(0.5f))
        assertTrue(abs(SettingsEdits.transparency(SettingsEdits.transparencyPercent(0.73f).toFloat()) - 0.73f) < 1e-6f)
    }
}

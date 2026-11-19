package org.jellyfesh.logic.sources

import kotlin.math.abs
import kotlin.math.roundToInt
import kotlinx.serialization.json.contentOrNull
import org.jellyfesh.logic.queue.QueueModel

/**
 * A colour well's value (settings schema profile 1.2, `format: color`): `#RRGGBB`, and the hue,
 * saturation and brightness its picker moves.
 */
object ColourWell {
    private val HEX = Regex("^#?([0-9A-Fa-f]{6})$")

    /** 0xRRGGBB, or null when [text] is not `#RRGGBB` (the `#` may be left out while typing). */
    fun parse(text: String): Int? = HEX.matchEntire(text.trim())?.groupValues?.get(1)?.toInt(16)

    /** [text] as `#RRGGBB` in capitals, or null when it is not a colour. */
    fun normalise(text: String): String? = parse(text)?.let(::hex)

    /** `#RRGGBB` for 0xRRGGBB. */
    fun hex(rgb: Int): String = "#%06X".format(rgb and 0xFFFFFF)

    /** Hue in degrees (0 until 360), saturation and brightness from 0 to 1. */
    data class Hsv(val hue: Float, val saturation: Float, val value: Float)

    fun hsv(rgb: Int): Hsv {
        val r = (rgb shr 16 and 0xFF) / 255f
        val g = (rgb shr 8 and 0xFF) / 255f
        val b = (rgb and 0xFF) / 255f
        val max = maxOf(r, g, b)
        val min = minOf(r, g, b)
        val delta = max - min
        val hue = when {
            delta == 0f -> 0f
            max == r -> 60f * (((g - b) / delta) % 6f)
            max == g -> 60f * (((b - r) / delta) + 2f)
            else -> 60f * (((r - g) / delta) + 4f)
        }
        return Hsv((hue + 360f) % 360f, if (max == 0f) 0f else delta / max, max)
    }

    fun rgb(hsv: Hsv): Int {
        val h = ((hsv.hue % 360f) + 360f) % 360f
        val s = hsv.saturation.coerceIn(0f, 1f)
        val v = hsv.value.coerceIn(0f, 1f)
        val c = v * s
        val x = c * (1f - abs((h / 60f) % 2f - 1f))
        val m = v - c
        val (r, g, b) = when {
            h < 60f -> Triple(c, x, 0f)
            h < 120f -> Triple(x, c, 0f)
            h < 180f -> Triple(0f, c, x)
            h < 240f -> Triple(0f, x, c)
            h < 300f -> Triple(x, 0f, c)
            else -> Triple(c, 0f, x)
        }
        fun channel(f: Float) = ((f + m) * 255f).roundToInt().coerceIn(0, 255)
        return (channel(r) shl 16) or (channel(g) shl 8) or channel(b)
    }
}

/**
 * An ordered checklist (settings schema profile 1.2): its value lists the items shown, in order.
 * The form shows those first, each with a handle to drag it, then the items not shown in the
 * schema's order.
 */
object Checklists {
    data class Row(val value: String, val label: String, val shown: Boolean)

    fun rows(field: SchemaField.Checklist, shown: List<String>): List<Row> {
        val labels = field.options.mapNotNull { o -> o.value.contentOrNull?.let { it to o.label } }.toMap()
        val first = shown.filter { it in labels }.map { Row(it, labels.getValue(it), shown = true) }
        val rest = labels.filterKeys { it !in shown }.map { (value, label) -> Row(value, label, shown = false) }
        return first + rest
    }

    /** Shows [value] (at the end of the shown items) or hides it. */
    fun toggle(shown: List<String>, value: String, on: Boolean): List<String> = when {
        on && value !in shown -> shown + value
        !on -> shown - value
        else -> shown
    }

    /** Moves the shown item at [from] to [to] (positions among the shown items). */
    fun move(shown: List<String>, from: Int, to: Int): List<String> = QueueModel.moved(shown, from, to)

    /** A shown item held by its handle: where it was and how far it has moved (pixels, down positive). */
    data class Drag(val from: Int, val offsetPx: Float)

    /** Where a dragged item lands among the [count] shown items: whole rows moved. */
    fun target(drag: Drag, rowPx: Float, count: Int): Int = QueueModel.target(drag.from, drag.offsetPx, rowPx, count)

    /** How far the shown item at [index] is drawn from its place while [drag] is held. */
    fun shift(index: Int, drag: Drag?, rowPx: Float, count: Int): Float {
        if (drag == null) return 0f
        val to = target(drag, rowPx, count)
        return when {
            index == drag.from -> drag.offsetPx
            drag.from < to && index in (drag.from + 1)..to -> -rowPx
            drag.from > to && index in to until drag.from -> rowPx
            else -> 0f
        }
    }

    /** The shown items after [drag] is let go. */
    fun drop(shown: List<String>, drag: Drag, rowPx: Float): List<String> = move(shown, drag.from, target(drag, rowPx, shown.size))
}

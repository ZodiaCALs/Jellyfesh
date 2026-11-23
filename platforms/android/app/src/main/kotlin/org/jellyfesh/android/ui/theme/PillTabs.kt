package org.jellyfesh.android.ui.theme

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.ui.components.JfText

/**
 * Jellyfesh Web's pill switcher (the phone layout's library tabs, Settings' sections, Search's
 * tabs): a capsule of the quiet fill with one pill per choice, the chosen one raised on Cloud with
 * shadow-1. When the choices fit they share the width; when they do not, each keeps its own width
 * and the row scrolls sideways, as the web's does on phones.
 */
@Composable
fun <T> PillTabs(
    options: List<T>,
    selected: T?,
    label: @Composable (T) -> String,
    onSelect: (T) -> Unit,
    description: String,
    modifier: Modifier = Modifier,
) {
    val colors = Jf.colors
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val inner = (constraints.maxWidth - with(androidx.compose.ui.platform.LocalDensity.current) { 8.dp.roundToPx() }).coerceAtLeast(0)
        Box(
            Modifier
                .clip(CircleShape)
                .background(colors.fill, CircleShape)
                .horizontalScroll(rememberScrollState())
                .padding(4.dp)
                .selectableGroup()
                .semantics { contentDescription = description },
        ) {
            Layout(
                content = {
                    options.forEach { option ->
                        val chosen = option == selected
                        Box(
                            Modifier
                                .heightIn(min = 36.dp)
                                .then(if (chosen) Modifier.jfShadow(1, CircleShape).background(colors.cloud, CircleShape) else Modifier)
                                .clip(CircleShape)
                                .selectable(chosen, remember { MutableInteractionSource() }, indication = null, role = Role.Tab) { onSelect(option) }
                                .padding(horizontal = 14.dp, vertical = 8.dp),
                            contentAlignment = Alignment.Center,
                        ) {
                            JfText(label(option), style = JfType.label, color = if (chosen) colors.ink90 else colors.ink70, maxLines = 1)
                        }
                    }
                },
            ) { measurables, constraints ->
                val gap = 4.dp.roundToPx()
                val naturals = measurables.map { it.maxIntrinsicWidth(constraints.maxHeight) }
                val used = naturals.sum() + gap * (measurables.size - 1).coerceAtLeast(0)
                // Fitting: each grows by an equal share of what is left (the web's flex 1 1 auto).
                val extra = if (measurables.isEmpty() || used >= inner) 0 else (inner - used) / measurables.size
                val placeables = measurables.mapIndexed { i, m ->
                    m.measure(Constraints(minWidth = naturals[i] + extra, maxWidth = naturals[i] + extra, maxHeight = constraints.maxHeight))
                }
                val height = placeables.maxOfOrNull { it.height } ?: 0
                val width = placeables.sumOf { it.width } + gap * (placeables.size - 1).coerceAtLeast(0)
                layout(width, height) {
                    var x = 0
                    placeables.forEach {
                        it.placeRelative(x, (height - it.height) / 2)
                        x += it.width + gap
                    }
                }
            }
        }
    }
}

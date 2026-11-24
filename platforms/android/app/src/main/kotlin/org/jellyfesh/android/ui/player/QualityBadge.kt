package org.jellyfesh.android.ui.player

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.ChoiceRow
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.TouchTarget
import org.jellyfesh.logic.core.QualityNote
import org.jellyfesh.logic.core.StreamingQuality
import org.jellyfesh.logic.player.QualityBadgeText
import org.jellyfesh.logic.player.QualityFormat
import org.jellyfesh.logic.player.QualityMenu
import org.jellyfesh.logic.player.QualityMenus
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.tokens.Tokens

/**
 * The quality badge (ADR-0026 §3; docs/ui.md, "The player bar and the mini player"), in the full
 * bar and Now Playing's dock as Jellyfesh Web has it: a pill with the playing quality's label, else
 * the chosen level ([QualityMenus]). It opens the quality menu as a sheet: the format line, then
 * the levels the core offers with Streaming Quality checked, or why there are none ("Files play as
 * they are.", "Quality is set in {Service}."). Choosing a level is the same SetSettings as
 * Settings › Playback › Streaming; the core reopens the song at the new level when it can seek.
 */
@Composable
fun QualityBadge(state: CoreState, modifier: Modifier = Modifier) {
    val menu = QualityMenus.of(state) ?: return
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val colors = Jf.colors
    val label = qualityBadgeLabel(menu.badge)
    val title = stringResource(R.string.streaming_quality)
    val description = stringResource(R.string.quality_badge_a11y, label)
    Box(
        modifier
            .heightIn(min = TouchTarget)
            .clickable(remember { MutableInteractionSource() }, indication = null, role = Role.Button) {
                overlays.showSheet(title) { dismiss ->
                    // The menu follows what plays while it is open (the format arrives once the song opens).
                    val now by graph.store.state.collectAsState()
                    QualityMenuContent(QualityMenus.of(now) ?: menu) { level ->
                        dismiss()
                        QualityMenus.choose(graph.store.state.value.settings, level)?.let(graph::send)
                    }
                }
            }
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .heightIn(min = 28.dp)
                .widthIn(max = 132.dp)
                .clip(CircleShape)
                .background(colors[Tokens.Line.fill])
                .border(1.dp, colors.lineSoft, CircleShape)
                .padding(horizontal = 10.dp, vertical = 5.dp),
            contentAlignment = Alignment.Center,
        ) {
            JfText(label, style = JfType.caption.copy(fontWeight = FontWeight.Bold, letterSpacing = 0.12.sp), color = colors.ink90, maxLines = 1)
        }
    }
}

/** The quality menu: the format line, then the levels with the chosen one checked, or why there are none. */
@Composable
private fun QualityMenuContent(menu: QualityMenu, choose: (StreamingQuality) -> Unit) {
    val colors = Jf.colors
    menu.format?.let { format ->
        JfText(qualityFormatLine(format), style = JfType.small, color = colors.ink70, maxLines = 2, modifier = Modifier.padding(horizontal = 20.dp))
    }
    if (menu.levels.isEmpty()) {
        menu.note?.let { note ->
            JfText(qualityNoteText(note), style = JfType.body, color = colors.ink90, modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp))
        }
    } else {
        Column(Modifier.selectableGroup().padding(top = 4.dp)) {
            menu.levels.forEach { level ->
                ChoiceRow(stringResource(streamingQualityName(level)), level == menu.chosen, { choose(level) })
            }
        }
    }
}

/** A Streaming Quality level's name (Settings › Playback › Streaming and the quality menu). */
fun streamingQualityName(level: StreamingQuality): Int = when (level) {
    StreamingQuality.Auto -> R.string.streaming_auto
    StreamingQuality.Lossless -> R.string.streaming_lossless
    StreamingQuality.High -> R.string.streaming_high
    StreamingQuality.Normal -> R.string.streaming_normal
    StreamingQuality.Low -> R.string.streaming_low
}

@Composable
private fun qualityBadgeLabel(badge: QualityBadgeText): String = when (badge) {
    is QualityBadgeText.Label -> stringResource(qualityLabel(badge.label))
    is QualityBadgeText.Level -> stringResource(streamingQualityName(badge.level))
}

@Composable
private fun qualityNoteText(note: QualityNote): String = when (note) {
    QualityNote.FilesPlayAsTheyAre -> stringResource(R.string.quality_files_as_they_are)
    is QualityNote.SetInService -> stringResource(R.string.quality_set_in_service, note.service)
}

/** "FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s". */
@Composable
private fun qualityFormatLine(format: QualityFormat): String = listOfNotNull(
    format.codec,
    format.bitDepth?.let { stringResource(R.string.info_bits, it) },
    format.sampleRate,
    format.bitrateKbps?.let { stringResource(R.string.quality_kbits, it) },
).joinToString(" · ")

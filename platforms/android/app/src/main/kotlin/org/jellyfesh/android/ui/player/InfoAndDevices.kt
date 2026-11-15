package org.jellyfesh.android.ui.player

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.ok
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.LyricsSource
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.Timing
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.format.TimeFormat
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.state.CoreState

/**
 * About This Song (docs/ui.md, Now Playing › Info): Title, Artist, Album, Year, Length, Format,
 * Lyrics and File. The year is the library's (Now Playing does not carry it).
 */
@Composable
fun InfoView(now: NowPlaying, lyrics: LyricsSnapshot?, durationUs: Long?, modifier: Modifier = Modifier) {
    val library = now.track as? TrackRef.Library
    val year = library?.let { ref -> rememberQuery(ref.id) { it.track(ref.id) }.ok?.year }
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        JfText(stringResource(R.string.info_heading), style = JfType.section, modifier = Modifier.semantics { heading() })
        InfoLine(stringResource(R.string.info_title), now.title)
        now.artist?.let { InfoLine(stringResource(R.string.info_artist), it) }
        now.album?.let { InfoLine(stringResource(R.string.info_album), it) }
        year?.let { InfoLine(stringResource(R.string.info_year), it.toString()) }
        durationUs?.let { InfoLine(stringResource(R.string.info_length), TimeFormat.clockUs(it)) }
        now.format?.let { f ->
            val text = Labels.format(f) + if (f.gapless) " · " + stringResource(R.string.info_gapless) else ""
            InfoLine(stringResource(R.string.info_format), text)
        }
        now.path?.let { InfoLine(stringResource(R.string.info_file), it) }
        val source = when (val s = lyrics?.source) {
            is LyricsSource.Sidecar -> stringResource(R.string.lyrics_source_sidecar, s.path.substringAfterLast('/'))
            LyricsSource.Embedded -> stringResource(R.string.lyrics_source_embedded)
            null -> stringResource(R.string.lyrics_source_none)
        }
        val timing = when (lyrics?.doc?.timing) {
            Timing.Word -> stringResource(R.string.timing_word)
            Timing.Line -> stringResource(R.string.timing_line)
            Timing.Unsynced -> stringResource(R.string.timing_unsynced)
            null -> null
        }
        InfoLine(stringResource(R.string.info_lyrics), listOfNotNull(source, timing).joinToString(" · "))
    }
}

@Composable
private fun InfoLine(label: String, value: String) {
    Column(Modifier.semantics(mergeDescendants = true) {}, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        JfText(label, style = JfType.caption, color = Jf.colors.ink50)
        JfText(value, style = JfType.small, color = Jf.colors.ink90)
    }
}

/**
 * The output picker: the system default and every device the core lists. Opening it asks the core
 * to look again (it never polls, ADR-0009).
 */
fun showDevices(overlays: Overlays, title: String) {
    overlays.showSheet(title) { dismiss -> DeviceList(dismiss) }
}

@Composable
private fun DeviceList(dismiss: () -> Unit) {
    val graph = LocalGraph.current
    val devices = graph.store.state.value.devices
    LaunchedEffect(Unit) { graph.send(Command.RefreshDevices) }
    Column(Modifier.padding(vertical = 4.dp)) {
        DeviceRow(stringResource(R.string.np_system_default), stringResource(R.string.np_follows_system), PlayerModel.isSelected(devices, null)) {
            graph.send(Command.SetOutputDevice(null))
            dismiss()
        }
        devices.devices.forEach { device ->
            val rates = device.sampleRates.takeIf { it.isNotEmpty() }?.joinToString(" · ") { Labels.sampleRate(it) }
            DeviceRow(device.name, rates, PlayerModel.isSelected(devices, device.id)) {
                graph.send(Command.SetOutputDevice(device.id))
                dismiss()
            }
        }
    }
}

@Composable
private fun DeviceRow(name: String, detail: String?, selected: Boolean, onClick: () -> Unit) {
    val colors = Jf.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .selectable(selected, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        JfIcon(R.drawable.ic_speaker, if (selected) colors.accentText else colors.ink70, size = 22.dp)
        Column(Modifier.weight(1f)) {
            JfText(name, style = JfType.listTitle, color = if (selected) colors.accentText else colors.ink90, maxLines = 2)
            if (detail != null) JfText(detail, style = JfType.listSecondary, color = colors.ink50, maxLines = 1)
        }
        if (selected) JfIcon(R.drawable.ic_check, colors.accentText, size = 20.dp)
    }
}

/** The name of the output in use, for the Speakers chip. */
@Composable
fun outputName(state: CoreState): String = PlayerModel.deviceName(state.devices) ?: stringResource(R.string.np_speakers)

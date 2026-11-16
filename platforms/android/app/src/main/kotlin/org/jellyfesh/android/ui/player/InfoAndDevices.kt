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
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.actions.rememberTrackActions
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.ok
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.LyricsSource
import org.jellyfesh.logic.core.RemoteDeviceKind
import org.jellyfesh.logic.core.Timing
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.format.TimeFormat
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.player.PlayerTrack
import org.jellyfesh.logic.player.RemoteModel
import org.jellyfesh.logic.state.CoreState

/**
 * About This Song (docs/ui.md, Now Playing › Info): Title, Artist, Album, Year, Length, Format,
 * Lyrics and File, and Open in {Service} under the heading when the song has a link. The year is
 * the library's (Now Playing does not carry it). A remote session's song has what its service
 * says: Title, Artist, Album and Length.
 */
@Composable
fun InfoView(track: PlayerTrack, lyrics: LyricsSnapshot?, durationUs: Long?, modifier: Modifier = Modifier) {
    val actions = rememberTrackActions()
    val now = track.local
    val library = now?.track as? TrackRef.Library
    val year = library?.let { ref -> rememberQuery(ref.id) { it.track(ref.id) }.ok?.year }
    val link = track.link
    val source = track.source
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        JfText(stringResource(R.string.info_heading), style = JfType.section, modifier = Modifier.semantics { heading() })
        if (link != null && source != null) {
            JfButton(actions.openInLabel(source), { actions.openLink(link) }, style = ButtonStyle.Tonal, icon = R.drawable.ic_external)
        }
        InfoLine(stringResource(R.string.info_title), track.title)
        track.artist?.let { InfoLine(stringResource(R.string.info_artist), it) }
        track.album?.let { InfoLine(stringResource(R.string.info_album), it) }
        year?.let { InfoLine(stringResource(R.string.info_year), it.toString()) }
        durationUs?.let { InfoLine(stringResource(R.string.info_length), TimeFormat.clockUs(it)) }
        if (now == null) return@Column
        now.format?.let { f ->
            val text = Labels.format(f) + if (f.gapless) " · " + stringResource(R.string.info_gapless) else ""
            InfoLine(stringResource(R.string.info_format), text)
        }
        now.path?.let { InfoLine(stringResource(R.string.info_file), it) }
        val lyricsSource = when (val s = lyrics?.source) {
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
        InfoLine(stringResource(R.string.info_lyrics), listOfNotNull(lyricsSource, timing).joinToString(" · "))
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
 * The device button: Devices while a remote session runs (with the playing device's kind as its
 * glyph), else the output picker.
 */
@Composable
fun DeviceButton(state: CoreState, modifier: Modifier = Modifier, size: Dp = 44.dp) {
    val overlays = LocalOverlays.current
    val remote = RemoteModel.session(state)
    val devicesTitle = stringResource(R.string.remote_devices)
    val outputTitle = stringResource(R.string.np_output_device)
    if (remote != null) {
        val label = remote.device?.let { "$devicesTitle, " + stringResource(R.string.remote_playing_on, it.name) } ?: devicesTitle
        JfIconButton(deviceIcon(remote.device?.kind), label, { showRemoteDevices(overlays, devicesTitle) }, modifier, tint = Jf.colors.ink90, size = size)
    } else {
        JfIconButton(R.drawable.ic_speaker, "$outputTitle, ${outputName(state)}", { showDevices(overlays, outputTitle) }, modifier, tint = Jf.colors.ink90, size = size)
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
        DeviceRow(R.drawable.ic_speaker, stringResource(R.string.np_system_default), stringResource(R.string.np_follows_system), PlayerModel.isSelected(devices, null)) {
            graph.send(Command.SetOutputDevice(null))
            dismiss()
        }
        devices.devices.forEach { device ->
            val rates = device.sampleRates.takeIf { it.isNotEmpty() }?.joinToString(" · ") { Labels.sampleRate(it) }
            DeviceRow(R.drawable.ic_speaker, device.name, rates, PlayerModel.isSelected(devices, device.id)) {
                graph.send(Command.SetOutputDevice(device.id))
                dismiss()
            }
        }
    }
}

/**
 * Devices (docs/ui.md, "Links and remote playback"): every device the remote source lists, with
 * its kind's glyph, the one playing checked; choosing another moves playback there (`Transfer`).
 * Opening it asks the service for its devices again (`Remote { Refresh }`), and the list follows
 * the Remote slice while it is open.
 */
fun showRemoteDevices(overlays: Overlays, title: String) {
    overlays.showSheet(title) { dismiss -> RemoteDeviceList(dismiss) }
}

@Composable
private fun RemoteDeviceList(dismiss: () -> Unit) {
    val graph = LocalGraph.current
    val state by graph.store.state.collectAsState()
    LaunchedEffect(Unit) { graph.send(RemoteModel.refresh) }
    val remote = state.remote
    val choices = RemoteModel.devices(remote)
    Column(Modifier.padding(vertical = 4.dp)) {
        if (choices.isEmpty()) {
            JfText(
                stringResource(R.string.remote_no_devices, RemoteModel.serviceName(remote, state.sources)),
                style = JfType.small,
                color = Jf.colors.ink70,
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
            )
        }
        choices.forEach { choice ->
            val device = choice.device
            DeviceRow(deviceIcon(device.kind), device.name, stringResource(deviceKindLabel(device.kind)), choice.checked) {
                RemoteModel.choose(remote, device)?.let(graph::send)
                dismiss()
            }
        }
    }
}

@Composable
private fun DeviceRow(icon: Int, name: String, detail: String?, selected: Boolean, onClick: () -> Unit) {
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
        JfIcon(icon, if (selected) colors.accentText else colors.ink70, size = 22.dp)
        Column(Modifier.weight(1f)) {
            JfText(name, style = JfType.listTitle, color = if (selected) colors.accentText else colors.ink90, maxLines = 2)
            if (detail != null) JfText(detail, style = JfType.listSecondary, color = colors.ink50, maxLines = 1)
        }
        if (selected) JfIcon(R.drawable.ic_check, colors.accentText, size = 20.dp)
    }
}

/** A remote device's glyph by its kind (the output speaker before the service names one). */
fun deviceIcon(kind: RemoteDeviceKind?): Int = when (kind) {
    RemoteDeviceKind.Computer -> R.drawable.ic_device_computer
    RemoteDeviceKind.Smartphone -> R.drawable.ic_device_phone
    RemoteDeviceKind.Speaker, null -> R.drawable.ic_speaker
    RemoteDeviceKind.Tv -> R.drawable.ic_device_tv
    RemoteDeviceKind.Car -> R.drawable.ic_device_car
    RemoteDeviceKind.Other -> R.drawable.ic_device_other
}

/** What a remote device is, in words (read under its name and by TalkBack). */
fun deviceKindLabel(kind: RemoteDeviceKind): Int = when (kind) {
    RemoteDeviceKind.Computer -> R.string.device_computer
    RemoteDeviceKind.Smartphone -> R.string.device_smartphone
    RemoteDeviceKind.Speaker -> R.string.device_speaker
    RemoteDeviceKind.Tv -> R.string.device_tv
    RemoteDeviceKind.Car -> R.string.device_car
    RemoteDeviceKind.Other -> R.string.device_other
}

/** The name of the output in use, for the Speakers chip. */
@Composable
fun outputName(state: CoreState): String = PlayerModel.deviceName(state.devices) ?: stringResource(R.string.np_speakers)

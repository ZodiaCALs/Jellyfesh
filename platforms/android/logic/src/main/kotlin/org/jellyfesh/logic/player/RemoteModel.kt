package org.jellyfesh.logic.player

import kotlin.math.roundToInt
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.RemoteAction
import org.jellyfesh.logic.core.RemoteDevice
import org.jellyfesh.logic.core.RemoteSnapshot
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.state.CoreState

/**
 * The remote session as the player views show it (docs/ui.md, "Links and remote playback"): the
 * service in control, "Playing on {Device}", the last error in a quieter line and the playing
 * device's volume.
 */
data class RemoteSession(
    val source: String,
    /** The service's name: Open in {Service}, and the notes in the Lyrics and Queue panes. */
    val service: String,
    /** The playing device, for "Playing on {Device}" and the device button's icon; null while the service names none. */
    val device: RemoteDevice?,
    /** What went wrong last talking to the service; null once a call succeeds. */
    val error: String?,
    /** The playing device's volume, 0 to 1; null hides the volume control (the device has none). */
    val volume: Float?,
)

/** One row of the Devices sheet: a device the service lists, checked when it is the one playing. */
data class DeviceChoice(val device: RemoteDevice, val checked: Boolean)

/**
 * Remote sessions (ADR-0018) for the views: the core runs the session and routes the transport
 * commands; this only reads the Remote slice and builds the commands the Devices sheet, the
 * volume and Stop Controlling send.
 */
object RemoteModel {
    /** The remote session to show, while the Remote slice has a source. */
    fun session(state: CoreState): RemoteSession? {
        val remote = state.remote
        val source = remote.source ?: return null
        val device = playingDevice(remote)
        return RemoteSession(
            source = source,
            service = serviceName(remote, state.sources),
            device = device,
            error = remote.error?.takeIf { it.isNotBlank() },
            volume = device?.volumePercent?.let { it.coerceIn(0, 100) / 100f },
        )
    }

    /** The service's name: the slice's, else the plugin's, else its id. */
    fun serviceName(remote: RemoteSnapshot, sources: SourcesSnapshot): String =
        remote.sourceName?.takeIf { it.isNotBlank() }
            ?: sources.plugins.firstOrNull { it.id == remote.source }?.name
            ?: remote.source.orEmpty()

    /** The device playing: the slice's, else the listed device marked active. */
    fun playingDevice(remote: RemoteSnapshot): RemoteDevice? = remote.device ?: remote.devices.firstOrNull { it.active }

    /**
     * The Devices sheet: every device the service lists, in its order, the playing one checked.
     * A playing device the list lacks (the service named it only in the session) comes first.
     */
    fun devices(remote: RemoteSnapshot): List<DeviceChoice> {
        val playing = playingDevice(remote)
        val listed = remote.devices.map { DeviceChoice(it, it.id == playing?.id) }
        return if (playing != null && remote.devices.none { it.id == playing.id }) listOf(DeviceChoice(playing, true)) + listed else listed
    }

    /** Opening the Devices sheet asks the service for its devices again (the core never polls them on its own). */
    val refresh: Command = Command.Remote(RemoteAction.Refresh)

    /** Stop Controlling: Jellyfesh lets go; the service keeps playing. */
    val end: Command = Command.Remote(RemoteAction.End)

    /** Choosing a device moves playback there; choosing the one playing changes nothing (null). */
    fun choose(remote: RemoteSnapshot, device: RemoteDevice): Command? =
        if (device.id == playingDevice(remote)?.id) null else Command.Remote(RemoteAction.Transfer(device.id))

    /** The volume control's value (0 to 1) as the playing device's volume in whole percent. */
    fun volume(fraction: Float): Command {
        val f = if (fraction.isNaN()) 0f else fraction.coerceIn(0f, 1f)
        return Command.Remote(RemoteAction.SetVolume((f * 100f).roundToInt()))
    }

    /** Whether [plugin] is a remote source: its songs start a remote session and never join the queue. */
    fun isRemoteSource(sources: SourcesSnapshot, plugin: String?): Boolean =
        plugin != null && sources.plugins.any { it.id == plugin && it.remotePlayback }

    /** Whether [track] belongs to a remote source (no Play Next, Play Last or Add to Playlist for it). */
    fun isRemote(sources: SourcesSnapshot, track: TrackRef): Boolean =
        track is TrackRef.Source && isRemoteSource(sources, track.track.plugin)

    /** Whether [collection] is a remote source's album, artist or playlist. */
    fun isRemote(sources: SourcesSnapshot, collection: CollectionRef): Boolean =
        collection is CollectionRef.Source && isRemoteSource(sources, collection.plugin)
}

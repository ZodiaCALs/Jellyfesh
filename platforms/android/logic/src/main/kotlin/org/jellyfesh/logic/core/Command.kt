package org.jellyfesh.logic.core

/** Everything a UI can ask the core to do (docs/core-api.md, "Commands"). Fire and forget. */
sealed interface Command {
    // ---- transport
    data object Play : Command
    data object Pause : Command
    data object TogglePlayPause : Command
    data object Stop : Command
    data object Next : Command
    /** Restarts the current track after its first three seconds, else goes back one entry. */
    data object Previous : Command
    data class Seek(val positionMs: Long) : Command
    /** Linear volume, 0 to 1. */
    data class SetVolume(val volume: Float) : Command
    data class SetMuted(val muted: Boolean) : Command

    // ---- queue
    data class PlayTracks(val tracks: List<TrackRef>, val start: Int) : Command
    data class PlayCollection(val collection: CollectionRef, val start: Int, val shuffle: Boolean) : Command
    data class Enqueue(val tracks: List<TrackRef>, val playNext: Boolean) : Command
    data class EnqueueCollection(val collection: CollectionRef, val playNext: Boolean) : Command
    data class RemoveFromQueue(val uid: Long) : Command
    /** Moves an entry to `to` in playback order. */
    data class MoveInQueue(val uid: Long, val to: Int) : Command
    data class JumpTo(val uid: Long) : Command
    data object ClearQueue : Command
    data class SetShuffle(val on: Boolean) : Command
    data class SetRepeat(val mode: RepeatMode) : Command

    // ---- settings and output
    data class SetSettings(val settings: Settings) : Command
    data class SetEq(val eq: EqSettings) : Command
    data class SetReplayGain(val mode: ReplayGainMode, val preampDb: Float) : Command
    data class SetCrossfade(val crossfade: CrossfadeSettings) : Command
    /** A device id from the Devices slice; null follows the system default. */
    data class SetOutputDevice(val id: String?) : Command
    /** Sent on the OS's device notifications; the core never polls. */
    data object RefreshDevices : Command

    // ---- library
    data class AddRoot(val path: String) : Command
    data class RemoveRoot(val path: String) : Command
    data object Rescan : Command
    data class CreatePlaylist(val name: String, val tracks: List<TrackRef>) : Command
    data class RenamePlaylist(val id: Long, val name: String) : Command
    data class DeletePlaylist(val id: Long) : Command
    data class AddToPlaylist(val id: Long, val tracks: List<TrackRef>) : Command
    data class RemoveFromPlaylist(val id: Long, val entries: List<Long>) : Command
    data class MovePlaylistEntry(val id: Long, val entry: Long, val to: Int) : Command
    data class ImportPlaylist(val path: String) : Command
    data class SetFavorite(val track: TrackRef, val favorite: Boolean) : Command

    // ---- online sources
    data class InstallPlugin(val path: String) : Command
    data class UninstallPlugin(val id: String) : Command
    data class SetPluginEnabled(val id: String, val enabled: Boolean) : Command
    data class SetPluginSettings(val id: String, val settingsJson: String) : Command
    /** The person accepted the community-plugin disclosure for this plugin (ADR-0012). */
    data class AcknowledgeUnofficial(val id: String) : Command
    data class BeginLogin(val plugin: String, val method: AuthMethod) : Command
    /**
     * Connects to a service from [SourcesSnapshot.presets]: sets the server's address (null for a
     * service that takes none), turns its plugin on and starts signing in, as [BeginLogin] does.
     * The steps arrive as Auth events for the service's plugin.
     */
    data class ConnectSource(val preset: String, val address: String?, val method: AuthMethod) : Command
    /** Secrets pass through once and are never stored (ADR-0015). */
    data class SubmitLogin(val plugin: String, val flow: Long, val fields: Map<String, String>) : Command
    data class CancelLogin(val plugin: String, val flow: Long) : Command
    data class Logout(val plugin: String) : Command
    data class Search(val query: String, val scope: SearchScope) : Command
    data class Browse(val plugin: String, val node: String?, val page: Int) : Command

    /**
     * The remote session (ADR-0018): what the transport commands cannot say. While a session runs,
     * Play, Pause, TogglePlayPause, Stop, Next, Previous, Seek, SetShuffle and SetRepeat control
     * the service's player instead of the engine (the core routes them).
     */
    data class Remote(val action: RemoteAction) : Command
}

/** What [Command.Remote] asks of the remote session. */
sealed interface RemoteAction {
    /** Asks the service for its devices and state now (the Devices sheet opens). */
    data object Refresh : RemoteAction

    /** Moves playback to the device with this id, playing there. */
    data class Transfer(val device: String) : RemoteAction

    /** The playing device's volume, 0–100. */
    data class SetVolume(val percent: Int) : RemoteAction

    /** Stops controlling the service (Stop Controlling); its player carries on as it was. */
    data object End : RemoteAction
}

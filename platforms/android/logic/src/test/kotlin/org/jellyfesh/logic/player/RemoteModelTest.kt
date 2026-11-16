package org.jellyfesh.logic.player

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.anchor
import org.jellyfesh.logic.core.ArtworkRef
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.PlaybackSnapshot
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.QueueSnapshot
import org.jellyfesh.logic.core.RemoteAction
import org.jellyfesh.logic.core.RemoteDevice
import org.jellyfesh.logic.core.RemoteDeviceKind
import org.jellyfesh.logic.core.RemoteSnapshot
import org.jellyfesh.logic.core.RepeatMode
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.state.CoreState

/** The player views while a remote session runs (docs/ui.md, "Links and remote playback"). */
class RemoteModelTest {
    private fun plugin(id: String, name: String, remote: Boolean) = PluginSummary(
        id, name, "1.1.0", "Someone", PluginBadge.Community, false, listOf("remote"), emptyList(), true, "ada", false, "{}", "{}", null,
        remotePlayback = remote,
    )

    private val sources = SourcesSnapshot.Empty.copy(
        plugins = listOf(plugin("spotify", "Spotify", remote = true), plugin("org.jellyfesh.subsonic", "Subsonic", remote = false)),
    )

    private val kitchen = RemoteDevice("d-kitchen", "Kitchen", RemoteDeviceKind.Speaker, active = true, volumePercent = 64)
    private val laptop = RemoteDevice("d-laptop", "Laptop", RemoteDeviceKind.Computer, active = false, volumePercent = null)
    private val phone = RemoteDevice("d-phone", "Phone", RemoteDeviceKind.Smartphone, active = false, volumePercent = 30)

    private val song = SourceItem(
        "spotify", ItemKind.Track, "t-9", "Gulls at Noon", "Saskia Merrowhill", "Harbour Lights", 178_000,
        ArtworkRef.Url("https://images.example/a.jpg"), true, null, TrackRef.Source(SourceTrack("spotify", "t-9", "Gulls at Noon")),
        emptyList(), link = "https://open.example/track/9",
    )

    private val session = RemoteSnapshot.Empty.copy(
        version = 5,
        source = "spotify",
        sourceName = "Spotify",
        status = Status.Paused,
        item = song,
        anchor = anchor(42_000, 0, rate = 0f, generation = 3, durationMs = 180_000),
        shuffle = true,
        repeat = RepeatMode.One,
        device = kitchen,
        devices = listOf(laptop, kitchen, phone),
    )

    private val local = NowPlaying(7, TrackRef.Library(7), "/m/7.flac", "Kettle Song", "Juniper Fathom", "Salt", 200_000, ArtworkRef.File("/c/7.png"), null)

    private val playingHere = CoreState(
        playback = PlaybackSnapshot.Initial.copy(version = 2, status = Status.Playing, anchor = anchor(1_000, 0)),
        queue = QueueSnapshot.Empty.copy(shuffle = false, repeat = RepeatMode.All),
        nowPlaying = local,
        sources = sources,
    )

    @Test
    fun theEngineShowsWithoutASession() {
        val track = PlayerModel.track(playingHere)!!
        assertEquals("Kettle Song", track.title)
        assertEquals("Juniper Fathom · Salt", track.subtitle)
        assertEquals(local, track.local)
        assertNull(track.link)
        assertFalse(PlayerModel.isRemote(playingHere))
        assertEquals(Status.Playing, PlayerModel.status(playingHere))
        assertEquals(playingHere.playback.anchor, PlayerModel.anchor(playingHere))
        assertEquals(RepeatMode.All, PlayerModel.repeat(playingHere))
        assertNull(RemoteModel.session(playingHere))
        assertNull(PlayerModel.track(CoreState()), "nothing to show")
    }

    @Test
    fun theSessionsTrackReplacesTheEnginesWhileItHasASource() {
        val state = playingHere.copy(remote = session)
        val track = PlayerModel.track(state)!!
        assertEquals("Gulls at Noon", track.title)
        assertEquals("Saskia Merrowhill · Harbour Lights", track.subtitle)
        assertEquals(ArtworkRef.Url("https://images.example/a.jpg"), track.artwork)
        assertEquals("https://open.example/track/9", track.link)
        assertEquals("spotify", track.source)
        assertNull(track.local)
        assertTrue(PlayerModel.isRemote(state))
        assertEquals(Status.Paused, PlayerModel.status(state))
        assertEquals(session.anchor, PlayerModel.anchor(state), "progress comes from the session's anchor")
        assertTrue(PlayerModel.shuffle(state))
        assertEquals(RepeatMode.One, PlayerModel.repeat(state))
        assertEquals(180_000_000L, PlayerModel.durationUs(state))
        assertEquals(178_000_000L, PlayerModel.durationUs(state.copy(remote = session.copy(anchor = session.anchor.copy(durationUs = null)))))
        // Before the service names a track, the service itself shows.
        assertEquals("Spotify", PlayerModel.track(state.copy(remote = session.copy(item = null)))?.title)
        // A session that ended shows the engine again.
        assertEquals("Kettle Song", PlayerModel.track(state.copy(remote = RemoteSnapshot.Empty.copy(version = 6)))?.title)
    }

    @Test
    fun theSessionNamesItsServiceDeviceAndVolume() {
        val shown = RemoteModel.session(playingHere.copy(remote = session))!!
        assertEquals("Spotify", shown.service)
        assertEquals(kitchen, shown.device)
        assertEquals(0.64f, shown.volume)
        assertNull(shown.error)
        // The plugin's name, then its id, when the slice has none.
        assertEquals("Spotify", RemoteModel.serviceName(session.copy(sourceName = null), sources))
        assertEquals("spotify", RemoteModel.serviceName(session.copy(sourceName = null), SourcesSnapshot.Empty))
        // A device without a volume hides the control; the active device stands in for a missing one.
        assertNull(RemoteModel.session(playingHere.copy(remote = session.copy(device = laptop)))!!.volume)
        assertEquals(kitchen, RemoteModel.session(playingHere.copy(remote = session.copy(device = null)))!!.device)
        assertEquals("Timed out", RemoteModel.session(playingHere.copy(remote = session.copy(error = "Timed out")))!!.error)
    }

    @Test
    fun theDevicesSheetChecksTheOnePlayingAndTransfersToAnother() {
        assertEquals(
            listOf(DeviceChoice(laptop, false), DeviceChoice(kitchen, true), DeviceChoice(phone, false)),
            RemoteModel.devices(session),
            "the service's order, the playing device checked",
        )
        val tv = RemoteDevice("d-tv", "Living Room", RemoteDeviceKind.Tv, active = true, volumePercent = 10)
        assertEquals(DeviceChoice(tv, true), RemoteModel.devices(session.copy(device = tv)).first(), "a playing device the list lacks comes first")
        assertEquals(Command.Remote(RemoteAction.Transfer("d-phone")), RemoteModel.choose(session, phone))
        assertNull(RemoteModel.choose(session, kitchen), "choosing the device playing changes nothing")
        assertEquals(Command.Remote(RemoteAction.Refresh), RemoteModel.refresh)
        assertEquals(Command.Remote(RemoteAction.End), RemoteModel.end)
    }

    @Test
    fun theVolumeIsWholePercent() {
        assertEquals(Command.Remote(RemoteAction.SetVolume(64)), RemoteModel.volume(0.636f))
        assertEquals(Command.Remote(RemoteAction.SetVolume(100)), RemoteModel.volume(1.4f))
        assertEquals(Command.Remote(RemoteAction.SetVolume(0)), RemoteModel.volume(-0.2f))
        assertEquals(Command.Remote(RemoteAction.SetVolume(0)), RemoteModel.volume(Float.NaN))
    }

    @Test
    fun aRemoteSourcesSongsAndListsAreKnownByTheirPlugin() {
        assertTrue(RemoteModel.isRemote(sources, TrackRef.Source(SourceTrack("spotify", "1", "S"))))
        assertFalse(RemoteModel.isRemote(sources, TrackRef.Source(SourceTrack("org.jellyfesh.subsonic", "1", "S"))))
        assertFalse(RemoteModel.isRemote(sources, TrackRef.Library(1)))
        assertTrue(RemoteModel.isRemote(sources, CollectionRef.Source("spotify", "pl")))
        assertFalse(RemoteModel.isRemote(sources, CollectionRef.Album(3)))
        assertFalse(RemoteModel.isRemoteSource(sources, null))
    }
}

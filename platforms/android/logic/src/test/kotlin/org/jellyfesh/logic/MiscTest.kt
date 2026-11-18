package org.jellyfesh.logic

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.ArtworkRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DeviceInfo
import org.jellyfesh.logic.core.DevicesSnapshot
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.QueueItem
import org.jellyfesh.logic.core.QueueSnapshot
import org.jellyfesh.logic.core.RemoteSnapshot
import org.jellyfesh.logic.core.RepeatMode
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.core.TrackInfo
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.player.AudioFocusPolicy
import org.jellyfesh.logic.player.FocusChange
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.player.SeekKind
import org.jellyfesh.logic.player.SessionModels
import org.jellyfesh.logic.player.SessionPlaybackState
import org.jellyfesh.logic.qr.QrMatrix
import org.jellyfesh.logic.state.CoreState

class QrMatrixTest {
    @Test
    fun encodesWithFinderPatterns() {
        val qr = assertNotNull(QrMatrix.encode("https://example.com/link?code=ABCD-1234"))
        assertTrue(qr.size >= 21 && (qr.size - 17) % 4 == 0, "a valid QR size: ${qr.size}")
        // The top-left finder: a dark 7x7 ring around a light ring and a dark 3x3 centre.
        assertTrue(qr[0, 0] && qr[6, 0] && qr[0, 6] && qr[6, 6] && qr[3, 3])
        assertTrue(!qr[1, 1] && !qr[5, 5])
        assertTrue(!qr[-1, 0] && !qr[qr.size, 0], "outside is light")
    }

    @Test
    fun tooMuchDataGivesNothing() {
        assertNull(QrMatrix.encode("x".repeat(8_000)))
    }
}

class LabelsTest {
    @Test
    fun artistAndAlbum() {
        assertEquals("Placeholder Artist · Sample Album", Labels.artistAlbum("Placeholder Artist", "Sample Album"))
        assertEquals("Placeholder Artist", Labels.artistAlbum("Placeholder Artist", " "))
        assertNull(Labels.artistAlbum(null, null))
    }

    @Test
    fun provisionalTitles() {
        assertEquals("01 Tide", Labels.provisionalTitle(TrackRef.File("/music/01 Tide.flac")))
        assertEquals("Song", Labels.provisionalTitle(TrackRef.Source(SourceTrack("p", "1", "Song"))))
        assertNull(Labels.provisionalTitle(TrackRef.Library(3)))
    }

    @Test
    fun streamFormat() {
        assertEquals("FLAC · 44.1 kHz · 2 ch", Labels.format(TrackInfo("flac", 44_100, 2, null, true, null)))
        assertEquals("MP3 · 48 kHz · 320 kbps · 2 ch", Labels.format(TrackInfo("mp3", 48_000, 2, null, true, 320_000)))
    }

    @Test
    fun initials() {
        assertEquals("S", Labels.initial("  sample"))
        assertEquals("清", Labels.initial("清晨"))
        assertEquals("♪", Labels.initial("..."))
    }
}

class PlayerModelTest {
    private fun now(uid: Long, artwork: ArtworkRef? = null) =
        NowPlaying(uid, TrackRef.Library(uid), "/m/$uid.flac", "Song $uid", "Artist", "Album", 200_000, artwork, null)

    @Test
    fun repeatCycles() {
        assertEquals(RepeatMode.All, PlayerModel.nextRepeat(RepeatMode.Off))
        assertEquals(RepeatMode.One, PlayerModel.nextRepeat(RepeatMode.All))
        assertEquals(RepeatMode.Off, PlayerModel.nextRepeat(RepeatMode.One))
        assertEquals(Command.Pause, PlayerModel.playPause(Status.Playing))
        assertEquals(Command.Play, PlayerModel.playPause(Status.Ended))
    }

    @Test
    fun deviceNames() {
        val devices = DevicesSnapshot(1, listOf(DeviceInfo("aaudio:3", "USB DAC", false, listOf(48_000))), "aaudio:3")
        assertEquals("USB DAC", PlayerModel.deviceName(devices))
        assertNull(PlayerModel.deviceName(devices.copy(selected = null)))
        assertTrue(PlayerModel.isSelected(devices, "aaudio:3"))
        assertTrue(PlayerModel.isSelected(devices.copy(selected = null), null))
    }

    @Test
    fun theSessionShowsTheCurrentEntryThenUpNext() {
        val state = CoreState(
            nowPlaying = now(7, ArtworkRef.File("/cache/artwork/a.jpg")),
            queue = QueueSnapshot.Empty.copy(version = 1, len = 3, position = 0, upNext = listOf(QueueItem(8, TrackRef.File("/m/Next One.mp3")))),
        ).let { it.copy(playback = it.playback.copy(status = Status.Playing)) }
        val model = SessionModels.from(state)
        assertEquals(listOf(7L, 8L), model.items.map { it.uid })
        assertEquals("file:///cache/artwork/a.jpg", model.items[0].artworkUri)
        assertEquals("Next One", model.items[1].title)
        assertEquals(SessionPlaybackState.Ready, model.state)
        assertTrue(model.playWhenReady)
        assertEquals(SessionPlaybackState.Idle, SessionModels.from(CoreState()).state)
    }

    @Test
    fun aRemoteSessionTakesTheSystemsMediaControls() {
        val song = SourceItem(
            source = "spotify", kind = ItemKind.Track, id = "t1", title = "Remote Song", subtitle = "Remote Artist",
            album = "Remote Album", durationMs = 180_000, artwork = ArtworkRef.Url("https://img.example/t1.jpg"),
            playable = true, unplayableReason = null, track = null, alsoIn = emptyList(),
        )
        val engine = CoreState(
            nowPlaying = now(7),
            queue = QueueSnapshot.Empty.copy(version = 1, len = 2, position = 0, upNext = listOf(QueueItem(8, TrackRef.Library(8)))),
        )
        val remote = RemoteSnapshot.Empty.copy(
            version = 3, source = "spotify", sourceName = "Spotify", status = Status.Playing, item = song,
            anchor = anchor(30_000, 1_000), shuffle = true, repeat = RepeatMode.All,
        )
        val model = SessionModels.from(engine.copy(remote = remote))
        // The service's song alone: its Up Next stays in the service.
        assertEquals(1, model.items.size)
        val item = model.items.single()
        assertEquals("Remote Song", item.title)
        assertEquals("Remote Artist", item.artist)
        assertEquals("https://img.example/t1.jpg", item.artworkUri)
        assertEquals(240_000L, item.durationMs, "the anchor's length first")
        assertNull(item.uid)
        assertTrue(model.playWhenReady)
        assertEquals(SessionPlaybackState.Ready, model.state)
        assertTrue(model.shuffle)
        assertEquals(RepeatMode.All, model.repeat)
        assertEquals(remote.anchor, model.anchor)
        // Skips and seeks go to the core, which routes them to the service; there is no entry to jump to.
        assertEquals(Command.Next, SessionModels.seekCommand(model, SeekKind.Next, -1, 0))
        assertEquals(Command.Previous, SessionModels.seekCommand(model, SeekKind.Previous, -1, 0))
        assertEquals(Command.Seek(42_000), SessionModels.seekCommand(model, SeekKind.InCurrentItem, 0, 42_000))
        assertNull(SessionModels.seekCommand(model, SeekKind.ToItem, 1, 0))
        // Paused, and before the service names its song (the service's name stands in, as in the player).
        val paused = SessionModels.from(engine.copy(remote = remote.copy(status = Status.Paused, item = null)))
        assertEquals("Spotify", paused.items.single().title)
        assertTrue(!paused.playWhenReady)
        // When the session ends, the engine's song and Up Next come back.
        assertEquals(listOf(7L, 8L), SessionModels.from(engine.copy(remote = remote.copy(source = null))).items.map { it.uid })
    }

    @Test
    fun systemSeeksBecomeCoreCommands() {
        val model = SessionModels.from(CoreState(nowPlaying = now(7), queue = QueueSnapshot.Empty.copy(upNext = listOf(QueueItem(8, TrackRef.Library(8))))))
        assertEquals(Command.Next, SessionModels.seekCommand(model, SeekKind.Next, 1, 0))
        assertEquals(Command.Previous, SessionModels.seekCommand(model, SeekKind.Previous, 0, 0))
        assertEquals(Command.Seek(42_000), SessionModels.seekCommand(model, SeekKind.InCurrentItem, 0, 42_000))
        assertEquals(Command.JumpTo(8), SessionModels.seekCommand(model, SeekKind.ToItem, 1, 0))
        assertEquals(Command.Seek(0), SessionModels.seekCommand(model, SeekKind.DefaultPosition, 0, 0))
        assertNull(SessionModels.seekCommand(model, SeekKind.ToItem, 5, 0))
    }

    @Test
    fun focusPausesAndResumesOnlyAfterATransientLoss() {
        val policy = AudioFocusPolicy()
        assertEquals(Command.Pause, policy.onChange(FocusChange.LossTransient, playing = true))
        assertEquals(Command.Play, policy.onChange(FocusChange.Gain, playing = false))
        assertNull(policy.onChange(FocusChange.Gain, playing = false), "only once")
        assertEquals(Command.Pause, policy.onChange(FocusChange.Loss, playing = true))
        assertNull(policy.onChange(FocusChange.Gain, playing = false), "a permanent loss never resumes")
        assertNull(policy.onChange(FocusChange.LossTransientCanDuck, playing = true), "the system ducks")
        policy.onChange(FocusChange.LossTransient, playing = true)
        policy.userActed()
        assertNull(policy.onChange(FocusChange.Gain, playing = false))
    }
}

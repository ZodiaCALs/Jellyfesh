package org.jellyfesh.logic.state

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.jellyfesh.logic.anchor
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.CoreSnapshot
import org.jellyfesh.logic.core.DevicesSnapshot
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.PlaybackSnapshot
import org.jellyfesh.logic.core.QueueSnapshot
import org.jellyfesh.logic.core.RemoteSnapshot
import org.jellyfesh.logic.core.ScanStatus
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.core.TrackRef

class CoreStoreTest {
    private fun playback(version: Long, status: Status = Status.Playing) =
        PlaybackSnapshot.Initial.copy(version = version, status = status, anchor = anchor(1_000, 0))

    private fun now(uid: Long, title: String = "Song $uid") =
        NowPlaying(uid, TrackRef.Library(uid), null, title, null, null, 200_000, null, null)

    private fun snapshot(playback: PlaybackSnapshot = PlaybackSnapshot.Initial, now: NowPlaying? = null) = CoreSnapshot(
        playback = playback,
        queue = QueueSnapshot.Empty,
        nowPlaying = now,
        lyrics = null,
        theme = null,
        libraryRevision = 3,
        scanning = false,
        settings = Settings(),
        devices = DevicesSnapshot.Empty,
        sources = SourcesSnapshot.Empty,
        muted = false,
    )

    @Test
    fun olderSlicesNeverReplaceNewerOnes() {
        var state = CoreState()
        state = CoreReducer.reduce(state, CoreEvent.Playback(playback(5)))
        state = CoreReducer.reduce(state, CoreEvent.Playback(playback(4, Status.Paused)))
        assertEquals(5, state.playback.version)
        assertEquals(Status.Playing, state.playback.status)
        state = CoreReducer.reduce(state, CoreEvent.Queue(QueueSnapshot.Empty.copy(version = 2, len = 9)))
        state = CoreReducer.reduce(state, CoreEvent.Queue(QueueSnapshot.Empty.copy(version = 1, len = 1)))
        assertEquals(9, state.queue.len)
    }

    @Test
    fun aSnapshotKeepsNewerEventsThatArrivedFirst() {
        var state = CoreReducer.reduce(CoreState(), CoreEvent.Playback(playback(7)))
        state = CoreReducer.merge(state, snapshot(playback = playback(6, Status.Paused), now = now(1)))
        assertTrue(state.ready)
        assertEquals(7, state.playback.version)
        assertEquals(1, state.nowPlaying?.uid)
        assertEquals(3, state.libraryRevision)
    }

    @Test
    fun lyricsShowOnlyForTheTrackPlayingNow() {
        var state = CoreReducer.reduce(CoreState(), CoreEvent.NowPlayingChanged(now(1)))
        state = CoreReducer.reduce(state, CoreEvent.Lyrics(LyricsSnapshot(1, null, null, null)))
        assertEquals(1, state.currentLyrics?.uid)
        state = CoreReducer.reduce(state, CoreEvent.NowPlayingChanged(now(2)))
        assertNull(state.currentLyrics, "the old track's lyrics are not shown")
        state = CoreReducer.reduce(state, CoreEvent.Lyrics(LyricsSnapshot(2, null, null, null)))
        assertEquals(2, state.currentLyrics?.uid)
    }

    @Test
    fun scanningFollowsScanEvents() {
        var state = CoreReducer.reduce(CoreState(), CoreEvent.Scan(ScanStatus.Progress(10, 2, "/music/a.flac")))
        assertTrue(state.scanning)
        state = CoreReducer.reduce(state, CoreEvent.Scan(ScanStatus.Finished(5, 0, 0, 5, 0)))
        assertFalse(state.scanning)
        assertTrue(state.lastScan is ScanStatus.Finished)
    }

    @Test
    fun bufferingBelongsToTheCurrentTrack() {
        var state = CoreReducer.reduce(CoreState(), CoreEvent.NowPlayingChanged(now(4)))
        state = CoreReducer.reduce(state, CoreEvent.Buffering(4, true))
        assertTrue(state.isBuffering)
        state = CoreReducer.reduce(state, CoreEvent.Buffering(3, false))
        assertTrue(state.isBuffering, "another uid's end does not clear it")
        state = CoreReducer.reduce(state, CoreEvent.Buffering(4, false))
        assertFalse(state.isBuffering)
    }

    private fun remote(version: Long, source: String?, error: String? = null) =
        RemoteSnapshot.Empty.copy(version = version, source = source, sourceName = source?.let { "Spotify" }, status = Status.Playing, error = error)

    @Test
    fun theRemoteSliceFollowsItsVersion() {
        var state = CoreReducer.reduce(CoreState(), CoreEvent.Remote(remote(3, "spotify")))
        assertEquals("spotify", state.remote.source)
        state = CoreReducer.reduce(state, CoreEvent.Remote(remote(2, null)))
        assertEquals("spotify", state.remote.source, "an older slice never replaces a newer one")
        state = CoreReducer.merge(state, snapshot().copy(remote = remote(1, null)))
        assertEquals(3, state.remote.version, "the snapshot keeps the newer event")
        state = CoreReducer.merge(state, snapshot().copy(remote = remote(4, null)))
        assertNull(state.remote.source)
    }

    @Test
    fun aSessionEndsWhenTheSourceGoes() {
        assertTrue(CoreReducer.endsRemoteSession(remote(1, "spotify"), remote(2, null, "Sign in to Spotify again.")))
        assertFalse(CoreReducer.endsRemoteSession(remote(1, null), remote(2, null, "Sign in to Spotify again.")), "no session to end")
        assertFalse(CoreReducer.endsRemoteSession(remote(1, "spotify"), remote(2, "spotify", "Timed out")), "an error during a session")
    }

    @OptIn(ExperimentalCoroutinesApi::class) // runCurrent
    @Test
    fun theSliceThatEndsASessionIsForwardedOnce() = runTest {
        val client = FakeCoreClient(snapshot())
        val store = CoreStore()
        val signals = mutableListOf<CoreEvent>()
        backgroundScope.launch { store.signals.collect { signals += it } }
        runCurrent()
        store.attach(client)
        // The session starts, fails to reach the service and ends right away: both slices arrive
        // back to back, and the one that ends it reaches the signal stream once.
        client.emit(CoreEvent.Remote(remote(1, "spotify")))
        client.emit(CoreEvent.Remote(remote(2, null, "Sign in to Spotify again.")))
        client.emit(CoreEvent.Remote(remote(3, null, "Sign in to Spotify again.")))
        runCurrent()
        assertEquals(listOf<CoreEvent>(CoreEvent.Remote(remote(2, null, "Sign in to Spotify again."))), signals)
        assertEquals(3, store.state.value.remote.version)
    }

    @OptIn(ExperimentalCoroutinesApi::class) // runCurrent
    @Test
    fun theStoreAttachesThenForwardsSignals() = runTest {
        val client = FakeCoreClient(snapshot(playback = playback(2), now = now(9)))
        val store = CoreStore()
        val signals = mutableListOf<CoreEvent>()
        backgroundScope.launch { store.signals.collect { signals += it } }
        runCurrent()
        store.attach(client)
        assertTrue(store.state.value.ready)
        assertEquals(9, store.state.value.nowPlaying?.uid)
        // Events arrive on the core's thread through the listener the store set.
        client.emit(CoreEvent.Playback(playback(3, Status.Paused)))
        client.emit(CoreEvent.CommandFailed(12, "nothing to play"))
        runCurrent()
        assertEquals(Status.Paused, store.state.first { it.playback.version == 3L }.playback.status)
        assertEquals(listOf<CoreEvent>(CoreEvent.CommandFailed(12, "nothing to play")), signals)
    }
}

package org.jellyfesh.android.ffi

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import kotlin.math.PI
import kotlin.math.sin
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.ffi.CoreConfig
import org.jellyfesh.ffi.OutputKind
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.CoreException
import org.jellyfesh.logic.core.ScanStatus
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.core.TrackQuery
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.core.TrackSort
import org.jellyfesh.logic.state.CoreStore
import org.jellyfesh.logic.theme.FallbackTheme
import org.junit.jupiter.api.Assumptions.assumeTrue

/**
 * The app's FFI adapter against the real core (built for this machine), through the same
 * CoreClient and CoreStore the app uses: the scenario of tests/contract/core-scenario.json, seen
 * from the shell's side.
 */
class UniffiCoreClientTest {
    private fun library(): File {
        val path = System.getProperty("uniffi.component.jellyfesh.libraryOverride").orEmpty()
        val file = File(path)
        assumeTrue(file.isFile, "jf-ffi is not built for this machine ($path); run cargo xtask bindings --lang kotlin")
        return file
    }

    private fun start(dir: File): UniffiCoreClient = UniffiCoreClient.startWithoutContext(
        CoreConfig(
            dataDir = dir.resolve("data").path,
            cacheDir = dir.resolve("cache").path,
            output = OutputKind.Null(realtime = false),
            restoreSession = false,
            persistSession = false,
            watchFolders = false,
        ),
    )

    @Test
    fun theScenarioThroughTheShellsStore() {
        library()
        val dir = Files.createTempDirectory("jf-android-ffi").toFile()
        try {
            val music = dir.resolve("music").apply { mkdirs() }
            writeTone(music.resolve("01 Harbor.wav"), 440.0)
            writeTone(music.resolve("02 Lantern.wav"), 660.0)

            val client = start(dir)
            val store = CoreStore()
            val events = LinkedBlockingQueue<CoreEvent>()
            // As the app does: the store listens and merges the snapshot; this test then also
            // keeps the events it needs to wait for (nothing is sent in between).
            store.attach(client)
            client.setListener { event ->
                store.onEvent(event)
                events.put(event)
            }
            assertTrue(store.state.value.ready)
            assertEquals("automatic", store.state.value.settings.appIcon)

            // Scan the folder.
            assertTrue(client.send(Command.AddRoot(music.path)) > 0)
            val scan = waitFor(events) { it is CoreEvent.Scan && it.status is ScanStatus.Finished } as CoreEvent.Scan
            assertEquals(2L, (scan.status as ScanStatus.Finished).added)
            assertEquals(listOf(music.path), client.roots())

            // Queries map into the shell's types.
            val tracks = client.tracks(TrackQuery(sort = TrackSort.Path))
            assertEquals(listOf("01 Harbor", "02 Lantern"), tracks.map { it.title })
            assertEquals(listOf("01 Harbor"), client.searchLibrary("harbor", 10).tracks.map { it.title })
            assertEquals(tracks[0], client.track(tracks[0].id))
            assertEquals(emptyList(), client.albumsSorted(org.jellyfesh.logic.core.AlbumSort.Title, false, 0, 50))
            assertNull(client.artworkThumbnail("0000000000000000", 128))

            // A playlist, through the signal stream's PlaylistCreated.
            val request = client.send(Command.CreatePlaylist("Evening", tracks.map { TrackRef.Library(it.id) }))
            val created = waitFor(events) { it is CoreEvent.PlaylistCreated && it.request == request } as CoreEvent.PlaylistCreated
            assertEquals(2, client.playlist(created.id)?.entryCount)
            assertEquals(listOf("01 Harbor", "02 Lantern"), client.playlistEntries(created.id, 0, 50).map { it.row.title })

            // Play it; the store mirrors the slices.
            client.send(Command.PlayCollection(CollectionRef.Playlist(created.id), start = 0, shuffle = false))
            waitFor(events) { it is CoreEvent.Playback && it.snapshot.status == Status.Ended }
            drain(events, 500)
            val state = store.state.value
            assertEquals(Status.Ended, state.playback.status)
            assertEquals(2, state.queue.len)
            val snapshot = client.snapshot()
            assertEquals(snapshot.playback, state.playback)
            assertEquals(snapshot.queue, state.queue)

            // Tones have no artwork: the core's theme is the fallback hue, which the app's
            // first-frame colours must match.
            val theme = assertNotNull(state.theme ?: snapshot.theme, "a theme slice after playing")
            assertEquals(FallbackTheme.HUE, theme.hue)
            assertEquals(FallbackTheme.light, theme.light.colors)
            assertEquals(FallbackTheme.dark, theme.dark.colors)

            // Shut down: queries fail with the shell's ShutDown.
            client.shutdown()
            assertFailsWith<CoreException.ShutDown> { client.stats() }
            assertEquals(0L, client.send(Command.Play))
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test
    fun everyCommandMaps() {
        val commands = listOf(
            Command.Play, Command.Pause, Command.TogglePlayPause, Command.Stop, Command.Next, Command.Previous,
            Command.Seek(-5), Command.SetVolume(2f), Command.SetMuted(true),
            Command.PlayTracks(listOf(TrackRef.File("/a.flac")), -1),
            Command.PlayCollection(CollectionRef.Favorites, 0, true),
            Command.Enqueue(listOf(TrackRef.Library(1)), true),
            Command.EnqueueCollection(CollectionRef.Source("p", "x"), false),
            Command.RemoveFromQueue(3), Command.MoveInQueue(3, 1), Command.JumpTo(3), Command.ClearQueue,
            Command.SetShuffle(true), Command.SetRepeat(org.jellyfesh.logic.core.RepeatMode.One),
            Command.SetSettings(org.jellyfesh.logic.core.Settings()),
            Command.SetEq(org.jellyfesh.logic.core.EqSettings.Default),
            Command.SetReplayGain(org.jellyfesh.logic.core.ReplayGainMode.Album, -3f),
            Command.SetCrossfade(org.jellyfesh.logic.core.CrossfadeSettings.Default),
            Command.SetOutputDevice(null), Command.RefreshDevices,
            Command.AddRoot("/m"), Command.RemoveRoot("/m"), Command.Rescan,
            Command.CreatePlaylist("x", emptyList()), Command.RenamePlaylist(1, "y"), Command.DeletePlaylist(1),
            Command.AddToPlaylist(1, emptyList()), Command.RemoveFromPlaylist(1, listOf(2)), Command.MovePlaylistEntry(1, 2, 0),
            Command.ImportPlaylist("/p.m3u"), Command.SetFavorite(TrackRef.Library(1), true),
            Command.InstallPlugin("/x.jfpkg"), Command.UninstallPlugin("p"), Command.SetPluginEnabled("p", true),
            Command.SetPluginSettings("p", "{}"), Command.AcknowledgeUnofficial("p"),
            Command.BeginLogin("p", org.jellyfesh.logic.core.AuthMethod.Qr),
            Command.ConnectSource("navidrome", "https://music.example", org.jellyfesh.logic.core.AuthMethod.PasswordExchange),
            Command.ConnectSource("hosted", null, org.jellyfesh.logic.core.AuthMethod.Oauth),
            Command.SubmitLogin("p", 1, mapOf("a" to "b")),
            Command.CancelLogin("p", 1), Command.Logout("p"),
            Command.Search("tide", org.jellyfesh.logic.core.SearchScope.Plugin("p")), Command.Browse("p", null, 0),
        )
        val mapped = commands.map { FfiMapping.ffi(it) }
        assertEquals(commands.size, mapped.size)
        // Out-of-range values are clamped rather than wrapped.
        assertEquals(org.jellyfesh.ffi.Command.Seek(positionMs = 0uL), mapped[6])
        assertEquals(org.jellyfesh.ffi.Command.SetVolume(volume = 1f), mapped[7])
    }

    @Test
    fun theDrawClockAndLyricCursorAreThePorts() {
        library()
        val clock = PortDrawClock()
        clock.update(org.jellyfesh.logic.core.PlaybackAnchor(1, 1_250_000, 0, 1f, 240_000_000), 0)
        assertEquals(1_250_000, clock.positionUs(0))
        assertEquals(750_000_000L, clock.waitUntilNs(2_000_000, 0))
        assertTrue(clock.isRunning)

        // One line from 1 s to 2 s with words at 1.0-1.4 s and 1.5-2.0 s.
        val timeline = PortLyricTimeline(
            org.jellyfesh.ffi.LyricTimeline(
                lineStarts = listOf(1_000u),
                lineEnds = listOf(2_000u),
                segmentStarts = listOf(1_000u, 2_000u),
                segmentLines = listOf(0u, UInt.MAX_VALUE),
                wordOffsets = listOf(0u, 2u),
                wordStarts = listOf(1_000u, 1_500u),
                wordEnds = listOf(1_400u, 2_000u),
            ),
        )
        assertEquals(org.jellyfesh.logic.time.LyricCursor(0, 0, 0.5f), timeline.cursor(1_200))
        assertEquals(1_400L, timeline.nextChange(1_200))
        assertEquals(org.jellyfesh.logic.time.LyricCursor.None, timeline.cursor(2_500))
        assertNull(timeline.nextChange(2_500))
    }

    private fun waitFor(events: LinkedBlockingQueue<CoreEvent>, done: (CoreEvent) -> Boolean): CoreEvent {
        val end = System.nanoTime() + 30_000_000_000L
        while (true) {
            val left = end - System.nanoTime()
            val event = events.poll(left.coerceAtLeast(0), TimeUnit.NANOSECONDS) ?: throw AssertionError("timed out")
            if (done(event)) return event
        }
    }

    private fun drain(events: LinkedBlockingQueue<CoreEvent>, quietMs: Long) {
        while (events.poll(quietMs, TimeUnit.MILLISECONDS) != null) Unit
    }

    /** Half a second of a sine tone as 16-bit stereo PCM WAV. */
    private fun writeTone(file: File, hz: Double, rate: Int = 44_100, seconds: Double = 0.5) {
        val frames = (rate * seconds).toInt()
        val data = ByteBuffer.allocate(frames * 4).order(ByteOrder.LITTLE_ENDIAN)
        for (i in 0 until frames) {
            val v = (sin(2 * PI * hz * i / rate) * 0.3 * Short.MAX_VALUE).toInt().toShort()
            data.putShort(v).putShort(v)
        }
        val header = ByteBuffer.allocate(44).order(ByteOrder.LITTLE_ENDIAN)
        header.put("RIFF".toByteArray()).putInt(36 + frames * 4).put("WAVE".toByteArray())
        header.put("fmt ".toByteArray()).putInt(16).putShort(1).putShort(2).putInt(rate).putInt(rate * 4).putShort(4).putShort(16)
        header.put("data".toByteArray()).putInt(frames * 4)
        file.writeBytes(header.array() + data.array())
    }
}

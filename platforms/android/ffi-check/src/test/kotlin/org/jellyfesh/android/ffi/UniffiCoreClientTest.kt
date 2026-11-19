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
import org.jellyfesh.logic.core.RemoteAction
import org.jellyfesh.logic.core.RemoteDeviceKind
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
            distribution = org.jellyfesh.ffi.Distribution.SOURCE,
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

            // Export Playlist… and Import Playlist… go through these after the document picker:
            // the core writes M3U8 to a path and reads one back as a new playlist named after it.
            val exported = dir.resolve("Evening.m3u8")
            client.exportPlaylist(created.id, exported.path)
            val m3u = exported.readText()
            assertTrue(m3u.startsWith("#EXTM3U"), m3u)
            assertTrue("01 Harbor.wav" in m3u && "02 Lantern.wav" in m3u, m3u)
            val importRequest = client.send(Command.ImportPlaylist(exported.path))
            val imported = waitFor(events) { it is CoreEvent.PlaylistCreated && it.request == importRequest } as CoreEvent.PlaylistCreated
            assertEquals("Evening", client.playlist(imported.id)?.name)
            assertEquals(listOf("01 Harbor", "02 Lantern"), client.playlistEntries(imported.id, 0, 50).map { it.row.title })

            // Dragging a row by its handle sends MovePlaylistEntry with the row it lands on.
            val second = client.playlistEntries(imported.id, 0, 50)[1]
            client.send(Command.MovePlaylistEntry(imported.id, second.entryId, 0))
            waitFor(events) { it is CoreEvent.LibraryChanged }
            assertEquals(listOf("02 Lantern", "01 Harbor"), client.playlistEntries(imported.id, 0, 50).map { it.row.title })

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
            Command.Remote(RemoteAction.Refresh), Command.Remote(RemoteAction.Transfer("d-1")),
            Command.Remote(RemoteAction.SetVolume(64)), Command.Remote(RemoteAction.End),
        )
        val mapped = commands.map { FfiMapping.ffi(it) }
        assertEquals(commands.size, mapped.size)
        // Out-of-range values are clamped rather than wrapped.
        assertEquals(org.jellyfesh.ffi.Command.Seek(positionMs = 0uL), mapped[6])
        assertEquals(org.jellyfesh.ffi.Command.SetVolume(volume = 1f), mapped[7])
        assertEquals(
            org.jellyfesh.ffi.Command.Remote(org.jellyfesh.ffi.RemoteAction.SetVolume(percent = 100u)),
            FfiMapping.ffi(Command.Remote(RemoteAction.SetVolume(140))),
        )
        assertEquals(org.jellyfesh.ffi.Command.Remote(org.jellyfesh.ffi.RemoteAction.Transfer(device = "d-1")), mapped[mapped.size - 3])
        assertEquals(org.jellyfesh.ffi.Command.Remote(org.jellyfesh.ffi.RemoteAction.End), mapped.last())
    }

    @Test
    fun theRemoteSliceAndLinksMapIntoTheShellsModel() {
        val kitchen = org.jellyfesh.ffi.RemoteDevice("d-1", "Kitchen", org.jellyfesh.ffi.RemoteDeviceKind.SPEAKER, true, 64u)
        val tv = org.jellyfesh.ffi.RemoteDevice("d-2", "Living Room", org.jellyfesh.ffi.RemoteDeviceKind.TV, false, null)
        val item = org.jellyfesh.ffi.SourceItem(
            source = "spotify",
            kind = org.jellyfesh.ffi.ItemKind.TRACK,
            id = "t-9",
            title = "Gulls at Noon",
            subtitle = "Saskia Merrowhill",
            album = null,
            durationMs = 178_000uL,
            artwork = org.jellyfesh.ffi.ArtworkRef.Url("https://images.example/a.jpg"),
            playable = true,
            unplayableReason = null,
            track = null,
            alsoIn = emptyList(),
            link = "https://open.example/track/9",
            artistId = "ar-3",
            albumId = null,
        )
        val slice = org.jellyfesh.ffi.RemoteSnapshot(
            version = 4uL,
            source = "spotify",
            sourceName = "Spotify",
            status = org.jellyfesh.ffi.Status.PLAYING,
            item = item,
            context = null,
            anchor = org.jellyfesh.ffi.PlaybackAnchor(3uL, 42_000_000, 7uL, 1f, 180_000_000),
            shuffle = true,
            repeat = org.jellyfesh.ffi.RepeatMode.ONE,
            device = kitchen,
            devices = listOf(kitchen, tv),
            error = null,
        )
        val event = with(FfiMapping) { org.jellyfesh.ffi.Event.Remote(slice).toLogic { PortLyricTimeline(it) } }
        val remote = (event as CoreEvent.Remote).snapshot
        assertEquals(4L, remote.version)
        assertEquals("Spotify", remote.sourceName)
        assertEquals(Status.Playing, remote.status)
        assertEquals("https://open.example/track/9", remote.item?.link)
        assertEquals(org.jellyfesh.logic.core.PlaybackAnchor(3, 42_000_000, 7, 1f, 180_000_000), remote.anchor)
        assertEquals(org.jellyfesh.logic.core.RepeatMode.One, remote.repeat)
        assertEquals(64, remote.device?.volumePercent)
        assertEquals(listOf(RemoteDeviceKind.Speaker, RemoteDeviceKind.Tv), remote.devices.map { it.kind })
        assertNull(remote.devices[1].volumePercent)
    }

    @Test
    fun theFreeFunctionsReachTheShell() {
        library()
        val dir = Files.createTempDirectory("jf-android-ffi").toFile()
        try {
            val client = start(dir)
            // The equalizer's Preset picker lists the core's presets, in its order.
            val presets = client.eqPresets()
            assertEquals(listOf("flat", "bass_boost", "vocal", "treble"), presets.map { it.id })
            assertTrue(presets.all { it.settings.bands.size == 10 && it.settings.preampDb == 0f })
            assertTrue(presets.first().settings.bands.all { it.gainDb == 0f })
            // Settings › About shows the version from the build, never a typed-in one.
            assertTrue(client.libraryVersion().matches(Regex("\\d+\\.\\d+\\.\\d+.*")), client.libraryVersion())
            client.shutdown()
        } finally {
            dir.deleteRecursively()
        }
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

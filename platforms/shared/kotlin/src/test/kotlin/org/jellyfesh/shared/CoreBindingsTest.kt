package org.jellyfesh.shared

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Path
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import kotlin.math.PI
import kotlin.math.sin
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.double
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.jellyfesh.ffi.AlbumSort
import org.jellyfesh.ffi.AndroidContext
import org.jellyfesh.ffi.Collection
import org.jellyfesh.ffi.Command
import org.jellyfesh.ffi.CoreConfig
import org.jellyfesh.ffi.CoreFfiException
import org.jellyfesh.ffi.CoreListener
import org.jellyfesh.ffi.Event
import org.jellyfesh.ffi.JellyfeshCore
import org.jellyfesh.ffi.OutputKind
import org.jellyfesh.ffi.ScanStatus
import org.jellyfesh.ffi.SecretStorage
import org.jellyfesh.ffi.SecretStore
import org.jellyfesh.ffi.SecretStoreException
import org.jellyfesh.ffi.Status
import org.jellyfesh.ffi.TrackQuery
import org.jellyfesh.ffi.TrackRef
import org.jellyfesh.ffi.TrackSort
import org.jellyfesh.ffi.libraryVersion
import org.jellyfesh.ffi.monotonicNowNs
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

/** The generated UniFFI bindings against the jf-ffi library built here: tests/contract/core-scenario.json. */
class CoreBindingsTest {
    @Test
    fun theSharedScenario(@TempDir dir: Path) {
        val scenario = Contract.read("core-scenario.json")
        val timeoutMs = scenario.field("timeout_ms").long()
        val quietMs = scenario.field("quiet_ms").long()
        val pollMs = scenario.field("poll_ms").long()
        val music = writeTones(dir.resolve("music").toFile(), scenario)
        val expect = scenario.field("expect").jsonObject
        val config = scenario.field("config").jsonObject

        // 1. Start with the scenario's config and temporary folders.
        val core = JellyfeshCore(
            CoreConfig(
                dataDir = dir.resolve("data").toString(),
                cacheDir = dir.resolve("cache").toString(),
                output = output(config.field("output").jsonObject),
                bufferMs = config.field("buffer_ms").long().toUInt(),
                restoreSession = config.field("restore_session").jsonPrimitive.boolean,
                persistSession = config.field("persist_session").jsonPrimitive.boolean,
                watchFolders = config.field("watch_folders").jsonPrimitive.boolean,
            ),
        )
        val events = LinkedBlockingQueue<Event>()
        // Calls come one at a time, in order, from the core's event thread. (JNA attaches that
        // native thread to the JVM afresh for each call, so the Java Thread object differs.)
        val inCall = AtomicInteger()
        val overlapped = AtomicBoolean()
        core.setListener(
            object : CoreListener {
                override fun onEvent(event: Event) {
                    if (inCall.incrementAndGet() > 1) overlapped.set(true)
                    events.put(event)
                    inCall.decrementAndGet()
                }
            },
        )

        // 2. AddRoot scans the folder.
        assertTrue(core.send(Command.AddRoot(path = music.path)) > 0u)
        val scan = waitFor(events, timeoutMs) { it is Event.Scan && it.status is ScanStatus.Finished }
        val report = ((scan.last() as Event.Scan).status as ScanStatus.Finished).report
        assertEquals(expect.field("scan_added").long().toULong(), report.added)

        // 3. Query and search the library.
        val tracks = core.tracks(TrackQuery(sort = TrackSort.PATH))
        val search = scenario.field("search").jsonObject
        val found = core.searchLibrary(search.field("text").text(), search.field("limit").long().toUInt())
        assertEquals(search.field("titles").texts(), found.tracks.map { it.title })
        assertEquals(listOf(music.path), core.roots())
        val refs = tracks.map { TrackRef.Library(id = it.id) }

        // 4. A playlist of every track.
        val name = scenario.field("playlist").jsonObject.field("name").text()
        val request = core.send(Command.CreatePlaylist(name = name, tracks = refs))
        val created = waitFor(events, timeoutMs) { it is Event.PlaylistCreated && it.request == request }
        val playlist = (created.last() as Event.PlaylistCreated).id
        val summary = core.playlist(playlist)!!
        assertEquals(name to 2u, summary.name to summary.entryCount)
        assertEquals(
            expect.field("playlist_titles").texts(),
            core.playlistEntries(playlist, 0u, 50u).map { it.row.title },
        )
        assertTrue(core.playlists().any { it.id == playlist })

        // 5. A favourite.
        val favorite = refs[tracks.indexOfFirst { it.title == scenario.field("favorite").text() }]
        core.send(Command.SetFavorite(track = favorite, favorite = true))
        poll(timeoutMs, pollMs, "the favourite") { core.isFavorite(favorite) }
        assertEquals(expect.field("favorite_titles").texts(), core.favorites(0u, 50u).map { it.title })

        // 6. Export.
        val export = dir.resolve(scenario.field("playlist").jsonObject.field("export_file").text()).toFile()
        core.exportPlaylist(playlist, export.path)
        val text = export.readText()
        for (needle in scenario.field("playlist").jsonObject.field("export_contains").texts()) {
            assertTrue(needle in text, "$needle missing from $text")
        }

        // 7. Play the playlist.
        val id = core.send(Command.PlayCollection(collection = Collection.Playlist(id = playlist), start = 0u, shuffle = false))
        assertTrue(id > 0u)
        val finalStatus = Status.valueOf(expect.field("final_status").text().uppercase())
        val seen = waitFor(events, timeoutMs) { it is Event.Playback && it.snapshot.status == finalStatus }
        seen += drain(events, quietMs)
        // Consecutive announcements of the same entry count once (see the scenario's "about").
        val announced = seen.filterIsInstance<Event.NowPlaying>().mapNotNull { it.snapshot }
        val entries = announced.filterIndexed { i, np -> i == 0 || announced[i - 1].uid != np.uid }
        assertEquals(
            expect.field("now_playing_titles").texts(),
            entries.map { it.title },
            "announced (uid, title): ${announced.map { it.uid to it.title }}",
        )
        val snapshot = core.snapshot()
        assertEquals(finalStatus, snapshot.playback.status)
        assertEquals(expect.field("queue_len").long().toUInt(), snapshot.queue.len)
        assertEquals(seen.filterIsInstance<Event.Playback>().last().snapshot, snapshot.playback)
        assertEquals(seen.filterIsInstance<Event.Queue>().last().snapshot, snapshot.queue)
        assertEquals(seen.filterIsInstance<Event.NowPlaying>().last().snapshot, snapshot.nowPlaying)
        assertTrue(!overlapped.get(), "listener calls overlapped")

        // 8. Play statistics.
        val played = expect.field("played_titles").texts()
        poll(timeoutMs, pollMs, "the plays") {
            core.recentlyPlayed(50u).map { it.title }.distinct().sorted() == played
        }

        // 9. Sorted albums and thumbnails.
        assertEquals(emptyList<Any>(), core.albumsSorted(AlbumSort.TITLE, false, 0u, 50u))
        assertNull(core.artworkThumbnail(scenario.field("unknown_artwork_hash").text(), 128u))

        // 10. Shutdown; errors are CoreFfiException subclasses.
        core.shutdown()
        assertThrows(CoreFfiException.ShutDown::class.java) { core.stats() }
        assertEquals(0uL, core.send(Command.Play))
        assertEquals(snapshot, core.snapshot())
        drain(events, 100)
        assertNull(events.poll(200, TimeUnit.MILLISECONDS), "no events after shutdown")
        core.close()
    }

    private fun poll(timeoutMs: Long, everyMs: Long, what: String, probe: () -> Boolean) {
        val end = System.nanoTime() + timeoutMs * 1_000_000
        while (!probe()) {
            if (System.nanoTime() > end) throw AssertionError("timed out waiting for $what")
            Thread.sleep(everyMs)
        }
    }

    /** The app's secret store: records each key asked for; every call throws when locked. */
    private class Keychain(private val locked: Boolean) : SecretStore {
        val asked = ConcurrentLinkedQueue<String>()

        private fun answer(key: String) {
            asked.add(key)
            if (locked) throw SecretStoreException.Unavailable(reason = "the keychain is locked")
        }

        override fun get(key: String): String? {
            answer(key)
            return null
        }

        override fun set(key: String, value: String) = answer(key)

        override fun delete(key: String) = answer(key)
    }

    @Test
    fun theAppsSecretStoreKeepsTheSignIns(@TempDir dir: Path) {
        for ((locked, storage) in listOf(false to SecretStorage.KEYSTORE, true to SecretStorage.SESSION_ONLY)) {
            val keychain = Keychain(locked)
            val folder = dir.resolve(if (locked) "locked" else "open")
            val core = JellyfeshCore.withSecretStore(
                CoreConfig(
                    dataDir = folder.resolve("data").toString(),
                    cacheDir = folder.resolve("cache").toString(),
                    output = OutputKind.Null(realtime = false),
                    restoreSession = false,
                    persistSession = false,
                    watchFolders = false,
                ),
                keychain,
            )
            // The core checks the store once at start by reading an entry no plugin can name.
            poll(10_000, 10, "the check") { keychain.asked.isNotEmpty() }
            assertEquals("jellyfesh/~probe", keychain.asked.first())
            poll(10_000, 10, "the slice") { core.snapshot().sources.secretStorage == storage }
            assertNull(core.snapshot().sources.unavailable)
            core.shutdown()
            core.close()
        }
    }

    @Test
    fun freeFunctionsAndTheAndroidContext() {
        assertTrue(libraryVersion().isNotEmpty())
        val before = monotonicNowNs()
        Thread.sleep(2)
        assertTrue(monotonicNowNs() >= before + 1_000_000u)
        // The same JNI path as on Android: loads the library through System and hands it a
        // context object (here any object; nothing reads it off Android).
        assertTrue(AndroidContext.initialize(Any()))
        assertTrue(AndroidContext.initialize(Any()), "a second call keeps the first context")
    }

    private fun output(json: JsonObject): OutputKind = when (json.field("type").text()) {
        "Null" -> OutputKind.Null(realtime = json.field("realtime").jsonPrimitive.boolean)
        "Device" -> OutputKind.Device(name = json["name"]?.takeUnless { it.isNull() }?.text())
        else -> error("unsupported output $json")
    }

    private fun waitFor(events: LinkedBlockingQueue<Event>, timeoutMs: Long, done: (Event) -> Boolean): MutableList<Event> {
        val end = System.nanoTime() + timeoutMs * 1_000_000
        val seen = mutableListOf<Event>()
        while (true) {
            val left = end - System.nanoTime()
            val event = events.poll(left.coerceAtLeast(0), TimeUnit.NANOSECONDS)
                ?: throw AssertionError("timed out; events seen: $seen")
            seen += event
            if (done(event)) return seen
        }
    }

    private fun drain(events: LinkedBlockingQueue<Event>, quietMs: Long): List<Event> =
        generateSequence { events.poll(quietMs, TimeUnit.MILLISECONDS) }.toList()

    /** Writes the scenario's tones as 16-bit stereo WAV files. */
    private fun writeTones(folder: File, scenario: JsonObject): File {
        folder.mkdirs()
        val rate = scenario.field("sample_rate").jsonPrimitive.int
        for (tone in scenario.field("tones").jsonArray) {
            tone as JsonObject
            val frequency = tone.field("frequency_hz").jsonPrimitive.double
            val frames = (rate * tone.field("seconds").jsonPrimitive.double).toInt()
            val wav = ByteBuffer.allocate(44 + frames * 4).order(ByteOrder.LITTLE_ENDIAN)
            wav.put("RIFF".toByteArray()).putInt(36 + frames * 4).put("WAVE".toByteArray())
            wav.put("fmt ".toByteArray()).putInt(16).putShort(1).putShort(2).putInt(rate).putInt(rate * 4)
                .putShort(4).putShort(16)
            wav.put("data".toByteArray()).putInt(frames * 4)
            for (i in 0 until frames) {
                val sample = (sin(2 * PI * frequency * i / rate) * 0.25 * Short.MAX_VALUE).toInt().toShort()
                wav.putShort(sample).putShort(sample)
            }
            folder.resolve(tone.field("file").text()).writeBytes(wav.array())
        }
        return folder
    }
}

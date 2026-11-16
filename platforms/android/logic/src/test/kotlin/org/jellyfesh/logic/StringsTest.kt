package org.jellyfesh.logic

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.w3c.dom.Element

/**
 * The app's strings (strings.xml in app/src/main/res/values and its translations): every
 * translation has every string of the English source with the same placeholders, and the English
 * uses docs/ui.md's words.
 */
class StringsTest {
    private val res: File by lazy {
        var dir: File? = File("").absoluteFile
        while (dir != null && !dir.resolve("app/src/main/res/values/strings.xml").isFile) dir = dir.parentFile
        requireNotNull(dir) { "app/src/main/res not found" }.resolve("app/src/main/res")
    }

    /** name -> the texts of a string (one) or of a plural's items, for one locale's file. */
    private fun read(folder: String): Map<String, List<String>> {
        val doc = DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(res.resolve("$folder/strings.xml"))
        val out = LinkedHashMap<String, List<String>>()
        val nodes = doc.documentElement.childNodes
        for (i in 0 until nodes.length) {
            val e = nodes.item(i) as? Element ?: continue
            if (e.getAttribute("translatable") == "false") continue
            out[e.getAttribute("name")] = when (e.tagName) {
                "plurals" -> (0 until e.childNodes.length).mapNotNull { (e.childNodes.item(it) as? Element)?.textContent }
                else -> listOf(e.textContent)
            }
        }
        return out
    }

    private val locales = listOf("values-zh-rCN", "values-zh-rTW", "values-ja")

    private val placeholder = Regex("%(\\d+\\$)?[,]?[sd]")

    @Test
    fun everyTranslationHasEveryStringWithItsPlaceholders() {
        val english = read("values")
        for (locale in locales) {
            val translated = read(locale)
            assertEquals(english.keys, translated.keys, "$locale has other strings than values/")
            for ((name, texts) in english) {
                val want = texts.flatMap { placeholder.findAll(it).map { m -> m.value.removePrefix("%").replace(",", "") } }.toSet()
                val got = translated.getValue(name).flatMap { placeholder.findAll(it).map { m -> m.value.removePrefix("%").replace(",", "") } }.toSet()
                assertEquals(want, got, "$locale/$name placeholders")
            }
        }
    }

    @Test
    fun theEnglishUsesTheSpecsWords() {
        val english = read("values").values.flatten()
        // docs/ui.md, "Words": the Not column.
        val avoided = listOf(
            "Preferences", "Add a music folder…", "Scan again", "Play Later", "Add to queue", "The Queue Is Empty",
            "About this track", "Date Added", "Nothing found.", "Installed sources", "Install from a file…",
            "Server address", "Sign in with a QR code", "Manage sources", "Follow System", "Length in seconds", "Purple",
            // v0.0.6: links and remote playback.
            "Open in Browser", "Open Link", "Connected to", "Casting to", "Disconnect", "Stop Remote",
        )
        for (word in avoided) {
            assertTrue(english.none { it.contains(word) }, "\"$word\" is in the strings")
        }
        val names = read("values")
        assertEquals("System", names.getValue("appearance_system").single())
        assertEquals("Reset to Flat", names.getValue("eq_reset").single())
        assertEquals("Server Address", names.getValue("sources_connect_address").single())
        assertEquals("Sources Settings…", names.getValue("sources_settings").single())
        assertEquals("Uninstall…", names.getValue("action_uninstall_ask").single())
        assertEquals("Violet", names.getValue("accent_violet").single())
        assertEquals("Open in %1\$s", names.getValue("action_open_in").single())
        assertEquals("Playing on %1\$s", names.getValue("remote_playing_on").single())
        assertEquals("Devices", names.getValue("remote_devices").single())
        assertEquals("Stop Controlling", names.getValue("remote_stop").single())
        assertEquals("Lyrics aren’t shown while %1\$s plays.", names.getValue("remote_lyrics").single())
        assertEquals("Up Next is in %1\$s.", names.getValue("remote_queue").single())
    }
}

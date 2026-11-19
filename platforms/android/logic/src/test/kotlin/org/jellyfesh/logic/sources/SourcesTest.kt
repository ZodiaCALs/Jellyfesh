package org.jellyfesh.logic.sources

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import org.jellyfesh.logic.core.AuthField
import org.jellyfesh.logic.core.AuthFieldKind
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.AuthStep
import org.jellyfesh.logic.core.AuthUpdate
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SourcePreset

class AuthFlowTest {
    private val fields = listOf(AuthField("server", "Server", false, "https://"), AuthField("password", "Password", true, null))

    @Test
    fun aPasswordFlowSubmitsOnceAndForgetsTheSecret() {
        val (start, begin) = AuthFlow.begin("org.jellyfesh.subsonic", AuthMethod.PasswordExchange)
        assertEquals(Command.BeginLogin("org.jellyfesh.subsonic", AuthMethod.PasswordExchange), begin)
        var session = AuthFlow.onUpdate(start, AuthUpdate("org.jellyfesh.subsonic", 5, AuthStep.AskFields(fields)))!!
        assertEquals(5L, session.flow)
        session = AuthFlow.edit(session, "server", "https://music.example")
        assertNull(AuthFlow.submit(session), "the password is still missing")
        session = AuthFlow.edit(session, "password", "hunter2")
        val (sent, command) = AuthFlow.submit(session)!!
        assertEquals(Command.SubmitLogin("org.jellyfesh.subsonic", 5, mapOf("server" to "https://music.example", "password" to "hunter2")), command)
        assertTrue(sent.values.isEmpty(), "secrets never stay in the shell")
    }

    private val navidrome = SourcePreset(
        id = "navidrome",
        name = "Navidrome",
        plugin = "org.jellyfesh.subsonic",
        authMethods = listOf(AuthMethod.PasswordExchange),
        needsAddress = true,
        address = null,
        addressExample = "http://192.168.1.20:4533",
        account = null,
    )

    @Test
    fun aServiceConnectsWithTheTrimmedAddressAndSignsInToItsPlugin() {
        val (session, command) = AuthFlow.connect(navidrome, "  https://music.example \n", AuthMethod.PasswordExchange)!!
        assertEquals(Command.ConnectSource("navidrome", "https://music.example", AuthMethod.PasswordExchange), command)
        // The plugin's own sign-in session, so the steps the plugin sends reach it.
        assertEquals(AuthFlow.begin("org.jellyfesh.subsonic", AuthMethod.PasswordExchange).first, session)
        val asked = AuthFlow.onUpdate(session, AuthUpdate("org.jellyfesh.subsonic", 7, AuthStep.AskFields(fields)))!!
        assertEquals(7L, asked.flow)
    }

    @Test
    fun anEmptyAddressSendsNothing() {
        assertFalse(AuthFlow.canConnect(navidrome, " "))
        assertNull(AuthFlow.connect(navidrome, "", AuthMethod.PasswordExchange))
        assertNull(AuthFlow.connect(navidrome, " \t ", AuthMethod.PasswordExchange))
        // A service that takes no address connects without one, whatever the field holds.
        val hosted = navidrome.copy(id = "hosted", needsAddress = false, addressExample = null)
        assertTrue(AuthFlow.canConnect(hosted, ""))
        assertEquals(Command.ConnectSource("hosted", null, AuthMethod.Qr), AuthFlow.connect(hosted, "https://music.example", AuthMethod.Qr)?.second)
    }

    @Test
    fun stepsForOtherPluginsOrFlowsAreIgnored() {
        val session = AuthSession("a", AuthMethod.Qr, flow = 2)
        assertEquals(session, AuthFlow.onUpdate(session, AuthUpdate("b", 2, AuthStep.Done("x"))))
        assertEquals(session, AuthFlow.onUpdate(session, AuthUpdate("a", 3, AuthStep.Done("x"))))
        assertTrue(AuthFlow.onUpdate(session, AuthUpdate("a", 2, AuthStep.Done("x")))!!.finished)
    }

    @Test
    fun theBrowserOpensOncePerUrl() {
        var session = AuthFlow.onUpdate(AuthSession("p", AuthMethod.Oauth), AuthUpdate("p", 1, AuthStep.OpenUrl("https://id.example/auth")))!!
        assertEquals("https://id.example/auth", session.urlToOpen)
        session = AuthFlow.urlOpened(session, "https://id.example/auth")
        assertNull(session.urlToOpen)
        assertEquals(
            Command.SubmitLogin("p", 1, mapOf(AuthFlow.REDIRECT_FIELD to "jellyfesh://auth?code=1")),
            AuthFlow.redirect(session, "jellyfesh://auth?code=1"),
        )
        assertEquals(Command.CancelLogin("p", 1), AuthFlow.cancel(session))
    }

    @Test
    fun optionalFieldsMayStayEmpty() {
        val form = listOf(
            AuthField("user", "User Name", false, null),
            AuthField("password", "Password", true, null),
            AuthField("device", "Device Name", false, "Jellyfesh", required = false, help = "Shown in the server’s list of devices."),
        )
        var session = AuthFlow.onUpdate(AuthSession("p", AuthMethod.PasswordExchange), AuthUpdate("p", 3, AuthStep.AskFields(form)))!!
        session = AuthFlow.edit(session, "user", "listener")
        assertEquals(listOf("password"), AuthFlow.missing(form, session.values).map { it.name })
        session = AuthFlow.edit(session, "password", "   ")
        assertEquals(listOf("password"), AuthFlow.missing(form, session.values).map { it.name }, "spaces alone do not count")
        session = AuthFlow.edit(session, "password", "hunter2")
        val (_, command) = AuthFlow.submit(session)!!
        // The empty optional field goes along empty, as in every app.
        assertEquals(Command.SubmitLogin("p", 3, mapOf("user" to "listener", "password" to "hunter2", "device" to "")), command)
    }

    @Test
    fun eachFieldIsTypedAsItsKindAsks() {
        fun field(kind: AuthFieldKind, secret: Boolean = false) = AuthField("f", "F", secret, null, kind)
        assertEquals(FieldInput.Text, AuthFlow.input(field(AuthFieldKind.Text)))
        assertEquals(FieldInput.Email, AuthFlow.input(field(AuthFieldKind.Email)))
        assertEquals(FieldInput.Url, AuthFlow.input(field(AuthFieldKind.Url)))
        assertEquals(FieldInput.Digits, AuthFlow.input(field(AuthFieldKind.Number)))
        assertEquals(FieldInput.Lines, AuthFlow.input(field(AuthFieldKind.CookieHeader)))
        // A secret is masked whatever its kind.
        assertEquals(FieldInput.Secret, AuthFlow.input(field(AuthFieldKind.Number, secret = true)))
        assertEquals(FieldInput.Secret, AuthFlow.input(field(AuthFieldKind.CookieHeader, secret = true)))
    }

    @Test
    fun aQuickConnectCodeIsItsOwnStepAndSpelledOut() {
        val session = AuthFlow.onUpdate(
            AuthSession("org.jellyfesh.jellyfin", AuthMethod.Qr),
            AuthUpdate("org.jellyfesh.jellyfin", 4, AuthStep.ShowCode("482 913", "Enter this code in Quick Connect on a signed-in device.")),
        )!!
        assertEquals("482 913", (session.step as AuthStep.ShowCode).code)
        assertFalse(session.finished)
        assertEquals("4 8 2 9 1 3", AuthFlow.spelled("482 913"))
        assertEquals("A B C 1", AuthFlow.spelled(" AB\tC1 "))
    }

    @Test
    fun theDisclosureGuardsCommunityPluginsOnly() {
        val community = PluginSummary("c", "C", "1", "someone", PluginBadge.Community, false, emptyList(), emptyList(), false, null, true, "{}", "{}", null)
        val official = community.copy(id = "o", badge = PluginBadge.Official, needsDisclosure = false)
        assertTrue(Disclosure.needsDisclosureToEnable(community, enable = true))
        assertFalse(Disclosure.needsDisclosureToEnable(community, enable = false))
        assertFalse(Disclosure.needsDisclosureToEnable(official, enable = true))
        assertEquals(listOf(Command.AcknowledgeUnofficial("c"), Command.SetPluginEnabled("c", true)), Disclosure.acceptedFor(community))
        assertTrue(Disclosure.needsDisclosureToAllow(true))
        assertFalse(Disclosure.needsDisclosureToAllow(false))
    }
}

class SettingsSchemaTest {
    private val schema = """
        {
          "type": "object",
          "required": ["server_url", "port"],
          "properties": {
            "server_url": {"type": "string", "title": "Server", "format": "uri", "examples": ["https://music.example"]},
            "api_key": {"type": "string", "title": "API key", "writeOnly": true},
            "port": {"type": "integer", "title": "Port", "minimum": 1, "maximum": 65535, "default": 4533},
            "quality": {"title": "Quality", "enum": ["low", "high"], "x-enum-labels": ["Low", "High"], "default": "high"},
            "scrobble": {"type": "boolean", "title": "Scrobble"},
            "nested": {"type": "object"}
          }
        }
    """.trimIndent()

    @Test
    fun parsesFieldsInOrder() {
        val fields = SettingsSchema.parse(schema)
        assertEquals(listOf("server_url", "api_key", "port", "quality", "scrobble"), fields.map { it.key })
        val server = fields[0] as SchemaField.Text
        assertTrue(server.url && server.required)
        assertEquals("https://music.example", server.placeholder)
        assertTrue((fields[1] as SchemaField.Text).secret)
        val quality = fields[3] as SchemaField.Choice
        assertEquals(listOf("Low", "High"), quality.options.map { it.label })
    }

    @Test
    fun readsDefaultsAndCurrentValues() {
        val fields = SettingsSchema.parse(schema)
        val values = SettingsSchema.values(fields, """{"server_url": "https://a.example", "scrobble": true}""")
        assertEquals("https://a.example", values["server_url"])
        assertEquals("4533", values["port"])
        assertEquals("high", values["quality"])
        assertEquals("true", values["scrobble"])
        assertEquals("", values["api_key"])
    }

    @Test
    fun validates() {
        val fields = SettingsSchema.parse(schema)
        val errors = SettingsSchema.validate(fields, mapOf("server_url" to "", "port" to "70000", "quality" to "medium"))
        assertEquals(mapOf("server_url" to FieldError.Required, "port" to FieldError.AboveMaximum, "quality" to FieldError.NotAnOption), errors)
        assertEquals(FieldError.NotAnInteger, SettingsSchema.validate(fields, mapOf("server_url" to "x", "port" to "1.5"))["port"])
        assertTrue(SettingsSchema.validate(fields, mapOf("server_url" to "x", "port" to "443")).isEmpty())
    }

    @Test
    fun writesTypedJsonAndKeepsUnknownKeys() {
        val fields = SettingsSchema.parse(schema)
        val out = SettingsSchema.toJson(
            fields,
            mapOf("server_url" to "https://b.example", "port" to "443", "scrobble" to "false", "quality" to "low", "api_key" to ""),
            original = """{"legacy": 1, "port": 4533}""",
        )
        val json = Json.parseToJsonElement(out).jsonObject
        assertEquals(JsonPrimitive(1), json["legacy"])
        assertEquals(JsonPrimitive(443L), json["port"])
        assertEquals(JsonPrimitive(false), json["scrobble"])
        assertEquals(JsonPrimitive("low"), json["quality"])
        assertEquals("null", json["api_key"].toString())
    }

    @Test
    fun anUnreadableSchemaHasNoFields() {
        assertTrue(SettingsSchema.parse("not json").isEmpty())
        assertTrue(SettingsSchema.parse("{}").isEmpty())
    }
}

/** Settings schema profile 1.2 (docs/plugins/authoring.md), as the Customize and AI Lyrics plugins use it. */
class SettingsSchemaProfile12Test {
    private val schema = """
        {
          "type": "object",
          "properties": {
            "corners": {"type": "number", "title": "Corners", "minimum": 0, "maximum": 2, "multipleOf": 0.05, "default": 1, "x-jf-group": "Look"},
            "typeface": {"type": "string", "title": "Typeface", "oneOf": [
              {"const": "system", "title": "System"}, {"const": "rounded", "title": "Rounded"},
              {"const": "serif", "title": "Serif"}, {"const": "mono", "title": "Monospaced"}], "default": "system", "x-jf-group": "Look"},
            "density": {"type": "string", "title": "Density", "oneOf": [
              {"const": "compact", "title": "Compact"}, {"const": "standard", "title": "Standard"}, {"const": "roomy", "title": "Roomy"}],
              "default": "standard", "x-jf-group": "Look"},
            "highlight_colour": {"type": "string", "format": "color", "title": "Highlight Colour", "default": "#ff8a3d", "x-jf-group": "Lyrics"},
            "home": {"type": "array", "title": "Home", "uniqueItems": true, "x-jf-group": "Layout",
              "items": {"type": "string", "oneOf": [
                {"const": "summary", "title": "Library Summary"}, {"const": "recently-added", "title": "Recently Added"},
                {"const": "favourites", "title": "Favourites"}]},
              "default": ["summary", "recently-added"]},
            "port": {"type": "integer", "title": "Port", "minimum": 1, "maximum": 65535},
            "automatic": {"type": "boolean", "title": "Create Automatically"}
          }
        }
    """.trimIndent()

    @Test
    fun readsTheNewKeywords() {
        val fields = SettingsSchema.parse(schema).associateBy { it.key }
        val corners = fields.getValue("corners") as SchemaField.Number
        assertTrue(corners.slider)
        assertEquals(0.05, corners.multipleOf)
        assertEquals("Look", corners.group)
        val typeface = fields.getValue("typeface") as SchemaField.Choice
        assertEquals(listOf("System", "Rounded", "Serif", "Monospaced"), typeface.options.map { it.label })
        // Four choices open a list; three fit a segmented control.
        assertFalse(typeface.segmented)
        assertTrue((fields.getValue("density") as SchemaField.Choice).segmented)
        assertEquals("#FF8A3D", (fields.getValue("highlight_colour") as SchemaField.Colour).default)
        val home = fields.getValue("home") as SchemaField.Checklist
        assertEquals(listOf("Library Summary", "Recently Added", "Favourites"), home.options.map { it.label })
        assertEquals(listOf("summary", "recently-added"), home.default)
        // A port's range is too wide for a slider.
        assertFalse((fields.getValue("port") as SchemaField.Number).slider)
    }

    @Test
    fun consecutiveFieldsOfAGroupShareItsHeading() {
        val groups = SettingsSchema.groups(SettingsSchema.parse(schema))
        assertEquals(listOf("Look", "Lyrics", "Layout", null), groups.map { it.title })
        assertEquals(listOf("corners", "typeface", "density"), groups[0].fields.map { it.key })
        assertEquals(listOf("port", "automatic"), groups[3].fields.map { it.key })
    }

    @Test
    fun valuesValidationAndJsonOfColoursChecklistsAndSliders() {
        val fields = SettingsSchema.parse(schema)
        val values = SettingsSchema.values(fields, """{"home": ["favourites", "summary"], "highlight_colour": "#00aaff"}""")
        assertEquals(listOf("favourites", "summary"), SettingsSchema.checklist(values["home"]))
        assertEquals("#00AAFF", values["highlight_colour"])
        assertEquals("1", values["corners"])
        // Defaults when the plugin has none stored.
        assertEquals(listOf("summary", "recently-added"), SettingsSchema.checklist(SettingsSchema.values(fields, "{}")["home"]))
        assertEquals("#FF8A3D", SettingsSchema.values(fields, "{}")["highlight_colour"])

        val bad = values + ("highlight_colour" to "orange") + ("home" to SettingsSchema.checklistText(listOf("summary", "summary")))
        assertEquals(mapOf("highlight_colour" to FieldError.NotAColour, "home" to FieldError.NotAnOption), SettingsSchema.validate(fields, bad))
        assertEquals(FieldError.NotAnOption, SettingsSchema.validate(fields, values + ("home" to "[\"charts\"]"))["home"])
        // Hiding every item of a checklist is allowed: the plugin decides what an empty list means.
        assertTrue(SettingsSchema.validate(fields, values + ("home" to "[]")).isEmpty())

        val json = Json.parseToJsonElement(
            SettingsSchema.toJson(fields, values + ("corners" to "1.25") + ("highlight_colour" to "ff8a3d"), "{}"),
        ).jsonObject
        assertEquals("[\"favourites\",\"summary\"]", json["home"].toString())
        assertEquals(JsonPrimitive("#FF8A3D"), json["highlight_colour"])
        assertEquals(JsonPrimitive(1.25), json["corners"])
    }

    @Test
    fun slidersSnapToTheirStepWithinTheirRange() {
        val corners = SettingsSchema.parse(schema).first() as SchemaField.Number
        assertEquals(0.75, SettingsSchema.snap(corners, 0.7712))
        assertEquals(2.0, SettingsSchema.snap(corners, 7.0))
        assertEquals(0.0, SettingsSchema.snap(corners, -1.0))
        assertEquals(0.3, SettingsSchema.snap(corners, 0.1 + 0.2))
        val textSize = corners.copy(minimum = 0.85, maximum = 1.3)
        assertEquals(1.05, SettingsSchema.snap(textSize, 1.06))
        assertEquals("0.75", SettingsSchema.numberText(0.75))
        assertEquals("1", SettingsSchema.numberText(1.0))
        assertEquals("1.05", SettingsSchema.numberText(1.05))
    }

    @Test
    fun everyLanguageButEnglishAsksForTheTranslatedSchema() {
        assertFalse(SettingsSchema.wantsTranslation("en"))
        assertFalse(SettingsSchema.wantsTranslation("en-GB"))
        assertFalse(SettingsSchema.wantsTranslation("und"))
        assertTrue(SettingsSchema.wantsTranslation("zh-Hans-CN"))
        assertTrue(SettingsSchema.wantsTranslation("zh-TW"))
        assertTrue(SettingsSchema.wantsTranslation("ja_JP"))
    }
}

class FormControlsTest {
    @Test
    fun colourWellsReadAndWriteHexAndHsv() {
        assertEquals(0xFF8A3D, ColourWell.parse("#ff8a3d"))
        assertEquals(0xFF8A3D, ColourWell.parse("FF8A3D"))
        assertNull(ColourWell.parse("#FF8A3"))
        assertNull(ColourWell.parse("orange"))
        assertEquals("#00AAFF", ColourWell.hex(0x00AAFF))
        for (rgb in listOf(0xFF8A3D, 0x000000, 0xFFFFFF, 0x3366CC, 0x12AB34, 0xFF0000)) {
            assertEquals(rgb, ColourWell.rgb(ColourWell.hsv(rgb)), ColourWell.hex(rgb))
        }
        val red = ColourWell.hsv(0xFF0000)
        assertEquals(0f, red.hue)
        assertEquals(1f, red.saturation)
        assertEquals(1f, red.value)
        assertEquals(0x0000FF, ColourWell.rgb(ColourWell.Hsv(240f, 1f, 1f)))
    }

    @Test
    fun checklistsShowTheShownItemsFirstAndMoveThem() {
        val field = SettingsSchema.parse(
            """{"properties": {"home": {"type": "array", "uniqueItems": true, "items": {"type": "string",
              "oneOf": [{"const": "a", "title": "A"}, {"const": "b", "title": "B"}, {"const": "c", "title": "C"}]}}}}""",
        ).single() as SchemaField.Checklist
        val rows = Checklists.rows(field, listOf("c", "a"))
        assertEquals(listOf("c", "a", "b"), rows.map { it.value })
        assertEquals(listOf(true, true, false), rows.map { it.shown })
        assertEquals(listOf("c", "a", "b"), Checklists.toggle(listOf("c", "a"), "b", on = true))
        assertEquals(listOf("a"), Checklists.toggle(listOf("c", "a"), "c", on = false))
        assertEquals(listOf("a", "c"), Checklists.move(listOf("c", "a"), 0, 1))
        assertEquals(listOf("c", "a"), Checklists.move(listOf("c", "a"), 0, 5))
    }

    @Test
    fun draggingAShownItemMovesTheOthersAsideAndDropsItWhereItLands() {
        val shown = listOf("a", "b", "c", "d")
        val drag = Checklists.Drag(from = 0, offsetPx = 110f)
        // Two rows of 52 px down (110 / 52 rounds to 2): b and c move up a row.
        assertEquals(2, Checklists.target(drag, 52f, 4))
        assertEquals(110f, Checklists.shift(0, drag, 52f, 4))
        assertEquals(-52f, Checklists.shift(1, drag, 52f, 4))
        assertEquals(-52f, Checklists.shift(2, drag, 52f, 4))
        assertEquals(0f, Checklists.shift(3, drag, 52f, 4))
        assertEquals(listOf("b", "c", "a", "d"), Checklists.drop(shown, drag, 52f))
        assertEquals(listOf("a", "d", "b", "c"), Checklists.drop(shown, Checklists.Drag(3, -100f), 52f))
        assertEquals(shown, Checklists.drop(shown, Checklists.Drag(1, 10f), 52f))
    }
}


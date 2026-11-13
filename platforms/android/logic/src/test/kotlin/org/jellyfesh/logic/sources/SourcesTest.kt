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
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.AuthStep
import org.jellyfesh.logic.core.AuthUpdate
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary

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
    fun qrCountdownRoundsUp() {
        val step = AuthStep.ShowQr("data", expiresAtMs = 10_500)
        assertEquals(1, AuthFlow.secondsLeft(step, 9_600))
        assertEquals(0, AuthFlow.secondsLeft(step, 11_000))
        assertNull(AuthFlow.secondsLeft(AuthStep.ShowQr("data", null), 0))
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

package org.jellyfesh.logic.sources

import org.jellyfesh.logic.core.AuthField
import org.jellyfesh.logic.core.AuthFieldKind
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.AuthStep
import org.jellyfesh.logic.core.AuthUpdate
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SourcePreset

/**
 * A sign-in flow as the sheet shows it. The core runs the flow (ADR-0004); the shell renders each
 * step natively: a QR code, the browser (Custom Tabs), a form, a wait, the result.
 */
data class AuthSession(
    val plugin: String,
    val method: AuthMethod,
    /** The core's flow id, known from the first step. */
    val flow: Long? = null,
    val step: AuthStep? = null,
    /** What the person typed into an [AuthStep.AskFields] form. Secrets live only here. */
    val values: Map<String, String> = emptyMap(),
    /** The URL already opened in the browser, so a repeated step does not open it twice. */
    val openedUrl: String? = null,
) {
    val finished: Boolean get() = step is AuthStep.Done || step is AuthStep.Failed

    /** The browser should open this URL now. */
    val urlToOpen: String?
        get() = (step as? AuthStep.OpenUrl)?.url?.takeIf { it != openedUrl }
}

object AuthFlow {
    /** Starts a flow: the session and the command to send. */
    fun begin(plugin: String, method: AuthMethod): Pair<AuthSession, Command> =
        AuthSession(plugin, method) to Command.BeginLogin(plugin, method)

    /**
     * Whether a service from Add a Source can be connected to with [address] in its field: a
     * self-hosted server needs one, and the core fails `ConnectSource` without it.
     */
    fun canConnect(preset: SourcePreset, address: String): Boolean = !preset.needsAddress || address.isNotBlank()

    /**
     * Starts a flow for a service from Add a Source: the session and the command to send, or null
     * while the service needs an address and [address] is blank. One `ConnectSource` sets the
     * address, turns the plugin on and begins signing in, so the sign-in never starts before the
     * address is set. The session is the plugin's, as [begin] makes it, because the steps arrive
     * as Auth events for the plugin, not for the service.
     */
    fun connect(preset: SourcePreset, address: String, method: AuthMethod): Pair<AuthSession, Command>? {
        if (!canConnect(preset, address)) return null
        val server = address.trim().takeIf { preset.needsAddress }
        return AuthSession(preset.plugin, method) to Command.ConnectSource(preset.id, server, method)
    }

    /** Applies a step from the core; steps for other plugins or flows are ignored. */
    fun onUpdate(session: AuthSession?, update: AuthUpdate): AuthSession? {
        if (session == null || update.plugin != session.plugin) return session
        if (session.flow != null && session.flow != update.flow) return session
        val values = if (update.step is AuthStep.AskFields) session.values.filterKeys { k -> update.step.fields.any { it.name == k } } else session.values
        return session.copy(flow = update.flow, step = update.step, values = values)
    }

    fun urlOpened(session: AuthSession, url: String): AuthSession = session.copy(openedUrl = url)

    fun edit(session: AuthSession, field: String, value: String): AuthSession =
        session.copy(values = session.values + (field to value))

    /** The required fields that are still empty (spaces alone do not count); optional ones may stay empty. */
    fun missing(fields: List<AuthField>, values: Map<String, String>): List<AuthField> =
        fields.filter { it.required && values[it.name].isNullOrBlank() }

    /**
     * How [field] is typed: masked when it is secret, several lines for a pasted cookie header,
     * otherwise one line with the keyboard its kind asks for (as every app does).
     */
    fun input(field: AuthField): FieldInput = when {
        field.secret -> FieldInput.Secret
        else -> when (field.kind) {
            AuthFieldKind.Text -> FieldInput.Text
            AuthFieldKind.Email -> FieldInput.Email
            AuthFieldKind.Url -> FieldInput.Url
            AuthFieldKind.Number -> FieldInput.Digits
            AuthFieldKind.CookieHeader -> FieldInput.Lines
        }
    }

    /** A sign-in code as TalkBack spells it, one character at a time: `AB 12` reads "A B 1 2". */
    fun spelled(code: String): String = code.filterNot { it.isWhitespace() }.toList().joinToString(" ")

    /**
     * Sends the form. The values leave the session at once: secrets pass through to the core one
     * time and are never kept (ADR-0015).
     */
    fun submit(session: AuthSession): Pair<AuthSession, Command>? {
        val flow = session.flow ?: return null
        val step = session.step as? AuthStep.AskFields ?: return null
        if (missing(step.fields, session.values).isNotEmpty()) return null
        val fields = step.fields.associate { it.name to session.values[it.name].orEmpty() }
        return session.copy(values = emptyMap(), step = AuthStep.Waiting("")) to Command.SubmitLogin(session.plugin, flow, fields)
    }

    fun cancel(session: AuthSession): Command? = session.flow?.takeIf { !session.finished }?.let { Command.CancelLogin(session.plugin, it) }

    /**
     * The redirect of an OAuth flow came back to the app (`jellyfesh://auth?...`): hand it to the
     * core as the flow's `redirect_url` field.
     */
    fun redirect(session: AuthSession, url: String): Command? {
        val flow = session.flow ?: return null
        if (session.method != AuthMethod.Oauth) return null
        return Command.SubmitLogin(session.plugin, flow, mapOf(REDIRECT_FIELD to url))
    }

    const val REDIRECT_FIELD = "redirect_url"
}

/** Where a sign-in field's answer is typed ([AuthFlow.input]). */
enum class FieldInput { Text, Email, Url, Digits, Secret, Lines }

/**
 * The community-plugin disclosure (ADR-0012): shown when the person turns on "Allow community
 * plugins" and when they enable each unofficial plugin the first time.
 */
object Disclosure {
    /** Turning community plugins on needs the disclosure first; turning them off does not. */
    fun needsDisclosureToAllow(allow: Boolean): Boolean = allow

    /** Enabling this plugin needs the disclosure first. */
    fun needsDisclosureToEnable(plugin: PluginSummary, enable: Boolean): Boolean =
        enable && plugin.badge == PluginBadge.Community && plugin.needsDisclosure

    /** What to send once the person accepted the disclosure for [plugin]. */
    fun acceptedFor(plugin: PluginSummary): List<Command> =
        listOf(Command.AcknowledgeUnofficial(plugin.id), Command.SetPluginEnabled(plugin.id, true))
}

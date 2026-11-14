package org.jellyfesh.android.ui.sources

import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.LocalAuthRedirects
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.signals.describeFailure
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfTextField
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.AuthStep
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SourcePreset
import org.jellyfesh.logic.format.TimeFormat
import org.jellyfesh.logic.qr.QrMatrix
import org.jellyfesh.logic.sources.AuthFlow
import org.jellyfesh.logic.sources.AuthSession

/**
 * Sign-in sheets. The core runs each flow (ADR-0004); the sheet draws its steps natively: a QR
 * code, the browser (Custom Tabs, with the redirect coming back through jellyfesh://auth), a form
 * whose secrets go to the core once and are never kept (ADR-0015), a wait and the result.
 */
@Stable
class SignIn internal constructor(private val overlays: Overlays, private val context: android.content.Context) {
    /** Signs in to an installed plugin (the sign-in rows on its page). */
    fun start(plugin: PluginSummary, method: AuthMethod) {
        show(plugin.name, AuthFlow.begin(plugin.id, method))
    }

    /**
     * Connects to a service from Add a Source with the address typed for it: the same sheet and
     * steps, begun with `ConnectSource` for the service's plugin. Nothing opens while the service
     * needs an address and none was typed, as the core would only fail the command.
     */
    fun connect(preset: SourcePreset, address: String, method: AuthMethod) {
        AuthFlow.connect(preset, address, method)?.let { show(preset.name, it) }
    }

    /** [name] is what the person chose (a plugin or a service); [start] begins the flow, and again on Retry. */
    private fun show(name: String, start: Pair<AuthSession, Command>) {
        overlays.showSheet(context.getString(R.string.sign_in_title, name)) { dismiss ->
            SignInContent(name, start, dismiss)
        }
    }
}

@Composable
fun rememberSignIn(): SignIn {
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    return remember(overlays, context) { SignIn(overlays, context) }
}

@Composable
private fun SignInContent(name: String, start: Pair<AuthSession, Command>, dismiss: () -> Unit) {
    val graph = LocalGraph.current
    val context = LocalContext.current
    val redirects = LocalAuthRedirects.current
    val (started, begin) = start
    var session by remember { mutableStateOf<AuthSession?>(null) }
    var failure by remember { mutableStateOf<String?>(null) }
    var commandId by remember { mutableLongStateOf(0L) }

    LaunchedEffect(start) {
        session = started
        commandId = graph.send(begin)
        if (commandId == 0L) failure = context.getString(R.string.sign_in_unavailable)
    }
    LaunchedEffect(started.plugin) {
        graph.store.signals.collect { event ->
            when (event) {
                is CoreEvent.Auth -> session = AuthFlow.onUpdate(session, event.update)
                is CoreEvent.CommandFailed -> if (event.id == commandId) failure = describeFailure(context, event.error)
                else -> Unit
            }
        }
    }
    LaunchedEffect(redirects) {
        redirects?.collect { url ->
            val s = session ?: return@collect
            AuthFlow.redirect(s, url)?.let(graph::send)
        }
    }
    // Leaving the sheet before the flow finished cancels it in the core.
    DisposableEffect(Unit) {
        onDispose { session?.let { AuthFlow.cancel(it) }?.let(graph::send) }
    }
    // The browser opens once per URL the flow asks for.
    val urlToOpen = session?.urlToOpen
    LaunchedEffect(urlToOpen) {
        val url = urlToOpen ?: return@LaunchedEffect
        openInBrowser(context, url)
        session = session?.let { AuthFlow.urlOpened(it, url) }
    }

    Column(Modifier.padding(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        val f = failure
        val step = session?.step
        when {
            f != null -> {
                JfText(f, style = JfType.small, color = Jf.colors.ink90)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) { JfButton(stringResource(R.string.action_close), dismiss) }
            }
            step == null -> JfText(stringResource(R.string.sign_in_starting), style = JfType.small, color = Jf.colors.ink70)
            step is AuthStep.ShowQr -> QrStep(name, step)
            step is AuthStep.OpenUrl -> {
                JfText(stringResource(R.string.sign_in_browser_body), style = JfType.small, color = Jf.colors.ink90)
                JfButton(stringResource(R.string.sign_in_open_again), { openInBrowser(context, step.url) }, icon = R.drawable.ic_external)
            }
            step is AuthStep.AskFields -> FieldsStep(session!!, step, onEdit = { field, value -> session = session?.let { AuthFlow.edit(it, field, value) } }) {
                val current = session ?: return@FieldsStep
                AuthFlow.submit(current)?.let { (next, command) ->
                    session = next
                    commandId = graph.send(command)
                }
            }
            step is AuthStep.Waiting -> JfText(step.message.ifBlank { stringResource(R.string.sign_in_waiting) }, style = JfType.small, color = Jf.colors.ink70)
            step is AuthStep.Done -> {
                JfText(stringResource(R.string.sign_in_done, step.account), style = JfType.body, color = Jf.colors.ink90)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) { JfButton(stringResource(R.string.action_done), dismiss, style = ButtonStyle.Accent) }
            }
            step is AuthStep.Failed -> {
                JfText(stringResource(R.string.sign_in_failed, step.message), style = JfType.small, color = Jf.colors.danger)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                    JfButton(stringResource(R.string.action_close), dismiss, style = ButtonStyle.Quiet)
                    JfButton(stringResource(R.string.action_retry), {
                        session = started
                        commandId = graph.send(begin)
                    }, style = ButtonStyle.Accent)
                }
            }
        }
    }
}

private fun openInBrowser(context: android.content.Context, url: String) {
    runCatching { CustomTabsIntent.Builder().setShowTitle(true).build().launchUrl(context, Uri.parse(url)) }
}

@Composable
private fun QrStep(name: String, step: AuthStep.ShowQr) {
    val matrix = remember(step.data) { QrMatrix.encode(step.data) }
    // A countdown that ticks once a second while the sheet is open (only with an expiry).
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    if (step.expiresAtMs != null) {
        LaunchedEffect(step) {
            while (true) {
                now = System.currentTimeMillis()
                delay(1_000 - now % 1_000)
            }
        }
    }
    val left = AuthFlow.secondsLeft(step, now)
    val label = stringResource(R.string.a11y_qr)
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (matrix != null) {
            // Dark modules on white with the quiet zone, whatever the appearance: scanners need it.
            Box(Modifier.background(Color.White, RoundedCornerShape(12.dp)).padding(8.dp)) {
                Canvas(Modifier.size(220.dp).semantics { contentDescription = label }) {
                    val modules = matrix.size + 2 * QrMatrix.QUIET_ZONE
                    val cell = size.minDimension / modules
                    for (y in 0 until matrix.size) {
                        for (x in 0 until matrix.size) {
                            if (matrix[x, y]) {
                                drawRect(
                                    Color.Black,
                                    Offset((x + QrMatrix.QUIET_ZONE) * cell, (y + QrMatrix.QUIET_ZONE) * cell),
                                    Size(cell + 0.5f, cell + 0.5f),
                                )
                            }
                        }
                    }
                }
            }
        }
        JfText(stringResource(R.string.sign_in_qr_body, name), style = JfType.small, textAlign = TextAlign.Center)
        if (left != null) {
            JfText(
                if (left > 0) stringResource(R.string.sign_in_qr_expires, TimeFormat.clock(left * 1_000)) else stringResource(R.string.sign_in_qr_expired),
                style = JfType.caption,
                color = Jf.colors.ink50,
            )
        }
    }
}

@Composable
private fun FieldsStep(session: AuthSession, step: AuthStep.AskFields, onEdit: (String, String) -> Unit, onSubmit: () -> Unit) {
    val missing = AuthFlow.missing(step.fields, session.values)
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        step.fields.forEach { field ->
            JfTextField(
                value = session.values[field.name].orEmpty(),
                onValueChange = { onEdit(field.name, it) },
                label = field.label,
                placeholder = field.placeholder,
                secret = field.secret,
                keyboard = if (field.name.contains("url", ignoreCase = true)) KeyboardType.Uri else KeyboardType.Text,
            )
        }
        if (step.fields.any { it.secret }) {
            JfText(stringResource(R.string.sign_in_secret_note), style = JfType.caption, color = Jf.colors.ink50)
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            JfButton(stringResource(R.string.sign_in_submit), onSubmit, style = ButtonStyle.Accent, enabled = missing.isEmpty())
        }
    }
}

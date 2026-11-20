package org.jellyfesh.android.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.BuildConfig
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.findActivity
import org.jellyfesh.android.ui.signals.describeFailure
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfTextField
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.LicenceError
import org.jellyfesh.logic.core.ProSnapshot
import org.jellyfesh.logic.settings.AboutInfo
import org.jellyfesh.logic.settings.LicenceAnswer
import org.jellyfesh.logic.settings.LicenceEntries
import org.jellyfesh.logic.settings.LicenceEntry
import org.jellyfesh.logic.settings.ProModel

/**
 * Jellyfesh Pro's sheets (docs/ui.md "The Pro sheet", ADR-0021): the Pro sheet any Pro action
 * opens while Pro is inactive, Enter Licence Key and Remove Licence…. The core decides whether
 * Pro is on; these send what the person asks (`ActivateLicence`, `RemoveLicence`) or start the
 * store's purchase.
 */
@Stable
class ProSheets internal constructor(private val overlays: Overlays, private val context: android.content.Context) {
    /** The Pro sheet. */
    fun open() {
        overlays.showSheet(context.getString(R.string.pro_title)) { dismiss -> ProSheetContent(this@ProSheets, dismiss) }
    }

    /** Enter Licence Key: the key, Cancel and Activate, and the core's answer. */
    fun enterLicence() {
        overlays.showSheet(context.getString(R.string.licence_title)) { dismiss -> LicenceEntryContent(dismiss) }
    }

    /** Remove Licence…, which asks first. */
    fun removeLicence(send: (Command) -> Unit) {
        overlays.confirm(
            context.getString(R.string.pro_remove_licence_title),
            context.getString(R.string.pro_remove_licence_body),
            context.getString(R.string.pro_remove_licence),
            destructive = true,
        ) { send(Command.RemoveLicence) }
    }

    /** For a Pro action: true when Pro is on and the action can go ahead; otherwise the Pro sheet opens. */
    fun require(pro: ProSnapshot): Boolean {
        if (ProModel.allows(pro)) return true
        open()
        return false
    }
}

@Composable
fun rememberProSheets(): ProSheets {
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    return remember(overlays, context) { ProSheets(overlays, context) }
}

/**
 * The ways to get Pro, for the Pro group and the Pro sheet: Buy Jellyfesh Pro – {price} and
 * Restore Purchases where this build's store sells it, Enter Licence Key… where keys work.
 * [beforeLicence] runs before Enter Licence Key opens (the Pro sheet closes first).
 */
@Composable
internal fun ProOffers(pro: ProSnapshot, sheets: ProSheets, modifier: Modifier = Modifier, beforeLicence: () -> Unit = {}) {
    val purchases = LocalGraph.current.purchases
    val context = LocalContext.current
    val sells = purchases != null && ProModel.sellsIn(pro, purchases.store)
    val keys = ProModel.takesLicenceKeys(pro)
    if (!sells && !keys) return
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (sells && purchases != null) {
            val price by purchases.price.collectAsState()
            JfButton(
                price?.let { stringResource(R.string.pro_buy, it) } ?: stringResource(R.string.pro_buy_plain),
                { context.findActivity()?.let(purchases::buy) },
                Modifier.fillMaxWidth(),
                style = ButtonStyle.Accent,
            )
            JfButton(stringResource(R.string.pro_restore), purchases::restore, Modifier.fillMaxWidth())
        }
        if (keys) {
            JfButton(
                stringResource(R.string.pro_enter_key),
                {
                    beforeLicence()
                    sheets.enterLicence()
                },
                Modifier.fillMaxWidth(),
                style = if (sells) ButtonStyle.Tonal else ButtonStyle.Accent,
                icon = R.drawable.ic_key,
            )
        }
    }
}

@Composable
private fun ProSheetContent(sheets: ProSheets, dismiss: () -> Unit) {
    val graph = LocalGraph.current
    val context = LocalContext.current
    val state by graph.store.state.collectAsState()
    val pro = state.pro
    // Bought or activated while the sheet is open: nothing left to offer.
    LaunchedEffect(pro.active) { if (pro.active) dismiss() }
    val links = remember { AboutInfo.links(BuildConfig.WEBSITE) }
    Column(Modifier.padding(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        JfText(stringResource(R.string.pro_pitch), style = JfType.body, color = Jf.colors.ink90)
        ProOffers(pro, sheets, beforeLicence = dismiss)
        JfButton(stringResource(R.string.pro_not_now), dismiss, Modifier.fillMaxWidth(), style = ButtonStyle.Quiet)
        if (links != null) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.CenterHorizontally)) {
                JfButton(stringResource(R.string.about_licence_agreement), { openLink(context, links.licenceAgreement) }, style = ButtonStyle.Quiet)
                JfButton(stringResource(R.string.about_privacy_policy), { openLink(context, links.privacyPolicy) }, style = ButtonStyle.Quiet)
            }
        }
    }
}

/**
 * Enter Licence Key: the key goes to the core as typed; the sheet stays until the core answers,
 * closing once Pro is on from the key, or saying why the key was refused.
 */
@Composable
private fun LicenceEntryContent(dismiss: () -> Unit) {
    val graph = LocalGraph.current
    val context = LocalContext.current
    val state by graph.store.state.collectAsState()
    var key by remember { mutableStateOf("") }
    var entry by remember { mutableStateOf(LicenceEntry()) }
    val answer = LicenceEntries.answer(entry, state.pro)
    LaunchedEffect(Unit) {
        graph.store.signals.collect { event ->
            if (event is CoreEvent.CommandFailed) entry = LicenceEntries.failed(entry, event.id, describeFailure(context, event.error))
        }
    }
    LaunchedEffect(answer) { if (answer == LicenceAnswer.Accepted) dismiss() }
    val error = when (answer) {
        is LicenceAnswer.Refused -> stringResource(licenceErrorText(answer.error))
        is LicenceAnswer.Failed -> answer.message
        else -> null
    }
    val waiting = answer == LicenceAnswer.Waiting
    Column(Modifier.padding(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        JfTextField(
            value = key,
            onValueChange = {
                key = it
                // A new key: the last one's answer no longer applies.
                if (!waiting) entry = LicenceEntry()
            },
            label = stringResource(R.string.licence_field),
            placeholder = "JFP1-…",
            error = error,
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
            JfButton(stringResource(R.string.action_cancel), dismiss, style = ButtonStyle.Quiet)
            JfButton(stringResource(R.string.licence_activate), {
                LicenceEntries.send(state.pro, key)?.let { (sent, command) ->
                    entry = sent
                    entry = LicenceEntries.sent(entry, graph.send(command))
                }
            }, style = ButtonStyle.Accent, enabled = key.isNotBlank() && !waiting)
        }
    }
}

/** Why a key was refused, in the words every app uses (docs/ui.md). */
internal fun licenceErrorText(error: LicenceError): Int = when (error) {
    LicenceError.Malformed -> R.string.licence_malformed
    LicenceError.UnknownKey, LicenceError.BadSignature -> R.string.licence_invalid
    LicenceError.WrongProduct -> R.string.licence_wrong_product
    LicenceError.Revoked -> R.string.licence_revoked
}

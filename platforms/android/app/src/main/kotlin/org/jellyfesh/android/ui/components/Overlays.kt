package org.jellyfesh.android.ui.components

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfMotion
import org.jellyfesh.android.ui.theme.JfTextField
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.tokens.Tokens
import org.jellyfesh.android.ui.theme.roundedTop
import org.jellyfesh.android.ui.theme.rounded

/** A bottom sheet's content; [dismiss] closes it. */
typealias SheetContent = @Composable ColumnScope.(dismiss: () -> Unit) -> Unit

/** One action of an action sheet (a row's "More" menu). */
data class SheetAction(val label: String, val icon: Int, val destructive: Boolean = false, val onClick: () -> Unit)

internal sealed interface DialogSpec {
    data class Confirm(val title: String, val message: String?, val confirm: String, val destructive: Boolean, val onConfirm: () -> Unit) : DialogSpec
    data class Prompt(val title: String, val label: String, val initial: String, val confirm: String, val onConfirm: (String) -> Unit) : DialogSpec
}

/**
 * Sheets, dialogs and short messages over the app, drawn in-app on solid Cloud (dialog glass is
 * 86 % opaque in Misty, so a live blur would add a third layer for nothing). While one is open the
 * content behind is hidden from accessibility services and Back closes it first.
 */
@Stable
class Overlays {
    internal var sheet by mutableStateOf<SheetContent?>(null)
    internal var sheetTitle by mutableStateOf<String?>(null)
    internal var dialog by mutableStateOf<DialogSpec?>(null)
    internal val messages = mutableStateListOf<String>()

    val isOpen: Boolean get() = sheet != null || dialog != null

    fun showSheet(title: String?, content: SheetContent) {
        sheetTitle = title
        sheet = content
    }

    fun dismissSheet() {
        sheet = null
    }

    /** An action sheet: a title, an optional subtitle and a list of actions. */
    fun showActions(title: String, subtitle: String?, actions: List<SheetAction>) {
        showSheet(title) { dismiss ->
            if (subtitle != null) {
                JfText(subtitle, style = JfType.small, color = Jf.colors.ink70, maxLines = 2, modifier = Modifier.padding(horizontal = 20.dp))
            }
            Column(Modifier.padding(top = 8.dp)) {
                actions.forEach { action ->
                    ActionRow(action.label, action.icon, action.destructive) {
                        dismiss()
                        action.onClick()
                    }
                }
            }
        }
    }

    fun confirm(title: String, message: String?, confirm: String, destructive: Boolean = false, onConfirm: () -> Unit) {
        dialog = DialogSpec.Confirm(title, message, confirm, destructive, onConfirm)
    }

    fun prompt(title: String, label: String, initial: String, confirm: String, onConfirm: (String) -> Unit) {
        dialog = DialogSpec.Prompt(title, label, initial, confirm, onConfirm)
    }

    /** A short message at the bottom for a few seconds (announced politely to TalkBack). */
    fun toast(text: String) {
        if (messages.lastOrNull() != text) messages += text
        while (messages.size > 3) messages.removeAt(0)
    }
}

val LocalOverlays = staticCompositionLocalOf<Overlays> { error("Overlays are missing") }

@Composable
fun OverlayHost(overlays: Overlays, bottomInset: androidx.compose.ui.unit.Dp) {
    val colors = Jf.colors
    val reduce = Jf.effects.reduceMotion
    // Messages.
    val message = overlays.messages.firstOrNull()
    LaunchedEffect(message) {
        if (message != null) {
            delay(4_000)
            overlays.messages.remove(message)
        }
    }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
        if (message != null) {
            Box(
                Modifier
                    .windowInsetsPadding(WindowInsets.navigationBars)
                    .padding(bottom = bottomInset + 12.dp, start = 16.dp, end = 16.dp)
                    .widthIn(max = 560.dp)
                    .shadow(8.dp, rounded(Tokens.Radius.rXl.dp))
                    .background(colors.cloud, rounded(Tokens.Radius.rXl.dp))
                    .border(1.dp, colors.line, rounded(Tokens.Radius.rXl.dp))
                    .clickable { overlays.messages.remove(message) }
                    .semantics { liveRegion = LiveRegionMode.Polite }
                    .padding(horizontal = 18.dp, vertical = 12.dp),
            ) {
                JfText(message, style = JfType.small, maxLines = 3)
            }
        }
    }
    // Sheet.
    val sheet = overlays.sheet
    BackHandler(enabled = sheet != null && overlays.dialog == null) { overlays.dismissSheet() }
    AnimatedVisibility(sheet != null, enter = fadeIn(JfMotion.fade(Tokens.Motion.dur1Ms, reduce)), exit = fadeOut(JfMotion.fadeOut(Tokens.Motion.dur1Ms, reduce))) {
        Box(
            Modifier
                .fillMaxSize()
                .background(colors.scrim)
                .clickable(remember { MutableInteractionSource() }, indication = null) { overlays.dismissSheet() },
        )
    }
    var lastSheet by remember { mutableStateOf<SheetContent?>(null) }
    if (sheet != null) lastSheet = sheet
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
        AnimatedVisibility(
            sheet != null,
            // It slides up; under reduced motion it fades in where it rests.
            enter = slideInVertically(JfMotion.fluid(Tokens.Motion.dur2Ms, reduce)) { it } +
                fadeIn(JfMotion.fade(Tokens.Motion.dur1Ms, reduce)),
            exit = slideOutVertically(JfMotion.exit(Tokens.Motion.dur1Ms, reduce)) { it } +
                fadeOut(JfMotion.fadeOut(Tokens.Motion.dur1Ms, reduce)),
        ) {
            val content = lastSheet
            val title = overlays.sheetTitle
            Column(
                Modifier
                    .widthIn(max = 640.dp)
                    .fillMaxWidth()
                    .shadow(16.dp, roundedTop(24.dp))
                    .background(colors.cloud, roundedTop(24.dp))
                    .border(1.dp, colors.glassEdge, roundedTop(24.dp))
                    .clickable(remember { MutableInteractionSource() }, indication = null) {}
                    .semantics { if (title != null) paneTitle = title }
                    .windowInsetsPadding(WindowInsets.navigationBars)
                    .imePadding()
                    .heightIn(max = 640.dp)
                    .verticalScroll(rememberScrollState())
                    .padding(top = 12.dp, bottom = 12.dp),
            ) {
                // The grabber.
                Box(Modifier.align(Alignment.CenterHorizontally).padding(bottom = 8.dp).background(colors.lineStrong, RoundedCornerShape(2.dp)).padding(horizontal = 18.dp, vertical = 2.dp))
                if (title != null) {
                    JfText(title, style = JfType.heading, maxLines = 2, modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp))
                }
                content?.invoke(this) { overlays.dismissSheet() }
            }
        }
    }
    // Dialog: scales from 96 % and fades in (dur-2, v0.0.4 motion); under reduced motion it only fades.
    val current = overlays.dialog
    BackHandler(enabled = current != null) { overlays.dialog = null }
    var lastDialog by remember { mutableStateOf<DialogSpec?>(null) }
    if (current != null) lastDialog = current
    AnimatedVisibility(
        current != null,
        enter = fadeIn(JfMotion.fade(Tokens.Motion.dur2Ms, reduce)) + scaleIn(JfMotion.fluid(Tokens.Motion.dur2Ms, reduce), initialScale = 0.96f),
        exit = fadeOut(JfMotion.fadeOut(Tokens.Motion.dur1Ms, reduce)),
    ) {
        val dialog = lastDialog ?: return@AnimatedVisibility
        Box(
            Modifier
                .fillMaxSize()
                .background(colors.scrim)
                .clickable(remember { MutableInteractionSource() }, indication = null) { overlays.dialog = null }
                .windowInsetsPadding(WindowInsets.safeDrawing)
                .imePadding()
                .padding(24.dp),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                Modifier
                    .widthIn(max = 420.dp)
                    .fillMaxWidth()
                    .shadow(16.dp, rounded(Tokens.Radius.r2xl.dp))
                    .background(colors.cloud, rounded(Tokens.Radius.r2xl.dp))
                    .border(1.dp, colors.glassEdge, rounded(Tokens.Radius.r2xl.dp))
                    .clickable(remember { MutableInteractionSource() }, indication = null) {}
                    .padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                when (dialog) {
                    is DialogSpec.Confirm -> {
                        JfText(dialog.title, style = JfType.heading, modifier = Modifier.semantics { paneTitle = dialog.title })
                        if (dialog.message != null) JfText(dialog.message, style = JfType.small, color = colors.ink70)
                        DialogButtons(
                            confirm = dialog.confirm,
                            destructive = dialog.destructive,
                            onCancel = { overlays.dialog = null },
                            onConfirm = {
                                overlays.dialog = null
                                dialog.onConfirm()
                            },
                        )
                    }
                    is DialogSpec.Prompt -> {
                        var text by remember(dialog) { mutableStateOf(dialog.initial) }
                        JfText(dialog.title, style = JfType.heading, modifier = Modifier.semantics { paneTitle = dialog.title })
                        JfTextField(text, { text = it }, label = dialog.label)
                        DialogButtons(
                            confirm = dialog.confirm,
                            destructive = false,
                            confirmEnabled = text.isNotBlank(),
                            onCancel = { overlays.dialog = null },
                            onConfirm = {
                                overlays.dialog = null
                                dialog.onConfirm(text.trim())
                            },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun DialogButtons(confirm: String, destructive: Boolean, onCancel: () -> Unit, onConfirm: () -> Unit, confirmEnabled: Boolean = true) {
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
        JfButton(stringResource(R.string.action_cancel), onCancel, style = ButtonStyle.Quiet)
        JfButton(confirm, onConfirm, style = if (destructive) ButtonStyle.Danger else ButtonStyle.Accent, enabled = confirmEnabled)
    }
}

/** A row of an action sheet. */
@Composable
fun ActionRow(label: String, icon: Int, destructive: Boolean = false, onClick: () -> Unit) {
    val colors = Jf.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .clickable(onClick = onClick)
            .padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        JfIcon(icon, if (destructive) colors.danger else colors.ink70, size = 22.dp)
        JfText(label, style = JfType.listTitle.copy(fontSize = JfType.body.fontSize), color = if (destructive) colors.danger else colors.ink90, maxLines = 1)
    }
}

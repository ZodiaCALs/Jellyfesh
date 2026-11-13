package org.jellyfesh.android.ui.settings

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.core.graphics.drawable.toBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Panel
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.settings.AppIcon
import org.jellyfesh.logic.settings.AppIcons
import org.jellyfesh.logic.settings.SettingsEdits

/** The launcher icon of each choice (adaptive icons with monochrome layers, scripts/gen_icons.py). */
fun iconMipmap(icon: AppIcon): Int = when (icon) {
    AppIcon.Ice -> R.mipmap.ic_launcher_ice
    AppIcon.Steel -> R.mipmap.ic_launcher_steel
    AppIcon.Original -> R.mipmap.ic_launcher_original
    AppIcon.NavyTide -> R.mipmap.ic_launcher_navy_tide
    AppIcon.Royal -> R.mipmap.ic_launcher_royal
    AppIcon.TealDeep -> R.mipmap.ic_launcher_teal_deep
    AppIcon.SeaGlass -> R.mipmap.ic_launcher_sea_glass
    AppIcon.Electric -> R.mipmap.ic_launcher_electric
    AppIcon.Abyss -> R.mipmap.ic_launcher_abyss
    AppIcon.Lagoon -> R.mipmap.ic_launcher_lagoon
}

fun iconLabel(icon: AppIcon): Int = when (icon) {
    AppIcon.Ice -> R.string.icon_ice
    AppIcon.Steel -> R.string.icon_steel
    AppIcon.Original -> R.string.icon_original
    AppIcon.NavyTide -> R.string.icon_navy_tide
    AppIcon.Royal -> R.string.icon_royal
    AppIcon.TealDeep -> R.string.icon_teal_deep
    AppIcon.SeaGlass -> R.string.icon_sea_glass
    AppIcon.Electric -> R.string.icon_electric
    AppIcon.Abyss -> R.string.icon_abyss
    AppIcon.Lagoon -> R.string.icon_lagoon
}

/**
 * Settings → Appearance → App icon (ADR-0017): Automatic (Ice on Android, since launchers do not
 * reliably follow dark mode for app icons) and the ten icons. A choice asks first, because
 * launchers may take seconds to refresh or drop home-screen shortcuts; the limits are written
 * under the picker instead of failing silently.
 */
@Composable
fun AppIconPicker(settings: Settings) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val confirmTitle = stringResource(R.string.app_icon_confirm_title)
    val confirmBody = stringResource(R.string.app_icon_confirm_body)
    val confirmLabel = stringResource(R.string.app_icon_confirm)
    val options: List<String> = listOf(AppIcons.AUTOMATIC) + AppIcon.entries.map { it.id }
    val selected = settings.appIcon.takeIf { s -> options.contains(s) } ?: AppIcons.AUTOMATIC
    Panel {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            JfText(stringResource(R.string.app_icon), style = JfType.listTitle.copy(fontSize = JfType.body.fontSize))
            JfText(stringResource(R.string.app_icon_desc), style = JfType.small, color = Jf.colors.ink70)
            BoxWithConstraints(Modifier.fillMaxWidth()) {
                val perRow = (maxWidth / 88.dp).toInt().coerceIn(3, 6)
                Column(Modifier.selectableGroup(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    options.chunked(perRow).forEach { row ->
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                            row.forEach { id ->
                                IconTile(id, id == selected, Modifier.weight(1f)) {
                                    if (id != selected) {
                                        val change = { graph.send(SettingsEdits.set(settings) { copy(appIcon = id) }) }
                                        if (AppIcons.switch(selected, id) == null) change() else overlays.confirm(confirmTitle, confirmBody, confirmLabel, onConfirm = { change() })
                                    }
                                }
                            }
                            repeat(perRow - row.size) { Spacer(Modifier.weight(1f)) }
                        }
                    }
                }
            }
            JfText(stringResource(R.string.app_icon_note), style = JfType.caption, color = Jf.colors.ink50)
        }
    }
}

@Composable
private fun IconTile(id: String, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val colors = Jf.colors
    val icon = AppIcons.launcherIcon(id)
    val label = if (id == AppIcons.AUTOMATIC) stringResource(R.string.app_icon_automatic) else stringResource(iconLabel(icon))
    val image = rememberLauncherIcon(iconMipmap(icon), 64)
    Column(
        modifier
            .selectable(selected, role = Role.RadioButton, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = label }
            .padding(vertical = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box {
            val shape = RoundedCornerShape(14.dp)
            Box(
                Modifier
                    .size(64.dp)
                    .then(if (selected) Modifier.border(4.dp, colors.accent, shape).padding(2.dp).border(2.dp, colors.cloud, shape) else Modifier)
                    .clip(shape)
                    .background(colors.cloudSoft),
            ) {
                if (image != null) Image(image, contentDescription = null, modifier = Modifier.size(64.dp))
            }
            if (selected) {
                Box(
                    Modifier
                        .align(Alignment.TopEnd)
                        .offset(x = 6.dp, y = (-6).dp)
                        .size(20.dp)
                        .background(colors.accent, CircleShape)
                        .border(2.dp, colors.cloud, CircleShape),
                    contentAlignment = Alignment.Center,
                ) { JfIcon(R.drawable.ic_check, colors.onAccent, size = 12.dp) }
            }
        }
        JfText(label, style = JfType.caption, color = if (selected) colors.accentText else colors.ink70, maxLines = 2)
    }
}

/**
 * A launcher icon as the launcher draws it: the adaptive icon (layers and the device's mask)
 * rendered into a bitmap off the main thread. Compose's painterResource cannot read adaptive icons.
 */
@Composable
private fun rememberLauncherIcon(mipmap: Int, sizeDp: Int): ImageBitmap? {
    val context = LocalContext.current
    val px = with(LocalDensity.current) { sizeDp.dp.roundToPx() }
    val image by produceState<ImageBitmap?>(null, mipmap, px) {
        value = withContext(Dispatchers.Default) {
            runCatching { ContextCompat.getDrawable(context, mipmap)?.toBitmap(px, px)?.asImageBitmap() }.getOrNull()
        }
    }
    return image
}

package org.jellyfesh.android.ui.nav

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.clickable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.settings.iconMipmap
import org.jellyfesh.android.ui.settings.rememberLauncherIcon
import org.jellyfesh.android.ui.theme.GlassGrade
import org.jellyfesh.android.ui.theme.GlassSurface
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.rounded
import org.jellyfesh.logic.library.OpenedFiles
import org.jellyfesh.logic.nav.Sidebar
import org.jellyfesh.logic.nav.SidebarPlace
import org.jellyfesh.logic.settings.AppIcons
import org.jellyfesh.tokens.Tokens

/** The sidebar column's width, its 12 dp margin included (Jellyfesh Web's 248 px). */
val SidebarWidth = 248.dp

/** Between the panel and the window's edges. */
private val PanelMargin = Tokens.Space.s3.dp

/**
 * The sidebar panel of windows 761 dp and wider (docs/ui.md, "Navigation"; Jellyfesh Web's): a
 * floating panel of Clear glass with the wordmark, the library's seven places, a separator, Search
 * and Sources, and Open Files… and Settings at its foot. The places follow the look's order
 * ([Sidebar.groups]); the chosen one takes the accent's soft tint.
 */
@Composable
fun SidebarPanel(selected: SidebarPlace, appIcon: String, onSelect: (SidebarPlace) -> Unit, modifier: Modifier = Modifier) {
    val openFiles = rememberOpenFiles()
    GlassSurface(
        modifier.padding(start = PanelMargin, top = PanelMargin, bottom = PanelMargin),
        grade = GlassGrade.Clear,
        shape = rounded(Tokens.Radius.r2xl.dp),
        raised = true,
    ) {
        Column(Modifier.fillMaxSize().padding(vertical = Tokens.Space.s5.dp, horizontal = Tokens.Space.s3.dp)) {
            Wordmark(appIcon)
            Column(
                Modifier.weight(1f).verticalScroll(rememberScrollState()).selectableGroup(),
                verticalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Sidebar.groups(Jf.look).forEachIndexed { index, group ->
                    if (index > 0) Separator()
                    group.forEach { place -> PlaceItem(place, place == selected) { onSelect(place) } }
                }
            }
            Column(Modifier.padding(top = Tokens.Space.s2.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                ActionItem(R.drawable.ic_file, stringResource(R.string.action_open_files), openFiles)
                PlaceItem(SidebarPlace.Settings, selected == SidebarPlace.Settings) { onSelect(SidebarPlace.Settings) }
            }
        }
    }
}

/** The app's icon (the one the person chose) and its name. */
@Composable
private fun Wordmark(appIcon: String) {
    val image = rememberLauncherIcon(iconMipmap(AppIcons.launcherIcon(appIcon)), 32)
    Row(
        Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, bottom = Tokens.Space.s6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(11.dp),
    ) {
        Box(Modifier.size(32.dp).clip(RoundedCornerShape(9.dp)).background(Jf.colors.cloudSoft)) {
            if (image != null) Image(image, contentDescription = null, modifier = Modifier.size(32.dp))
        }
        JfText(
            stringResource(R.string.app_name),
            style = JfType.body.copy(fontSize = 16.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.16).sp),
            maxLines = 1,
        )
    }
}

@Composable
private fun Separator() {
    Box(Modifier.fillMaxWidth().padding(horizontal = Tokens.Space.s3.dp, vertical = Tokens.Space.s2.dp).height(1.dp).background(Jf.colors.lineSoft))
}

/** A place: 42 dp high, its glyph and name; the chosen one on the accent's soft tint, in accent text. */
@Composable
private fun PlaceItem(place: SidebarPlace, chosen: Boolean, onClick: () -> Unit) {
    val colors = Jf.colors
    val shape = rounded(Tokens.Radius.rLg.dp)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 42.dp)
            .clip(shape)
            .background(if (chosen) colors.accentSoft else Color.Transparent, shape)
            .selectable(chosen, remember { MutableInteractionSource() }, indication = null, role = Role.Tab, onClick = onClick)
            .padding(horizontal = Tokens.Space.s3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Tokens.Space.s3.dp),
    ) {
        val tint = if (chosen) colors.accentText else colors.ink70
        JfIcon(placeIcon(place), tint, size = 20.dp)
        JfText(
            stringResource(placeLabel(place)),
            style = JfType.body.copy(fontSize = 14.5.sp, fontWeight = if (chosen) FontWeight.SemiBold else FontWeight.Medium),
            color = tint,
            maxLines = 1,
        )
    }
}

/** A foot action that is not a place (Open Files…). */
@Composable
private fun ActionItem(icon: Int, label: String, onClick: () -> Unit) {
    val colors = Jf.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 42.dp)
            .clip(rounded(Tokens.Radius.rLg.dp))
            .clickable(remember { MutableInteractionSource() }, indication = null, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label }
            .padding(horizontal = Tokens.Space.s3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Tokens.Space.s3.dp),
    ) {
        JfIcon(icon, colors.ink70, size = 20.dp)
        JfText(label, style = JfType.body.copy(fontSize = 14.5.sp, fontWeight = FontWeight.Medium), color = colors.ink70, maxLines = 1)
    }
}

fun placeLabel(place: SidebarPlace): Int = when (place) {
    SidebarPlace.Home -> R.string.tab_home
    SidebarPlace.Albums -> R.string.section_albums
    SidebarPlace.Artists -> R.string.section_artists
    SidebarPlace.Songs -> R.string.section_songs
    SidebarPlace.Playlists -> R.string.section_playlists
    SidebarPlace.Favourites -> R.string.section_favourites
    SidebarPlace.Downloads -> R.string.section_downloads
    SidebarPlace.Search -> R.string.tab_search
    SidebarPlace.Sources -> R.string.tab_sources
    SidebarPlace.Settings -> R.string.settings_title
}

fun placeIcon(place: SidebarPlace): Int = when (place) {
    SidebarPlace.Home -> R.drawable.ic_home
    SidebarPlace.Albums -> R.drawable.ic_album
    SidebarPlace.Artists -> R.drawable.ic_artist
    SidebarPlace.Songs -> R.drawable.ic_song
    SidebarPlace.Playlists -> R.drawable.ic_playlist
    SidebarPlace.Favourites -> R.drawable.ic_heart
    SidebarPlace.Downloads -> R.drawable.ic_download
    SidebarPlace.Search -> R.drawable.ic_search
    SidebarPlace.Sources -> R.drawable.ic_sources
    SidebarPlace.Settings -> R.drawable.ic_settings
}

/**
 * Open Files…: the system's document picker for audio files, which then play at once without
 * joining the library ([OpenedFiles]). Returns the action that opens the picker.
 */
@Composable
fun rememberOpenFiles(): () -> Unit {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { documents ->
        if (documents.isEmpty()) return@rememberLauncherForActivityResult
        scope.launch {
            if (graph.library.playFiles(documents) == 0) overlays.toast(context.getString(R.string.open_files_none))
        }
    }
    return remember(picker) { { picker.launch(OpenedFiles.TYPES) } }
}

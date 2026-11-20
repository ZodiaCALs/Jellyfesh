package org.jellyfesh.android.ui.settings

import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.jellyfesh.android.BuildConfig
import org.jellyfesh.android.QueryResult
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.BackBar
import org.jellyfesh.android.ui.components.ChoiceRow
import org.jellyfesh.android.ui.components.Divider
import org.jellyfesh.android.ui.components.InitialTile
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.NavRow
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Panel
import org.jellyfesh.android.ui.components.ScreenTitle
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.SettingRow
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.ok
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.library.barFade
import org.jellyfesh.android.ui.library.listPadding
import org.jellyfesh.android.ui.library.rememberLibraryAccess
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.sources.PluginIcon
import org.jellyfesh.android.ui.sources.PresetRow
import org.jellyfesh.android.ui.sources.pluginStatus
import org.jellyfesh.android.ui.sources.rememberConnectSheet
import org.jellyfesh.android.ui.sources.rememberDisclosures
import org.jellyfesh.android.ui.sources.rememberPluginInstaller
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.GlassGrade
import org.jellyfesh.android.ui.theme.GlassSurface
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfSlider
import org.jellyfesh.android.ui.theme.JfSwitch
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.MistyField
import org.jellyfesh.android.ui.theme.SegmentedControl
import org.jellyfesh.android.ui.theme.TouchTarget
import org.jellyfesh.android.ui.theme.VerticalBandSlider
import org.jellyfesh.android.ui.theme.topEdgeFade
import org.jellyfesh.logic.core.AppearancePref
import org.jellyfesh.logic.core.BackdropStyle
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.HueSource
import org.jellyfesh.logic.core.LyricsAlign
import org.jellyfesh.logic.core.LyricsSize
import org.jellyfesh.logic.core.ReplayGainMode
import org.jellyfesh.logic.core.ScanStatus
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.nav.SettingsSection
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.settings.AboutInfo
import org.jellyfesh.logic.settings.EqChoice
import org.jellyfesh.logic.settings.EqLabels
import org.jellyfesh.logic.settings.EqPresets
import org.jellyfesh.logic.settings.SettingsEdits
import org.jellyfesh.logic.settings.SettingsGroup
import org.jellyfesh.logic.settings.SettingsLayout
import org.jellyfesh.logic.sources.Disclosure
import org.jellyfesh.logic.sources.YourSources
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.logic.theme.AccentChoices
import org.jellyfesh.logic.theme.Palette
import org.jellyfesh.tokens.Tokens

/**
 * Settings. The core owns every setting: a change sends a modified copy of the slice
 * (`SetSettings`) or the narrower command the core offers (EQ, ReplayGain, crossfade, output), and
 * the screen shows the slice the core sends back. The sections and their groups come in the order
 * every app uses (docs/ui.md, "Settings"; [SettingsLayout]).
 */
@Composable
fun SettingsScreen(section: SettingsSection?, state: CoreState) {
    val nav = LocalNav.current
    Box(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()), contentPadding = listPadding()) {
            item(key = "bar-space") { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            item(key = "title") { ScreenTitle(stringResource(section?.let(::sectionTitle) ?: R.string.settings_title)) }
            if (section == null) {
                sectionList()
            } else {
                SettingsLayout.groups(section).forEach { group(it, state) }
            }
        }
        BackBar(onBack = nav::back, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars))
    }
}

fun sectionTitle(section: SettingsSection): Int = when (section) {
    SettingsSection.Appearance -> R.string.settings_appearance
    SettingsSection.Playback -> R.string.settings_playback
    SettingsSection.Library -> R.string.settings_library
    SettingsSection.Sources -> R.string.settings_sources
    SettingsSection.Extensions -> R.string.settings_extensions
    SettingsSection.About -> R.string.settings_about
}

private fun sectionDescription(section: SettingsSection): Int = when (section) {
    SettingsSection.Appearance -> R.string.settings_appearance_desc
    SettingsSection.Playback -> R.string.settings_playback_desc
    SettingsSection.Library -> R.string.settings_library_desc
    SettingsSection.Sources -> R.string.settings_sources_desc
    SettingsSection.Extensions -> R.string.settings_extensions_desc
    SettingsSection.About -> R.string.settings_about_desc
}

private fun sectionIcon(section: SettingsSection): Int = when (section) {
    SettingsSection.Appearance -> R.drawable.ic_palette
    SettingsSection.Playback -> R.drawable.ic_eq
    SettingsSection.Library -> R.drawable.ic_library
    SettingsSection.Sources -> R.drawable.ic_sources
    SettingsSection.Extensions -> R.drawable.ic_sparkle
    SettingsSection.About -> R.drawable.ic_info
}

private fun LazyListScope.sectionList() {
    item(key = "sections") {
        val nav = LocalNav.current
        Panel {
            SettingsSection.entries.forEachIndexed { i, s ->
                if (i > 0) Divider()
                NavRow(
                    title = stringResource(sectionTitle(s)),
                    subtitle = stringResource(sectionDescription(s)),
                    onClick = { nav.push(Screen.Settings(s)) },
                    leading = { InitialTile(stringResource(sectionTitle(s)), icon = sectionIcon(s)) },
                )
            }
        }
    }
}

/** One group of a settings page: its heading (when it has a name of its own) and its panel. */
private fun LazyListScope.group(group: SettingsGroup, state: CoreState) {
    when (group) {
        SettingsGroup.Theme -> theme(state)
        SettingsGroup.AccentColour -> accentColour(state.settings)
        SettingsGroup.Transparency -> transparency(state.settings)
        SettingsGroup.NowPlayingBackdrop -> backdrop(state.settings)
        SettingsGroup.Lyrics -> lyrics(state.settings)
        SettingsGroup.AppIcon -> {
            item(key = "icon-h") { SectionHeader(stringResource(R.string.app_icon)) }
            item(key = "icon") { AppIconPicker(state.settings) }
        }
        SettingsGroup.Customize -> customize(state)
        SettingsGroup.Output -> output(state)
        SettingsGroup.Crossfade -> crossfade(state.settings)
        SettingsGroup.VolumeLevelling -> volumeLevelling(state.settings)
        SettingsGroup.Equalizer -> {
            item(key = "eq-h") { SectionHeader(stringResource(R.string.eq_title)) }
            item(key = "eq") { Equalizer(state.settings) }
        }
        SettingsGroup.Startup -> startup(state.settings)
        SettingsGroup.MusicOnThisPhone -> musicOnThisPhone()
        SettingsGroup.MusicFolders -> musicFolders(state)
        SettingsGroup.WatchFolders -> watchFolders(state.settings)
        SettingsGroup.RescanLibrary -> rescan(state)
        SettingsGroup.AddSource -> addSource(state)
        SettingsGroup.Installed -> installed(state)
        SettingsGroup.CommunityPlugins -> communityPlugins(state)
        SettingsGroup.JellyfeshPro -> jellyfeshPro(state)
        SettingsGroup.InstalledExtensions -> installedExtensions(state)
        SettingsGroup.About -> about()
        SettingsGroup.Updates -> updates(state)
    }
}

/** Sends a modified copy of the settings slice. */
@Composable
private fun rememberEdit(settings: Settings): (Settings.() -> Settings) -> Unit {
    val graph = LocalGraph.current
    return { change -> graph.send(SettingsEdits.set(settings, change)) }
}

// ---- Appearance (docs/ui.md; docs/design-controls.md, "Preferences → Appearance") ----------------
// Grouped cards; every change applies at once and the core keeps it (SetSettings).

private fun LazyListScope.theme(state: CoreState) {
    val settings = state.settings
    item(key = "mode-h") { SectionHeader(stringResource(R.string.appearance_theme)) }
    item(key = "mode") {
        val edit = rememberEdit(settings)
        Panel {
            SettingColumn(null, stringResource(R.string.appearance_theme_desc)) {
                Row(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    AppearancePref.entries.forEach { pref ->
                        val label = stringResource(
                            when (pref) {
                                AppearancePref.System -> R.string.appearance_system
                                AppearancePref.Light -> R.string.appearance_light
                                AppearancePref.Dark -> R.string.appearance_dark
                            },
                        )
                        PreviewChoice(label, settings.appearance == pref, { edit { copy(appearance = pref) } }) {
                            ThemePreview(state, pref)
                        }
                    }
                }
            }
        }
    }
}

private fun LazyListScope.accentColour(settings: Settings) {
    item(key = "colour-h") { SectionHeader(stringResource(R.string.accent_title)) }
    item(key = "colour") {
        val edit = rememberEdit(settings)
        val fixed = settings.hueSource as? HueSource.Fixed
        val dark = Jf.colors.dark
        val themeHue = Jf.colors.hue
        var hue by remember(fixed?.hue) { mutableStateOf(fixed?.hue ?: themeHue) }
        val chosen = AccentChoices.swatchIndex(settings.hueSource)
        Panel {
            SettingRow(stringResource(R.string.accent_follow_artwork), description = stringResource(R.string.accent_desc)) {
                JfSwitch(fixed == null, { follow -> edit { copy(hueSource = SettingsEdits.hue(if (follow) null else hue)) } })
            }
            if (fixed != null) {
                Divider()
                val swatchesLabel = stringResource(R.string.accent_title)
                Row(
                    Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .padding(horizontal = 12.dp, vertical = 8.dp)
                        .selectableGroup()
                        .semantics { contentDescription = swatchesLabel },
                ) {
                    AccentChoices.SWATCH_HUES.forEachIndexed { i, swatch ->
                        Swatch(
                            colour = Color(AccentChoices.accent(swatch, dark)),
                            label = stringResource(swatchName(AccentChoices.SWATCH_NAMES[i])),
                            selected = i == chosen,
                            onClick = { edit { copy(hueSource = SettingsEdits.hue(swatch)) } },
                        )
                    }
                }
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    JfText(stringResource(R.string.accent_custom), style = JfType.small, color = Jf.colors.ink70)
                    JfSlider(
                        value = hue,
                        range = 0f..359f,
                        step = 1f,
                        onChange = { hue = it },
                        onChangeFinished = { edit { copy(hueSource = SettingsEdits.hue(hue)) } },
                        description = stringResource(R.string.colour_hue),
                        valueText = stringResource(R.string.hue_value, hue.toInt()),
                        track = Brush.horizontalGradient(AccentChoices.hueTrack(dark).map { Color(it) }),
                        modifier = Modifier.weight(1f),
                    )
                    Box(Modifier.size(28.dp).clip(CircleShape).background(Color(AccentChoices.accent(hue, dark))).border(2.dp, Jf.colors.glassEdge, CircleShape))
                }
            }
        }
    }
}

/** A swatch's name, by its id in [AccentChoices.SWATCH_NAMES]. */
private fun swatchName(id: String): Int = when (id) {
    "blue" -> R.string.accent_blue
    "cyan" -> R.string.accent_cyan
    "teal" -> R.string.accent_teal
    "green" -> R.string.accent_green
    "olive" -> R.string.accent_olive
    "orange" -> R.string.accent_orange
    "red" -> R.string.accent_red
    "rose" -> R.string.accent_rose
    "magenta" -> R.string.accent_magenta
    else -> R.string.accent_violet
}

private fun LazyListScope.transparency(settings: Settings) {
    item(key = "transparency-h") { SectionHeader(stringResource(R.string.transparency_title)) }
    item(key = "transparency") {
        val edit = rememberEdit(settings)
        var percent by remember(settings.transparency) { mutableStateOf(SettingsEdits.transparencyPercent(settings.transparency).toFloat()) }
        Panel {
            SettingColumn(null, stringResource(R.string.transparency_desc)) {
                TransparencyPreview()
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    JfText(stringResource(R.string.transparency_solid), style = JfType.caption, color = Jf.colors.ink50)
                    JfSlider(
                        value = percent,
                        range = 0f..100f,
                        step = 5f,
                        enabled = !settings.reduceEffects,
                        onChange = { percent = it },
                        onChangeFinished = { edit { copy(transparency = SettingsEdits.transparency(percent)) } },
                        description = stringResource(R.string.transparency_title),
                        valueText = stringResource(R.string.transparency_value, percent.toInt()),
                        modifier = Modifier.weight(1f),
                    )
                    JfText(stringResource(R.string.transparency_clear), style = JfType.caption, color = Jf.colors.ink50)
                }
            }
            Divider()
            SettingRow(stringResource(R.string.reduce_effects), description = stringResource(R.string.reduce_effects_desc)) {
                JfSwitch(settings.reduceEffects, { on -> edit { copy(reduceEffects = on) } })
            }
        }
    }
}

private fun LazyListScope.backdrop(settings: Settings) {
    item(key = "background-h") { SectionHeader(stringResource(R.string.style_title)) }
    item(key = "background") {
        val edit = rememberEdit(settings)
        Panel {
            SettingColumn(null, stringResource(R.string.style_desc)) {
                Row(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    BackdropStyle.entries.forEach { style ->
                        val label = stringResource(if (style == BackdropStyle.Artwork) R.string.style_artwork else R.string.style_misty)
                        PreviewChoice(label, settings.style == style, { edit { copy(style = style) } }) {
                            if (style == BackdropStyle.Misty) MistyField(Modifier.fillMaxSize()) else Box(Modifier.fillMaxSize().background(Jf.colors.ink30))
                        }
                    }
                }
            }
        }
    }
}

/** Appearance › Lyrics: the only place the lyrics settings live (docs/ui.md). */
private fun LazyListScope.lyrics(settings: Settings) {
    item(key = "lyrics-h") { SectionHeader(stringResource(R.string.np_lyrics)) }
    item(key = "lyrics") {
        val edit = rememberEdit(settings)
        Panel {
            SettingColumn(stringResource(R.string.lyrics_size), null) {
                SegmentedControl(
                    options = LyricsSize.entries,
                    selected = settings.lyricsSize,
                    label = {
                        stringResource(
                            when (it) {
                                LyricsSize.Small -> R.string.lyrics_size_small
                                LyricsSize.Medium -> R.string.lyrics_size_medium
                                LyricsSize.Large -> R.string.lyrics_size_large
                            },
                        )
                    },
                    onSelect = { edit { copy(lyricsSize = it) } },
                    description = stringResource(R.string.lyrics_size),
                )
            }
            Divider()
            SettingColumn(stringResource(R.string.lyrics_align), null) {
                SegmentedControl(
                    options = LyricsAlign.entries,
                    selected = settings.lyricsAlign,
                    label = { stringResource(if (it == LyricsAlign.Center) R.string.lyrics_align_centre else R.string.lyrics_align_start) },
                    onSelect = { edit { copy(lyricsAlign = it) } },
                    description = stringResource(R.string.lyrics_align),
                )
            }
            Divider()
            SettingRow(stringResource(R.string.show_translation), description = stringResource(R.string.show_translation_desc)) {
                JfSwitch(settings.showTranslation, { on -> edit { copy(showTranslation = on) } })
            }
            Divider()
            SettingRow(stringResource(R.string.show_romanization), description = stringResource(R.string.show_romanization_desc)) {
                JfSwitch(settings.showRomanization, { on -> edit { copy(showRomanization = on) } })
            }
            Divider()
            // Desktop Lyrics, the other v0.0.7 switch here, is for desktops (docs/ui.md, Allowed differences).
            SettingRow(stringResource(R.string.show_lyrics_bar), description = stringResource(R.string.show_lyrics_bar_desc)) {
                JfSwitch(settings.showLyricsBar, { on -> edit { copy(showLyricsBar = on) } })
            }
        }
    }
}

/** A choice with a small picture above its label (Theme, Now Playing background). */
@Composable
private fun PreviewChoice(label: String, selected: Boolean, onClick: () -> Unit, preview: @Composable () -> Unit) {
    val colors = Jf.colors
    val shape = RoundedCornerShape(Tokens.Radius.rMd.dp)
    Column(
        Modifier
            .width(88.dp)
            .clip(RoundedCornerShape(Tokens.Radius.rLg.dp))
            .selectable(selected, remember { MutableInteractionSource() }, indication = null, role = Role.RadioButton, onClick = onClick)
            .padding(4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(
            Modifier
                .size(width = 80.dp, height = 52.dp)
                .clip(shape)
                .border(if (selected) 2.dp else 1.dp, if (selected) colors.accentText else colors.line, shape),
        ) { preview() }
        JfText(label, style = JfType.label, color = if (selected) colors.ink90 else colors.ink70, maxLines = 1)
    }
}

/** A small window in the core's colours: the field, a card, two ink lines and the accent (light, dark, or both for System). */
@Composable
private fun ThemePreview(state: CoreState, pref: AppearancePref) {
    val palettes = when (pref) {
        AppearancePref.Light -> listOf(Palette.of(state.theme, false))
        AppearancePref.Dark -> listOf(Palette.of(state.theme, true))
        AppearancePref.System -> listOf(Palette.of(state.theme, false), Palette.of(state.theme, true))
    }
    Canvas(Modifier.fillMaxSize()) {
        val part = size.width / palettes.size
        palettes.forEachIndexed { i, p ->
            val x = i * part
            drawRect(Color(p[Tokens.Field.sky.name]), Offset(x, 0f), Size(part, size.height))
            val card = 6.dp.toPx()
            drawRoundRect(Color(p[Tokens.Glass.cloud.name]), Offset(x + card, card), Size(part - 2 * card, size.height - 2 * card), CornerRadius(4.dp.toPx()))
            drawRoundRect(Color(p[Tokens.Ink.ink90.name]), Offset(x + 2 * card, 2 * card), Size((part - 4 * card) * 0.7f, 3.dp.toPx()), CornerRadius(2.dp.toPx()))
            drawRoundRect(Color(p[Tokens.Ink.ink50.name]), Offset(x + 2 * card, 2 * card + 6.dp.toPx()), Size((part - 4 * card) * 0.45f, 3.dp.toPx()), CornerRadius(2.dp.toPx()))
            drawCircle(Color(p[Tokens.Accent.accent.name]), 4.dp.toPx(), Offset(x + part - 2 * card, size.height - 2 * card))
        }
    }
}

/** An accent swatch: a 32 dp circle in a 48 dp target, ringed when chosen. */
@Composable
private fun Swatch(colour: Color, label: String, selected: Boolean, onClick: () -> Unit) {
    val colors = Jf.colors
    Box(
        Modifier
            .size(TouchTarget)
            .selectable(selected, remember { MutableInteractionSource() }, indication = null, role = Role.RadioButton, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        if (selected) Box(Modifier.size(42.dp).border(2.dp, colors.ink90, CircleShape))
        Box(Modifier.size(32.dp).clip(CircleShape).background(colour).border(1.dp, colors.lineSoft, CircleShape))
    }
}

/** The transparency's live preview: Veil glass over the Misty field with the accent behind it. */
@Composable
private fun TransparencyPreview() {
    val colors = Jf.colors
    Box(Modifier.fillMaxWidth().height(64.dp).clip(RoundedCornerShape(Tokens.Radius.rMd.dp))) {
        MistyField(Modifier.fillMaxSize())
        Box(Modifier.padding(start = 16.dp).size(40.dp).align(Alignment.CenterStart).clip(CircleShape).background(colors.accent))
        GlassSurface(Modifier.padding(start = 36.dp, top = 14.dp, bottom = 14.dp, end = 16.dp).fillMaxSize(), grade = GlassGrade.Veil, shape = RoundedCornerShape(Tokens.Radius.rSm.dp)) {}
    }
}

// ---- Playback -------------------------------------------------------------------------------------

private fun LazyListScope.output(state: CoreState) {
    item(key = "output-h") { SectionHeader(stringResource(R.string.output_title)) }
    item(key = "output") {
        val graph = LocalGraph.current
        LaunchedEffect(Unit) { graph.send(Command.RefreshDevices) }
        val devices = state.devices
        Panel {
            SettingColumn(stringResource(R.string.np_output_device), stringResource(R.string.output_desc)) {}
            Column(Modifier.selectableGroup()) {
                DeviceChoice(stringResource(R.string.np_system_default), PlayerModel.isSelected(devices, null)) { graph.send(Command.SetOutputDevice(null)) }
                devices.devices.forEach { d ->
                    Divider()
                    DeviceChoice(d.name, PlayerModel.isSelected(devices, d.id)) { graph.send(Command.SetOutputDevice(d.id)) }
                }
            }
        }
    }
}

private fun LazyListScope.crossfade(settings: Settings) {
    item(key = "crossfade-h") { SectionHeader(stringResource(R.string.crossfade_title)) }
    item(key = "crossfade") {
        val graph = LocalGraph.current
        val cf = settings.crossfade
        var seconds by remember(cf.durationMs) { mutableStateOf(cf.durationMs / 1_000f) }
        Panel {
            SettingRow(stringResource(R.string.crossfade_title), description = stringResource(R.string.crossfade_desc)) {
                JfSwitch(cf.enabled, { graph.send(Command.SetCrossfade(cf.copy(enabled = it))) })
            }
            Divider()
            Column(Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    JfText(stringResource(R.string.crossfade_length), style = JfType.listTitle, modifier = Modifier.weight(1f))
                    JfText(stringResource(R.string.seconds_value, seconds.toInt()), style = JfType.time, color = Jf.colors.ink70)
                }
                JfSlider(
                    value = seconds,
                    range = 1f..12f,
                    step = 1f,
                    enabled = cf.enabled,
                    onChange = { seconds = it },
                    onChangeFinished = { graph.send(Command.SetCrossfade(SettingsEdits.crossfade(cf, seconds))) },
                    description = stringResource(R.string.crossfade_length),
                    valueText = stringResource(R.string.seconds_value, seconds.toInt()),
                )
            }
            Divider()
            SettingRow(stringResource(R.string.album_continuity), description = stringResource(R.string.album_continuity_desc)) {
                JfSwitch(cf.albumContinuity, { graph.send(Command.SetCrossfade(cf.copy(albumContinuity = it))) }, enabled = cf.enabled)
            }
        }
    }
}

/** Volume Levelling: the ReplayGain control and its preamp. */
private fun LazyListScope.volumeLevelling(settings: Settings) {
    item(key = "rg-h") { SectionHeader(stringResource(R.string.replaygain_title)) }
    item(key = "rg") {
        val graph = LocalGraph.current
        var preamp by remember(settings.preampDb) { mutableStateOf(settings.preampDb) }
        Panel {
            SettingColumn(stringResource(R.string.replaygain_control), stringResource(R.string.replaygain_desc)) {
                SegmentedControl(
                    options = ReplayGainMode.entries,
                    selected = settings.replaygainMode,
                    label = {
                        stringResource(
                            when (it) {
                                ReplayGainMode.Off -> R.string.rg_off
                                ReplayGainMode.Track -> R.string.rg_track
                                ReplayGainMode.Album -> R.string.rg_album
                            },
                        )
                    },
                    onSelect = { graph.send(Command.SetReplayGain(it, preamp)) },
                    description = stringResource(R.string.replaygain_control),
                )
            }
            DecibelSlider(
                title = stringResource(R.string.preamp),
                value = preamp,
                range = -15f..15f,
                enabled = settings.replaygainMode != ReplayGainMode.Off,
                onChange = { preamp = SettingsEdits.preamp(it) },
                onChangeFinished = { graph.send(Command.SetReplayGain(settings.replaygainMode, preamp)) },
            )
        }
    }
}

private fun LazyListScope.startup(settings: Settings) {
    item(key = "startup-h") { SectionHeader(stringResource(R.string.startup_title)) }
    item(key = "startup") {
        val edit = rememberEdit(settings)
        Panel {
            SettingRow(stringResource(R.string.resume_title), description = stringResource(R.string.resume_desc)) {
                JfSwitch(settings.resumeOnLaunch, { on -> edit { copy(resumeOnLaunch = on) } })
            }
        }
    }
}

@Composable
private fun DeviceChoice(name: String, selected: Boolean, onClick: () -> Unit) {
    ChoiceRow(name, selected, onClick, inset = 16.dp)
}

/** A title, the value in dB and a slider in 0.5 dB steps (the preamps). */
@Composable
private fun DecibelSlider(
    title: String,
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    enabled: Boolean,
    onChange: (Float) -> Unit,
    onChangeFinished: () -> Unit,
) {
    Column(Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            JfText(title, style = JfType.listTitle, modifier = Modifier.weight(1f))
            JfText(stringResource(R.string.db_value, EqLabels.gain(value)), style = JfType.time, color = Jf.colors.ink70)
        }
        JfSlider(
            value = value,
            range = range,
            step = 0.5f,
            enabled = enabled,
            onChange = onChange,
            onChangeFinished = onChangeFinished,
            description = title,
            valueText = stringResource(R.string.db_value, EqLabels.gain(value)),
        )
    }
}

/**
 * The equalizer (docs/ui.md): the switch, the Preset picker (the core's `eq_presets()`, and
 * Custom once a band moves), the preamp, the ten bands (±12 dB in 0.5 dB steps) and Reset to
 * Flat. Sliders send while dragging at most every 60 ms, and once when the finger lifts.
 */
@Composable
private fun Equalizer(settings: Settings) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val presets by rememberQuery(Unit) { it.eqPresets() }
    val known = (presets as? QueryResult.Ok)?.value.orEmpty()
    var eq by remember(settings.eq) { mutableStateOf(settings.eq) }
    var lastSent by remember { mutableStateOf(0L) }
    fun send(force: Boolean) {
        val now = System.currentTimeMillis()
        if (force || now - lastSent >= 60) {
            lastSent = now
            graph.send(Command.SetEq(eq))
        }
    }
    val presetTitle = stringResource(R.string.eq_preset)
    Panel {
        SettingRow(stringResource(R.string.eq_title)) {
            JfSwitch(eq.enabled, {
                eq = eq.copy(enabled = it)
                send(true)
            })
        }
        Divider()
        val choose = {
            overlays.showSheet(presetTitle) { dismiss ->
                val current = EqPresets.choice(eq, known)
                Column(Modifier.selectableGroup().padding(top = 4.dp)) {
                    EqPresets.options(eq, known).forEach { option ->
                        ChoiceRow(eqChoiceLabel(option), option == current, {
                            dismiss()
                            if (option != current) {
                                eq = EqPresets.select(eq, option, known)
                                send(true)
                            }
                        })
                    }
                }
            }
        }
        SettingRow(
            presetTitle,
            // Nothing until the core's presets have answered, rather than a passing "Custom".
            description = if (presets is QueryResult.Ok) eqChoiceLabel(EqPresets.choice(eq, known)) else null,
            onClick = choose.takeIf { eq.enabled && known.isNotEmpty() },
        ) {
            JfIcon(R.drawable.ic_chevron_right, Jf.colors.ink30, size = 18.dp)
        }
        Divider()
        DecibelSlider(
            title = stringResource(R.string.preamp),
            value = eq.preampDb,
            range = -SettingsEdits.EQ_RANGE_DB..SettingsEdits.EQ_RANGE_DB,
            enabled = eq.enabled,
            onChange = {
                eq = SettingsEdits.eqPreamp(eq, it)
                send(false)
            },
            onChangeFinished = { send(true) },
        )
        Row(
            Modifier.fillMaxWidth().height(220.dp).padding(horizontal = 4.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
        ) {
            eq.bands.forEachIndexed { i, band ->
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    JfText(EqLabels.gain(band.gainDb), style = JfType.micro, color = Jf.colors.ink50, maxLines = 1)
                    VerticalBandSlider(
                        value = band.gainDb,
                        range = -SettingsEdits.EQ_RANGE_DB..SettingsEdits.EQ_RANGE_DB,
                        step = 0.5f,
                        enabled = eq.enabled,
                        onChange = {
                            eq = SettingsEdits.eqGain(eq, i, it)
                            send(false)
                        },
                        onChangeFinished = { send(true) },
                        description = stringResource(R.string.eq_band, EqLabels.frequency(band.freqHz)),
                        valueText = stringResource(R.string.db_value, EqLabels.gain(band.gainDb)),
                        modifier = Modifier.weight(1f),
                    )
                    JfText(EqLabels.frequency(band.freqHz), style = JfType.micro, color = Jf.colors.ink70, maxLines = 1)
                }
            }
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp), horizontalArrangement = Arrangement.End) {
            JfButton(stringResource(R.string.eq_reset), {
                eq = EqPresets.resetToFlat(eq, known)
                send(true)
            }, style = ButtonStyle.Quiet, enabled = eq.enabled)
        }
    }
}

/** A preset's name, translated by its stable id; a preset this version does not know shows its id. */
@Composable
private fun eqChoiceLabel(choice: EqChoice): String = when (choice) {
    EqChoice.Custom -> stringResource(R.string.eq_preset_custom)
    is EqChoice.Preset -> when (choice.id) {
        "flat" -> stringResource(R.string.eq_preset_flat)
        "bass_boost" -> stringResource(R.string.eq_preset_bass_boost)
        "vocal" -> stringResource(R.string.eq_preset_vocal)
        "treble" -> stringResource(R.string.eq_preset_treble)
        else -> EqLabels.presetName(choice.id)
    }
}

// ---- Library --------------------------------------------------------------------------------------

/** Android's own first group (an allowed difference): access to music, and Find Music on This Phone. */
private fun LazyListScope.musicOnThisPhone() {
    item(key = "access-h") { SectionHeader(stringResource(R.string.music_access_title)) }
    item(key = "access") {
        val graph = LocalGraph.current
        val access = rememberLibraryAccess()
        val overlays = LocalOverlays.current
        val context = LocalContext.current
        val scope = rememberCoroutineScope()
        Panel {
            if (!access.hasMusicPermission) {
                SettingRow(stringResource(R.string.action_allow_music), description = stringResource(R.string.music_access_desc)) {
                    JfButton(stringResource(R.string.action_allow), access.requestMusic, style = ButtonStyle.Accent)
                }
            } else {
                JfText(stringResource(R.string.music_access_granted), style = JfType.small, color = Jf.colors.ink70, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
                Divider()
                SettingRow(stringResource(R.string.find_music), description = stringResource(R.string.find_music_desc)) {
                    JfButton(stringResource(R.string.action_find), {
                        scope.launch {
                            val added = graph.library.addMediaStoreRoots()
                            overlays.toast(
                                if (added > 0) {
                                    context.resources.getQuantityString(R.plurals.toast_folders_added, added, added)
                                } else {
                                    context.getString(R.string.toast_no_new_folders)
                                },
                            )
                        }
                    })
                }
            }
        }
    }
}

/** Music Folders: each folder with Remove Folder… (asks first), then Add Folder…. */
private fun LazyListScope.musicFolders(state: CoreState) {
    item(key = "folders-h") { SectionHeader(stringResource(R.string.folders_title)) }
    item(key = "folders") {
        val graph = LocalGraph.current
        val overlays = LocalOverlays.current
        val access = rememberLibraryAccess()
        val roots by rememberQuery(state.libraryRevision) { it.roots() }
        val removeBody = stringResource(R.string.folder_remove_body)
        val removeAsk = stringResource(R.string.action_remove_folder_ask)
        val removeLabel = stringResource(R.string.action_remove_folder)
        val context = LocalContext.current
        Panel {
            JfText(stringResource(R.string.folders_desc), style = JfType.small, color = Jf.colors.ink70, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            when (val r = roots) {
                QueryResult.Loading -> Unit
                is QueryResult.NotAvailable -> Notice(stringResource(R.string.not_available_yet))
                is QueryResult.Failed -> Notice(stringResource(R.string.query_failed, r.message), warning = true)
                is QueryResult.Ok -> {
                    if (r.value.isEmpty()) {
                        JfText(stringResource(R.string.folders_empty), style = JfType.small, color = Jf.colors.ink50, modifier = Modifier.padding(16.dp))
                    }
                    r.value.forEach { path ->
                        val name = path.substringAfterLast('/').ifEmpty { path }
                        Divider()
                        SettingRow(name, description = path) {
                            JfIconButton(R.drawable.ic_trash, removeAsk, {
                                overlays.confirm(context.getString(R.string.folder_remove_title, name), removeBody, removeLabel, destructive = true) {
                                    graph.send(Command.RemoveRoot(path))
                                }
                            })
                        }
                    }
                }
            }
            Divider()
            SettingRow(stringResource(R.string.action_add_folder), onClick = access.addFolder) {
                JfIcon(R.drawable.ic_add, Jf.colors.accentText, size = 20.dp)
            }
        }
    }
}

private fun LazyListScope.watchFolders(settings: Settings) {
    item(key = "watch") {
        val edit = rememberEdit(settings)
        Panel {
            SettingRow(stringResource(R.string.watch_title), description = stringResource(R.string.watch_desc)) {
                JfSwitch(settings.watchFolders, { on -> edit { copy(watchFolders = on) } })
            }
        }
    }
}

/**
 * Rescan Library, as every app shows it: the sentence beside the Rescan Library button (off while
 * a scan runs), the scan's progress while it runs, and what the last scan did.
 */
private fun LazyListScope.rescan(state: CoreState) {
    item(key = "scan") {
        val graph = LocalGraph.current
        Panel {
            val progress = state.lastScan as? ScanStatus.Progress
            SettingRow(
                stringResource(R.string.rescan_desc),
                description = if (state.scanning && progress != null) stringResource(R.string.scanning, progress.filesSeen.toInt()) else null,
            ) {
                JfButton(stringResource(R.string.rescan_title), { graph.send(Command.Rescan) }, icon = R.drawable.ic_refresh, enabled = !state.scanning)
            }
            val last = when (val scan = state.lastScan) {
                is ScanStatus.Finished -> stringResource(R.string.last_scan_value, scan.added, scan.updated, scan.removed)
                is ScanStatus.Failed -> scan.message
                is ScanStatus.Progress, null -> null
            }
            if (last != null) {
                Divider()
                SettingRow(stringResource(R.string.last_scan), description = last)
            }
        }
    }
}

// ---- Sources --------------------------------------------------------------------------------------

/** Add a Source: the core's services, as on the Sources tab; choosing one asks for its address and sign-in. */
private fun LazyListScope.addSource(state: CoreState) {
    val presets = state.sources.presets
    if (presets.isEmpty()) return
    item(key = "add-h") { SectionHeader(stringResource(R.string.sources_add)) }
    item(key = "add") {
        val connect = rememberConnectSheet()
        Panel {
            presets.forEachIndexed { i, preset ->
                if (i > 0) Divider()
                PresetRow(preset, state.sources.plugins) { connect.open(preset) }
            }
        }
    }
}

/**
 * Installed, with Install from File… in its heading: each source opens its own page. Store builds
 * install no plugins from files (the core's `can_install_plugins`), so their heading has no
 * button (docs/ui.md, Allowed differences).
 */
private fun LazyListScope.installed(state: CoreState) {
    item(key = "installed-h") {
        if (state.sources.canInstallPlugins) {
            val install = rememberPluginInstaller()
            SectionHeader(stringResource(R.string.sources_installed)) {
                JfButton(stringResource(R.string.action_install_plugin), install, style = ButtonStyle.Quiet, icon = R.drawable.ic_add)
            }
        } else {
            SectionHeader(stringResource(R.string.sources_installed))
        }
    }
    item(key = "installed") {
        val nav = LocalNav.current
        val sources = YourSources.sources(state.sources.plugins)
        Panel {
            if (sources.isEmpty()) {
                SettingRow(stringResource(R.string.sources_empty_title), description = stringResource(R.string.sources_empty_body))
            }
            sources.forEachIndexed { i, plugin ->
                if (i > 0) Divider()
                NavRow(plugin.name, pluginStatus(plugin), onClick = { nav.push(Screen.Source(plugin.id)) }, leading = { PluginIcon(plugin.name, plugin.icon) })
            }
        }
    }
}

/**
 * Community Plugins: what they are, and Allow Community Plugins behind the disclosure (ADR-0012).
 * Not in store builds, which allow no community plugins (`can_install_plugins` false).
 */
private fun LazyListScope.communityPlugins(state: CoreState) {
    if (!state.sources.canInstallPlugins) return
    val settings = state.settings
    item(key = "community-h") { SectionHeader(stringResource(R.string.community_plugins)) }
    item(key = "community") {
        val edit = rememberEdit(settings)
        val disclosures = rememberDisclosures()
        Panel {
            SettingRow(stringResource(R.string.allow_community), description = stringResource(R.string.allow_community_desc)) {
                JfSwitch(settings.allowCommunityPlugins, { on ->
                    if (Disclosure.needsDisclosureToAllow(on)) {
                        disclosures.allowCommunity { edit { copy(allowCommunityPlugins = true) } }
                    } else {
                        edit { copy(allowCommunityPlugins = false) }
                    }
                })
            }
        }
    }
}

// ---- About ----------------------------------------------------------------------------------------

/**
 * About: the version from the build (never typed in), and the website with its third-party
 * notices, the Licence Agreement and the Privacy Policy when the build knows the website
 * (BuildConfig.WEBSITE, app/build.gradle.kts).
 */
private fun LazyListScope.about() {
    item(key = "about") {
        val context = LocalContext.current
        val core = rememberQuery(Unit) { it.libraryVersion() }
        val links = remember { AboutInfo.links(BuildConfig.WEBSITE) }
        Panel {
            SettingRow(stringResource(R.string.about_version_title)) {
                JfText(AboutInfo.version(BuildConfig.VERSION_NAME, core.ok), style = JfType.small, color = Jf.colors.ink70)
            }
            if (links != null) {
                Divider()
                SettingRow(stringResource(R.string.about_website), description = links.website, onClick = { openLink(context, links.website) }) {
                    JfIcon(R.drawable.ic_external, Jf.colors.ink50, size = 20.dp)
                }
                Divider()
                LinkRow(stringResource(R.string.about_licences)) { openLink(context, links.licences) }
                Divider()
                LinkRow(stringResource(R.string.about_licence_agreement)) { openLink(context, links.licenceAgreement) }
                Divider()
                LinkRow(stringResource(R.string.about_privacy_policy)) { openLink(context, links.privacyPolicy) }
            }
        }
    }
    item(key = "about-design") {
        JfText(stringResource(R.string.about_design), style = JfType.caption, color = Jf.colors.ink50, modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp))
    }
}

/**
 * Direct downloads (the Update slice's `supported`; store builds are updated by their store):
 * "Jellyfesh {version} Is Available" with View Release, Check for Updates and Check for Updates
 * Automatically. The core checks; nothing is installed.
 */
private fun LazyListScope.updates(state: CoreState) {
    val update = state.update
    if (!update.supported) return
    item(key = "updates-h") { SectionHeader(stringResource(R.string.update_title)) }
    item(key = "updates") {
        val graph = LocalGraph.current
        val context = LocalContext.current
        val edit = rememberEdit(state.settings)
        Panel {
            val available = update.available
            if (available != null) {
                SettingRow(stringResource(R.string.update_available, available.version)) {
                    JfButton(stringResource(R.string.update_view_release), { openLink(context, available.url) }, style = ButtonStyle.Accent)
                }
                Divider()
            }
            SettingRow(
                stringResource(R.string.update_check),
                description = if (update.checking) stringResource(R.string.update_checking) else update.error,
                onClick = if (update.checking) null else ({ graph.send(Command.CheckForUpdates) }),
            ) {
                JfIcon(R.drawable.ic_refresh, Jf.colors.ink50, size = 20.dp)
            }
            Divider()
            SettingRow(stringResource(R.string.update_auto)) {
                JfSwitch(state.settings.checkForUpdates, { on -> edit { copy(checkForUpdates = on) } })
            }
        }
    }
}

/** A row that opens a web page. */
@Composable
internal fun LinkRow(title: String, description: String? = null, onClick: () -> Unit) {
    SettingRow(title, description = description, onClick = onClick) {
        JfIcon(R.drawable.ic_external, Jf.colors.ink50, size = 20.dp)
    }
}

/** Opens a web page in a Custom Tab (the browser the person uses). */
internal fun openLink(context: android.content.Context, url: String) {
    runCatching { CustomTabsIntent.Builder().setShowTitle(true).build().launchUrl(context, Uri.parse(url)) }
}

/** A setting with its control under the title (segmented controls need the width); no title under a group heading that says it. */
@Composable
private fun SettingColumn(title: String?, description: String?, control: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (title != null) JfText(title, style = JfType.listTitle.copy(fontSize = JfType.body.fontSize))
        if (description != null) JfText(description, style = JfType.small, color = Jf.colors.ink70)
        control()
    }
}

package org.jellyfesh.android.ui.settings

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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.statusBars
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
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.jellyfesh.android.QueryResult
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.BackBar
import org.jellyfesh.android.ui.components.Divider
import org.jellyfesh.android.ui.components.InitialTile
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.NavRow
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Panel
import org.jellyfesh.android.ui.components.ScreenTitle
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.SettingRow
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.library.barFade
import org.jellyfesh.android.ui.library.listPadding
import org.jellyfesh.android.ui.library.rememberLibraryAccess
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.sources.rememberDisclosures
import org.jellyfesh.android.ui.sources.rememberPluginInstaller
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfSlider
import org.jellyfesh.android.ui.theme.JfSwitch
import org.jellyfesh.android.ui.theme.GlassGrade
import org.jellyfesh.android.ui.theme.GlassSurface
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.MistyField
import org.jellyfesh.android.ui.theme.TouchTarget
import org.jellyfesh.android.ui.theme.SegmentedControl
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
import org.jellyfesh.logic.settings.EqLabels
import org.jellyfesh.logic.settings.SettingsEdits
import org.jellyfesh.logic.sources.Disclosure
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.logic.theme.AccentChoices
import org.jellyfesh.logic.theme.Palette
import org.jellyfesh.tokens.Tokens

/**
 * Settings. The core owns every setting: a change sends a modified copy of the slice
 * (`SetSettings`) or the narrower command the core offers (EQ, ReplayGain, crossfade, output), and
 * the screen shows the slice the core sends back.
 */
@Composable
fun SettingsScreen(section: SettingsSection?, state: CoreState) {
    val nav = LocalNav.current
    Box(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()), contentPadding = listPadding()) {
            item(key = "bar-space") { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            item(key = "title") { ScreenTitle(stringResource(section?.let(::sectionTitle) ?: R.string.settings_title)) }
            when (section) {
                null -> sectionList()
                SettingsSection.Appearance -> appearance(state)
                SettingsSection.Playback -> playback(state)
                SettingsSection.Library -> library(state)
                SettingsSection.Lyrics -> lyrics(state.settings)
                SettingsSection.Sources -> sources(state)
            }
        }
        BackBar(onBack = nav::back, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars))
    }
}

fun sectionTitle(section: SettingsSection): Int = when (section) {
    SettingsSection.Appearance -> R.string.settings_appearance
    SettingsSection.Playback -> R.string.settings_playback
    SettingsSection.Library -> R.string.settings_library
    SettingsSection.Lyrics -> R.string.settings_lyrics
    SettingsSection.Sources -> R.string.settings_sources
}

private fun sectionDescription(section: SettingsSection): Int = when (section) {
    SettingsSection.Appearance -> R.string.settings_appearance_desc
    SettingsSection.Playback -> R.string.settings_playback_desc
    SettingsSection.Library -> R.string.settings_library_desc
    SettingsSection.Lyrics -> R.string.settings_lyrics_desc
    SettingsSection.Sources -> R.string.settings_sources_desc
}

private fun sectionIcon(section: SettingsSection): Int = when (section) {
    SettingsSection.Appearance -> R.drawable.ic_palette
    SettingsSection.Playback -> R.drawable.ic_eq
    SettingsSection.Library -> R.drawable.ic_library
    SettingsSection.Lyrics -> R.drawable.ic_lyrics
    SettingsSection.Sources -> R.drawable.ic_sources
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
    item(key = "about") {
        Column(Modifier.fillMaxWidth().padding(24.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            JfText(stringResource(R.string.about_version, ABOUT_VERSION), style = JfType.caption, color = Jf.colors.ink50)
            JfText(stringResource(R.string.about_design), style = JfType.caption, color = Jf.colors.ink50)
        }
    }
}

private const val ABOUT_VERSION = "0.0.4"

/** Sends a modified copy of the settings slice. */
@Composable
private fun rememberEdit(settings: Settings): (Settings.() -> Settings) -> Unit {
    val graph = LocalGraph.current
    return { change -> graph.send(SettingsEdits.set(settings, change)) }
}

// ---- Appearance (docs/design-controls.md, "Preferences → Appearance") ------------------------------
// Grouped cards; every change applies at once and the core keeps it (SetSettings).

private fun LazyListScope.appearance(state: CoreState) {
    val settings = state.settings
    item(key = "mode") {
        val edit = rememberEdit(settings)
        Panel {
            SettingColumn(stringResource(R.string.appearance_theme), stringResource(R.string.appearance_theme_desc)) {
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
                            label = stringResource(R.string.hue_value, swatch.toInt()),
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
    item(key = "transparency") {
        val edit = rememberEdit(settings)
        var percent by remember(settings.transparency) { mutableStateOf(SettingsEdits.transparencyPercent(settings.transparency).toFloat()) }
        Panel {
            SettingColumn(stringResource(R.string.transparency_title), stringResource(R.string.transparency_desc)) {
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
    item(key = "background") {
        val edit = rememberEdit(settings)
        Panel {
            SettingColumn(stringResource(R.string.style_title), stringResource(R.string.style_desc)) {
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
    item(key = "lyrics-look") {
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
        }
    }
    item(key = "icon") { AppIconPicker(settings) }
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

private fun LazyListScope.playback(state: CoreState) {
    val settings = state.settings
    item(key = "output-h") { SectionHeader(stringResource(R.string.output_title)) }
    item(key = "output") {
        val graph = LocalGraph.current
        LaunchedEffect(Unit) { graph.send(Command.RefreshDevices) }
        val devices = state.devices
        Panel {
            JfText(stringResource(R.string.output_desc), style = JfType.small, color = Jf.colors.ink70, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            DeviceChoice(stringResource(R.string.np_system_default), PlayerModel.isSelected(devices, null)) { graph.send(Command.SetOutputDevice(null)) }
            devices.devices.forEach { d ->
                Divider()
                DeviceChoice(d.name, PlayerModel.isSelected(devices, d.id)) { graph.send(Command.SetOutputDevice(d.id)) }
            }
        }
    }
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
    item(key = "rg-h") { SectionHeader(stringResource(R.string.replaygain_title)) }
    item(key = "rg") {
        val graph = LocalGraph.current
        var preamp by remember(settings.preampDb) { mutableStateOf(settings.preampDb) }
        Panel {
            SettingColumn(stringResource(R.string.replaygain_title), stringResource(R.string.replaygain_desc)) {
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
                    description = stringResource(R.string.replaygain_title),
                )
            }
            Column(Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    JfText(stringResource(R.string.preamp), style = JfType.listTitle, modifier = Modifier.weight(1f))
                    JfText(stringResource(R.string.db_value, EqLabels.gain(preamp)), style = JfType.time, color = Jf.colors.ink70)
                }
                JfSlider(
                    value = preamp,
                    range = -15f..15f,
                    step = 0.5f,
                    enabled = settings.replaygainMode != ReplayGainMode.Off,
                    onChange = { preamp = SettingsEdits.preamp(it) },
                    onChangeFinished = { graph.send(Command.SetReplayGain(settings.replaygainMode, preamp)) },
                    description = stringResource(R.string.preamp),
                    valueText = stringResource(R.string.db_value, EqLabels.gain(preamp)),
                )
            }
        }
    }
    item(key = "eq-h") { SectionHeader(stringResource(R.string.eq_title)) }
    item(key = "eq") { Equalizer(settings) }
    item(key = "resume") {
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
    SettingRow(name, onClick = onClick) {
        if (selected) org.jellyfesh.android.ui.components.JfIcon(R.drawable.ic_check, Jf.colors.accentText, size = 20.dp, description = stringResource(R.string.state_selected))
    }
}

/**
 * The ten-band equalizer: one vertical slider per band (±12 dB in 0.5 dB steps), sent while
 * dragging at most every 60 ms and once when the finger lifts.
 */
@Composable
private fun Equalizer(settings: Settings) {
    val graph = LocalGraph.current
    var eq by remember(settings.eq) { mutableStateOf(settings.eq) }
    var lastSent by remember { mutableStateOf(0L) }
    fun send(force: Boolean) {
        val now = System.currentTimeMillis()
        if (force || now - lastSent >= 60) {
            lastSent = now
            graph.send(Command.SetEq(eq))
        }
    }
    Panel {
        SettingRow(stringResource(R.string.eq_title)) {
            JfSwitch(eq.enabled, {
                eq = eq.copy(enabled = it)
                send(true)
            })
        }
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
                eq = SettingsEdits.eqFlat(eq)
                send(true)
            }, style = ButtonStyle.Quiet, enabled = eq.enabled)
        }
    }
}

// ---- Library --------------------------------------------------------------------------------------

private fun LazyListScope.library(state: CoreState) {
    item(key = "access") {
        val graph = LocalGraph.current
        val access = rememberLibraryAccess()
        val overlays = LocalOverlays.current
        val context = LocalContext.current
        val scope = rememberCoroutineScope()
        Panel {
            SettingRow(
                stringResource(R.string.music_access_title),
                description = stringResource(if (access.hasMusicPermission) R.string.music_access_granted else R.string.music_access_desc),
            ) {
                if (!access.hasMusicPermission) JfButton(stringResource(R.string.action_allow), access.requestMusic, style = ButtonStyle.Accent)
            }
            if (access.hasMusicPermission) {
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
    item(key = "folders-h") {
        SectionHeader(stringResource(R.string.folders_title)) {
            val access = rememberLibraryAccess()
            JfButton(stringResource(R.string.action_add_folder), access.addFolder, style = ButtonStyle.Quiet, icon = R.drawable.ic_add)
        }
    }
    item(key = "folders") {
        val graph = LocalGraph.current
        val overlays = LocalOverlays.current
        val roots by rememberQuery(state.libraryRevision) { it.roots() }
        val removeBody = stringResource(R.string.folder_remove_body)
        val removeLabel = stringResource(R.string.action_remove)
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
                        Divider()
                        SettingRow(path.substringAfterLast('/').ifEmpty { path }, description = path) {
                            JfIconButton(R.drawable.ic_trash, removeLabel, {
                                overlays.confirm(context.getString(R.string.folder_remove_title, path.substringAfterLast('/')), removeBody, removeLabel, destructive = true) {
                                    graph.send(Command.RemoveRoot(path))
                                }
                            })
                        }
                    }
                }
            }
        }
    }
    item(key = "scan") {
        val graph = LocalGraph.current
        val edit = rememberEdit(state.settings)
        val stats by rememberQuery(state.libraryRevision) { it.stats() }
        Panel {
            val progress = state.lastScan as? ScanStatus.Progress
            SettingRow(
                stringResource(R.string.rescan_title),
                description = when {
                    state.scanning && progress != null -> stringResource(R.string.scanning, progress.filesSeen.toInt())
                    else -> stringResource(R.string.rescan_desc)
                },
            ) {
                JfButton(stringResource(R.string.action_rescan), { graph.send(Command.Rescan) }, enabled = !state.scanning)
            }
            Divider()
            SettingRow(stringResource(R.string.watch_title), description = stringResource(R.string.watch_desc)) {
                JfSwitch(state.settings.watchFolders, { on -> edit { copy(watchFolders = on) } })
            }
            (stats as? QueryResult.Ok)?.value?.let { s ->
                Divider()
                JfText(
                    listOf(
                        pluralStringResource(R.plurals.count_songs, s.tracks.toInt(), s.tracks.toInt()),
                        pluralStringResource(R.plurals.count_albums, s.albums.toInt(), s.albums.toInt()),
                        pluralStringResource(R.plurals.count_artists, s.artists.toInt(), s.artists.toInt()),
                    ).joinToString(" · "),
                    style = JfType.caption,
                    color = Jf.colors.ink50,
                    modifier = Modifier.padding(16.dp),
                )
            }
        }
    }
}

// ---- Lyrics ---------------------------------------------------------------------------------------

private fun LazyListScope.lyrics(settings: Settings) {
    item(key = "lyrics") {
        val edit = rememberEdit(settings)
        Panel {
            SettingRow(stringResource(R.string.show_translation), description = stringResource(R.string.show_translation_desc)) {
                JfSwitch(settings.showTranslation, { on -> edit { copy(showTranslation = on) } })
            }
            Divider()
            SettingRow(stringResource(R.string.show_romanization), description = stringResource(R.string.show_romanization_desc)) {
                JfSwitch(settings.showRomanization, { on -> edit { copy(showRomanization = on) } })
            }
        }
    }
}

// ---- Sources --------------------------------------------------------------------------------------

private fun LazyListScope.sources(state: CoreState) {
    item(key = "community") {
        val edit = rememberEdit(state.settings)
        val disclosures = rememberDisclosures()
        Panel {
            SettingRow(stringResource(R.string.allow_community), description = stringResource(R.string.allow_community_desc)) {
                JfSwitch(state.settings.allowCommunityPlugins, { on ->
                    if (Disclosure.needsDisclosureToAllow(on)) {
                        disclosures.allowCommunity { edit { copy(allowCommunityPlugins = true) } }
                    } else {
                        edit { copy(allowCommunityPlugins = false) }
                    }
                })
            }
        }
    }
    item(key = "install") {
        val install = rememberPluginInstaller()
        Panel {
            SettingRow(stringResource(R.string.action_install_plugin), description = stringResource(R.string.install_plugin_desc), onClick = install)
        }
    }
    item(key = "installed-h") { SectionHeader(stringResource(R.string.sources_installed)) }
    item(key = "installed") {
        val nav = LocalNav.current
        Panel {
            if (state.sources.plugins.isEmpty()) {
                JfText(stringResource(R.string.sources_empty_body), style = JfType.small, color = Jf.colors.ink50, modifier = Modifier.padding(16.dp))
            }
            state.sources.plugins.forEachIndexed { i, plugin ->
                if (i > 0) Divider()
                NavRow(plugin.name, plugin.version, onClick = { nav.push(Screen.Source(plugin.id)) }, leading = { InitialTile(plugin.name, icon = R.drawable.ic_sources) })
            }
        }
    }
    item(key = "bundled") { Notice(stringResource(R.string.sources_bundled_note)) }
}

/** A setting with its control under the title (segmented controls need the width). */
@Composable
private fun SettingColumn(title: String, description: String?, control: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        JfText(title, style = JfType.listTitle.copy(fontSize = JfType.body.fontSize))
        if (description != null) JfText(description, style = JfType.small, color = Jf.colors.ink70)
        control()
    }
}

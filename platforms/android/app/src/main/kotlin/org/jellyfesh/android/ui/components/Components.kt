package org.jellyfesh.android.ui.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.isSecondaryPressed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.images.rememberArtwork
import org.jellyfesh.android.ui.LocalWindowWidthDp
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.GlassGrade
import org.jellyfesh.android.ui.theme.GlassSurface
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfMotion
import org.jellyfesh.android.ui.theme.LevelGlyph
import org.jellyfesh.android.ui.theme.LocalPlaybackRunning
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.jfShadow
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.format.TimeFormat
import org.jellyfesh.logic.theme.Luminous
import org.jellyfesh.tokens.Tokens
import org.jellyfesh.android.ui.theme.rounded
import org.jellyfesh.android.ui.theme.spaced

/**
 * Artwork at [size]: the image once it has loaded off the main thread, else a placeholder tile
 * with the title's first letter on the accent's soft tint. Decorative: rows say what they are.
 */
@Composable
fun ArtworkImage(key: ArtKey?, title: String, size: Dp, shape: Shape, modifier: Modifier = Modifier) {
    val px = with(LocalDensity.current) { size.roundToPx() }
    val image = rememberArtwork(LocalGraph.current.images, key, px)
    ArtworkTile(image, title, shape, modifier.size(size))
}

/** Artwork that fills the width it is given, square ([ArtworkImage] for a known width). */
@Composable
fun ArtworkFill(key: ArtKey?, title: String, shape: Shape, sizePx: Int, modifier: Modifier = Modifier) {
    val image = rememberArtwork(LocalGraph.current.images, key, sizePx)
    ArtworkTile(image, title, shape, modifier.fillMaxWidth().aspectRatio(1f))
}

@Composable
private fun ArtworkTile(image: androidx.compose.ui.graphics.ImageBitmap?, title: String, shape: Shape, modifier: Modifier) {
    val colors = Jf.colors
    Box(modifier.clip(shape).background(colors.accentSoft), contentAlignment = Alignment.Center) {
        if (image != null) {
            Image(image, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        } else {
            JfText(Labels.initial(title), style = JfType.heading, color = colors.accentText, maxLines = 1)
        }
    }
}

/**
 * Right-click (a mouse's secondary button, a touchpad's two-finger click) runs [onClick]: the
 * element's menu, as in every app (docs/ui.md, "Rows and cards"). The press is consumed before the
 * element's own click sees it, so a right-click never also plays or opens.
 */
fun Modifier.onSecondaryClick(onClick: (() -> Unit)?): Modifier = if (onClick == null) this else pointerInput(onClick) {
    awaitPointerEventScope {
        while (true) {
            val event = awaitPointerEvent(PointerEventPass.Initial)
            if (event.type == PointerEventType.Press && event.buttons.isSecondaryPressed) {
                event.changes.forEach { it.consume() }
                onClick()
            }
        }
    }
}

/**
 * A track row (52 dp, Tokens.Player.rowHeight): artwork or a number, title and a secondary line,
 * the duration and a More button. The row playing now is accent-soft with its title in accent
 * text, marked as selected, and (numbered rows) shows the level glyph, which moves only while
 * music plays (after Apple Music, v0.0.4). Rows and cards (v0.0.9): More shows on every row
 * (phones have no hover); a long press or a right-click opens the same menu ([onMenu]); a mouse
 * over the row or keyboard focus on it shows the play button over its artwork or number, as on
 * Jellyfesh Web (the row plays from there when chosen).
 */
@Composable
fun TrackRow(
    title: String,
    subtitle: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    art: ArtKey? = null,
    showArt: Boolean = true,
    number: Int? = null,
    durationMs: Long? = null,
    playing: Boolean = false,
    missing: Boolean = false,
    favorite: Boolean = false,
    /** A source's song whose file is on this device: the Downloaded mark (ADR-0025). */
    downloaded: Boolean = false,
    onMore: (() -> Unit)? = null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
    /** False for a row that does nothing when chosen (a missing file, a song that cannot play here and has no link). */
    enabled: Boolean = !missing,
    /** Drawn in the faint ink: a missing file, or a source's song that cannot play here (its [subtitle] says why). */
    dimmed: Boolean = missing,
    /** Long press and right-click: the row's menu; More's, unless the row draws its own More in [trailing]. */
    onMenu: (() -> Unit)? = onMore,
) {
    val colors = Jf.colors
    val moreLabel = stringResource(R.string.action_more)
    val menuLabel = stringResource(R.string.a11y_show_menu)
    val haptics = LocalHapticFeedback.current
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    val focused by interaction.collectIsFocusedAsState()
    val showPlay = enabled && !playing && (hovered || focused)
    Row(
        modifier
            .fillMaxWidth()
            // The look's density spaces the rows (never under the 48 dp touch target).
            .heightIn(min = maxOf(spaced(Tokens.Player.rowHeight.dp + 8.dp), 48.dp))
            .then(if (playing) Modifier.padding(horizontal = 6.dp).background(colors.accentSoft, rounded(Tokens.Radius.rMd.dp)) else Modifier)
            .onSecondaryClick(onMenu)
            .combinedClickable(
                interaction,
                indication = LocalIndication.current,
                enabled = enabled,
                role = Role.Button,
                onLongClickLabel = menuLabel.takeIf { onMenu != null },
                onLongClick = onMenu,
                onClick = onClick,
            )
            // A row that does nothing when chosen still opens its menu on a long press (TalkBack has More).
            .then(
                if (!enabled && onMenu != null) {
                    Modifier.pointerInput(onMenu) {
                        detectTapGestures(onLongPress = {
                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            onMenu()
                        })
                    }
                } else {
                    Modifier
                },
            )
            .semantics { selected = playing }
            .padding(horizontal = if (playing) 10.dp else 16.dp, vertical = spaced(4.dp)),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        when {
            number != null -> Box(Modifier.width(28.dp), contentAlignment = Alignment.Center) {
                when {
                    playing -> LevelGlyph(LocalPlaybackRunning.current)
                    showPlay -> JfIcon(R.drawable.ic_play, colors.ink90, size = 16.dp)
                    else -> JfText(number.toString(), style = JfType.time, color = colors.ink50, maxLines = 1)
                }
            }
            showArt -> Box {
                val shape = rounded(Tokens.Artwork.radiusList.dp)
                ArtworkImage(art, title, 44.dp, shape)
                if (showPlay) {
                    Box(Modifier.matchParentSize().background(colors.scrim, shape), contentAlignment = Alignment.Center) {
                        JfIcon(R.drawable.ic_play, colors.onAccent, size = 18.dp)
                    }
                }
            }
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            JfText(
                title,
                style = JfType.listTitle,
                color = when {
                    dimmed -> colors.ink30
                    playing -> colors.accentText
                    else -> colors.ink90
                },
                maxLines = 1,
            )
            val second = if (missing) stringResource(R.string.track_missing) else subtitle
            if (second != null) JfText(second, style = JfType.listSecondary, color = colors.ink70, maxLines = 1)
        }
        if (favorite) JfIcon(R.drawable.ic_heart_filled, colors.accentText, size = 16.dp, description = stringResource(R.string.a11y_favourite))
        if (downloaded) JfIcon(R.drawable.ic_downloaded, colors.ink50, size = 16.dp, description = stringResource(R.string.a11y_downloaded))
        if (durationMs != null && durationMs > 0) {
            JfText(TimeFormat.clock(durationMs), style = JfType.time, color = colors.ink50, maxLines = 1)
        }
        trailing?.invoke(this)
        if (onMore != null) JfIconButton(R.drawable.ic_more, moreLabel, onMore, size = 40.dp, iconSize = 20.dp)
    }
}

/**
 * A row with a leading glyph tile, a title, a secondary line and a chevron (artists, playlists).
 * With [onMore], a long press or a right-click opens the same menu (docs/ui.md, "Rows and cards").
 */
@Composable
fun NavRow(
    title: String,
    subtitle: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    leading: @Composable () -> Unit = {},
    onMore: (() -> Unit)? = null,
    /** The glyph at the end: the chevron of a row that opens a page, or another (Open in {Service}). */
    trailingIcon: Int = R.drawable.ic_chevron_right,
    /** A short badge before the chevron ("Pro" on what Jellyfesh Pro opens while it is inactive). */
    badge: String? = null,
) {
    val colors = Jf.colors
    val menuLabel = stringResource(R.string.a11y_show_menu)
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = maxOf(spaced(60.dp), 48.dp))
            .onSecondaryClick(onMore)
            .combinedClickable(role = Role.Button, onLongClickLabel = menuLabel.takeIf { onMore != null }, onLongClick = onMore, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = spaced(6.dp)),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        leading()
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            JfText(title, style = JfType.listTitle, maxLines = 1)
            if (subtitle != null) JfText(subtitle, style = JfType.listSecondary, color = colors.ink70, maxLines = 1)
        }
        if (badge != null) org.jellyfesh.android.ui.theme.Badge(badge, warning = false)
        if (onMore != null) JfIconButton(R.drawable.ic_more, stringResource(R.string.action_more), onMore, size = 40.dp, iconSize = 20.dp)
        JfIcon(trailingIcon, colors.ink30, size = 18.dp)
    }
}

/** A round tile with a letter (artists) or a glyph. */
@Composable
fun InitialTile(text: String, size: Dp = 44.dp, icon: Int? = null) {
    val colors = Jf.colors
    Box(Modifier.size(size).clip(CircleShape).background(colors.accentSoft), contentAlignment = Alignment.Center) {
        if (icon != null) JfIcon(icon, colors.accentText, size = size * 0.5f) else JfText(Labels.initial(text), style = JfType.label, color = colors.accentText)
    }
}

/**
 * An album card for grids and shelves (Luminous): square artwork with artwork.radius-grid (16)
 * and shadow-2, title and artist. Hovering with a mouse (desktop mode) or keyboard focus on the
 * card or a button in it lifts the cover 4 dp and shows a 36 dp glass play button in its bottom
 * start corner ([onPlay]) and More in its top end corner; a press scales it to 97 % (after Apple
 * Music). A tap opens it; More, a long press or a right-click opens its menu ([onMenu]: Play,
 * Shuffle, Play Next, Play Last, Add to Playlist…, Go to Artist, Download and Open in {Service},
 * as the item allows; docs/ui.md, "Rows and cards").
 */
@Composable
fun AlbumCard(
    title: String,
    subtitle: String?,
    art: ArtKey?,
    sizePx: Int,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    onMenu: (() -> Unit)? = null,
    onPlay: (() -> Unit)? = null,
) {
    val colors = Jf.colors
    val menuLabel = stringResource(R.string.a11y_show_menu)
    val reduce = Jf.effects.reduceMotion
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    val pressed by interaction.collectIsPressedAsState()
    // Focus on the card or on its play button or More: the buttons stay while keys move between them.
    var focusWithin by remember { mutableStateOf(false) }
    val raised = hovered || focusWithin
    val lift by animateDpAsState(if (raised) 4.dp else 0.dp, JfMotion.fluid(Tokens.Motion.dur1Ms, reduce), label = "lift")
    val press by animateFloatAsState(if (pressed) 0.97f else 1f, JfMotion.spring(Tokens.Motion.dur0Ms, reduce), label = "press")
    val controls by animateFloatAsState(if (raised && (onPlay != null || onMenu != null)) 1f else 0f, JfMotion.fluid(Tokens.Motion.dur1Ms, reduce), label = "controls")
    val coverShape = rounded(Tokens.Artwork.radiusGrid.dp)
    Column(
        modifier
            .clip(rounded(Tokens.Radius.rLg.dp))
            .hoverable(interaction)
            .onFocusChanged { focusWithin = it.hasFocus }
            .onSecondaryClick(onMenu)
            .combinedClickable(
                interaction,
                indication = LocalIndication.current,
                role = Role.Button,
                onLongClickLabel = menuLabel.takeIf { onMenu != null },
                onLongClick = onMenu,
                onClick = onClick,
            )
            .semantics(mergeDescendants = true) {}
            .padding(4.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(
            Modifier
                .graphicsLayer {
                    translationY = -lift.toPx()
                    scaleX = press
                    scaleY = press
                }
                .jfShadow(if (raised) 3 else 2, coverShape),
        ) {
            ArtworkFill(art, title, coverShape, sizePx)
            if (onPlay != null && controls > 0f) {
                val playLabel = stringResource(R.string.card_play, title)
                GlassSurface(
                    Modifier
                        .align(Alignment.BottomStart)
                        .padding(8.dp)
                        .size(36.dp)
                        .graphicsLayer { alpha = controls }
                        .clickable(role = Role.Button, onClick = onPlay)
                        .semantics { contentDescription = playLabel },
                    grade = GlassGrade.Mist,
                    shape = CircleShape,
                    // A third live layer would break the glass budget; the tint alone reads on any cover.
                    blur = false,
                ) {
                    JfIcon(R.drawable.ic_play, colors.ink90, Modifier.align(Alignment.Center), size = 16.dp)
                }
            }
            if (onMenu != null && controls > 0f) {
                val moreLabel = stringResource(R.string.card_more, title)
                GlassSurface(
                    Modifier
                        .align(Alignment.TopEnd)
                        .padding(8.dp)
                        .size(36.dp)
                        .graphicsLayer { alpha = controls }
                        .clickable(role = Role.Button, onClick = onMenu)
                        .semantics { contentDescription = moreLabel },
                    grade = GlassGrade.Mist,
                    shape = CircleShape,
                    blur = false,
                ) {
                    JfIcon(R.drawable.ic_more, colors.ink90, Modifier.align(Alignment.Center), size = 18.dp)
                }
            }
        }
        Column(Modifier.padding(horizontal = 2.dp)) {
            JfText(title, style = JfType.listTitle, maxLines = 1)
            if (subtitle != null) JfText(subtitle, style = JfType.listSecondary, color = colors.ink70, maxLines = 1)
        }
    }
}

/** A section heading (Misty Heading, 20/700), marked as a heading for TalkBack. */
@Composable
fun SectionHeader(text: String, modifier: Modifier = Modifier, action: (@Composable () -> Unit)? = null) {
    Row(
        modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = spaced(20.dp), bottom = spaced(8.dp)),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        JfText(text, style = JfType.section, maxLines = 2, modifier = Modifier.weight(1f).semantics { heading() })
        action?.invoke()
    }
}

/**
 * A page's title block, as Jellyfesh Web's: a large title (type.text-h2, 30.4, under 900 dp;
 * type.text-h1, 48, from 900 dp), a line under it and actions on the right (a sort menu).
 */
@Composable
fun ScreenTitle(title: String, modifier: Modifier = Modifier, subtitle: String? = null, actions: @Composable RowScope.() -> Unit = {}) {
    val colors = Jf.colors
    val width = LocalWindowWidthDp.current
    val size = Luminous.pageTitleSp(width, Tokens.TypeScale.textH2, Tokens.TypeScale.textH1)
    Row(
        modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            JfText(title, style = JfType.pageTitle(size), maxLines = 2, modifier = Modifier.semantics { heading() })
            if (subtitle != null) JfText(subtitle, style = JfType.small, color = colors.ink70, maxLines = 2)
        }
        actions()
    }
}

/**
 * A detail page's top bar: Jellyfesh Web's Back (the chevron and "Back" in accent text), then the
 * title once the header has scrolled away, and the page's actions.
 */
@Composable
fun BackBar(onBack: () -> Unit, modifier: Modifier = Modifier, title: String? = null, actions: @Composable RowScope.() -> Unit = {}) {
    Row(
        modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        BackButton(onBack)
        if (title != null) {
            JfText(title, style = JfType.label.copy(fontSize = JfType.body.fontSize), maxLines = 1, modifier = Modifier.weight(1f).padding(horizontal = 4.dp))
        } else {
            Spacer(Modifier.weight(1f))
        }
        actions()
    }
}

/** Back as on Jellyfesh Web: the chevron and the word, in accent text, on a capsule that tints when pressed. */
@Composable
fun BackButton(onBack: () -> Unit, modifier: Modifier = Modifier) {
    val colors = Jf.colors
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    Row(
        modifier
            .heightIn(min = 48.dp)
            .clip(CircleShape)
            .background(if (pressed) colors.fillPress else androidx.compose.ui.graphics.Color.Transparent, CircleShape)
            .clickable(interaction, indication = null, role = Role.Button, onClick = onBack)
            .padding(start = 6.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        JfIcon(R.drawable.ic_back, colors.accentText, size = 18.dp)
        JfText(stringResource(R.string.action_back), style = JfType.label.copy(fontSize = JfType.body.fontSize), color = colors.accentText, maxLines = 1)
    }
}

/** An empty list: a glyph, a title, a sentence and an optional action. */
@Composable
fun EmptyState(icon: Int, title: String, body: String?, modifier: Modifier = Modifier, action: (@Composable () -> Unit)? = null) {
    val colors = Jf.colors
    Column(
        modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 40.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(Modifier.size(64.dp).clip(CircleShape).background(colors.accentSoft), contentAlignment = Alignment.Center) {
            JfIcon(icon, colors.accentText, size = 28.dp)
        }
        JfText(title, style = JfType.heading, textAlign = TextAlign.Center)
        if (body != null) JfText(body, style = JfType.small, color = colors.ink70, textAlign = TextAlign.Center, modifier = Modifier.widthIn(max = 420.dp))
        if (action != null) Box(Modifier.padding(top = 6.dp)) { action() }
    }
}

/** A notice in a list: a feature not in this build yet, or a failure ([title] above the text, when it has one). */
@Composable
fun Notice(text: String, modifier: Modifier = Modifier, warning: Boolean = false, action: Pair<String, () -> Unit>? = null, title: String? = null) {
    val colors = Jf.colors
    Row(
        modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
            .clip(rounded(Tokens.Radius.rLg.dp))
            .background(colors.fill)
            .border(1.dp, colors.lineSoft, rounded(Tokens.Radius.rLg.dp))
            .padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        JfIcon(if (warning) R.drawable.ic_warning else R.drawable.ic_info, if (warning) colors.warning else colors.accentText, size = 20.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            if (title != null) JfText(title, style = JfType.label, color = colors.ink90)
            JfText(text, style = JfType.small, color = colors.ink70)
        }
        if (action != null) JfButton(action.first, action.second, style = ButtonStyle.Quiet)
    }
}

/** A quiet "Loading…" row while the first page of a list loads. */
@Composable
fun LoadingRow(modifier: Modifier = Modifier) {
    val text = stringResource(R.string.state_loading)
    Box(
        modifier.fillMaxWidth().height(72.dp).clearAndSetSemantics { contentDescription = text },
        contentAlignment = Alignment.Center,
    ) {
        JfText(text, style = JfType.small, color = Jf.colors.ink50)
    }
}

/**
 * A grouped surface (Luminous): settings groups and short lists take the Mist glass tint without
 * live blur, Misty's edge and shadow-1, radius 24 (r-2xl); solid Cloud where glass falls back.
 */
@Composable
fun Panel(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val shape = rounded(Tokens.Radius.r2xl.dp)
    GlassSurface(
        modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = spaced(6.dp)),
        grade = GlassGrade.Mist,
        shape = shape,
        blur = false,
        raised = true,
    ) {
        Column(Modifier.fillMaxWidth().clip(shape).padding(vertical = 4.dp), content = content)
    }
}

/** A row inside a [Panel]: a title, an optional description and a control at the end. */
@Composable
fun SettingRow(
    title: String,
    modifier: Modifier = Modifier,
    description: String? = null,
    onClick: (() -> Unit)? = null,
    control: (@Composable () -> Unit)? = null,
) {
    val colors = Jf.colors
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = maxOf(spaced(56.dp), 48.dp))
            .then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier)
            .padding(horizontal = 16.dp, vertical = spaced(8.dp)),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            JfText(title, style = JfType.listTitle.copy(fontSize = JfType.body.fontSize))
            if (description != null) JfText(description, style = JfType.small, color = colors.ink70)
        }
        control?.invoke()
    }
}

/**
 * One choice of a list that picks one (a sort menu, the equalizer's presets, the output device):
 * its label, and a check mark when chosen. TalkBack reads it as a radio button. [inset] is 20 dp
 * in sheets and 16 dp in a [Panel].
 */
@Composable
fun ChoiceRow(label: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, inset: Dp = 20.dp) {
    val colors = Jf.colors
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .selectable(selected, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = inset),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        JfText(
            label,
            style = JfType.listTitle.copy(fontSize = JfType.body.fontSize),
            color = if (selected) colors.accentText else colors.ink90,
            maxLines = 1,
            modifier = Modifier.weight(1f),
        )
        if (selected) JfIcon(R.drawable.ic_check, colors.accentText, size = 20.dp)
    }
}

/** A hairline between rows of a panel. */
@Composable
fun Divider(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().padding(start = 16.dp).height(1.dp).background(Jf.colors.lineSoft))
}

/** Lets a full-width header inside padded content reach the edges again ([horizontal] each side). */
fun Modifier.bleed(horizontal: Dp): Modifier = layout { measurable, constraints ->
    val extra = (horizontal * 2).roundToPx()
    val placeable = measurable.measure(
        constraints.copy(
            minWidth = (constraints.minWidth + extra).coerceAtLeast(0),
            maxWidth = if (constraints.hasBoundedWidth) constraints.maxWidth + extra else constraints.maxWidth,
        ),
    )
    val width = (placeable.width - extra).coerceAtLeast(0)
    layout(width, placeable.height) { placeable.place(-horizontal.roundToPx(), 0) }
}

package org.jellyfesh.android.ui.player

import android.text.format.DateUtils
import android.text.format.Formatter
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.launch
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.ui.components.ArtworkImage
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LoadingRow
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.signals.describeFailure
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.AudioFormat
import org.jellyfesh.logic.core.CommentEntry
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.DetailsPage
import org.jellyfesh.logic.core.FactKey
import org.jellyfesh.logic.core.ItemRef
import org.jellyfesh.logic.core.QualityLabel
import org.jellyfesh.logic.format.TimeFormat
import org.jellyfesh.logic.library.DetailsModel
import org.jellyfesh.logic.library.DetailsState
import org.jellyfesh.logic.sources.CommentsModel
import org.jellyfesh.logic.sources.CommentsState
import org.jellyfesh.logic.sources.Links

/** What the asker already knows of an item, shown until (and where) the core's details say nothing. */
data class KnownInfo(
    val title: String,
    val artist: String? = null,
    val album: String? = null,
    val durationMs: Long? = null,
    /** The source a source item comes from (a plugin id), for the Source row. */
    val source: String? = null,
)

/** Asks the core for [item]'s details and follows the answer. */
@Composable
fun rememberDetails(item: ItemRef?): DetailsState {
    val graph = LocalGraph.current
    val context = LocalContext.current
    var details by remember(item) { mutableStateOf(DetailsState()) }
    LaunchedEffect(item) {
        if (item == null) return@LaunchedEffect
        launch(start = CoroutineStart.UNDISPATCHED) {
            graph.store.signals.collect { event ->
                details = when (event) {
                    is CoreEvent.Details -> DetailsModel.onPage(details, event.page)
                    is CoreEvent.CommandFailed -> DetailsModel.onFailed(details, event.id, describeFailure(context, event.error))
                    else -> details
                }
            }
        }
        details = DetailsModel.load(item, graph::send)
    }
    return details
}

/**
 * About This Song (docs/ui.md, "Details"): Title, Artist, Album, Version, Year, Genre, Composer,
 * Lyricist, Label, Length and the other facts the core has, then Audio: Quality (the core's
 * label), Format, Bitrate, Size, ReplayGain, and File or Source. [decoded] is what plays as decoded
 * (Now Playing's Info pane), which goes before the file's own format. Rows without a value are
 * left out.
 */
@Composable
fun ColumnScope.DetailsRows(details: DetailsState, known: KnownInfo, decoded: AudioFormat? = null, extra: @Composable ColumnScope.() -> Unit = {}) {
    val page = details.page
    val context = LocalContext.current
    InfoRow(stringResource(R.string.info_title), page?.title?.takeIf { it.isNotBlank() } ?: known.title)
    (page?.subtitle?.takeIf { it.isNotBlank() } ?: known.artist)?.let { InfoRow(stringResource(R.string.info_artist), it) }
    known.album?.let { InfoRow(stringResource(R.string.info_album), it) }
    for (key in DetailsModel.ABOUT_KEYS) {
        DetailsModel.fact(page, key)?.let { InfoRow(factLabel(key), it) }
    }
    (page?.technical?.durationMs ?: known.durationMs)?.takeIf { it > 0 }?.let { InfoRow(stringResource(R.string.info_length), TimeFormat.clock(it)) }
    for (fact in DetailsModel.moreFacts(page)) InfoRow(factLabel(fact.key), fact.value)
    extra()
    val technical = page?.technical
    val format = decoded ?: technical?.format
    val quality = decoded?.quality ?: technical?.format?.quality
    val size = technical?.fileSizeBytes?.takeIf { it > 0 }
    val (trackGain, albumGain) = DetailsModel.gains(technical?.replaygain)
    val file = technical?.path ?: (page?.item as? ItemRef.File)?.path
    val source = known.source?.let { Links.serviceName(LocalGraph.current.store.state.value.sources, it) }
    if (quality != null || format != null || size != null || trackGain != null || albumGain != null || file != null || source != null) {
        JfText(stringResource(R.string.info_audio), style = JfType.section, modifier = Modifier.padding(top = 8.dp).semantics { heading() })
        quality?.let { InfoRow(stringResource(R.string.info_quality), stringResource(qualityLabel(it))) }
        format?.let { formatLine(it) }?.takeIf { it.isNotEmpty() }?.let { InfoRow(stringResource(R.string.info_format), it) }
        format?.bitrateKbps?.takeIf { it > 0 }?.let { InfoRow(stringResource(R.string.info_bitrate), stringResource(R.string.info_kbps, it)) }
        size?.let { InfoRow(stringResource(R.string.info_size), Formatter.formatFileSize(context, it)) }
        listOfNotNull(
            trackGain?.let { stringResource(R.string.info_gain_track, it) },
            albumGain?.let { stringResource(R.string.info_gain_album, it) },
        ).takeIf { it.isNotEmpty() }?.let { InfoRow(stringResource(R.string.info_replaygain), it.joinToString(" · ")) }
        when {
            file != null -> InfoRow(stringResource(R.string.info_file), file)
            source != null -> InfoRow(stringResource(R.string.info_source), source)
        }
    }
    page?.attribution?.takeIf { it.isNotBlank() }?.let { JfText(stringResource(R.string.source_attribution, it), style = JfType.caption, color = Jf.colors.ink50) }
    details.error?.let { Notice(it, warning = true) }
}

/** "FLAC · 24-bit · 96 kHz · Stereo". */
@Composable
private fun formatLine(format: AudioFormat): String {
    val parts = DetailsModel.formatParts(format)
    return listOfNotNull(
        parts.codec,
        parts.bitDepth?.let { stringResource(R.string.info_bits, it) },
        parts.sampleRate,
        parts.channels?.let {
            when (it) {
                1 -> stringResource(R.string.info_mono)
                2 -> stringResource(R.string.info_stereo)
                else -> stringResource(R.string.info_channels, it)
            }
        },
    ).joinToString(" · ")
}

/** The core's quality label, worded. */
fun qualityLabel(label: QualityLabel): Int = when (label) {
    QualityLabel.HiResLossless -> R.string.quality_hires
    QualityLabel.Lossless -> R.string.quality_lossless
    QualityLabel.High -> R.string.quality_high
    QualityLabel.Standard -> R.string.quality_standard
}

@Composable
private fun factLabel(key: FactKey): String = when (key) {
    FactKey.Version -> stringResource(R.string.info_version)
    FactKey.Genre -> stringResource(R.string.info_genre)
    FactKey.Language -> stringResource(R.string.info_language)
    FactKey.Released -> stringResource(R.string.info_year)
    FactKey.Label -> stringResource(R.string.info_label)
    FactKey.Composer -> stringResource(R.string.info_composer)
    FactKey.Lyricist -> stringResource(R.string.info_lyricist)
    FactKey.Arranger -> stringResource(R.string.info_arranger)
    FactKey.Producer -> stringResource(R.string.info_producer)
    FactKey.Popularity -> stringResource(R.string.info_popularity)
    FactKey.Quality -> stringResource(R.string.info_quality)
    FactKey.Isrc -> stringResource(R.string.info_isrc)
    FactKey.Disc -> stringResource(R.string.info_disc)
    FactKey.Track -> stringResource(R.string.info_track)
    is FactKey.Other -> key.label
}

@Composable
fun InfoRow(label: String, value: String) {
    Column(Modifier.semantics(mergeDescendants = true) {}, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        JfText(label, style = JfType.caption, color = Jf.colors.ink50)
        JfText(value, style = JfType.small, color = Jf.colors.ink90)
    }
}

/**
 * Get Info (the song menu, album and artist More menus): the item's details in a sheet. Albums
 * and artists show their notes or biography first.
 */
@Stable
class InfoSheets internal constructor(private val overlays: Overlays, private val context: android.content.Context) {
    fun show(item: ItemRef, known: KnownInfo) {
        val song = item is ItemRef.Track || item is ItemRef.File || (item as? ItemRef.Source)?.kind == org.jellyfesh.logic.core.ItemKind.Track
        overlays.showSheet(if (song) context.getString(R.string.info_heading) else known.title) {
            val details = rememberDetails(item)
            Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (details.loading && details.page == null) LoadingRow()
                details.page?.description?.takeIf { it.isNotBlank() }?.let { Blurb(it) }
                DetailsRows(details, known)
            }
        }
    }
}

@Composable
fun rememberInfoSheets(): InfoSheets {
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    return remember(overlays, context) { InfoSheets(overlays, context) }
}

/** Notes or a biography: a few lines, and More to read the rest. */
@Composable
fun Blurb(text: String, modifier: Modifier = Modifier) {
    var open by remember(text) { mutableStateOf(false) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        JfText(text, style = JfType.small, color = Jf.colors.ink70, maxLines = if (open) Int.MAX_VALUE else 4)
        if (!open && text.length > 160) JfButton(stringResource(R.string.action_more), { open = true }, style = ButtonStyle.Quiet)
    }
}

/**
 * Now Playing's Comments pane (sources that show comments): "Comments from {Service}", Popular
 * (the hot ones) then Latest, each with its author, picture, time, likes and text; more load at
 * the end. Read-only. Empty: "No Comments Yet".
 */
@Composable
fun CommentsView(item: ItemRef?, service: String?, modifier: Modifier = Modifier) {
    val graph = LocalGraph.current
    val context = LocalContext.current
    var comments by remember(item) { mutableStateOf(CommentsState()) }
    LaunchedEffect(item) {
        if (item == null) return@LaunchedEffect
        launch(start = CoroutineStart.UNDISPATCHED) {
            graph.store.signals.collect { event ->
                comments = when (event) {
                    is CoreEvent.Comments -> CommentsModel.onPage(comments, event.page)
                    is CoreEvent.CommandFailed -> CommentsModel.onFailed(comments, event.id, describeFailure(context, event.error))
                    else -> comments
                }
            }
        }
        comments = CommentsModel.load(item, graph::send)
    }
    LazyColumn(modifier.fillMaxSize().padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        item(key = "from") {
            val from = comments.attribution ?: service
            JfText(
                if (from != null) stringResource(R.string.comments_from, from) else stringResource(R.string.np_comments),
                style = JfType.section,
                modifier = Modifier.padding(top = 12.dp).semantics { heading() },
            )
        }
        if (comments.hot.isNotEmpty()) {
            item(key = "hot-h") { JfText(stringResource(R.string.comments_popular), style = JfType.label, color = Jf.colors.ink70) }
            items(comments.hot, key = { "hot:${it.id}" }) { CommentRow(it) }
        }
        if (comments.comments.isNotEmpty()) {
            item(key = "latest-h") { JfText(stringResource(R.string.comments_latest), style = JfType.label, color = Jf.colors.ink70) }
            items(comments.comments, key = { "c:${it.id}" }) { CommentRow(it) }
        }
        val error = comments.error
        when {
            error != null -> item(key = "state") { Notice(error, warning = true) }
            CommentsModel.empty(comments) -> item(key = "state") { EmptyState(R.drawable.ic_comment, stringResource(R.string.comments_none), null) }
            !comments.loaded || comments.request != 0L -> item(key = "state") { LoadingRow() }
            comments.next != null -> item(key = "state") {
                // The end of the list asks for the next page.
                LaunchedEffect(comments.next) { comments = CommentsModel.more(comments, graph::send) }
                LoadingRow()
            }
        }
        item(key = "end") { androidx.compose.foundation.layout.Spacer(Modifier.padding(bottom = 16.dp)) }
    }
}

@Composable
private fun CommentRow(comment: CommentEntry) {
    val colors = Jf.colors
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.Top) {
        ArtworkImage(ArtKey.of(comment.avatar), comment.author, 36.dp, CircleShape)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                JfText(comment.author, style = JfType.label, maxLines = 1, modifier = Modifier.weight(1f, fill = false))
                comment.timeMs?.let { JfText(DateUtils.getRelativeTimeSpanString(it).toString(), style = JfType.caption, color = colors.ink50, maxLines = 1) }
            }
            JfText(comment.text, style = JfType.small, color = colors.ink90)
            comment.likes?.takeIf { it > 0 }?.let { likes ->
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
                    JfIcon(R.drawable.ic_heart, colors.ink50, size = 14.dp)
                    JfText(likes.toString(), style = JfType.caption, color = colors.ink50)
                }
            }
        }
    }
}

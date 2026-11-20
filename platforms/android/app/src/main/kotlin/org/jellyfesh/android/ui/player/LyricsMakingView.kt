package org.jellyfesh.android.ui.player

import android.text.format.Formatter
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.AppGraph
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.settings.ProSheets
import org.jellyfesh.android.ui.settings.rememberProSheets
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.LevelGlyph
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.LyricsJobStage
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.SongRef
import org.jellyfesh.logic.lyrics.LyricsMaking
import org.jellyfesh.logic.lyrics.LyricsMakingModel
import org.jellyfesh.logic.lyrics.MakeStep
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.settings.Extensions

/**
 * Making lyrics with AI Lyrics (ADR-0019): Create Synced Lyrics, Try Again and Make Again open the
 * Pro sheet while Pro is inactive and ask "Send This Song to {origin}?" before the first online
 * job; otherwise they send `MakeLyrics` and the core answers. Delete Made Lyrics asks first.
 */
@Stable
class LyricsMaker internal constructor(
    private val graph: AppGraph,
    private val overlays: Overlays,
    private val context: android.content.Context,
    private val pro: ProSheets,
) {
    fun make(song: SongRef) {
        when (val step = LyricsMakingModel.make(graph.store.state.value.aiLyrics, song)) {
            is MakeStep.Send -> graph.send(step.command)
            MakeStep.ProSheet -> pro.open()
            is MakeStep.AskConsent -> overlays.confirm(
                context.getString(R.string.ai_consent_title, step.origin),
                context.getString(R.string.ai_consent_body, step.origin),
                context.getString(R.string.action_allow),
            ) { step.allow.forEach(graph::send) }
        }
    }

    fun deleteMade(song: SongRef) {
        overlays.confirm(
            context.getString(R.string.ai_delete_made_title),
            context.getString(R.string.ai_delete_made_body),
            context.getString(R.string.action_delete),
            destructive = true,
        ) { graph.send(Command.DeleteMadeLyrics(song)) }
    }
}

@Composable
fun rememberLyricsMaker(): LyricsMaker {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val pro = rememberProSheets()
    return remember(graph, overlays, context, pro) { LyricsMaker(graph, overlays, context, pro) }
}

/**
 * What the Lyrics pane offers for the song playing: Create Synced Lyrics (Time These Lyrics over
 * plain lyrics), the job's stage with its progress and Cancel, "Couldn't make lyrics: …" with Try
 * Again, the model to download, or the API key to add.
 */
@Composable
fun LyricsMakingCard(step: LyricsMaking, lyrics: LyricsSnapshot?, modifier: Modifier = Modifier) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val context = LocalContext.current
    val maker = rememberLyricsMaker()
    val colors = Jf.colors
    Column(
        modifier.widthIn(max = 420.dp).fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        when (step) {
            LyricsMaking.None -> Unit
            LyricsMaking.Create -> JfButton(
                stringResource(if (LyricsMakingModel.timesExisting(lyrics)) R.string.ai_time else R.string.ai_create),
                { maker.make(SongRef.Current) },
                style = ButtonStyle.Accent,
                icon = R.drawable.ic_sparkle,
            )
            is LyricsMaking.Running -> {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    LevelGlyph(running = true)
                    JfText(stageText(step.stage), style = JfType.body, color = colors.ink90)
                }
                step.progress?.let { ProgressTrack(it) }
                JfButton(stringResource(R.string.action_cancel), { graph.send(Command.CancelLyrics(step.job)) }, style = ButtonStyle.Quiet)
            }
            is LyricsMaking.Failed -> {
                Note(stringResource(R.string.ai_failed, step.message))
                JfButton(stringResource(R.string.action_retry), { maker.make(SongRef.Current) })
            }
            is LyricsMaking.DownloadModel -> {
                Note(stringResource(R.string.ai_need_model, step.name, Formatter.formatShortFileSize(context, step.bytes)))
                JfButton(stringResource(R.string.ai_model_download), { graph.send(Command.DownloadSpeechModel(step.id)) }, style = ButtonStyle.Accent)
            }
            is LyricsMaking.DownloadingModel -> {
                Note(stringResource(R.string.ai_downloading_model, step.name, (step.fraction * 100).toInt()))
                ProgressTrack(step.fraction)
                JfButton(stringResource(R.string.action_cancel), { graph.send(Command.CancelSpeechModelDownload(step.id)) }, style = ButtonStyle.Quiet)
            }
            LyricsMaking.SignIn -> {
                Note(stringResource(R.string.ai_need_key))
                JfButton(stringResource(R.string.ai_open_settings), {
                    val state = graph.store.state.value
                    Extensions.aiLyrics(state.sources.plugins, state.aiLyrics)?.let { nav.push(Screen.Extension(it.id)) }
                })
            }
            is LyricsMaking.Unavailable -> Note(step.reason)
        }
    }
}

@Composable
private fun Note(text: String) {
    JfText(text, style = JfType.small, color = Jf.colors.ink70, textAlign = TextAlign.Center)
}

/** A job's stage while it runs: "Reading Audio…", "Sending to {origin}…", "Recognising… 42%", "Timing Words…". */
@Composable
private fun stageText(stage: LyricsJobStage): String = when (stage) {
    LyricsJobStage.Waiting -> stringResource(R.string.ai_stage_waiting)
    LyricsJobStage.ReadingAudio -> stringResource(R.string.ai_stage_reading)
    is LyricsJobStage.Sending -> stringResource(R.string.ai_stage_sending, stage.origin)
    is LyricsJobStage.Recognising -> stringResource(R.string.ai_stage_recognising, (stage.fraction.coerceIn(0f, 1f) * 100).toInt())
    LyricsJobStage.TimingWords -> stringResource(R.string.ai_stage_timing)
    LyricsJobStage.Done, is LyricsJobStage.Failed, LyricsJobStage.Cancelled -> ""
}

/** A thin bar for a fraction from 0 to 1. */
@Composable
private fun ProgressTrack(fraction: Float) {
    val colors = Jf.colors
    val value = fraction.coerceIn(0f, 1f)
    Box(
        Modifier
            .fillMaxWidth()
            .height(4.dp)
            .clip(RoundedCornerShape(2.dp))
            .background(colors.lineStrong)
            .semantics { progressBarRangeInfo = ProgressBarRangeInfo(value, 0f..1f) },
    ) {
        Box(Modifier.fillMaxWidth(value).fillMaxHeight().background(colors.accent))
    }
}

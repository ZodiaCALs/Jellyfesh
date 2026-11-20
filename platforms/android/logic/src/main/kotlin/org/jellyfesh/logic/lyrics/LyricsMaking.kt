package org.jellyfesh.logic.lyrics

import org.jellyfesh.logic.core.AiLyricsNeed
import org.jellyfesh.logic.core.AiLyricsSnapshot
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.LyricsJob
import org.jellyfesh.logic.core.LyricsJobStage
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.LyricsSource
import org.jellyfesh.logic.core.SongRef
import org.jellyfesh.logic.core.SpeechModelStatus
import org.jellyfesh.logic.core.Timing
import org.jellyfesh.logic.core.TrackRef

/**
 * What the Lyrics pane offers about making lyrics for the song playing (docs/ui.md, "AI Lyrics,
 * Customize and Pro"; ADR-0019). The core decides what is needed first (`need`), runs the jobs and
 * keeps the made lyrics; this reads the `AiLyrics` slice for the pane.
 */
sealed interface LyricsMaking {
    /** Nothing: AI Lyrics is off or not installed, or nothing is left to do. */
    data object None : LyricsMaking

    /** Create Synced Lyrics (Time These Lyrics over plain lyrics); Pro or the question about sending audio come when it is chosen. */
    data object Create : LyricsMaking

    /** The song's job: its stage, its progress when it can be measured (0–1), and Cancel. */
    data class Running(val job: Long, val stage: LyricsJobStage, val progress: Float?) : LyricsMaking

    /** "Couldn't make lyrics: {message}" and Try Again. */
    data class Failed(val message: String) : LyricsMaking

    /** "Download the {Model} model ({size}) to make lyrics on this device." and Download. */
    data class DownloadModel(val id: String, val name: String, val bytes: Long) : LyricsMaking

    /** "Downloading {Model}… 35%" and Cancel. */
    data class DownloadingModel(val id: String, val name: String, val fraction: Float) : LyricsMaking

    /** "Add your API key to make lyrics online." and Open AI Lyrics Settings. */
    data object SignIn : LyricsMaking

    /** Why lyrics cannot be made, in the core's words. */
    data class Unavailable(val reason: String) : LyricsMaking
}

/** What choosing Create Synced Lyrics (or Try Again, Make Again) does now. */
sealed interface MakeStep {
    data class Send(val command: Command) : MakeStep

    /** Jellyfesh Pro is inactive: the Pro sheet. */
    data object ProSheet : MakeStep

    /** "Send This Song to {origin}?" first; Allow sends [allow]. */
    data class AskConsent(val origin: String, val allow: List<Command>) : MakeStep
}

object LyricsMakingModel {
    /** A job still waiting or running (Cancel stops it). */
    fun isActive(stage: LyricsJobStage): Boolean = when (stage) {
        LyricsJobStage.Done, is LyricsJobStage.Failed, LyricsJobStage.Cancelled -> false
        else -> true
    }

    /** The newest job for the queue entry [uid] (the song playing). */
    fun jobFor(ai: AiLyricsSnapshot, uid: Long?): LyricsJob? = uid?.let { u -> ai.jobs.lastOrNull { it.uid == u } }

    /** The newest job for a library song. */
    fun jobForSong(ai: AiLyricsSnapshot, libraryId: Long): LyricsJob? = ai.jobs.lastOrNull { it.libraryId == libraryId }

    /** The pane's offer: the song's job first (running, or failed), then, while AI Lyrics is available, what making lyrics needs. */
    fun step(job: LyricsJob?, ai: AiLyricsSnapshot): LyricsMaking {
        if (job != null) {
            val stage = job.stage
            if (isActive(stage)) return LyricsMaking.Running(job.id, stage, progress(stage))
            if (stage is LyricsJobStage.Failed) return LyricsMaking.Failed(stage.message)
        }
        if (!ai.available) return LyricsMaking.None
        return when (val need = ai.need) {
            AiLyricsNeed.Nothing, AiLyricsNeed.Pro, is AiLyricsNeed.Consent -> LyricsMaking.Create
            AiLyricsNeed.SignIn -> LyricsMaking.SignIn
            is AiLyricsNeed.Model -> {
                val downloading = ai.models.firstOrNull { it.id == need.id }?.state as? SpeechModelStatus.Downloading
                if (downloading != null) {
                    LyricsMaking.DownloadingModel(need.id, need.name, downloading.fraction.coerceIn(0f, 1f))
                } else {
                    LyricsMaking.DownloadModel(need.id, need.name, need.bytes)
                }
            }
            is AiLyricsNeed.Unavailable -> need.reason.trim().takeIf { it.isNotEmpty() }?.let { LyricsMaking.Unavailable(it) } ?: LyricsMaking.None
        }
    }

    /**
     * Whether the pane shows [step] for [lyrics]: always without lyrics, below plain (unsynced)
     * ones, and over synced ones only while their song's job runs or after it failed.
     */
    fun shows(step: LyricsMaking, lyrics: LyricsSnapshot?): Boolean {
        if (step == LyricsMaking.None) return false
        val doc = lyrics?.doc
        if (doc == null || doc.lines.isEmpty() || doc.timing == Timing.Unsynced) return true
        return step is LyricsMaking.Running || step is LyricsMaking.Failed
    }

    /** The song has plain lyrics: Create Synced Lyrics reads Time These Lyrics. */
    fun timesExisting(lyrics: LyricsSnapshot?): Boolean {
        val doc = lyrics?.doc ?: return false
        return doc.lines.isNotEmpty() && doc.timing == Timing.Unsynced
    }

    /** A running job's progress for a bar (Recognising only). */
    fun progress(stage: LyricsJobStage): Float? = (stage as? LyricsJobStage.Recognising)?.fraction?.takeIf { it.isFinite() }?.coerceIn(0f, 1f)

    /** What making lyrics for [song] does now: the job, the Pro sheet, or the question about sending audio. */
    fun make(ai: AiLyricsSnapshot, song: SongRef): MakeStep = when (val need = ai.need) {
        AiLyricsNeed.Pro -> MakeStep.ProSheet
        is AiLyricsNeed.Consent -> {
            val plugin = ai.plugin
            if (plugin == null) {
                MakeStep.Send(Command.MakeLyrics(song))
            } else {
                MakeStep.AskConsent(need.origin, listOf(Command.AllowAudioUpload(plugin, need.origin), Command.MakeLyrics(song)))
            }
        }
        // Otherwise the core makes them, or answers with what is missing.
        else -> MakeStep.Send(Command.MakeLyrics(song))
    }

    /** The song menu's Create Synced Lyrics: library songs, while AI Lyrics is available. */
    fun offersFor(ai: AiLyricsSnapshot, track: TrackRef): Boolean = ai.available && track is TrackRef.Library

    /** The engine of lyrics AI Lyrics made ("Made by AI Lyrics · {engine}"), else null. */
    fun madeWith(lyrics: LyricsSnapshot?): String? = (lyrics?.source as? LyricsSource.Made)?.engine
}

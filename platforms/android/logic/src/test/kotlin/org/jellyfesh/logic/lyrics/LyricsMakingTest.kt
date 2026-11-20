package org.jellyfesh.logic.lyrics

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.AiLyricsNeed
import org.jellyfesh.logic.core.AiLyricsSnapshot
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.LyricDoc
import org.jellyfesh.logic.core.LyricFormat
import org.jellyfesh.logic.core.LyricLine
import org.jellyfesh.logic.core.LyricsJob
import org.jellyfesh.logic.core.LyricsJobStage
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.LyricsSource
import org.jellyfesh.logic.core.SongRef
import org.jellyfesh.logic.core.SpeechModel
import org.jellyfesh.logic.core.SpeechModelStatus
import org.jellyfesh.logic.core.TextDir
import org.jellyfesh.logic.core.Timing
import org.jellyfesh.logic.core.TrackRef

class LyricsMakingTest {
    private val ai = AiLyricsSnapshot.Empty.copy(version = 2, available = true, plugin = "org.jellyfesh.ai-lyrics")

    private fun job(stage: LyricsJobStage, uid: Long? = 7, id: Long = 1) = LyricsJob(id, "Song", "Artist", uid, null, stage)

    private fun lyrics(timing: Timing?, source: LyricsSource? = null) = LyricsSnapshot(
        uid = 7,
        doc = timing?.let {
            LyricDoc(
                null, it, 0, emptyList(),
                listOf(LyricLine(0, if (it == Timing.Unsynced) null else 0L, null, null, "la", emptyList(), emptyList(), TextDir.Auto)),
                emptyList(), LyricFormat.Lrc, null,
            )
        },
        timeline = null,
        source = source,
    )

    @Test
    fun theSongsJobComesFirst() {
        val running = LyricsMakingModel.step(job(LyricsJobStage.Recognising(0.42f)), ai)
        assertEquals(LyricsMaking.Running(1, LyricsJobStage.Recognising(0.42f), 0.42f), running)
        assertEquals(LyricsMaking.Running(1, LyricsJobStage.ReadingAudio, null), LyricsMakingModel.step(job(LyricsJobStage.ReadingAudio), ai))
        assertEquals(LyricsMaking.Failed("no speech"), LyricsMakingModel.step(job(LyricsJobStage.Failed("no speech")), ai))
        // A finished or cancelled job leaves the offer.
        assertEquals(LyricsMaking.Create, LyricsMakingModel.step(job(LyricsJobStage.Done), ai))
        assertEquals(LyricsMaking.Create, LyricsMakingModel.step(job(LyricsJobStage.Cancelled), ai))
        // A job keeps showing while AI Lyrics is turned off.
        assertEquals(LyricsMaking.Failed("x"), LyricsMakingModel.step(job(LyricsJobStage.Failed("x")), ai.copy(available = false)))
    }

    @Test
    fun theNeedSaysWhatComesFirst() {
        assertEquals(LyricsMaking.None, LyricsMakingModel.step(null, ai.copy(available = false)))
        // Pro and the question about sending audio come when Create is chosen.
        assertEquals(LyricsMaking.Create, LyricsMakingModel.step(null, ai.copy(need = AiLyricsNeed.Pro)))
        assertEquals(LyricsMaking.Create, LyricsMakingModel.step(null, ai.copy(need = AiLyricsNeed.Consent("https://api.openai.com"))))
        assertEquals(LyricsMaking.SignIn, LyricsMakingModel.step(null, ai.copy(need = AiLyricsNeed.SignIn)))
        val model = AiLyricsNeed.Model("base", "Base", 147_951_465)
        assertEquals(LyricsMaking.DownloadModel("base", "Base", 147_951_465), LyricsMakingModel.step(null, ai.copy(need = model)))
        val downloading = ai.copy(need = model, models = listOf(SpeechModel("base", "Base", 147_951_465, SpeechModelStatus.Downloading(0.35f))))
        assertEquals(LyricsMaking.DownloadingModel("base", "Base", 0.35f), LyricsMakingModel.step(null, downloading))
        assertEquals(LyricsMaking.Unavailable("No speech engine."), LyricsMakingModel.step(null, ai.copy(need = AiLyricsNeed.Unavailable(" No speech engine. "))))
        assertEquals(LyricsMaking.None, LyricsMakingModel.step(null, ai.copy(need = AiLyricsNeed.Unavailable(""))))
    }

    @Test
    fun thePaneShowsTheOfferWithoutSyncedLyricsAndJobsOverThem() {
        assertTrue(LyricsMakingModel.shows(LyricsMaking.Create, null))
        assertTrue(LyricsMakingModel.shows(LyricsMaking.Create, lyrics(null)))
        assertTrue(LyricsMakingModel.shows(LyricsMaking.Create, lyrics(Timing.Unsynced)))
        assertTrue(LyricsMakingModel.timesExisting(lyrics(Timing.Unsynced)))
        assertFalse(LyricsMakingModel.timesExisting(lyrics(Timing.Line)))
        assertFalse(LyricsMakingModel.shows(LyricsMaking.Create, lyrics(Timing.Word)))
        assertTrue(LyricsMakingModel.shows(LyricsMaking.Failed("x"), lyrics(Timing.Word)))
        assertTrue(LyricsMakingModel.shows(LyricsMaking.Running(1, LyricsJobStage.TimingWords, null), lyrics(Timing.Line)))
        assertFalse(LyricsMakingModel.shows(LyricsMaking.None, null))
    }

    @Test
    fun creatingSendsTheJobOrAsksFirst() {
        assertEquals(MakeStep.Send(Command.MakeLyrics(SongRef.Current)), LyricsMakingModel.make(ai, SongRef.Current))
        assertEquals(MakeStep.ProSheet, LyricsMakingModel.make(ai.copy(need = AiLyricsNeed.Pro), SongRef.Current))
        val ask = LyricsMakingModel.make(ai.copy(need = AiLyricsNeed.Consent("https://api.groq.com")), SongRef.Library(5))
        assertEquals(
            MakeStep.AskConsent(
                "https://api.groq.com",
                listOf(Command.AllowAudioUpload("org.jellyfesh.ai-lyrics", "https://api.groq.com"), Command.MakeLyrics(SongRef.Library(5))),
            ),
            ask,
        )
        // A model or a key missing: the core answers the job with what is missing.
        assertEquals(MakeStep.Send(Command.MakeLyrics(SongRef.Current)), LyricsMakingModel.make(ai.copy(need = AiLyricsNeed.SignIn), SongRef.Current))
    }

    @Test
    fun jobsAreTheSongsAndMadeLyricsSayHow() {
        val jobs = ai.copy(jobs = listOf(job(LyricsJobStage.Done, uid = 7, id = 1), job(LyricsJobStage.Waiting, uid = 7, id = 2), job(LyricsJobStage.Waiting, uid = 8, id = 3)))
        assertEquals(2L, LyricsMakingModel.jobFor(jobs, 7)?.id)
        assertNull(LyricsMakingModel.jobFor(jobs, null))
        assertEquals(4L, LyricsMakingModel.jobForSong(ai.copy(jobs = listOf(LyricsJob(4, "S", null, null, 12, LyricsJobStage.Waiting))), 12)?.id)
        assertEquals("Whisper Base", LyricsMakingModel.madeWith(lyrics(Timing.Word, LyricsSource.Made("org.jellyfesh.ai-lyrics", "Whisper Base"))))
        assertNull(LyricsMakingModel.madeWith(lyrics(Timing.Word, LyricsSource.Embedded)))
        assertTrue(LyricsMakingModel.offersFor(ai, TrackRef.Library(3)))
        assertFalse(LyricsMakingModel.offersFor(ai.copy(available = false), TrackRef.Library(3)))
    }
}

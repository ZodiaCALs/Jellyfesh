package org.jellyfesh.logic.settings

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.DownloadItem
import org.jellyfesh.logic.core.DownloadState
import org.jellyfesh.logic.core.DownloadsSnapshot
import org.jellyfesh.logic.core.SourceTrack

/** Settings › Downloads (ADR-0025 §2). */
class DownloadSettingsModelTest {
    @Test
    fun aFieldButtonAddsItsFieldAfterASpaceUnlessANamePartStartsAnyway() {
        assertEquals("{title}", DownloadSettingsModel.appendField("", "{title}"))
        assertEquals("{album_artist}/{album}/{track} {title}", DownloadSettingsModel.appendField("{album_artist}/{album}/{track}", "{title}"))
        assertEquals("{album}/{title}", DownloadSettingsModel.appendField("{album}/", "{title}"))
        assertEquals("{artist} - {title}", DownloadSettingsModel.appendField("{artist} - ", "{title}"))
        assertEquals("{track}.{title}", DownloadSettingsModel.appendField("{track}.", "{title}"))
        assertEquals(8, DownloadSettingsModel.FIELDS.size)
    }

    @Test
    fun downloadsAtOnceStaysWithinOneToFour() {
        assertEquals(listOf(1, 2, 3, 4), DownloadSettingsModel.AT_ONCE)
        assertEquals(1, DownloadSettingsModel.atOnce(0))
        assertEquals(4, DownloadSettingsModel.atOnce(9))
        assertEquals(3, DownloadSettingsModel.atOnce(3))
    }

    @Test
    fun removeAllNeedsSomethingToRemove() {
        assertFalse(DownloadSettingsModel.canRemoveAll(DownloadsSnapshot.Empty))
        val queued = DownloadItem(1, SourceTrack("org.jellyfesh.jellyfin", "t-1", "Morning Harbour"), DownloadState.Queued, 0, null, null, null, false)
        assertTrue(DownloadSettingsModel.canRemoveAll(DownloadsSnapshot.Empty.copy(items = listOf(queued))))
        assertTrue(DownloadSettingsModel.canRemoveAll(DownloadsSnapshot.Empty.copy(bytesUsed = 1_024)))
    }
}

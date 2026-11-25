package org.jellyfesh.logic.library

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Android 8 and 9's storage permission for downloads (ADR-0025 §2). */
class StorageAccessTest {
    @Test
    fun onlyAndroid8And9NeedItUntilItIsGranted() {
        assertTrue(StorageAccess.needsAsking(26, granted = false))
        assertTrue(StorageAccess.needsAsking(27, granted = false))
        assertTrue(StorageAccess.needsAsking(28, granted = false))
        assertFalse(StorageAccess.needsAsking(28, granted = true))
        // Android 10 has the legacy storage flag; Android 11 and later let the core write by path.
        assertFalse(StorageAccess.needsAsking(29, granted = false))
        assertFalse(StorageAccess.needsAsking(30, granted = false))
        assertFalse(StorageAccess.needsAsking(37, granted = false))
        assertEquals("android.permission.WRITE_EXTERNAL_STORAGE", StorageAccess.PERMISSION)
    }

    @Test
    fun itIsAskedOnceBeforeTheFirstDownloadThenTheDownloadFails() {
        assertEquals(StorageStep.Download, StorageAccess.beforeDownload(30, granted = false, asked = false))
        assertEquals(StorageStep.Download, StorageAccess.beforeDownload(28, granted = true, asked = true))
        assertEquals(StorageStep.Ask, StorageAccess.beforeDownload(28, granted = false, asked = false))
        assertEquals(StorageStep.Refused, StorageAccess.beforeDownload(28, granted = false, asked = true))
        assertEquals(StorageStep.Download, StorageAccess.afterAnswer(granted = true))
        assertEquals(StorageStep.Refused, StorageAccess.afterAnswer(granted = false))
    }

    @Test
    fun theWayToAllowItLaterIsAndroidsDialogWhileItShowsItElseTheSettingsPage() {
        assertEquals(StorageGrant.Ask, StorageAccess.grant(rationale = true))
        assertEquals(StorageGrant.OpenSettings, StorageAccess.grant(rationale = false))
        // The pages say it is off only once Jellyfesh asked and Android 8 or 9 refused.
        assertTrue(StorageAccess.showsOff(27, granted = false, asked = true))
        assertFalse(StorageAccess.showsOff(27, granted = false, asked = false))
        assertFalse(StorageAccess.showsOff(27, granted = true, asked = true))
        assertFalse(StorageAccess.showsOff(31, granted = false, asked = true))
    }
}

package org.jellyfesh.logic.library

/** What choosing Download does on this device. */
enum class StorageStep {
    /** Send the download. */
    Download,

    /** Android's dialog first: once, before the first download. */
    Ask,

    /** The download fails: say why, with the way to allow it later. */
    Refused,
}

/** The way to allow it later: Android's dialog again while it shows it, else Jellyfesh's settings page. */
enum class StorageGrant { Ask, OpenSettings }

/**
 * Where downloads may write on each Android version (ADR-0025 §2), the core writing by path to
 * Music/Jellyfesh: from Android 11 the system lets it and indexes the files in MediaStore; Android
 * 10 has the legacy storage flag; Android 8 and 9 need `WRITE_EXTERNAL_STORAGE`, declared only up
 * to SDK 28 and asked once before the first download. Never the all-files permission.
 */
object StorageAccess {
    const val PERMISSION = "android.permission.WRITE_EXTERNAL_STORAGE"

    /** Android 9 (API 28): the last version that needs it. */
    const val LAST_SDK = 28

    /** Whether this Android version needs the permission to write downloads. */
    fun required(sdkInt: Int): Boolean = sdkInt <= LAST_SDK

    /** Whether a download needs the permission first: on Android 8 and 9, until it is granted. */
    fun needsAsking(sdkInt: Int, granted: Boolean): Boolean = required(sdkInt) && !granted

    /**
     * What choosing Download does: download where nothing is needed or it is granted; Android's
     * dialog once, before the first download ([asked]: Jellyfesh asked on this install); after that,
     * the download fails with the way to allow it.
     */
    fun beforeDownload(sdkInt: Int, granted: Boolean, asked: Boolean): StorageStep = when {
        !needsAsking(sdkInt, granted) -> StorageStep.Download
        !asked -> StorageStep.Ask
        else -> StorageStep.Refused
    }

    /** Android's dialog answered for a waiting download. */
    fun afterAnswer(granted: Boolean): StorageStep = if (granted) StorageStep.Download else StorageStep.Refused

    /** The way to allow it later ([rationale]: Android shows its dialog again, `shouldShowRequestPermissionRationale`). */
    fun grant(rationale: Boolean): StorageGrant = if (rationale) StorageGrant.Ask else StorageGrant.OpenSettings

    /** Whether Downloads and Settings › Downloads say storage access is off: needed, asked and refused. */
    fun showsOff(sdkInt: Int, granted: Boolean, asked: Boolean): Boolean = needsAsking(sdkInt, granted) && asked
}

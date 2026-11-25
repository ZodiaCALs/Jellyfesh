package org.jellyfesh.logic.settings

import org.jellyfesh.logic.core.DownloadsSnapshot

/**
 * Settings › Downloads (ADR-0025 §2; docs/ui.md, "Settings"): the File Names fields, Downloads at
 * Once kept within 1 to 4, and whether Remove All Downloads… has anything to remove. The folder,
 * the preview of a file's path and the space used are the core's.
 */
object DownloadSettingsModel {
    /** The File Names fields, in the order Settings offers them. */
    val FIELDS: List<String> = listOf("{artist}", "{album_artist}", "{album}", "{title}", "{track}", "{disc}", "{year}", "{source}")

    const val AT_ONCE_MIN = 1
    const val AT_ONCE_MAX = 4

    /** Downloads at Once's choices. */
    val AT_ONCE: List<Int> = (AT_ONCE_MIN..AT_ONCE_MAX).toList()

    fun atOnce(value: Int): Int = value.coerceIn(AT_ONCE_MIN, AT_ONCE_MAX)

    /** How long typing in File Names pauses before the template is saved. */
    const val TEMPLATE_SAVE_DELAY_MS: Long = 600

    /**
     * [field] added at the end of [template] (a field button): after a space, unless the template
     * is empty or ends where a name part starts anyway (a space, a slash, `-`, `_`, `.`, `(`, `[`).
     */
    fun appendField(template: String, field: String): String {
        val last = template.lastOrNull() ?: return field
        return if (last.isWhitespace() || last in "/-_.([") template + field else "$template $field"
    }

    /** Whether Remove All Downloads… has anything to remove. */
    fun canRemoveAll(downloads: DownloadsSnapshot): Boolean = downloads.items.isNotEmpty() || downloads.bytesUsed > 0
}

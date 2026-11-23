package org.jellyfesh.logic.playlist

import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.library.LibraryRoots
import org.jellyfesh.logic.queue.QueueModel

/** A playlist row being dragged by its handle. */
data class PlaylistDrag(
    /** The entry dragged. */
    val entryId: Long,
    /** Its row (its position in the playlist) when the drag began. */
    val from: Int,
    /** How far it has moved, in pixels (positive is down). */
    val offsetPx: Float,
)

/**
 * A drop the core has not confirmed yet. The rows show in the new order until the list is read
 * again after a library change later than [revision], so the row does not jump back meanwhile.
 */
data class PendingMove(val from: Int, val to: Int, val revision: Long)

/**
 * Reordering a playlist's songs by dragging a handle (docs/ui.md, "Playlists"). The core owns the
 * order: a drop becomes `MovePlaylistEntry` (positions count from 0 in the playlist's order) and
 * the list shows the move locally until the core's next answer.
 */
object PlaylistReorder {
    /** The row a drag would land on: whole rows moved, within the playlist's [count] rows. */
    fun target(drag: PlaylistDrag, rowPx: Float, count: Int): Int = QueueModel.target(drag.from, drag.offsetPx, rowPx, count)

    /** How far row [index] is drawn from its place while [drag] is held. */
    fun shift(index: Int, drag: PlaylistDrag?, rowPx: Float, count: Int): Float {
        if (drag == null) return 0f
        val to = target(drag, rowPx, count)
        return when {
            index == drag.from -> drag.offsetPx
            drag.from < to && index in (drag.from + 1)..to -> -rowPx
            drag.from > to && index in to until drag.from -> rowPx
            else -> 0f
        }
    }

    /**
     * Ends a drag made while the library was at [revision]: the move to show until the core
     * confirms it and the command that makes it, or null when the row stayed where it was.
     */
    fun drop(playlist: Long, drag: PlaylistDrag, rowPx: Float, count: Int, revision: Long): Pair<PendingMove, Command>? {
        val to = target(drag, rowPx, count)
        if (to == drag.from) return null
        return PendingMove(drag.from, to, revision) to Command.MovePlaylistEntry(playlist, drag.entryId, to)
    }

    /** TalkBack's Move up (-1) and Move down (+1): null at either end. */
    fun step(playlist: Long, entryId: Long, index: Int, by: Int, count: Int): Command? {
        val to = index + by
        if (by == 0 || index !in 0 until count || to !in 0 until count) return null
        return Command.MovePlaylistEntry(playlist, entryId, to)
    }

    /** The loaded row to draw at [index] while [pending] waits for the core. */
    fun rowAt(index: Int, pending: PendingMove?): Int {
        val p = pending ?: return index
        return when {
            index == p.to -> p.from
            p.from < p.to && index in p.from until p.to -> index + 1
            p.from > p.to && index in (p.to + 1)..p.from -> index - 1
            else -> index
        }
    }

    /** [pending] while the rows on screen were read at or before its revision; null once newer rows arrived. */
    fun stillPending(pending: PendingMove?, rowsRevision: Long): PendingMove? = pending?.takeIf { rowsRevision <= it.revision }
}

/**
 * Import Playlist… and Export Playlist… with the system's document picker (the Storage Access
 * Framework). The core reads and writes M3U8 by path (`ImportPlaylist`, `export_playlist`); these
 * are the pure parts of handing it a picked document.
 */
object PlaylistFiles {
    const val EXTENSION = "m3u8"

    /**
     * The type Export Playlist… creates its file as. Android's own type table maps `m3u8` to it
     * and back, so the picker keeps the `.m3u8` name instead of adding an extension.
     */
    const val EXPORT_TYPE = "audio/x-mpegurl"

    /** The types Import Playlist… offers: every name M3U and M3U8 files go by, and untyped files. */
    val IMPORT_TYPES: List<String> = listOf(
        "audio/x-mpegurl",
        "audio/mpegurl",
        "application/x-mpegurl",
        "application/vnd.apple.mpegurl",
        "application/octet-stream",
    )

    /** The name Export Playlist… suggests: the playlist's name, safe for any file system, with `.m3u8`. */
    fun exportName(playlist: String): String = "${safeName(playlist).ifEmpty { "Playlist" }}.$EXTENSION"

    /**
     * The name a picked file is copied under when the core cannot read it where it is: its own
     * name (the core names the playlist after the file), kept as M3U or M3U8.
     */
    fun importName(displayName: String?): String {
        val name = safeName(displayName.orEmpty()).ifEmpty { "Playlist.$EXTENSION" }
        val lower = name.lowercase()
        return if (lower.endsWith(".m3u8") || lower.endsWith(".m3u")) name else "$name.$EXTENSION"
    }

    /**
     * The path of a picked document on this phone's storage, from its document id: the external
     * storage provider's `primary:Music/Mix.m3u8` or `1A2B-3C4D:Lists/Mix.m3u`, or the downloads
     * provider's `raw:/storage/emulated/0/Download/Mix.m3u8`; null for other providers. The core
     * resolves a playlist's relative lines against its folder, so reading it in place is better
     * than a copy.
     */
    fun pathOfDocument(documentId: String, primaryRoot: String = "/storage/emulated/0"): String? {
        if (documentId.startsWith("raw:")) return documentId.removePrefix("raw:").takeIf { it.startsWith("/") && !it.endsWith("/") }
        // A volume's id alone names its top folder, never a file.
        if (documentId.substringAfter(':', "").trim('/').isEmpty()) return null
        return LibraryRoots.pathOfTreeDocument(documentId, primaryRoot)
    }

    /** Characters file systems and document providers refuse, and control characters, become `_`. */
    internal fun safeName(name: String): String =
        name.map { c -> if (c in FORBIDDEN || c.isISOControl()) '_' else c }.joinToString("").trim().trim('.').trim()

    private const val FORBIDDEN = "/\\:*?\"<>|"
}

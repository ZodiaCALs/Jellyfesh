package org.jellyfesh.logic.library

import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.playlist.PlaylistFiles

/**
 * Open Files… at the sidebar's foot (docs/ui.md, "Navigation"), as on Jellyfesh Web: the picked
 * files play at once, in the order picked, and never join the library. Each plays where it is
 * when its provider names a path the core can read ([PlaylistFiles.pathOfDocument]), else from a
 * copy in the cache.
 */
object OpenedFiles {
    /** What the system's picker offers. */
    val TYPES: Array<String> = arrayOf("audio/*")

    /**
     * The name the [index]th picked file is copied under: its own name made safe for any file
     * system, after its place in the order, so two files of the same name both play.
     */
    fun copyName(index: Int, displayName: String?): String {
        val name = PlaylistFiles.safeName(displayName.orEmpty()).ifEmpty { "Song" }
        return "${index + 1} $name"
    }

    /** Plays [paths] from the first; null when nothing could be read. */
    fun command(paths: List<String>): Command? =
        if (paths.isEmpty()) null else Command.PlayTracks(paths.map { TrackRef.File(it) }, 0)
}

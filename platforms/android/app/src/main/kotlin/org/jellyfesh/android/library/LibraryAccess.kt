package org.jellyfesh.android.library

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import android.provider.MediaStore
import android.provider.OpenableColumns
import androidx.core.content.ContextCompat
import java.io.File
import java.io.IOException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.jellyfesh.android.AppGraph
import org.jellyfesh.android.QueryResult
import org.jellyfesh.android.R
import org.jellyfesh.android.valueOrNull
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.library.LibraryRoots
import org.jellyfesh.logic.library.OpenedFiles
import org.jellyfesh.logic.playlist.PlaylistFiles

/**
 * Android library access, option C (docs/step-0 "Android library access", implemented here for
 * v0.0.3 while the choice is open): with the media permission, MediaStore lists the folders that
 * hold music at once and they become the core's library roots, so the library fills without the
 * person choosing anything; the core then reads every file's tags itself. Folders the system does
 * not index, or every folder when the permission is declined, are added with the Storage Access
 * Framework.
 *
 * Both paths hand the core filesystem paths (`AddRoot`): on Android 11+ the media permission lets
 * native code read shared storage through FUSE. A SAF folder on a provider without paths (a cloud
 * drive) is refused with a message until the core's FileSystem trait gets its SAF backend.
 */
class LibraryAccess(private val context: Context, private val graph: AppGraph) {
    /** The permission that gives MediaStore and file access to audio. */
    val mediaPermission: String =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) Manifest.permission.READ_MEDIA_AUDIO else Manifest.permission.READ_EXTERNAL_STORAGE

    fun hasMediaPermission(): Boolean =
        ContextCompat.checkSelfPermission(context, mediaPermission) == PackageManager.PERMISSION_GRANTED

    /** Called once the core runs: adds MediaStore's music folders when the permission is granted. */
    suspend fun onCoreReady() {
        if (hasMediaPermission()) addMediaStoreRoots()
    }

    /** The music folders MediaStore knows, added as roots the core does not have yet. */
    suspend fun addMediaStoreRoots(): Int = withContext(Dispatchers.IO) {
        if (!hasMediaPermission()) return@withContext 0
        val paths = ArrayList<String>()
        @Suppress("DEPRECATION") // DATA still holds the path the core needs; there is no replacement.
        val column = MediaStore.Audio.Media.DATA
        context.contentResolver.query(
            MediaStore.Audio.Media.EXTERNAL_CONTENT_URI,
            arrayOf(column),
            "${MediaStore.Audio.Media.IS_MUSIC} != 0",
            null,
            null,
        )?.use { cursor ->
            val index = cursor.getColumnIndexOrThrow(column)
            while (cursor.moveToNext()) cursor.getString(index)?.let(paths::add)
        }
        val wanted = LibraryRoots.fromMediaFiles(paths, volumes())
        val existing = graph.query { it.roots() }.valueOrNull().orEmpty()
        val missing = LibraryRoots.missing(wanted, existing)
        missing.forEach { graph.send(Command.AddRoot(it)) }
        missing.size
    }

    /**
     * A folder the person picked with the system's folder picker: keeps read access across
     * restarts and adds its path as a root. Returns the path, or null when the provider has none.
     */
    fun addTree(tree: Uri): String? {
        runCatching {
            context.contentResolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        val documentId = runCatching { DocumentsContract.getTreeDocumentId(tree) }.getOrNull() ?: return null
        val path = LibraryRoots.pathOfTreeDocument(documentId, primaryVolume()) ?: return null
        graph.send(Command.AddRoot(path))
        return path
    }

    /**
     * A `.jfpkg` the person picked: copied to the cache (the core installs from a path) and
     * installed. Returns false when it could not be read.
     */
    suspend fun installPlugin(document: Uri): Boolean = withContext(Dispatchers.IO) {
        val folder = File(context.cacheDir, "plugins").apply { mkdirs() }
        val target = File(folder, "install-${System.currentTimeMillis()}.jfpkg")
        val copied = runCatching {
            context.contentResolver.openInputStream(document)?.use { input -> target.outputStream().use { input.copyTo(it) } } != null
        }.getOrDefault(false)
        if (copied) graph.send(Command.InstallPlugin(target.path))
        copied
    }

    /**
     * Import Playlist…: an M3U or M3U8 file the person picked, handed to the core's
     * `ImportPlaylist`. Read where it is when the core can (its relative lines then resolve
     * against its own folder), else copied to the cache under its own name, so the playlist is
     * still named after the file. Returns false when it could not be read; the core then reports
     * the new playlist (PlaylistCreated) or why it failed.
     */
    suspend fun importPlaylist(document: Uri): Boolean = withContext(Dispatchers.IO) {
        val inPlace = runCatching { DocumentsContract.getDocumentId(document) }.getOrNull()
            ?.let { PlaylistFiles.pathOfDocument(it, primaryVolume()) }
            ?.takeIf { File(it).canRead() }
        val path = inPlace ?: copyForImport(document) ?: return@withContext false
        graph.send(Command.ImportPlaylist(path)) != 0L
    }

    private fun copyForImport(document: Uri): String? {
        val folder = File(context.cacheDir, "playlists/import-${System.currentTimeMillis()}").apply { mkdirs() }
        val target = File(folder, PlaylistFiles.importName(displayName(document)))
        val copied = runCatching {
            context.contentResolver.openInputStream(document)?.use { input -> target.outputStream().use { input.copyTo(it) } } != null
        }.getOrDefault(false)
        return target.path.takeIf { copied }
    }

    private fun displayName(document: Uri): String? = runCatching {
        context.contentResolver.query(document, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        }
    }.getOrNull()

    /**
     * Export Playlist…: the core writes playlist [id] as M3U8 (`export_playlist`) to the cache,
     * and the file is copied into the document the person created. Null when it is done, else why
     * it failed. Songs are written with their full paths (they are not below the cache).
     */
    suspend fun exportPlaylist(id: Long, document: Uri): String? = withContext(Dispatchers.IO) {
        val folder = File(context.cacheDir, "playlists").apply { mkdirs() }
        val temp = File(folder, "export-${System.currentTimeMillis()}.${PlaylistFiles.EXTENSION}")
        try {
            when (val written = graph.query { it.exportPlaylist(id, temp.path) }) {
                is QueryResult.Ok -> Unit
                is QueryResult.Failed -> return@withContext written.message
                is QueryResult.NotAvailable, QueryResult.Loading -> return@withContext context.getString(R.string.not_available_yet)
            }
            val resolver = context.contentResolver
            // "wt" truncates a file being replaced; not every provider takes it.
            val out = runCatching { resolver.openOutputStream(document, "wt") }.getOrNull() ?: resolver.openOutputStream(document)
            out?.use { stream -> temp.inputStream().use { it.copyTo(stream) } } ?: return@withContext context.getString(R.string.toast_playlist_unwritable)
            null
        } catch (e: IOException) {
            e.message ?: e.toString()
        } catch (e: SecurityException) {
            e.message ?: e.toString()
        } finally {
            temp.delete()
        }
    }

    /**
     * Open Files… (the sidebar's foot, as on Jellyfesh Web): plays the picked files without adding
     * them to the library ([OpenedFiles]). Each plays where it is when the core can read it there,
     * else from a copy in the cache, which the next Open Files… replaces. Returns how many play.
     */
    suspend fun playFiles(documents: List<Uri>): Int = withContext(Dispatchers.IO) {
        val parent = File(context.cacheDir, "opened")
        val folder = File(parent, System.currentTimeMillis().toString()).apply { mkdirs() }
        val paths = documents.mapIndexedNotNull { index, document ->
            val inPlace = runCatching { DocumentsContract.getDocumentId(document) }.getOrNull()
                ?.let { PlaylistFiles.pathOfDocument(it, primaryVolume()) }
                ?.takeIf { File(it).canRead() }
            inPlace ?: copyOpened(document, File(folder, OpenedFiles.copyName(index, displayName(document))))
        }
        val command = OpenedFiles.command(paths) ?: return@withContext 0
        graph.send(command)
        // Copies from an earlier Open Files… leave the queue with this one.
        parent.listFiles()?.filter { it != folder }?.forEach { it.deleteRecursively() }
        paths.size
    }

    private fun copyOpened(document: Uri, target: File): String? {
        val copied = runCatching {
            context.contentResolver.openInputStream(document)?.use { input -> target.outputStream().use { input.copyTo(it) } } != null
        }.getOrDefault(false)
        return target.path.takeIf { copied }
    }

    /** Storage volume roots (`/storage/emulated/0`, SD cards), from the app's own folders on them. */
    private fun volumes(): List<String> =
        context.getExternalFilesDirs(null).mapNotNull { dir -> dir?.path?.substringBefore("/Android/", missingDelimiterValue = "")?.takeIf { it.isNotEmpty() } }

    private fun primaryVolume(): String = volumes().firstOrNull() ?: "/storage/emulated/0"
}

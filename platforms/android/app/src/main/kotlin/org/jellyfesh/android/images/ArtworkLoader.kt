package org.jellyfesh.android.images

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.jellyfesh.android.AppGraph
import org.jellyfesh.android.valueOrNull
import org.jellyfesh.logic.core.ArtworkRef
import org.jellyfesh.logic.images.ArtworkSizes

/** Artwork a view can show. */
@Immutable
sealed interface ArtKey {
    /** Library artwork by content hash: the core's cached thumbnail. */
    data class Hash(val hash: String) : ArtKey

    /** An image file: Now Playing artwork or a baked backdrop. */
    data class File(val path: String) : ArtKey

    companion object {
        /**
         * The key for a core artwork reference. URLs from online sources give null: the shell does
         * no networking (repo-layout rules), so such rows show their placeholder, as on Linux.
         */
        fun of(ref: ArtworkRef?): ArtKey? = (ref as? ArtworkRef.File)?.let { File(it.path) }

        fun ofHash(hash: String?): ArtKey? = hash?.takeIf { it.isNotEmpty() }?.let(::Hash)
    }
}

/**
 * Loads artwork off the main thread: the core's thumbnail for a hash (a synchronous query that
 * may write the file), decoded at about the size it is shown and kept in a byte-budget LRU.
 * Requests are tied to the composables that asked, so rows scrolled away cancel their loads and
 * the rows on screen win. Two loads at a time, so a fast fling does not flood the disk.
 */
class ArtworkLoader(context: Context, private val graph: AppGraph) {
    private val cache = object : LruCache<String, ImageBitmap>(ArtworkSizes.cacheBudget(Runtime.getRuntime().maxMemory())) {
        override fun sizeOf(key: String, value: ImageBitmap): Int = ArtworkSizes.bytes(value.width, value.height)
    }

    @Suppress("OPT_IN_USAGE")
    private val io = Dispatchers.IO.limitedParallelism(2)

    private fun cacheKey(key: ArtKey, sizePx: Int): String = when (key) {
        is ArtKey.Hash -> "h:${key.hash}@${ArtworkSizes.bucket(sizePx)}"
        is ArtKey.File -> "f:${key.path}@$sizePx"
    }

    /** The decoded image when it is in memory already (no I/O; safe on the main thread). */
    fun cached(key: ArtKey, sizePx: Int): ImageBitmap? = cache.get(cacheKey(key, sizePx))

    /** Loads and decodes [key] for about [sizePx] pixels; null when there is none. */
    suspend fun load(key: ArtKey, sizePx: Int): ImageBitmap? {
        val ck = cacheKey(key, sizePx)
        cache.get(ck)?.let { return it }
        return withContext(io) {
            cache.get(ck)?.let { return@withContext it }
            val path = when (key) {
                is ArtKey.Hash -> graph.query { it.artworkThumbnail(key.hash, ArtworkSizes.bucket(sizePx)) }.valueOrNull()
                is ArtKey.File -> key.path
            } ?: return@withContext null
            val image = decode(path, sizePx) ?: return@withContext null
            cache.put(ck, image)
            image
        }
    }

    /** Drops every decoded image (the system is short of memory). */
    fun trim() = cache.evictAll()

    private fun decode(path: String, sizePx: Int): ImageBitmap? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(path, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
        val options = BitmapFactory.Options().apply {
            inSampleSize = ArtworkSizes.sampleSize(bounds.outWidth, bounds.outHeight, sizePx)
            inPreferredConfig = Bitmap.Config.ARGB_8888
        }
        val bitmap = BitmapFactory.decodeFile(path, options) ?: return null
        // Uploads the pixels to the GPU now, on this thread, instead of at the first draw.
        bitmap.prepareToDraw()
        return bitmap.asImageBitmap()
    }
}

/**
 * The image for [key] at about [sizePx] pixels: at once when it is in memory, else after it
 * loads; null meanwhile (and when there is none), so the caller shows its placeholder.
 */
@Composable
fun rememberArtwork(loader: ArtworkLoader, key: ArtKey?, sizePx: Int): ImageBitmap? {
    // Keyed, so a new key never shows the previous key's image while it loads.
    val state = remember(key, sizePx) { mutableStateOf(key?.let { loader.cached(it, sizePx) }) }
    LaunchedEffect(key, sizePx) {
        if (state.value == null && key != null) state.value = loader.load(key, sizePx)
    }
    return state.value
}

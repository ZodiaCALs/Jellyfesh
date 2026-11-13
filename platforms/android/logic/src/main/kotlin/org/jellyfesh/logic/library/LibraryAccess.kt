package org.jellyfesh.logic.library

/**
 * Android library access, option C of the Step 0 index (decided for v0.0.3 here, still open in
 * the index): MediaStore gives an instant list of the folders that hold music, which become the
 * core's library roots (the core then reads every file's tags itself), and the person can add
 * folders the system does not index with the Storage Access Framework, or use only those when
 * they decline the media permission.
 *
 * Both paths end as filesystem paths for `AddRoot`: on Android 11+ the core reads shared storage
 * through the media permission's FUSE access. These helpers are the pure parts.
 */
object LibraryRoots {
    /**
     * The folders to scan for the audio files MediaStore listed: each file's folder, folded into
     * the shortest set of folders that covers them (a folder inside another root is dropped), and
     * never a storage volume's top level (the core would walk Android/ and every app's files).
     *
     * @param volumes storage volume roots such as `/storage/emulated/0`.
     */
    fun fromMediaFiles(paths: Iterable<String>, volumes: List<String>): List<String> {
        val roots = volumes.map { it.trimEnd('/') }.toSet()
        val folders = sortedSetOf<String>()
        for (path in paths) {
            val folder = path.substringBeforeLast('/', missingDelimiterValue = "")
            if (folder.isEmpty() || folder in roots) continue
            folders += topFolderUnderVolume(folder, volumes) ?: folder
        }
        return minimalCover(folders)
    }

    /**
     * The first folder below a volume root, so that `/storage/emulated/0/Music/A/B` becomes
     * `/storage/emulated/0/Music`; null when [folder] is not on a known volume.
     */
    fun topFolderUnderVolume(folder: String, volumes: List<String>): String? {
        val volume = volumes.map { it.trimEnd('/') }.filter { folder == it || folder.startsWith("$it/") }.maxByOrNull { it.length }
            ?: return null
        if (folder == volume) return null
        val rest = folder.removePrefix("$volume/")
        return "$volume/${rest.substringBefore('/')}"
    }

    /** Drops every folder that lies inside another one of the set. */
    fun minimalCover(folders: Collection<String>): List<String> {
        val out = ArrayList<String>()
        for (folder in folders.map { it.trimEnd('/') }.distinct().sorted()) {
            if (out.none { folder == it || folder.startsWith("$it/") }) out += folder
        }
        return out
    }

    /** Roots to add: the wanted ones the core does not have yet. */
    fun missing(wanted: List<String>, existing: List<String>): List<String> {
        val have = existing.map { it.trimEnd('/') }
        return wanted.filter { w -> have.none { w == it || w.startsWith("$it/") } }
    }

    /**
     * The filesystem path of a Storage Access Framework tree on external storage, from its tree
     * document id (`primary:Music/Rock` or `1A2B-3C4D:Albums`); null for other providers (a cloud
     * drive), whose files the core cannot read by path in this release.
     */
    fun pathOfTreeDocument(documentId: String, primaryRoot: String = "/storage/emulated/0"): String? {
        val colon = documentId.indexOf(':')
        if (colon <= 0) return null
        val volume = documentId.substring(0, colon)
        val relative = documentId.substring(colon + 1).trim('/')
        val root = when {
            volume.equals("primary", ignoreCase = true) -> primaryRoot.trimEnd('/')
            VOLUME_ID.matches(volume) -> "/storage/$volume"
            else -> return null
        }
        return if (relative.isEmpty()) root else "$root/$relative"
    }

    /** A removable volume's id, as in `/storage/1A2B-3C4D`. */
    private val VOLUME_ID = Regex("[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}")
}

/**
 * A long list read from the core in pages (`albums(offset, limit)` and friends): the pages a view
 * needs for its visible rows, and which to drop when the library changes. The core answers each
 * page synchronously; the app runs [load] off the main thread.
 */
class PagedList<T>(val pageSize: Int = 100) {
    private val pages = HashMap<Int, List<T>>()

    /** The library revision the pages were read at. */
    var revision: Long = -1
        private set

    /** True once a page came back shorter than [pageSize]: the list ends inside it. */
    var endPage: Int? = null
        private set

    /** The number of rows known so far (all of them once [endPage] is known). */
    val knownCount: Int
        get() {
            val end = endPage
            return if (end != null) end * pageSize + (pages[end]?.size ?: 0) else (pages.keys.maxOrNull()?.let { (it + 1) * pageSize } ?: 0)
        }

    operator fun get(index: Int): T? = pages[index / pageSize]?.getOrNull(index % pageSize)

    /** Pages to load so rows [first, last] (and one page ahead) are present. */
    fun pagesFor(first: Int, last: Int): List<Int> {
        if (last < first) return emptyList()
        val from = (first / pageSize).coerceAtLeast(0)
        val to = last / pageSize + 1
        return (from..to).filter { it !in pages && (endPage == null || it <= endPage!!) }
    }

    /** Stores a page that was read at [atRevision]; pages from an older revision are dropped. */
    fun put(page: Int, rows: List<T>, atRevision: Long) {
        if (atRevision < revision) return
        if (atRevision > revision) invalidate(atRevision)
        pages[page] = rows
        if (rows.size < pageSize) endPage = minOf(endPage ?: page, page)
    }

    /** The library changed: forget every page. */
    fun invalidate(newRevision: Long) {
        pages.clear()
        endPage = null
        revision = newRevision
    }

    /** Every row loaded so far, in order (for small lists). */
    fun loaded(): List<T> = pages.keys.sorted().flatMap { pages[it].orEmpty() }
}

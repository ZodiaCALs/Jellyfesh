package org.jellyfesh.android.ui.data

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.staticCompositionLocalOf
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import org.jellyfesh.android.AppGraph
import org.jellyfesh.android.QueryResult
import org.jellyfesh.logic.core.CoreClient
import org.jellyfesh.logic.library.PagedList

val LocalGraph = staticCompositionLocalOf<AppGraph> { error("AppGraph is missing") }

/**
 * A core query run off the main thread whenever [keys] change: pass the library revision among
 * them for lists that must follow catalogue, playlist and favourite changes (the core sends
 * LibraryChanged and the lists query again, core-api.md). Loading only until the first answer:
 * a later run keeps the previous answer on screen until the new one arrives.
 */
@Composable
fun <T> rememberQuery(vararg keys: Any?, block: (CoreClient) -> T): State<QueryResult<T>> {
    val graph = LocalGraph.current
    return produceState<QueryResult<T>>(QueryResult.Loading, *keys, graph.client) {
        value = graph.query(block)
    }
}

/**
 * A long list read from the core page by page as rows come into view (ADR-0001: paging, not
 * pushing). Rows are read through [get]; reading [count] and [version] subscribes the list to new
 * pages. Pages of an older library revision are dropped when the revision changes.
 */
@Stable
class PagedSource<T>(
    private val graph: AppGraph,
    private val scope: CoroutineScope,
    pageSize: Int,
    private val fetch: (CoreClient, offset: Int, limit: Int) -> List<T>,
) {
    private val list = PagedList<T>(pageSize)
    private val loading = HashSet<Int>()
    private var revision = -1L

    /** Bumped whenever a page arrives or the list resets. */
    var version by mutableIntStateOf(0)
        private set

    /** The first page failed, or the feature is not available yet. */
    var failure by mutableStateOf<QueryResult<Nothing>?>(null)
        private set

    /** True until the first page answers. */
    val isLoading: Boolean get() = version == 0 && failure == null

    val count: Int get() {
        version
        return list.knownCount
    }

    /** True when the end of the list is known (the last page came back short). */
    val complete: Boolean get() {
        version
        return list.endPage != null
    }

    operator fun get(index: Int): T? {
        version
        return list[index]
    }

    /** Every row loaded so far. */
    fun loaded(): List<T> {
        version
        return list.loaded()
    }

    /**
     * Loads the pages rows [first] to [last] need, at library [atRevision]. After a library change
     * (which the core also sends for play counts) the visible pages are read again in place: the
     * old rows stay on screen until the new ones arrive, instead of the list blanking out.
     */
    fun request(first: Int, last: Int, atRevision: Long) {
        val pages = if (atRevision != revision) {
            revision = atRevision
            loading.clear()
            val from = (first / list.pageSize).coerceAtLeast(0)
            (from..(maxOf(first, last) / list.pageSize + 1)).toList()
        } else {
            list.pagesFor(first, maxOf(first, last))
        }
        for (page in pages) {
            if (!loading.add(page)) continue
            scope.launch {
                val result = graph.query { fetch(it, page * list.pageSize, list.pageSize) }
                loading.remove(page)
                when (result) {
                    is QueryResult.Ok -> {
                        list.put(page, result.value, atRevision)
                        if (page == 0) failure = null
                        version++
                    }
                    is QueryResult.NotAvailable -> if (page == 0) failure = result
                    is QueryResult.Failed -> if (page == 0) failure = result
                    QueryResult.Loading -> Unit
                }
            }
        }
    }
}

/**
 * A [PagedSource] that follows a lazy list's visible rows and the library [revision]. The list
 * shows [rowOffset] items (headers) before the first row.
 */
@Composable
fun <T> rememberPagedSource(
    listState: LazyListState,
    revision: Long,
    vararg keys: Any?,
    pageSize: Int = 100,
    rowOffset: Int = 0,
    fetch: (CoreClient, Int, Int) -> List<T>,
): PagedSource<T> {
    val graph = LocalGraph.current
    val scope = rememberCoroutineScope()
    val source = remember(*keys, graph.client) { PagedSource(graph, scope, pageSize, fetch) }
    LaunchedEffect(source, revision) {
        snapshotFlow {
            val info = listState.layoutInfo
            val first = (info.visibleItemsInfo.firstOrNull()?.index ?: 0) - rowOffset
            val last = (info.visibleItemsInfo.lastOrNull()?.index ?: 0) - rowOffset
            first.coerceAtLeast(0) to last.coerceAtLeast(0)
        }.distinctUntilChanged().collect { (first, last) -> source.request(first, last, revision) }
    }
    return source
}

/** [rememberPagedSource] for a lazy grid. */
@Composable
fun <T> rememberPagedGridSource(
    gridState: LazyGridState,
    revision: Long,
    vararg keys: Any?,
    pageSize: Int = 100,
    rowOffset: Int = 0,
    fetch: (CoreClient, Int, Int) -> List<T>,
): PagedSource<T> {
    val graph = LocalGraph.current
    val scope = rememberCoroutineScope()
    val source = remember(*keys, graph.client) { PagedSource(graph, scope, pageSize, fetch) }
    LaunchedEffect(source, revision) {
        snapshotFlow {
            val info = gridState.layoutInfo
            val first = (info.visibleItemsInfo.firstOrNull()?.index ?: 0) - rowOffset
            val last = (info.visibleItemsInfo.lastOrNull()?.index ?: 0) - rowOffset
            first.coerceAtLeast(0) to last.coerceAtLeast(0)
        }.distinctUntilChanged().collect { (first, last) -> source.request(first, last, revision) }
    }
    return source
}

/** The value of an answered query, else null. */
val <T> State<QueryResult<T>>.ok: T?
    get() = (value as? QueryResult.Ok<T>)?.value

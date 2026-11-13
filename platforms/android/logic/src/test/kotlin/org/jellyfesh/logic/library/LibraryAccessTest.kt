package org.jellyfesh.logic.library

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class LibraryRootsTest {
    private val volumes = listOf("/storage/emulated/0", "/storage/1A2B-3C4D")

    @Test
    fun mediaFilesFoldIntoTopFolders() {
        val roots = LibraryRoots.fromMediaFiles(
            listOf(
                "/storage/emulated/0/Music/Artist/Album/01.flac",
                "/storage/emulated/0/Music/Other/02.mp3",
                "/storage/emulated/0/Download/song.ogg",
                "/storage/1A2B-3C4D/Albums/x/03.m4a",
                "/storage/emulated/0/ringtone-at-root.mp3",
                "/data/somewhere/else/track.flac",
            ),
            volumes,
        )
        assertEquals(
            listOf("/data/somewhere/else", "/storage/1A2B-3C4D/Albums", "/storage/emulated/0/Download", "/storage/emulated/0/Music"),
            roots,
        )
    }

    @Test
    fun aFolderInsideAnotherIsDropped() {
        assertEquals(listOf("/a/b"), LibraryRoots.minimalCover(listOf("/a/b/c", "/a/b", "/a/b/")))
        assertEquals(listOf("/a/b", "/a/bc"), LibraryRoots.minimalCover(listOf("/a/bc", "/a/b")))
    }

    @Test
    fun onlyNewRootsAreAdded() {
        assertEquals(
            listOf("/storage/emulated/0/Download"),
            LibraryRoots.missing(listOf("/storage/emulated/0/Music/Rock", "/storage/emulated/0/Download"), listOf("/storage/emulated/0/Music")),
        )
    }

    @Test
    fun treeDocumentsMapToPaths() {
        assertEquals("/storage/emulated/0/Music/Rock", LibraryRoots.pathOfTreeDocument("primary:Music/Rock"))
        assertEquals("/storage/emulated/0", LibraryRoots.pathOfTreeDocument("primary:"))
        assertEquals("/storage/1A2B-3C4D/Albums", LibraryRoots.pathOfTreeDocument("1A2B-3C4D:Albums"))
        assertNull(LibraryRoots.pathOfTreeDocument("acc=1;doc=encoded=abc"))
        assertNull(LibraryRoots.pathOfTreeDocument("home:Documents"))
    }
}

class PagedListTest {
    @Test
    fun loadsVisiblePagesAndOneAhead() {
        val list = PagedList<Int>(pageSize = 10)
        assertEquals(listOf(0, 1), list.pagesFor(0, 7))
        list.put(0, (0 until 10).toList(), atRevision = 1)
        assertEquals(listOf(1, 2), list.pagesFor(5, 15))
        list.put(1, (10 until 14).toList(), atRevision = 1)
        assertEquals(1, list.endPage)
        assertEquals(14, list.knownCount)
        assertEquals(emptyList(), list.pagesFor(0, 40), "nothing past the end")
        assertEquals(12, list[12])
        assertNull(list[15])
    }

    @Test
    fun aLibraryChangeDropsEveryPage() {
        val list = PagedList<Int>(pageSize = 2)
        list.put(0, listOf(1, 2), atRevision = 1)
        list.put(1, listOf(3, 4), atRevision = 2)
        assertNull(list[0], "the revision-1 page was dropped")
        assertEquals(4, list[3])
        list.put(0, listOf(9, 9), atRevision = 1)
        assertNull(list[0], "an older revision's page is ignored")
        assertEquals(listOf(3, 4), list.loaded())
    }
}

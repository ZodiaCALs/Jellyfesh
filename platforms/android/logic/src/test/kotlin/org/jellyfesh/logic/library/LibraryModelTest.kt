package org.jellyfesh.logic.library

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.AlbumSort
import org.jellyfesh.logic.core.Artist
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.CoreSnapshot
import org.jellyfesh.logic.core.DevicesSnapshot
import org.jellyfesh.logic.core.LibraryStats
import org.jellyfesh.logic.core.PlaybackSnapshot
import org.jellyfesh.logic.core.PlaylistEntry
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.QueueSnapshot
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.Track
import org.jellyfesh.logic.core.TrackQuery
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.core.TrackRow
import org.jellyfesh.logic.core.TrackSort
import org.jellyfesh.logic.state.FakeCoreClient

class LibraryModelTest {
    // ---- the sort menus (docs/ui.md, "Albums" and "Songs") ---------------------------------------

    @Test
    fun theMenusListTheSpecsFieldsInOrder() {
        assertEquals(listOf(AlbumSort.Title, AlbumSort.Artist, AlbumSort.Year, AlbumSort.Added), SortMenus.ALBUM_FIELDS)
        assertEquals(listOf(TrackSort.Title, TrackSort.Artist, TrackSort.Album, TrackSort.Year, TrackSort.Added), SortMenus.SONG_FIELDS)
        assertEquals(SortOrder(AlbumSort.Title, false), SortMenus.DEFAULT_ALBUMS)
        assertEquals(SortOrder(TrackSort.Artist, false), SortMenus.DEFAULT_SONGS)
    }

    @Test
    fun aFieldStartsTheWayPeopleReadItAndDescendingTurnsItRound() {
        assertEquals(SortOrder(AlbumSort.Added, true), SortMenus.album(AlbumSort.Added))
        assertEquals(SortOrder(AlbumSort.Year, true), SortMenus.album(AlbumSort.Year))
        assertEquals(SortOrder(AlbumSort.Artist, false), SortMenus.album(AlbumSort.Artist))
        assertEquals(SortOrder(TrackSort.Added, true), SortMenus.song(TrackSort.Added))
        assertEquals(SortOrder(TrackSort.Album, false), SortMenus.song(TrackSort.Album))
        assertEquals(SortOrder(TrackSort.Album, true), SortMenus.descending(SortMenus.song(TrackSort.Album), true))
        assertEquals(SortOrder(AlbumSort.Added, false), SortMenus.descending(SortMenus.album(AlbumSort.Added), false))
    }

    @Test
    fun songsAreAskedForInTheOrderChosen() {
        val query = SortMenus.songsQuery(SortOrder(TrackSort.Year, true), offset = 200, limit = 100)
        assertEquals(TrackQuery(offset = 200, limit = 100, sort = TrackSort.Year, descending = true), query)
    }

    @Test
    fun playAndShuffleCoverEverySongInTheOrderShown() {
        // The core's collection of the whole library, in the page's order: no paging in the app.
        assertEquals(CollectionRef.Songs(TrackSort.Album, false), SortMenus.allSongs(SortOrder(TrackSort.Album, false)))
        assertEquals(CollectionRef.Songs(TrackSort.Added, true), SortMenus.allSongs(SortMenus.song(TrackSort.Added)))
    }

    @Test
    fun everySongCanBeAddedToAPlaylistPageByPage() {
        val library = object : org.jellyfesh.logic.core.CoreClient by FakeCoreClient(snapshot) {
            val all = (1L..1_203L).map { id -> Track(id, "/m/$id.flac", "Song $id", null, null, null, null, null, null, null, null, null, null, null, null, null, null) }
            val asked = mutableListOf<TrackQuery>()
            override fun tracks(query: TrackQuery): List<Track> {
                asked += query
                return all.drop(query.offset).take(query.limit)
            }
        }
        val songs = CollectionTracks.of(library, CollectionRef.Songs(TrackSort.Title, true))
        assertEquals(1_203, songs.size)
        assertTrue(library.asked.all { it.sort == TrackSort.Title && it.descending })
        assertEquals(listOf(0, 500, 1_000), library.asked.map { it.offset })
    }

    // ---- Add to Playlist… for a collection ---------------------------------------------------------

    private val snapshot = CoreSnapshot(
        PlaybackSnapshot.Initial, QueueSnapshot.Empty, null, null, null, 0, false, Settings(), DevicesSnapshot.Empty, SourcesSnapshot.Empty, false,
    )

    private class Library(snapshot: CoreSnapshot) : org.jellyfesh.logic.core.CoreClient by FakeCoreClient(snapshot) {
        val artistTracks = (1L..1_203L).map { id -> Track(id, "/m/$id.flac", "Song $id", "Artist", null, null, null, null, null, null, null, null, null, null, null, null, null) }
        val queries = mutableListOf<TrackQuery>()
        val entries = listOf(
            PlaylistEntry(1, 0, TrackRow(TrackRef.Library(7), "A", null, null, null, null, null, false, false)),
            PlaylistEntry(2, 1, TrackRow(TrackRef.Source(SourceTrack("p", "x", "Streamed")), "B", null, null, null, null, null, false, false)),
            PlaylistEntry(3, 2, TrackRow(TrackRef.File("/m/loose.mp3"), "C", null, null, null, null, null, false, true)),
        )

        override fun albumTracks(albumId: Long): List<Track> = listOf(3L, 4L).map { Track(it, "/m/$it", "S", null, null, null, albumId, null, null, null, null, null, null, null, null, null, null) }

        override fun tracks(query: TrackQuery): List<Track> {
            queries += query
            return artistTracks.filter { query.artistId == 9L }.drop(query.offset).take(query.limit)
        }

        override fun playlistEntries(id: Long, offset: Int, limit: Int): List<PlaylistEntry> = entries.drop(offset).take(limit)

        override fun favorites(offset: Int, limit: Int): List<TrackRow> = listOf(TrackRow(TrackRef.Library(5), "F", null, null, null, null, null, true, false)).drop(offset).take(limit)
    }

    @Test
    fun anAlbumAddsItsSongs() {
        assertEquals(listOf<TrackRef>(TrackRef.Library(3), TrackRef.Library(4)), CollectionTracks.of(Library(snapshot), CollectionRef.Album(1)))
    }

    @Test
    fun anArtistsSongsAreGatheredPageByPage() {
        val library = Library(snapshot)
        val refs = CollectionTracks.of(library, CollectionRef.Artist(9))
        assertEquals(1_203, refs.size)
        assertEquals(TrackRef.Library(1), refs.first())
        assertEquals(listOf(0, 500, 1_000), library.queries.map { it.offset })
        assertTrue(library.queries.all { it.artistId == 9L && it.limit == CollectionTracks.PAGE })
    }

    @Test
    fun aPlaylistKeepsWhatPlaylistsCanHold() {
        // A source's song stays out, as the song menu leaves it out of Add to Playlist….
        assertEquals(
            listOf(TrackRef.Library(7), TrackRef.File("/m/loose.mp3")),
            CollectionTracks.of(Library(snapshot), CollectionRef.Playlist(2)),
        )
        assertEquals(listOf<TrackRef>(TrackRef.Library(5)), CollectionTracks.of(Library(snapshot), CollectionRef.Favorites))
        val list = listOf(TrackRef.Library(1), TrackRef.Source(SourceTrack("p", "y", "S")))
        assertEquals(listOf<TrackRef>(TrackRef.Library(1)), CollectionTracks.of(Library(snapshot), CollectionRef.Tracks(list)))
    }

    @Test
    fun aSourcesContainerCannotBeAdded() {
        assertFalse(CollectionTracks.canAdd(CollectionRef.Source("p", "album", ItemKind.Album)))
        assertTrue(CollectionTracks.canAdd(CollectionRef.Album(1)))
        assertEquals(emptyList(), CollectionTracks.of(Library(snapshot), CollectionRef.Source("p", "album", ItemKind.Album)))
    }

    // ---- Go to Artist ------------------------------------------------------------------------------

    @Test
    fun theArtistIsFoundByNameExactlyFirst() {
        val artists = listOf(Artist(1, "the band", 3), Artist(2, "The Band", 10))
        assertEquals(2L, ArtistMatch.find("The Band", artists)?.id)
        assertEquals(1L, ArtistMatch.find("THE BAND", artists)?.id)
        assertNull(ArtistMatch.find("Someone Else", artists))
    }

    // ---- Home --------------------------------------------------------------------------------------

    private fun stats(tracks: Long) = LibraryStats(1, tracks, 0, 0, 0, 0, 0)

    private fun plugin(account: String?) = PluginSummary(
        "org.jellyfesh.subsonic", "Subsonic", "1.0.0", "Jellyfesh", PluginBadge.Official, false, emptyList(),
        listOf(AuthMethod.PasswordExchange), true, account, false, "{}", "{}", null,
    )

    @Test
    fun theWelcomeShowsWithNothingInTheLibraryAndNoSourceSignedIn() {
        val none = SourcesSnapshot.Empty
        assertTrue(HomeModel.showsWelcome(stats(0), scanning = false, sources = none))
        assertTrue(HomeModel.showsWelcome(stats(0), scanning = false, sources = none.copy(plugins = listOf(plugin(null)))))
        assertFalse(HomeModel.showsWelcome(stats(0), scanning = false, sources = none.copy(plugins = listOf(plugin("ada")))))
        assertFalse(HomeModel.showsWelcome(stats(12), scanning = false, sources = none))
        assertFalse(HomeModel.showsWelcome(stats(0), scanning = true, sources = none), "a first scan is filling it")
        assertFalse(HomeModel.showsWelcome(null, scanning = false, sources = none), "not before the library answered")
    }

    // ---- the song menu -----------------------------------------------------------------------------

    @Test
    fun theSongMenuIsTheSpecsInOrder() {
        assertEquals(
            listOf(
                TrackMenuItem.PlayNext,
                TrackMenuItem.PlayLast,
                TrackMenuItem.AddToPlaylist,
                TrackMenuItem.AddToFavourites,
                TrackMenuItem.GoToAlbum,
                TrackMenuItem.GoToArtist,
            ),
            TrackMenu.items(TrackRef.Library(1), favourite = false, hasAlbum = true, hasArtist = true),
        )
        assertEquals(
            TrackMenuItem.RemoveFromFavourites,
            TrackMenu.items(TrackRef.Library(1), favourite = true, hasAlbum = false, hasArtist = false)[3],
        )
    }

    @Test
    fun aSourcesSongCannotGoInAPlaylist() {
        val items = TrackMenu.items(TrackRef.Source(SourceTrack("p", "1", "S")), favourite = null, hasAlbum = false, hasArtist = false)
        assertEquals(listOf(TrackMenuItem.PlayNext, TrackMenuItem.PlayLast), items)
    }

    @Test
    fun openInTheServiceComesLastWhenTheSongHasALink() {
        val items = TrackMenu.items(TrackRef.Library(1), favourite = false, hasAlbum = true, hasArtist = true, hasLink = true)
        assertEquals(TrackMenuItem.OpenLink, items.last())
        assertEquals(TrackMenuItem.GoToArtist, items[items.size - 2], "after Go to Artist")
        assertFalse(TrackMenuItem.OpenLink in TrackMenu.items(TrackRef.Library(1), favourite = false, hasAlbum = true, hasArtist = true))
    }

    @Test
    fun createSyncedLyricsFollowsGoToArtist() {
        val items = TrackMenu.items(TrackRef.Library(1), favourite = false, hasAlbum = true, hasArtist = true, hasLink = true, makesLyrics = true)
        assertEquals(listOf(TrackMenuItem.GoToArtist, TrackMenuItem.CreateSyncedLyrics, TrackMenuItem.OpenLink), items.takeLast(3))
        assertFalse(TrackMenuItem.CreateSyncedLyrics in TrackMenu.items(TrackRef.Library(1), favourite = false, hasAlbum = true, hasArtist = true))
    }

    @Test
    fun getInfoComesBeforeOpenInTheService() {
        val items = TrackMenu.items(TrackRef.Library(1), favourite = false, hasAlbum = true, hasArtist = true, hasLink = true, getInfo = true)
        assertEquals(listOf(TrackMenuItem.GoToArtist, TrackMenuItem.GetInfo, TrackMenuItem.OpenLink), items.takeLast(3))
    }

    @Test
    fun aRemoteSourcesSongNeverOffersTheQueue() {
        val song = TrackRef.Source(SourceTrack("spotify", "7", "Song"))
        assertEquals(listOf(TrackMenuItem.OpenLink), TrackMenu.items(song, favourite = null, hasAlbum = false, hasArtist = false, remote = true, hasLink = true))
        assertEquals(emptyList(), TrackMenu.items(song, favourite = null, hasAlbum = false, hasArtist = false, remote = true))
    }

    // ---- Go to Album and Go to Artist (docs/ui.md, "Rows and cards"; ADR-0027) ---------------------

    private val jellyfin = "org.jellyfesh.jellyfin"

    @Test
    fun aLibrarySongGoesToTheLibrarysAlbumAndArtist() {
        val song = TrackRef.Library(7)
        assertEquals(GoTo.LibraryAlbum(3, "Harbour Lights"), GoToPlaces.album(song, "Morning", albumId = 3, albumTitle = "Harbour Lights", sourceAlbumId = null, artist = "Ada"))
        assertEquals(GoTo.LibraryArtist("Ada"), GoToPlaces.artist(song, "Morning", artist = "Ada", sourceArtistId = null))
        // Unknown album and artist: neither is offered.
        assertNull(GoToPlaces.album(song, "Morning", albumId = null, albumTitle = "Harbour Lights", sourceAlbumId = null, artist = "Ada"))
        assertNull(GoToPlaces.artist(song, "Morning", artist = " ", sourceArtistId = null))
        // A file is not in the library: it has no artist page.
        assertNull(GoToPlaces.artist(TrackRef.File("/m/loose.mp3"), "Loose", artist = "Ada", sourceArtistId = null))
    }

    @Test
    fun aSourcesSongGoesToItsSourcesPagesWhereverItShows() {
        // A row from Home, Favourites, a playlist or the queue keeps the ids it was played with.
        val song = TrackRef.Source(SourceTrack(jellyfin, "t-1", "Morning", artist = "Ada", album = "Harbour Lights"))
        assertEquals(
            GoTo.SourcePage(jellyfin, "al-9", ItemKind.Album, "Harbour Lights", subtitle = "Ada"),
            GoToPlaces.album(song, "Morning", albumId = null, albumTitle = "Harbour Lights", sourceAlbumId = "al-9", artist = "Ada"),
        )
        assertEquals(
            GoTo.SourcePage(jellyfin, "ar-4", ItemKind.Artist, "Ada"),
            GoToPlaces.artist(song, "Morning", artist = "Ada", sourceArtistId = "ar-4"),
        )
        // Without the source's ids (rows kept before v0.0.9), neither is offered.
        assertNull(GoToPlaces.album(song, "Morning", albumId = null, albumTitle = "Harbour Lights", sourceAlbumId = null, artist = "Ada"))
        assertNull(GoToPlaces.artist(song, "Morning", artist = "Ada", sourceArtistId = null))
        // A library song never goes to a source's page.
        assertNull(GoToPlaces.artist(TrackRef.Library(7), "Morning", artist = null, sourceArtistId = "ar-4"))
    }

    @Test
    fun anAlbumsCardGoesToItsArtist() {
        assertEquals(GoTo.LibraryArtist("Ada"), GoToPlaces.artist(CollectionRef.Album(3), "Harbour Lights", artist = "Ada", sourceArtistId = null))
        assertNull(GoToPlaces.artist(CollectionRef.Album(3), "Harbour Lights", artist = null, sourceArtistId = null))
        val sourceAlbum = CollectionRef.Source(jellyfin, "al-9", ItemKind.Album)
        assertEquals(
            GoTo.SourcePage(jellyfin, "ar-4", ItemKind.Artist, "Ada"),
            GoToPlaces.artist(sourceAlbum, "Harbour Lights", artist = "Ada", sourceArtistId = "ar-4"),
        )
        assertNull(GoToPlaces.artist(sourceAlbum, "Harbour Lights", artist = "Ada", sourceArtistId = null))
        // Playlists have no artist.
        assertNull(GoToPlaces.artist(CollectionRef.Playlist(2), "Road Trip", artist = "Ada", sourceArtistId = null))
        assertNull(GoToPlaces.artist(sourceAlbum.copy(kind = ItemKind.Playlist), "Road Trip", artist = "Ada", sourceArtistId = "ar-4"))
    }

    @Test
    fun aCardsMenuIsTheSpecsInOrder() {
        // Play, Shuffle, Play Next, Play Last, Add to Playlist…, Go to Artist, Download, Open in {Service}.
        assertEquals(
            listOf(CollectionMenuItem.Play, CollectionMenuItem.Shuffle, CollectionMenuItem.PlayNext, CollectionMenuItem.PlayLast, CollectionMenuItem.AddToPlaylist, CollectionMenuItem.GoToArtist),
            CollectionMenu.items(CollectionRef.Album(3), withPlay = true, hasArtist = true),
        )
        assertEquals(
            listOf(
                CollectionMenuItem.Play, CollectionMenuItem.Shuffle, CollectionMenuItem.PlayNext, CollectionMenuItem.PlayLast,
                CollectionMenuItem.GoToArtist, CollectionMenuItem.Download, CollectionMenuItem.OpenLink,
            ),
            CollectionMenu.items(CollectionRef.Source(jellyfin, "al-9", ItemKind.Album), withPlay = true, hasLink = true, download = true, hasArtist = true),
        )
    }

    @Test
    fun theMoreMenuOfACollection() {
        val album = CollectionRef.Album(4)
        assertEquals(
            listOf(CollectionMenuItem.PlayNext, CollectionMenuItem.PlayLast, CollectionMenuItem.AddToPlaylist),
            CollectionMenu.items(album, withPlay = false),
        )
        assertEquals(
            listOf(CollectionMenuItem.Play, CollectionMenuItem.Shuffle, CollectionMenuItem.PlayNext, CollectionMenuItem.PlayLast, CollectionMenuItem.AddToPlaylist),
            CollectionMenu.items(album, withPlay = true),
        )
        val sourceAlbum = CollectionRef.Source("org.jellyfesh.subsonic", "al-1", ItemKind.Album)
        assertEquals(
            listOf(CollectionMenuItem.Play, CollectionMenuItem.Shuffle, CollectionMenuItem.PlayNext, CollectionMenuItem.PlayLast, CollectionMenuItem.OpenLink),
            CollectionMenu.items(sourceAlbum, withPlay = true, hasLink = true),
            "a source's album cannot be gathered into a playlist",
        )
        assertEquals(
            listOf(CollectionMenuItem.Play, CollectionMenuItem.Shuffle, CollectionMenuItem.OpenLink),
            CollectionMenu.items(CollectionRef.Source("spotify", "pl-1", ItemKind.Playlist), withPlay = true, remote = true, hasLink = true),
            "a remote source's playlist plays (a remote session) but never joins the queue",
        )
    }
}

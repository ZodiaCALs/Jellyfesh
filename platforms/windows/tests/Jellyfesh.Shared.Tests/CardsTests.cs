using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>
/// Album and playlist cards and their menus (docs/ui.md, "Rows and cards", v0.0.9): what the play
/// button and Play, Shuffle, Play Next, Play Last and Add to Playlist… act on, and Go to Artist.
/// </summary>
public class CardsTests
{
    private const string Spotify = "org.spotify";

    [Fact]
    public void A_card_plays_its_album_or_playlist_and_is_queued_unless_its_source_plays_on_other_devices()
    {
        var sources = new SourcesState(1, [Plugin(Spotify, remote: true), Plugin("org.jelly", remote: false)], [], false);
        var album = new AlbumItem(new Album(7, "Tides", "Band", 2024, 9, null), KeyLocalizer.Instance);
        var jelly = Entry("org.jelly", ItemKind.Album, "a1");
        var spotify = Entry(Spotify, ItemKind.Playlist, "p1");

        Assert.Equal(new Collection.Album(7), CardActions.CollectionOf(album));
        Assert.Equal(new Collection.Source("org.jelly", "a1", ItemKind.Album), CardActions.CollectionOf(jelly));
        Assert.Equal(new Collection.Source(Spotify, "p1", ItemKind.Playlist), CardActions.CollectionOf(spotify)); // Play and Shuffle start a remote session
        Assert.Null(CardActions.CollectionOf(new AlbumItem(0))); // a placeholder
        Assert.Null(CardActions.CollectionOf(Entry("org.jelly", ItemKind.Artist, "r1")));
        Assert.Null(CardActions.CollectionOf(Entry("org.jelly", ItemKind.Album, "a2", playable: false)));

        Assert.True(CardActions.CanQueue(sources, album) && CardActions.CanAddToPlaylist(sources, album));
        Assert.True(CardActions.CanQueue(sources, jelly) && CardActions.CanAddToPlaylist(sources, jelly));
        Assert.False(CardActions.OffersQueue(sources, spotify)); // never queued (ADR-0018)
        Assert.False(CardActions.CanQueue(sources, spotify) || CardActions.CanAddToPlaylist(sources, spotify));
        Assert.False(CardActions.CanQueue(sources, Entry("org.jelly", ItemKind.Artist, "r1")));

        var unlisted = new SourceEntryItem(jelly.Item with { Kind = ItemKind.Playlist }, KeyLocalizer.Instance) { SongsListed = false };
        Assert.True(CardActions.CanQueue(sources, unlisted));
        Assert.False(CardActions.CanAddToPlaylist(sources, unlisted)); // its songs are listed only in the service
    }

    [Fact]
    public async Task Go_to_Artist_leads_to_the_albums_artist_in_the_library_or_on_its_source()
    {
        using var h = new AppHarness();
        h.Core.ArtistRows.Add(new Artist(4, "band", 9));
        h.Core.ArtistRows.Add(new Artist(3, "Band", 9));
        h.Started();
        var album = new AlbumItem(new Album(7, "Tides", "Band", 2024, 9, null), KeyLocalizer.Instance);
        Assert.True(CardActions.HasArtist(album));
        Assert.Equal(3, (await CardActions.ArtistAsync(h.S, album))?.Id);
        var loose = new AlbumItem(new Album(8, "Loose Ends", null, null, 1, null), KeyLocalizer.Instance);
        Assert.False(CardActions.HasArtist(loose));
        Assert.Null(await CardActions.ArtistAsync(h.S, loose));

        Assert.Equal(new SourceTarget("org.jelly", "r9", "Band", ItemKind.Artist), CardActions.SourceArtist(Entry("org.jelly", ItemKind.Album, "a1", artistId: "r9")));
        Assert.Null(CardActions.SourceArtist(Entry("org.jelly", ItemKind.Album, "a2"))); // the source does not name it
        Assert.Null(CardActions.SourceArtist(Entry("org.jelly", ItemKind.Artist, "r9", artistId: "r9")));
    }

    [Fact]
    public async Task Add_to_Playlist_reads_a_source_albums_songs_page_by_page()
    {
        using var h = new AppHarness().Started();
        var reading = PlaylistActions.TracksOfAsync(h.S, new Collection.Source("org.jelly", "a1", ItemKind.Album));
        Assert.Equal(new CoreCommand.Browse("org.jelly", "a1", 0), h.Core.Sent[^1]);
        var sent = h.Core.Sent.Count;
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate((ulong)sent, "org.jelly", "a1", [Song("s1"), Song("s2", playable: false)], 1, null)));

        await Poll(() => h.Core.Sent.Count > sent);
        Assert.Equal(new CoreCommand.Browse("org.jelly", "a1", 1), h.Core.Sent[^1]);
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate((ulong)h.Core.Sent.Count, "org.jelly", "a1", [Song("s3")], null, null)));

        var tracks = await reading;
        Assert.Equal(["s1", "s3"], tracks.Select(t => Assert.IsType<TrackRef.Source>(t).Track.Id)); // what plays here
    }

    [Fact]
    public async Task Add_to_Playlist_keeps_the_songs_read_before_a_page_fails()
    {
        using var h = new AppHarness().Started();
        var reading = PlaylistActions.TracksOfAsync(h.S, new Collection.Source("org.jelly", "p1", ItemKind.Playlist));
        var sent = h.Core.Sent.Count;
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate((ulong)sent, "org.jelly", "p1", [Song("s1")], 1, null)));

        await Poll(() => h.Core.Sent.Count > sent);
        h.Emit(new CoreEvent.CommandFailed((ulong)h.Core.Sent.Count, "The service did not answer."));

        Assert.Equal(["s1"], (await reading).Select(t => Assert.IsType<TrackRef.Source>(t).Track.Id));
    }

    private static async Task Poll(Func<bool> done)
    {
        for (var i = 0; i < 500 && !done(); i++)
        {
            await Task.Delay(10, TestContext.Current.CancellationToken);
        }

        Assert.True(done());
    }

    private static PluginInfo Plugin(string id, bool remote) =>
        new(id, id, "1.0.0", "Someone", PluginBadge.Official, false, ["search", "browse"], [AuthMethod.Oauth], true, "ana", false, "{}", "{}", null, remote);

    private static SourceEntryItem Entry(string source, ItemKind kind, string id, bool playable = true, string? artistId = null) =>
        new(new SourceItem(source, kind, id, "Title " + id, "Band", null, null, null, playable, null, null, [], ArtistId: artistId), KeyLocalizer.Instance);

    private static SourceItem Song(string id, bool playable = true) =>
        new("org.jelly", ItemKind.Track, id, "Song " + id, "Band", null, 180_000, null, playable, null,
            new TrackRef.Source(new SourceTrack("org.jelly", id, "Song " + id, "Band", null, 180_000, null, null)), []);
}

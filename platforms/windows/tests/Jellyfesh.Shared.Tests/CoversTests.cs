using Jellyfesh.Core;
using Jellyfesh.Services;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Source pictures come through the core, never fetched by the app (docs/core-api.md, "Covers").</summary>
public class CoversTests
{
    private const string Url = "https://music.example/art/7.jpg";

    private static async Task Poll(Func<bool> done)
    {
        for (var i = 0; i < 200 && !done(); i++)
        {
            await Task.Delay(10, TestContext.Current.CancellationToken);
        }

        Assert.True(done());
    }

    [Fact]
    public async Task A_cached_picture_is_loaded_at_once_and_asked_for_once()
    {
        var core = new FakeCore { SourceArtworkPath = (url, size) => $"/cache/source-artwork/{size}.jpg" };
        var covers = new ThumbnailService(core);
        Assert.Equal("/cache/source-artwork/256.jpg", await covers.GetSourceAsync(Url, 256, TestContext.Current.CancellationToken));
        Assert.Equal("/cache/source-artwork/256.jpg", await covers.GetSourceAsync(Url, 256, TestContext.Current.CancellationToken));
        Assert.Equal([(Url, 256u)], core.ArtworkRequests);
    }

    [Fact]
    public async Task A_picture_being_fetched_arrives_with_the_artwork_event()
    {
        var core = new FakeCore();
        var covers = new ThumbnailService(core);
        var waiting = covers.GetSourceAsync(Url, 512, TestContext.Current.CancellationToken);
        var second = covers.GetSourceAsync(Url, 512, TestContext.Current.CancellationToken);
        await Poll(() => core.ArtworkRequests.Count == 1);
        Assert.False(waiting.IsCompleted);

        covers.OnArtwork(new ArtworkReady("https://music.example/other.jpg", 512, "/cache/other.jpg"));
        Assert.False(waiting.IsCompleted);
        covers.OnArtwork(new ArtworkReady(Url, 512, "/cache/source-artwork/7-512.jpg"));
        Assert.Equal("/cache/source-artwork/7-512.jpg", await waiting);
        Assert.Equal("/cache/source-artwork/7-512.jpg", await second);
        Assert.Single(core.ArtworkRequests); // both waited for one request
    }

    [Fact]
    public async Task A_picture_that_cannot_be_had_is_not_asked_for_again_for_a_while()
    {
        long now = 0;
        var core = new FakeCore();
        var covers = new ThumbnailService(core, nowMs: () => now);
        var waiting = covers.GetSourceAsync(Url, 128, TestContext.Current.CancellationToken);
        await Poll(() => core.ArtworkRequests.Count == 1);
        covers.OnArtwork(new ArtworkReady(Url, 128, null));
        Assert.Null(await waiting);
        Assert.Null(await covers.GetSourceAsync(Url, 128, TestContext.Current.CancellationToken));
        Assert.Single(core.ArtworkRequests);

        now += (long)ThumbnailService.SourceRetryAfter.TotalMilliseconds;
        _ = covers.GetSourceAsync(Url, 128, TestContext.Current.CancellationToken);
        await Poll(() => core.ArtworkRequests.Count == 2);
    }

    [Fact]
    public async Task A_build_without_source_pictures_stops_asking()
    {
        var core = new FakeCore { FailQueriesWith = new CoreQueryException(CoreQueryError.NotAvailable, "not yet") };
        var covers = new ThumbnailService(core);
        Assert.Null(await covers.GetSourceAsync(Url, 64, TestContext.Current.CancellationToken));
        Assert.Null(await covers.GetSourceAsync("https://music.example/art/8.jpg", 64, TestContext.Current.CancellationToken));
        Assert.Equal(1, core.QueryCount);
    }

    [Fact]
    public async Task A_source_row_shows_its_placeholder_until_the_core_has_the_picture()
    {
        using var h = new AppHarness().Started();
        var item = new SourceItem("org.x", ItemKind.Album, "al:1", "Night Swim", "Sample Artist", null, null,
            new ArtworkRef.Url(Url), false, null, null, []);
        var row = new SourceEntryItem(item, h.S.L);
        Assert.Null(row.ArtworkUri); // never the URL itself
        Assert.Equal(Url, row.SourceArtworkUrl);

        row.RequestArtwork(h.S.Thumbnails, 40 * 1.5);
        await Poll(() => h.Core.ArtworkRequests.Count == 1);
        Assert.Equal((Url, 64u), h.Core.ArtworkRequests[0]);
        h.Emit(new CoreEvent.Artwork(new ArtworkReady(Url, 64, "/cache/source-artwork/7-64.jpg")));
        await Poll(() => row.ArtworkUri is not null);
        Assert.Equal(FileUris.From("/cache/source-artwork/7-64.jpg"), row.ArtworkUri);
    }

    [Fact]
    public async Task Now_playing_shows_a_source_picture_from_the_cache_and_hands_its_file_to_the_media_controls()
    {
        using var h = new AppHarness().Started();
        var track = new TrackRef.Source(new SourceTrack("org.x", "t1", "Lantern", "Sample Artist", null, 184_000, Url, null));
        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(3, track, null, "Lantern", "Sample Artist", null, 184_000, new ArtworkRef.Url(Url), null)));
        var player = h.App.Player;
        Assert.Null(player.ArtworkUri);
        Assert.Null(player.ArtworkPath);
        await Poll(() => h.Core.ArtworkRequests.Count == 1);
        Assert.Equal((Url, 1024u), h.Core.ArtworkRequests[0]);

        // The same song again (its decoded format arrives) asks nothing more.
        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(3, track, null, "Lantern", "Sample Artist", null, 184_000, new ArtworkRef.Url(Url), null)));
        h.Emit(new CoreEvent.Artwork(new ArtworkReady(Url, 1024, "/cache/source-artwork/7-1024.jpg")));
        await Poll(() => player.ArtworkPath is not null);
        Assert.Equal("/cache/source-artwork/7-1024.jpg", player.ArtworkPath);
        Assert.Equal(FileUris.From("/cache/source-artwork/7-1024.jpg"), player.ArtworkUri);
        Assert.Single(h.Core.ArtworkRequests);
    }
}

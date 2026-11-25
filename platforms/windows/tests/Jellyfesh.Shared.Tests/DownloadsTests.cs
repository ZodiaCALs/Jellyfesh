using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>The Downloads page (ADR-0025; docs/ui.md "Downloads"), from the Downloads slice.</summary>
public class DownloadsTests
{
    [Fact]
    public void In_Progress_says_Waiting_a_percentage_or_why_it_failed()
    {
        using var h = new AppHarness(English.Instance).Started();
        var page = Open(h);
        h.Emit(new CoreEvent.Downloads(State(1,
            Item(1, "One", "Night Swim", new DownloadState.Queued()),
            Item(2, "Two", "Night Swim", new DownloadState.Downloading(), done: 250, total: 1000),
            Item(3, "Three", "Night Swim", new DownloadState.Downloading()),
            Item(4, "Four", "Night Swim", new DownloadState.Failed(new DownloadError.NotAllowed("Jellyfin"))),
            Item(5, "Five", null, new DownloadState.Failed(new DownloadError.Network("the connection was reset"))))));

        Assert.Equal(["Waiting", "25%", "Downloading…", "Your Jellyfin server doesn't allow downloads for this account.", "the connection was reset"],
            page.InProgress.Select(r => r.Status));
        Assert.Equal([false, true, false, false, false], page.InProgress.Select(r => r.HasProgress));
        Assert.Equal([false, false, false, true, true], page.InProgress.Select(r => r.IsFailed));
        Assert.Equal("Sample Artist · Night Swim", page.InProgress[0].Subtitle);
        Assert.Equal("Sample Artist", page.InProgress[4].Subtitle);
        Assert.True(page.HasInProgress);
        Assert.False(page.HasDownloaded);
        Assert.False(page.IsEmpty);

        // Progress moves the same rows.
        var row = page.InProgress[1];
        h.Emit(new CoreEvent.Downloads(State(2,
            Item(1, "One", "Night Swim", new DownloadState.Queued()),
            Item(2, "Two", "Night Swim", new DownloadState.Downloading(), done: 999, total: 1000),
            Item(3, "Three", "Night Swim", new DownloadState.Downloading()),
            Item(4, "Four", "Night Swim", new DownloadState.Failed(new DownloadError.NotAllowed("Jellyfin"))),
            Item(5, "Five", null, new DownloadState.Failed(new DownloadError.Network("the connection was reset"))))));
        Assert.Same(row, page.InProgress[1]);
        Assert.Equal(("99%", 99d), (row.Status, row.Percent));

        page.RetryCommand.Execute(page.InProgress[3]);
        Assert.Equal(new CoreCommand.RetryDownload(4), h.Core.Sent[^1]);
        page.RetryCommand.Execute(page.InProgress[0]); // only a failed one is tried again
        Assert.Equal(new CoreCommand.RetryDownload(4), h.Core.Sent[^1]);
        page.CancelCommand.Execute(page.InProgress[1]);
        Assert.Equal(new CoreCommand.CancelDownload(2), h.Core.Sent[^1]);
        page.Deactivate();
    }

    [Fact]
    public void While_the_core_pauses_downloads_the_page_says_why()
    {
        using var h = new AppHarness(English.Instance).Started();
        var page = Open(h);
        h.Emit(new CoreEvent.Downloads(Paused(1, DownloadsPaused.Offline, Item(1, "One", "A", new DownloadState.Queued()))));
        Assert.Equal("Downloads resume when you're back online.", page.PausedText);
        h.Emit(new CoreEvent.Downloads(Paused(2, DownloadsPaused.Metered, Item(1, "One", "A", new DownloadState.Queued()))));
        Assert.Equal("Downloads resume on Wi-Fi.", page.PausedText);
        h.Emit(new CoreEvent.Downloads(Paused(3, null, Item(1, "One", "A", new DownloadState.Queued()))));
        Assert.False(page.IsPaused);

        // With nothing waiting there is nothing to resume.
        h.Emit(new CoreEvent.Downloads(Paused(4, DownloadsPaused.Offline, Item(1, "One", "A", new DownloadState.Done(), libraryId: 11))));
        Assert.False(page.IsPaused);
        page.Deactivate();
    }

    [Fact]
    public async Task Downloaded_songs_come_by_album_play_and_are_removed_after_asking()
    {
        using var h = new AppHarness(English.Instance).Started();
        var page = Open(h);
        h.Emit(new CoreEvent.Downloads(State(1,
            Item(1, "One", "Night Swim", new DownloadState.Done(), libraryId: 11),
            Item(2, "Two", "Night Swim", new DownloadState.Done(), libraryId: 12),
            Item(3, "Loose", null, new DownloadState.Done(), libraryId: 13))));

        Assert.Equal(["Night Swim", "Unknown Album"], page.Albums.Select(a => a.Title));
        Assert.Equal(["One", "Two"], page.Albums[0].Songs.Select(s => s.Title));
        Assert.Equal("3 songs · 0.1 MB used", page.Summary);
        Assert.True(page.HasDownloaded);
        Assert.False(page.HasInProgress);

        page.PlaySongCommand.Execute(page.Albums[0].Songs[1]);
        var play = Assert.IsType<CoreCommand.PlayTracks>(h.Core.Sent[^1]);
        Assert.Equal([new TrackRef.Library(11), new TrackRef.Library(12), new TrackRef.Library(13)], play.Tracks);
        Assert.Equal(1u, play.Start);

        page.ShuffleCommand.Execute(null);
        var shuffle = Assert.IsType<CoreCommand.PlayCollection>(h.Core.Sent[^1]);
        Assert.Equal((0u, true), (shuffle.Start, shuffle.Shuffle)); // the core picks where to start
        Assert.Equal(3, Assert.IsType<Collection.Tracks>(shuffle.Collection).Items.Count);

        // Remove Download… asks first: no, then yes.
        h.Platform.Confirm = false;
        await page.RemoveSongCommand.ExecuteAsync(page.Albums[0].Songs[0]);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.RemoveDownload);
        Assert.Contains("Remove the Download of “One”?", h.Platform.Confirmations);

        h.Platform.Confirm = true;
        await page.RemoveAlbumCommand.ExecuteAsync(page.Albums[0]);
        Assert.Equal([new CoreCommand.RemoveDownload(1), new CoreCommand.RemoveDownload(2)], h.Core.Sent.OfType<CoreCommand.RemoveDownload>());
        Assert.Contains("Remove the Download of “Night Swim”?", h.Platform.Confirmations);
        page.Deactivate();
    }

    [Fact]
    public void Nothing_downloaded_is_the_empty_page()
    {
        using var h = new AppHarness(English.Instance).Started();
        var page = Open(h);
        Assert.True(page.IsEmpty);
        Assert.Equal(string.Empty, page.Summary);
        page.PlayCommand.Execute(null);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.PlayCollection);
        page.Deactivate();
    }

    private static DownloadsViewModel Open(AppHarness h)
    {
        var page = h.App.CreateDownloads();
        page.Activate();
        h.Ui.RunAll();
        return page;
    }

    private static DownloadsState State(ulong version, params DownloadItem[] items) => Paused(version, null, items);

    private static DownloadsState Paused(ulong version, DownloadsPaused? paused, params DownloadItem[] items) =>
        new(version, @"C:\Users\ana\Music\Jellyfesh", items, (ulong)items.Count(i => i.State is DownloadState.Done) * 40_000, paused);

    private static DownloadItem Item(ulong id, string title, string? album, DownloadState state, ulong done = 0, ulong? total = null, long? libraryId = null) =>
        new(id, new SourceTrack("org.jellyfin", "t" + id, title, "Sample Artist", album, 200_000, null, null), state, done, total,
            libraryId is null ? null : $@"C:\Users\ana\Music\Jellyfesh\{title}.flac", libraryId, true);
}

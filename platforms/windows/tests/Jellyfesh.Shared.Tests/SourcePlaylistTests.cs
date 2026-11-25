using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>A source playlist's page (ADR-0024 §2 and §3): songs listed only in the service, and Save as Playlist….</summary>
public class SourcePlaylistTests
{
    [Fact]
    public void A_playlist_whose_songs_the_service_keeps_says_so_and_still_plays()
    {
        using var h = Harness();
        var page = h.App.CreateSourceBrowse(new SourceTarget("org.jelly", "p9", "Road Trip", ItemKind.Playlist, SongsListed: false));
        page.Activate();
        h.Ui.RunAll();

        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.Browse); // nothing to list
        Assert.True(page.SongsListedElsewhere);
        Assert.Equal("Jelly lists this playlist's songs only in Jelly.", page.SongsListedElsewhereText);
        Assert.True(page.CanPlay);
        Assert.False(page.IsNothingHere);
        Assert.False(page.CanSaveCopy); // no songs to copy

        page.PlayCommand.Execute(null);
        var play = Assert.IsType<CoreCommand.PlayCollection>(h.Core.Sent[^1]);
        Assert.Equal(new Collection.Source("org.jelly", "p9", ItemKind.Playlist), play.Collection);
        page.Deactivate();
    }

    [Fact]
    public async Task Save_as_Playlist_asks_for_the_name_and_copies_the_songs()
    {
        using var h = Harness();
        var page = h.App.CreateSourceBrowse(new SourceTarget("org.jelly", "p1", "Summer", ItemKind.Playlist));
        page.Activate();
        h.Ui.RunAll();
        Assert.True(page.CanSaveCopy);
        Assert.True(page.HasMore);
        Assert.False(page.SongsListedElsewhere);
        Assert.Contains(h.Core.Sent, c => c is CoreCommand.Browse);

        // Cancelled: nothing is sent.
        h.Platform.Prompt = null;
        await page.SaveAsPlaylistCommand.ExecuteAsync(null);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.SavePlaylistCopy);

        h.Platform.Prompt = "  Summer (copy)  ";
        await page.SaveAsPlaylistCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.SavePlaylistCopy("org.jelly", "p1", "Summer (copy)"), h.Core.Sent[^1]);
        h.Emit(new CoreEvent.PlaylistCreated((ulong)h.Core.Sent.Count, 42));
        var saved = Assert.Single(h.App.Notifications.Items);
        Assert.Equal(("Playlist Saved", "Summer (copy)"), (saved.Title, saved.Message));
        page.Deactivate();
    }

    [Fact]
    public async Task Save_as_Playlist_before_the_core_can_says_it_is_not_in_this_build()
    {
        using var h = Harness();
        var page = h.App.CreateSourceBrowse(new SourceTarget("org.jelly", "p1", "Summer", ItemKind.Playlist));
        page.Activate();
        h.Ui.RunAll();
        await page.SaveAsPlaylistCommand.ExecuteAsync(null);
        h.Emit(new CoreEvent.CommandFailed((ulong)h.Core.Sent.Count, "not available in this build yet"));
        var notice = Assert.Single(h.App.Notifications.Items);
        Assert.Equal((NotificationSeverity.Informational, "Save as Playlist isn't available in this build yet."), (notice.Severity, notice.Message));
        page.Deactivate();
    }

    [Fact]
    public void Albums_and_folders_offer_no_copy()
    {
        using var h = Harness();
        Assert.False(h.App.CreateSourceBrowse(new SourceTarget("org.jelly", "a1", "Album", ItemKind.Album)).CanSaveCopy);
        Assert.False(h.App.CreateSourceBrowse(new SourceTarget("org.jelly", "f1", "Folder", ItemKind.Folder)).CanSaveCopy);
        Assert.False(h.App.CreateSourceBrowse(new SourceTarget("org.jelly", null, "Jelly", null)).SongsListedElsewhere);
    }

    private static AppHarness Harness()
    {
        var h = new AppHarness(English.Instance);
        var jelly = new PluginInfo("org.jelly", "Jelly", "1.0.0", "Someone", PluginBadge.Official, false, ["search", "browse"],
            [AuthMethod.PasswordExchange], true, "ana", false, "{}", "{}", null);
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [jelly], [], false) };
        return h.Started();
    }
}

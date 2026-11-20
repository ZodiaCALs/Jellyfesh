using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>A source's Discover page (docs/ui.md, "Discover"; docs/core-api.md, "Discover").</summary>
public class DiscoverTests
{
    private static PluginInfo Source() =>
        new("org.x", "Example", "1.0.0", "Someone", PluginBadge.Official, false, ["search", "browse"], [], true, "ana", false, "{}", "{}", null, Discover: true);

    private static SourceItem Item(string id, ItemKind kind, bool playable = true) =>
        new("org.x", kind, id, "Item " + id, "Artist", null, 180_000, new ArtworkRef.Url("https://music.example/" + id + ".jpg"), playable, playable ? null : "not here",
            kind == ItemKind.Track ? new TrackRef.Source(new SourceTrack("org.x", id, "Item " + id, "Artist", null, 180_000, null, null)) : null, []);

    private static AppHarness Start()
    {
        var h = new AppHarness();
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [Source()], [], false) };
        return h.Started();
    }

    [Fact]
    public async Task Shelves_come_from_the_cores_answer_to_this_request()
    {
        using var h = Start();
        var discover = h.App.CreateDiscover("org.x", "Example");
        await discover.LoadAsync();
        Assert.True(discover.IsLoading);
        Assert.Equal(new CoreCommand.Discover("org.x"), h.Core.Sent[^1]);
        Assert.Equal("Source_SignedInAs", discover.Status);
        discover.Activate();
        var request = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.Discover(new DiscoverPage(request + 1, "org.x", [], "someone else's"))); // not ours
        Assert.True(discover.IsLoading);

        h.Emit(new CoreEvent.Discover(new DiscoverPage(request, "org.x",
        [
            new Shelf("for_you", "For You", ShelfStyle.Row, [Item("a1", ItemKind.Album), Item("t1", ItemKind.Track)], "node:you"),
            new Shelf("charts", "Top 50", ShelfStyle.Chart, [Item("t2", ItemKind.Track), Item("t3", ItemKind.Track)], null),
            new Shelf("empty", "Nothing", ShelfStyle.Grid, [], null),
        ], null)));
        Assert.False(discover.IsLoading);
        Assert.Equal(2, discover.Shelves.Count); // an empty shelf without more is left out
        var row = discover.Shelves[0];
        Assert.Equal(("For You", true, true), (row.Title, row.IsRow, row.HasMore));

        // A shelf the core makes is worded by its id; a plugin's keeps its own title.
        var chinese = new Words(new() { ["Shelf_for_you"] = "为你推荐" });
        Assert.Equal("为你推荐", ShelfViewModel.Words(new Shelf("for_you", "For You", ShelfStyle.Row, [], null), chinese));
        Assert.Equal("Top 50", ShelfViewModel.Words(new Shelf("charts", "Top 50", ShelfStyle.Chart, [], null), chinese));
        Assert.IsType<SourceEntryItem>(row.Items[0]);
        Assert.IsType<TrackItem>(row.Items[1]);
        var chart = discover.Shelves[1];
        Assert.Equal(("Top 50", true), (chart.Title, chart.IsChart));
        Assert.Equal(["1", "2"], chart.Items.OfType<TrackItem>().Select(t => t.Number));
        Assert.False(discover.IsEmpty);
    }

    [Fact]
    public void Items_open_their_pages_or_play_with_their_shelf_and_see_all_and_browse_open_pages()
    {
        using var h = Start();
        var discover = h.App.CreateDiscover("org.x", "Example");
        var opened = new List<SourceTarget>();
        discover.OpenRequested += opened.Add;
        var shelf = new ShelfViewModel(new Shelf("top", "Top", ShelfStyle.Chart, [Item("t1", ItemKind.Track), Item("t2", ItemKind.Track), Item("a1", ItemKind.Album)], "chart:top"), h.S.L);

        discover.Open(shelf, shelf.Items[1]);
        var play = Assert.IsType<CoreCommand.PlayTracks>(h.Core.Sent[^1]);
        Assert.Equal((2, 1u), (play.Tracks.Count, play.Start));

        discover.Open(shelf, shelf.Items[2]);
        Assert.Equal(("a1", ItemKind.Album), (opened[^1].Node, opened[^1].Kind));

        discover.SeeAllCommand.Execute(shelf);
        Assert.Equal(("chart:top", (ItemKind?)null), (opened[^1].Node, opened[^1].Kind));

        discover.BrowseCommand.Execute(null);
        Assert.Equal(("org.x", (string?)null), (opened[^1].Plugin, opened[^1].Node));
    }

    [Fact]
    public async Task A_failure_shows_the_cores_words_and_try_again_asks_again()
    {
        using var h = Start();
        var discover = h.App.CreateDiscover("org.x", "Example");
        discover.Activate();
        await discover.LoadAsync();
        var request = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.CommandFailed(request, "the service did not answer"));
        Assert.Equal("the service did not answer", discover.Error);
        Assert.True(discover.HasError);
        Assert.False(discover.IsLoading);
        Assert.Empty(h.App.Notifications.Items);

        await discover.TryAgainCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.Discover("org.x"), h.Core.Sent[^1]);
        Assert.False(discover.HasError);
    }

    private sealed class Words(Dictionary<string, string> table) : Jellyfesh.Formatting.ILocalizer
    {
        public System.Globalization.CultureInfo Culture => System.Globalization.CultureInfo.InvariantCulture;

        public string Get(string key) => table.TryGetValue(key, out var value) ? value : key;
    }
}

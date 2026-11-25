using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Search by source (ADR-0024 §4): the tabs, sources that keep their results apart, and Show More.</summary>
public class SearchTabsTests
{
    [Fact]
    public async Task Tabs_are_All_Library_and_each_source_that_found_something()
    {
        using var h = Harness();
        var search = await SearchAsync(h, "harbour");
        Assert.Equal(["all", "library"], search.Tabs.Select(t => t.Id));
        Assert.Equal(["All", "Library"], search.Tabs.Select(t => t.Title));
        Assert.True(search.Tabs[0].IsSelected);
        var id = h.Core.LastSentId<CoreCommand.Search>();

        // Sub found songs and albums; Jelly found nothing: only Sub gets a tab, in the order searched.
        h.Emit(
            new CoreEvent.SearchResults(Section(id, "org.sub", [Song("org.sub", "s1")], Groups("org.sub", songs: 1, albums: 1, next: "c2"))),
            new CoreEvent.SearchResults(new SearchUpdate(id, "harbour", "org.jelly", [], null, true, Groups: [])));
        Assert.Equal(["all", "library", "org.sub"], search.Tabs.Select(t => t.Id));
        Assert.Equal("Sub", search.Tabs[2].Title);

        // Sub's tab: its Songs and Albums, Songs with Show More.
        search.SelectTabCommand.Execute("org.sub");
        Assert.True(search.ShowsSource);
        Assert.False(search.ShowsLibraryResults);
        Assert.Equal([false, false, true], search.Tabs.Select(t => t.IsSelected));
        var sub = search.SelectedSource!;
        Assert.Equal(["Songs", "Albums"], sub.Kinds.Select(k => k.Title));
        Assert.Equal((1, true, "Show More"), (sub.Kinds[0].Tracks.Count, sub.Kinds[0].HasMore, sub.Kinds[0].ShowMoreLabel));
        Assert.Equal((1, false), (sub.Kinds[1].Entries.Count, sub.Kinds[1].HasMore));
        Assert.False(search.NothingFound);

        // Library: nothing in the library for these words.
        search.SelectTabCommand.Execute("library");
        Assert.True(search.ShowsLibraryResults);
        Assert.Null(search.SelectedSource);
        Assert.True(search.NothingFound);

        // Clearing the box goes back to All, with no tabs.
        search.Query = string.Empty;
        await search.SubmitCommand.ExecuteAsync(null);
        Assert.Empty(search.Tabs);
        Assert.True(search.ShowsAll);
        Assert.False(search.NothingFound);
        search.Deactivate();
    }

    [Fact]
    public async Task A_source_that_keeps_its_results_apart_is_linked_from_All_and_never_beside_other_rows()
    {
        using var h = Harness();
        var search = await SearchAsync(h, "harbour");
        var id = h.Core.LastSentId<CoreCommand.Search>();
        h.Emit(new CoreEvent.SearchResults(Section(id, "org.jelly", [Song("org.jelly", "j1")], Groups("org.jelly", songs: 1), merged: false, done: true)));

        var jelly = search.OnlineGroups.Single(g => g.Source == "org.jelly");
        Assert.False(jelly.IsShown); // no rows under From Your Sources
        Assert.True(jelly.IsApart);
        Assert.Equal(["org.jelly"], search.ApartSources.Select(g => g.Source)); // "Results from Jelly"
        Assert.Contains(search.Tabs, t => t.Id == "org.jelly");
        Assert.True(search.HasResults);
        Assert.False(search.NothingFound);

        // The link opens its tab.
        search.SelectTabCommand.Execute(jelly.Source);
        Assert.Same(jelly, search.SelectedSource);
        Assert.Single(search.SelectedSource!.Kinds[0].Tracks);
        search.Deactivate();
    }

    [Fact]
    public async Task Show_More_asks_the_core_for_the_next_page_and_adds_it()
    {
        using var h = Harness();
        var search = await SearchAsync(h, "harbour");
        var id = h.Core.LastSentId<CoreCommand.Search>();
        h.Emit(new CoreEvent.SearchResults(Section(id, "org.sub", [Song("org.sub", "s1")], Groups("org.sub", songs: 1, next: "c2"), done: true)));
        var songs = search.OnlineGroups[0].Kinds[0];

        songs.ShowMoreCommand.Execute(null);
        Assert.Equal(new CoreCommand.SearchMore("org.sub", ItemKind.Track, "c2"), h.Core.Sent[^1]);
        Assert.True(songs.IsLoadingMore);
        Assert.Equal("Loading…", songs.ShowMoreLabel);
        Assert.False(songs.ShowMoreCommand.CanExecute(null)); // one page at a time
        var more = (ulong)h.Core.Sent.Count;

        h.Emit(new CoreEvent.SearchPage(new SearchMorePage(more, "harbour", "org.sub", ItemKind.Track, [Song("org.sub", "s2"), Song("org.sub", "s3")], "c3", null)));
        Assert.Equal(3, songs.Tracks.Count);
        Assert.Equal(("c3", false), (songs.Next, songs.IsLoadingMore));

        // The last page has no cursor: Show More goes.
        songs.ShowMoreCommand.Execute(null);
        var last = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.SearchPage(new SearchMorePage(last, "harbour", "org.sub", ItemKind.Track, [Song("org.sub", "s4")], null, null)));
        Assert.Equal(4, songs.Tracks.Count);
        Assert.False(songs.HasMore);

        // A page for another request changes nothing.
        h.Emit(new CoreEvent.SearchPage(new SearchMorePage(999, "harbour", "org.sub", ItemKind.Track, [Song("org.sub", "x")], "c9", null)));
        Assert.Equal(4, songs.Tracks.Count);
        search.Deactivate();
    }

    [Fact]
    public async Task Show_More_that_fails_says_why_and_can_be_tried_again()
    {
        using var h = Harness();
        var search = await SearchAsync(h, "harbour");
        var id = h.Core.LastSentId<CoreCommand.Search>();
        h.Emit(new CoreEvent.SearchResults(Section(id, "org.sub", [], Groups("org.sub", albums: 2, next: "c2"), done: true)));
        var albums = search.OnlineGroups[0].Kinds.Single();
        Assert.Equal(ItemKind.Album, albums.Kind);

        albums.ShowMoreCommand.Execute(null);
        h.Emit(new CoreEvent.SearchPage(new SearchMorePage((ulong)h.Core.Sent.Count, "harbour", "org.sub", ItemKind.Album, [], null, "the server timed out")));
        Assert.Equal(("the server timed out", "c2"), (albums.MoreError, albums.Next));
        Assert.True(albums.ShowMoreCommand.CanExecute(null));

        // The command itself failing (the source went away) is shown the same way, not as a notice.
        albums.ShowMoreCommand.Execute(null);
        Assert.Null(albums.MoreError);
        h.Emit(new CoreEvent.CommandFailed((ulong)h.Core.Sent.Count, "the source is disabled"));
        Assert.Equal(("the source is disabled", false), (albums.MoreError, albums.IsLoadingMore));
        Assert.Empty(h.App.Notifications.Items);
        search.Deactivate();
    }

    [Fact]
    public async Task A_section_without_groups_still_gives_its_tab()
    {
        using var h = Harness();
        var search = await SearchAsync(h, "harbour");
        var id = h.Core.LastSentId<CoreCommand.Search>();
        var album = new SourceItem("org.sub", ItemKind.Album, "a1", "Harbour", "Band", null, null, null, true, null, null, []);
        h.Emit(new CoreEvent.SearchResults(new SearchUpdate(id, "harbour", "org.sub", [Song("org.sub", "s1"), album], null, true)));
        Assert.Equal([ItemKind.Track, ItemKind.Album], search.OnlineGroups[0].Kinds.Select(k => k.Kind));
        Assert.All(search.OnlineGroups[0].Kinds, k => Assert.False(k.HasMore));
        search.Deactivate();
    }

    private static AppHarness Harness()
    {
        var h = new AppHarness(English.Instance);
        var sub = new PluginInfo("org.sub", "Sub", "1.0.0", "Someone", PluginBadge.Official, false, ["search", "browse"],
            [AuthMethod.PasswordExchange], true, "ana", false, "{}", "{}", null);
        var jelly = sub with { Id = "org.jelly", Name = "Jelly" };
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [sub, jelly], [], false) };
        return h.Started();
    }

    private static async Task<SearchViewModel> SearchAsync(AppHarness h, string query)
    {
        var search = h.App.CreateSearch();
        search.Activate();
        search.Query = query;
        await search.SubmitCommand.ExecuteAsync(null);
        return search;
    }

    private static SearchUpdate Section(ulong id, string source, IReadOnlyList<SourceItem> items, IReadOnlyList<SearchGroup> groups, bool merged = true, bool done = false) =>
        new(id, "harbour", source, items, null, done, merged, groups);

    private static IReadOnlyList<SearchGroup> Groups(string source, int songs = 0, int albums = 0, string? next = null)
    {
        var groups = new List<SearchGroup>();
        if (songs > 0)
        {
            groups.Add(new SearchGroup(ItemKind.Track, Enumerable.Range(1, songs).Select(i => Song(source, $"s{i}")).ToList(), next, null));
        }

        if (albums > 0)
        {
            var items = Enumerable.Range(1, albums)
                .Select(i => new SourceItem(source, ItemKind.Album, $"a{i}", $"Album {i}", "Band", null, null, null, true, null, null, []))
                .ToList();
            groups.Add(new SearchGroup(ItemKind.Album, items, songs > 0 ? null : next, null));
        }

        return groups;
    }

    private static SourceItem Song(string source, string id) =>
        new(source, ItemKind.Track, id, "Song " + id, "Band", null, 180_000, null, true, null,
            new TrackRef.Source(new SourceTrack(source, id, "Song " + id, "Band", null, 180_000, null, null)), []);
}

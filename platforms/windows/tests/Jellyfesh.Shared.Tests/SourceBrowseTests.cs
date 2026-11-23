using System.Globalization;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>
/// A source's pages (docs/parity.md N6, N17, N18, N19): albums and playlists open a page with their
/// songs, Play and Shuffle instead of playing at once; songs play with the rest of their album after
/// them; "Content from {Service}"; Load More; the source's failures and empty pages; "Also in …".
/// </summary>
public class SourceBrowseTests
{
    private static readonly Dictionary<string, string> English = new(StringComparer.Ordinal)
    {
        ["Kind_Album"] = "Album",
        ["Kind_Playlist"] = "Playlist",
        ["Kind_Artist"] = "Artist",
        ["Kind_Folder"] = "Folder",
        ["Count_Songs_One"] = "{0} song",
        ["Count_Songs_Other"] = "{0} songs",
        ["Count_Minutes_One"] = "{0} minute",
        ["Count_Minutes_Other"] = "{0} minutes",
        ["Album_Summary"] = "{0}, {1}",
        ["Source_Attribution"] = "Content from {0}",
        ["Link_OpenIn"] = "Open in {0}",
        ["Search_AlsoIn"] = "Also in {0}",
        ["List_Separator"] = ", ",
    };

    [Fact]
    public void An_album_lists_its_songs_with_play_shuffle_and_the_total()
    {
        var h = Harness(out var sub);
        var album = Item(ItemKind.Album, "al1", "Harbour", "Band", playable: true);
        var page = h.App.CreateSourceBrowse(SourceTarget.Of(album));
        page.Activate();

        Assert.Equal(("Album", "Harbour", "Band"), (page.KindLabel, page.Title, page.Subtitle));
        Assert.True(page.IsCollection);
        Assert.True(page.IsNumbered);
        Assert.Equal(new CoreCommand.Browse(sub.Id, "al1", 0), h.Core.Sent[^1]);
        Assert.False(page.CanPlay); // nothing listed yet

        // The first page: a disc folder, then two songs.
        var first = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate(first, sub.Id, "al1", [Item(ItemKind.Folder, "d1", "Disc 1"), Song("s1", 200_000), Song("s2", 160_000)], 1, null)));
        Assert.True(page.CanPlay);
        Assert.True(page.HasNextPage);
        Assert.False(page.IsComplete);
        Assert.Equal(string.Empty, page.Footer); // the total waits for every page
        var tracks = page.Tracks.ToList();
        Assert.Equal(["1", "2"], tracks.Select(t => t.Number));

        page.LoadMoreCommand.Execute(null);
        Assert.Equal(new CoreCommand.Browse(sub.Id, "al1", 1), h.Core.Sent[^1]);
        var second = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate(second, sub.Id, "al1", [Song("s3", 240_000)], null, null)));
        Assert.True(page.IsComplete);
        Assert.False(page.HasNextPage);
        Assert.Equal("3 songs", page.Facts);
        Assert.Equal("3 songs, 10 minutes", page.Footer);
        Assert.Equal("3", page.Tracks.Last().Number);

        // Play and Shuffle play the whole album through the core, which fetches every page.
        page.PlayCommand.Execute(null);
        Assert.Equal(new CoreCommand.PlayCollection(new Collection.Source(sub.Id, "al1"), 0, false), h.Core.Sent[^1]);
        page.ShuffleCommand.Execute(null);
        Assert.Equal(new CoreCommand.PlayCollection(new Collection.Source(sub.Id, "al1"), 0, true), h.Core.Sent[^1]);

        // A song plays the album from itself: its place counts everything the source listed (the folder too).
        page.OpenCommand.Execute(page.Tracks.ElementAt(1));
        Assert.Equal(new CoreCommand.PlayCollection(new Collection.Source(sub.Id, "al1"), 2, false), h.Core.Sent[^1]);

        page.PlayNextCommand.Execute(null);
        Assert.Equal(new CoreCommand.EnqueueCollection(new Collection.Source(sub.Id, "al1"), true), h.Core.Sent[^1]);
        page.PlayLastCommand.Execute(null);
        Assert.Equal(new CoreCommand.EnqueueCollection(new Collection.Source(sub.Id, "al1"), false), h.Core.Sent[^1]);
        page.Deactivate();
    }

    [Fact]
    public void A_source_that_asks_for_credit_says_so_and_a_remote_one_is_not_queued()
    {
        var h = Harness(out _, attribution: "Spotify", remote: true);
        var playlist = Item(ItemKind.Playlist, "p1", "Mix", "Someone", playable: true) with { Source = "org.spotify" };
        var page = h.App.CreateSourceBrowse(SourceTarget.Of(playlist));
        page.Activate();
        var request = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate(request, "org.spotify", "p1", [Song("s1", 1_000) with { Source = "org.spotify" }], null, null)));

        Assert.Equal("Content from Spotify", page.Attribution);
        Assert.True(page.CanPlay);
        Assert.False(page.IsNumbered);
        Assert.False(page.CanQueue); // its songs play on the person's own devices
        Assert.Equal("Open in Spotify", page.OpenInLabel);
        page.PlayNextCommand.Execute(null);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.EnqueueCollection);
        page.Deactivate();

        var plain = Harness(out var sub);
        var other = plain.App.CreateSourceBrowse(SourceTarget.Root(sub));
        Assert.Null(other.Attribution);
        Assert.False(other.HasAttribution);
    }

    [Fact]
    public void Nothing_that_plays_here_makes_the_link_the_main_action()
    {
        var h = Harness(out var sub);
        var album = Item(ItemKind.Album, "al1", "VIP", null, playable: true) with { Link = "https://music.example/album/al1" };
        var page = h.App.CreateSourceBrowse(SourceTarget.Of(album));
        page.Activate();
        var request = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate(request, sub.Id, "al1", [Song("s1", 1_000) with { Playable = false, Track = null }], null, null)));

        Assert.False(page.CanPlay);
        Assert.True(page.OpensLink);
        page.OpenLinkCommand.Execute(null);
        Assert.Equal(["https://music.example/album/al1"], h.Platform.Opened);
        page.Deactivate();
    }

    [Fact]
    public void A_page_says_when_the_source_did_not_answer_or_has_nothing()
    {
        var h = Harness(out var sub);
        var page = h.App.CreateSourceBrowse(SourceTarget.Root(sub));
        page.Activate();
        var request = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate(request, sub.Id, null, [], null, "The server took too long.")));
        Assert.True(page.HasError);
        Assert.Equal("The server took too long.", page.Error);
        Assert.False(page.IsNothingHere);
        page.Deactivate();

        var empty = h.App.CreateSourceBrowse(SourceTarget.Root(sub));
        empty.Activate();
        var again = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate(again, sub.Id, null, [], null, null)));
        Assert.True(empty.IsNothingHere);
        Assert.False(empty.HasError);
        empty.Deactivate();
    }

    [Fact]
    public void A_page_asks_again_for_what_arrived_while_it_was_hidden()
    {
        var h = Harness(out var sub);
        var page = h.App.CreateSourceBrowse(SourceTarget.Root(sub));
        page.Activate();
        var sent = h.Core.Sent.Count;
        page.Deactivate();

        // Its answer comes while another page is shown: it is not seen.
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate((ulong)sent, sub.Id, null, [Song("s1", 1_000)], null, null)));
        Assert.Empty(page.Rows);

        page.Activate();
        Assert.Equal(sent + 1, h.Core.Sent.Count);
        Assert.Equal(new CoreCommand.Browse(sub.Id, null, 0), h.Core.Sent[^1]);
        var request = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate(request, sub.Id, null, [Song("s1", 1_000)], null, null)));
        Assert.Single(page.Rows);
        page.Deactivate();
        page.Activate();
        Assert.Equal(sent + 1, h.Core.Sent.Count); // nothing outstanding: nothing asked again
        page.Deactivate();
    }

    [Fact]
    public async Task Search_names_the_other_sources_that_have_an_item()
    {
        var h = Harness(out var sub);
        var search = h.App.CreateSearch();
        search.Activate();
        search.Query = "harbour";
        await search.SubmitCommand.ExecuteAsync(null);
        var id = (ulong)h.Core.Sent.Count;
        var song = Song("s1", 1_000) with { AlsoIn = ["org.jelly", "org.sub"] };
        h.Emit(new CoreEvent.SearchResults(new SearchUpdate(id, "harbour", sub.Id, [song], null, true)));

        Assert.Equal("Also in Jelly", Assert.Single(search.OnlineGroups.Single(g => g.Source == sub.Id).Tracks).Origin);
        search.Deactivate();
    }

    private static AppHarness Harness(out PluginInfo plugin, string? attribution = null, bool remote = false)
    {
        var h = new AppHarness(new TableLocalizer(English));
        plugin = new PluginInfo(
            remote ? "org.spotify" : "org.sub", remote ? "Spotify" : "Sub", "1.0.0", "Someone", PluginBadge.Official, false, ["search", "browse"],
            [AuthMethod.PasswordExchange], true, "ana", false, "{}", "{}", null, remote, attribution);
        var jelly = plugin with { Id = "org.jelly", Name = "Jelly", Attribution = null, RemotePlayback = false };
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [plugin, jelly], [], false) };
        return h.Started();
    }

    private static SourceItem Item(ItemKind kind, string id, string title, string? subtitle = null, bool playable = false) =>
        new("org.sub", kind, id, title, subtitle, null, null, null, playable, null, null, []);

    private static SourceItem Song(string id, ulong ms) =>
        new("org.sub", ItemKind.Track, id, "Song " + id, "Band", null, ms, null, true, null,
            new TrackRef.Source(new SourceTrack("org.sub", id, "Song " + id, "Band", null, ms, null, null)), []);

    private sealed class TableLocalizer(Dictionary<string, string> table) : ILocalizer
    {
        public CultureInfo Culture => CultureInfo.InvariantCulture;

        public string Get(string key) => table.TryGetValue(key, out var value) ? value : key;
    }
}

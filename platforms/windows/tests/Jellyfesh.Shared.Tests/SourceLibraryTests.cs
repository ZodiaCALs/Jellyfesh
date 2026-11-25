using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>The sources' likes and playlists on Favourites and Playlists (ADR-0024 §2 and §3).</summary>
public class SourceLibraryTests
{
    private const long Now = 1_800_000_000_000;

    [Fact]
    public void The_page_refreshes_the_lists_when_it_opens_and_again_after_ten_minutes()
    {
        using var h = new AppHarness(English.Instance).Started();
        var now = Now;
        var sections = new SourceLibrarySections(h.S, SourceLibraryPage.Playlists, () => now);
        sections.Activate();
        Assert.Equal(new CoreCommand.RefreshSourceLibrary(null), h.Core.Sent[^1]);
        var sent = h.Core.Sent.Count;

        now += 9 * 60_000;
        sections.Activate();
        Assert.Equal(sent, h.Core.Sent.Count); // nothing polls, and nine minutes is not old

        now += 60_000;
        sections.Activate();
        Assert.Equal(new CoreCommand.RefreshSourceLibrary(null), h.Core.Sent[^1]);
    }

    [Fact]
    public void Favourites_show_each_sources_likes_and_Playlists_its_playlists()
    {
        using var h = new AppHarness(English.Instance).Started();
        var likes = new SourceLibrarySections(h.S, SourceLibraryPage.Favourites, () => Now);
        var playlists = new SourceLibrarySections(h.S, SourceLibraryPage.Playlists, () => Now);
        var state = new SourceLibraryState(3, [
            new SourceLibrary("org.spotify", "Spotify", "Spotify", [
                List(SourceLibraryPart.LikedSongs, [Song("org.spotify", "s1"), Song("org.spotify", "s2")], refreshed: Now - 5 * 60_000),
                List(SourceLibraryPart.LikedAlbums, [Album("org.spotify", "a1")], refreshed: Now - 5 * 60_000),
                List(SourceLibraryPart.Playlists, [Playlist("org.spotify", "p1", listed: false)], refreshed: Now - 20_000),
            ]),
            new SourceLibrary("org.jelly", "Jelly", null, [
                List(SourceLibraryPart.LikedSongs, [], loading: true),
                List(SourceLibraryPart.LikedArtists, []), // nothing, not loading: left out
            ]),
            new SourceLibrary("org.quiet", "Quiet", null, []), // no lists yet: no section
        ]);
        likes.Update(state);
        playlists.Update(state);

        Assert.Equal(["Spotify", "Jelly"], likes.Sections.Select(s => s.Name));
        var spotify = likes.Sections[0];
        Assert.Equal("Content from Spotify", spotify.Attribution);
        Assert.Equal("Updated 5 minutes ago", spotify.UpdatedText);
        Assert.Equal([SourceLibraryPart.LikedSongs, SourceLibraryPart.LikedAlbums], spotify.Lists.Select(l => l.Part));
        Assert.Equal(["Songs", "Albums"], spotify.Lists.Select(l => l.Title));
        Assert.All(spotify.Lists, l => Assert.True(l.ShowsTitle)); // titled: there are several
        Assert.Equal((2, 1), (spotify.Lists[0].Tracks.Count, spotify.Lists[1].Entries.Count));

        var jelly = likes.Sections[1];
        Assert.False(jelly.HasAttribution);
        Assert.False(jelly.HasUpdated);
        var loading = Assert.Single(jelly.Lists);
        Assert.True(loading.IsLoading); // "Loading…"
        Assert.False(loading.ShowsTitle);

        var only = Assert.Single(playlists.Sections);
        Assert.Equal("Updated just now", only.UpdatedText);
        var list = Assert.Single(only.Lists);
        Assert.False(Assert.Single(list.Entries).SongsListed);
        Assert.True(likes.HasSections && playlists.HasSections);
    }

    [Fact]
    public void Show_More_adds_a_page_which_stays_until_the_list_is_loaded_again()
    {
        using var h = new AppHarness(English.Instance).Started();
        var sections = new SourceLibrarySections(h.S, SourceLibraryPage.Playlists, () => Now);
        var first = List(SourceLibraryPart.Playlists, [Playlist("org.jelly", "p1")], refreshed: Now - 60_000, next: "c2");
        sections.Update(State(first));
        var list = sections.Sections[0].Lists[0];
        Assert.True(list.HasMore);

        list.ShowMoreCommand.Execute(null);
        Assert.Equal(new CoreCommand.SourceLibraryMore("org.jelly", SourceLibraryPart.Playlists, "c2"), h.Core.Sent[^1]);
        Assert.Equal(("Loading…", false), (list.ShowMoreLabel, list.ShowMoreCommand.CanExecute(null)));
        sections.OnNotification(new CoreEvent.SourceLibraryPage(new SourceLibraryMorePage(
            (ulong)h.Core.Sent.Count, "org.jelly", SourceLibraryPart.Playlists, [new SourceLibraryEntry(Playlist("org.jelly", "p2"), true, true)], null, null)));
        Assert.Equal(["p1", "p2"], list.Entries.Select(e => e.Item.Id));
        Assert.False(list.HasMore);

        // The same list again (a slice for another reason) keeps the added page; loaded again, it starts over.
        sections.Update(State(first));
        Assert.Equal(2, list.Entries.Count);
        sections.Update(State(first with { RefreshedAtMs = Now }));
        Assert.Equal(["p1"], list.Entries.Select(e => e.Item.Id));
        Assert.Equal("c2", list.Next);
    }

    [Fact]
    public void Show_More_that_the_core_cannot_do_yet_says_so_under_the_list()
    {
        using var h = new AppHarness(English.Instance).Started();
        var sections = new SourceLibrarySections(h.S, SourceLibraryPage.Favourites, () => Now);
        sections.Update(State(List(SourceLibraryPart.LikedSongs, [Song("org.jelly", "s1")], refreshed: Now, next: "c2")));
        var list = sections.Sections[0].Lists[0];
        list.ShowMoreCommand.Execute(null);
        sections.OnNotification(new CoreEvent.CommandFailed((ulong)h.Core.Sent.Count, "not available in this build yet"));
        Assert.Equal(("not available in this build yet", false), (list.MoreError, list.IsLoadingMore));
        Assert.True(list.ShowMoreCommand.CanExecute(null));
        Assert.Empty(h.App.Notifications.Items);
    }

    [Fact]
    public void A_liked_song_plays_with_its_list_and_a_playlist_opens_its_page()
    {
        using var h = new AppHarness(English.Instance).Started();
        var sections = new SourceLibrarySections(h.S, SourceLibraryPage.Favourites, () => Now);
        sections.Update(new SourceLibraryState(1, [new SourceLibrary("org.jelly", "Jelly", null, [
            List(SourceLibraryPart.LikedSongs, [Song("org.jelly", "s1"), Song("org.jelly", "s2"), Song("org.jelly", "s3")], refreshed: Now),
        ])]));
        var songs = sections.Sections[0].Lists[0].Tracks;
        sections.PlayTrack(songs[1]);
        var play = Assert.IsType<CoreCommand.PlayTracks>(h.Core.Sent[^1]);
        Assert.Equal((3, 1u), (play.Tracks.Count, play.Start));

        var playlists = new SourceLibrarySections(h.S, SourceLibraryPage.Playlists, () => Now);
        playlists.Update(State(List(SourceLibraryPart.Playlists, [Playlist("org.jelly", "p9", listed: false)], refreshed: Now)));
        SourceTarget? opened = null;
        playlists.OpenRequested += t => opened = t;
        playlists.Open(playlists.Sections[0].Lists[0].Entries[0]);
        Assert.Equal(("org.jelly", "p9", ItemKind.Playlist, false), (opened!.Plugin, opened.Node, opened.Kind, opened.SongsListed));
    }

    [Fact]
    public void The_pages_show_the_sources_after_their_own_rows_and_follow_the_slice()
    {
        using var h = new AppHarness(English.Instance).Started();
        var page = h.App.CreatePlaylists();
        page.Activate();
        h.Ui.RunAll();
        Assert.Contains(new CoreCommand.RefreshSourceLibrary(null), h.Core.Sent);
        Assert.False(page.Sources.HasSections);

        h.Emit(new CoreEvent.SourceLibrary(State(List(SourceLibraryPart.Playlists, [Playlist("org.jelly", "p1")], refreshed: Now))));
        Assert.True(page.Sources.HasSections);
        page.Deactivate();

        var favourites = h.App.CreateFavorites();
        favourites.Activate();
        h.Ui.RunAll();
        Assert.False(favourites.Sources.HasSections); // that slice has no likes
        favourites.Deactivate();
    }

    private static SourceLibraryState State(SourceLibraryList list) =>
        new(1, [new SourceLibrary("org.jelly", "Jelly", null, [list])]);

    /// <summary>The subtitle the test gives a playlist whose songs the service lists only in its own app.</summary>
    private const string Unlisted = "(songs listed only in the service)";

    private static SourceLibraryList List(SourceLibraryPart part, IReadOnlyList<SourceItem> items, long? refreshed = null, bool loading = false, string? next = null) =>
        new(part, items.Select(i => new SourceLibraryEntry(i, true, i.Subtitle != Unlisted)).ToList(), (uint)items.Count, next, refreshed, loading, null);

    private static SourceItem Song(string source, string id) =>
        new(source, ItemKind.Track, id, "Song " + id, "Band", null, 180_000, null, true, null,
            new TrackRef.Source(new SourceTrack(source, id, "Song " + id, "Band", null, 180_000, null, null)), []);

    private static SourceItem Album(string source, string id) =>
        new(source, ItemKind.Album, id, "Album " + id, "Band", null, null, null, true, null, null, []);

    private static SourceItem Playlist(string source, string id, bool listed = true) =>
        new(source, ItemKind.Playlist, id, "Playlist " + id, listed ? null : Unlisted, null, null, null, true, null, null, []);
}

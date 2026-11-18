using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>
/// The shared UI specification (docs/ui.md) as the Windows view models carry it out: the pages'
/// content and order, the steps that ask first, and the sign-in buttons. The key localizer returns
/// keys, so the strings are checked by key.
/// </summary>
public class UiSpecTests
{
    [Fact]
    public async Task Home_shows_the_library_size_and_recently_added_albums()
    {
        using var h = new AppHarness();
        h.Core.StatsValue = new LibraryStats(1, 1_204, 96, 41, 0, 0, 0);
        h.Core.AlbumRows.Add(new Album(7, "Tides", "Band", 2024, 9, null));
        h.Core.TrackRows.Add(Samples.Track(1, "Harbour Lights"));
        h.Core.FavoriteRows.Add(new TrackRow(new TrackRef.Library(1), "Harbour Lights", "Band", "Tides", 200_000, null, null, false, false));
        h.Started();
        var home = h.App.CreateHome();
        await home.LoadAsync();

        Assert.Equal("Home_LibrarySize", home.LibrarySize);
        Assert.Equal((AlbumSort.Added, true), h.Core.LastAlbumSort); // newest albums first, from the core
        Assert.Equal(["Tides"], home.RecentlyAdded.Select(a => a.Title));
        Assert.Equal(["Harbour Lights"], home.RecentlyPlayed.Select(t => t.Title));
        Assert.Equal(["Harbour Lights"], home.MostPlayed.Select(t => t.Title));
        Assert.False(home.ShowWelcome);
    }

    [Fact]
    public async Task Home_welcomes_until_there_is_music_or_a_signed_in_source()
    {
        using var h = new AppHarness();
        var source = Plugin("org.jellyfesh.subsonic", account: null);
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [source], [], false) };
        h.Started();
        var home = h.App.CreateHome();
        home.Activate();
        await home.LoadAsync();
        Assert.True(home.ShowWelcome);
        Assert.Equal(string.Empty, home.LibrarySize);

        // Add Music Folder… adds the folder directly.
        await home.AddFolderCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.AddRoot("/music/new"), h.Core.Sent[^1]);

        // Signing in to a source ends the welcome, with the library still empty.
        h.Emit(new CoreEvent.Sources(new SourcesState(2, [source with { Account = "ana" }], [], false)));
        Assert.False(home.ShowWelcome);
        home.Deactivate();
    }

    [Fact]
    public async Task Albums_sort_by_the_menu_with_descending()
    {
        using var h = new AppHarness();
        h.Core.AlbumRows.Add(new Album(1, "Tides", "Band", 2024, 9, null));
        h.Core.StatsValue = new LibraryStats(1, 9, 1, 1, 0, 0, 0);
        h.Started();
        var albums = h.App.CreateAlbums();
        Assert.Equal([AlbumSort.Title, AlbumSort.Artist, AlbumSort.Year, AlbumSort.Added], albums.Sorts);
        await albums.LoadAsync();
        h.Ui.RunUntil(() => h.Core.LastAlbumSort is not null);
        Assert.Equal((AlbumSort.Title, false), h.Core.LastAlbumSort);

        // Recently Added starts newest first; Descending turns it round.
        albums.Sort = AlbumSort.Added;
        Assert.True(albums.Descending);
        await albums.LoadAsync();
        h.Ui.RunUntil(() => h.Core.LastAlbumSort == (AlbumSort.Added, true));
        albums.Descending = false;
        await albums.LoadAsync();
        h.Ui.RunUntil(() => h.Core.LastAlbumSort == (AlbumSort.Added, false));
        albums.Sort = AlbumSort.Artist;
        Assert.False(albums.Descending);

        await albums.AddFolderCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.AddRoot("/music/new"), h.Core.Sent[^1]);
    }

    [Fact]
    public async Task Songs_play_and_shuffle_in_the_chosen_order()
    {
        using var h = new AppHarness();
        h.Core.TrackRows.Add(Samples.Track(1, "First"));
        h.Core.TrackRows.Add(Samples.Track(2, "Second"));
        h.Core.StatsValue = new LibraryStats(1, 2, 1, 1, 0, 0, 0);
        h.Started();
        var songs = h.App.CreateSongs();
        Assert.Equal([TrackSort.Title, TrackSort.Artist, TrackSort.Album, TrackSort.Year, TrackSort.Added], songs.Sorts);
        Assert.Equal(TrackSort.Title, songs.Sort);

        songs.Sort = TrackSort.Year;
        Assert.True(songs.Descending);
        await songs.PlayCommand.ExecuteAsync(null);
        Assert.Contains(new TrackQuery(0, SongsViewModel.WholeList, TrackSort.Year, true), Queries(h));
        var play = Assert.IsType<CoreCommand.PlayTracks>(h.Core.Sent[^1]);
        Assert.Equal([new TrackRef.Library(1), new TrackRef.Library(2)], play.Tracks);
        Assert.Equal(0u, play.Start);

        songs.Descending = false;
        await songs.ShuffleCommand.ExecuteAsync(null);
        Assert.Contains(new TrackQuery(0, SongsViewModel.WholeList, TrackSort.Year, false), Queries(h));
        var shuffle = Assert.IsType<CoreCommand.PlayCollection>(h.Core.Sent[^1]);
        Assert.True(shuffle.Shuffle);
        Assert.Equal(2, Assert.IsType<Collection.Tracks>(shuffle.Collection).Items.Count);
    }

    [Fact]
    public async Task Artist_and_playlist_pages_offer_the_more_menu()
    {
        using var h = new AppHarness();
        h.Core.TrackRows.Add(Samples.Track(1, "First"));
        var row = new TrackRow(new TrackRef.Library(1), "First", "Artist", "Album", 200_000, null, null, false, false);
        h.Core.PlaylistRows.Add(new PlaylistSummary(5, "Road trip", 1, 200_000, 0, null));
        h.Core.PlaylistEntryRows[5] = [new PlaylistEntry(50, 0, row)];
        h.Started();

        var artist = h.App.CreateArtist(new Artist(3, "Artist", 1));
        artist.PlayNextCommand.Execute(null);
        Assert.Equal(new CoreCommand.EnqueueCollection(new Collection.Artist(3), true), h.Core.Sent[^1]);
        artist.PlayLastCommand.Execute(null);
        Assert.Equal(new CoreCommand.EnqueueCollection(new Collection.Artist(3), false), h.Core.Sent[^1]);
        Assert.Equal([new TrackRef.Library(1)], await artist.TracksAsync());

        var playlist = h.App.CreatePlaylist(h.Core.PlaylistRows[0]);
        await playlist.LoadAsync();
        playlist.PlayNextCommand.Execute(null);
        Assert.Equal(new CoreCommand.EnqueueCollection(new Collection.Playlist(5), true), h.Core.Sent[^1]);
        Assert.Equal([new TrackRef.Library(1)], await playlist.TracksAsync());

        // Add to Playlist… → New Playlist… asks for the name first, then makes it with the songs.
        await PlaylistActions.CreateAsync(h.S, await playlist.TracksAsync());
        var created = Assert.IsType<CoreCommand.CreatePlaylist>(h.Core.Sent[^1]);
        Assert.Equal("Road trip", created.Name);
        Assert.Equal([new TrackRef.Library(1)], created.Tracks);
        h.Platform.Prompt = null;
        var sent = h.Core.Sent.Count;
        await PlaylistActions.CreateAsync(h.S, []);
        Assert.Equal(sent, h.Core.Sent.Count); // cancelled: nothing is made

        // Delete Playlist… on the playlist page asks first, then the page goes back.
        var deleted = 0;
        playlist.Deleted += () => deleted++;
        h.Platform.Confirm = false;
        await playlist.DeleteCommand.ExecuteAsync(null);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.DeletePlaylist);
        Assert.Equal(0, deleted);
        h.Platform.Confirm = true;
        await playlist.DeleteCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.DeletePlaylist(5), h.Core.Sent[^1]);
        Assert.Equal(("Playlist_DeleteTitle", "Playlist_DeleteBody", "Playlist_DeleteConfirm"), h.Platform.Asked[^1]);
        Assert.Equal(1, deleted);
    }

    [Fact]
    public async Task Clear_queue_asks_first()
    {
        using var h = new AppHarness().Started();
        var queue = h.App.Queue;
        await queue.ClearCommand.ExecuteAsync(null);
        Assert.Empty(h.Platform.Asked); // nothing to clear

        h.Emit(new CoreEvent.Queue(QueueState.Initial with { Version = 1, Length = 1, Position = 0, Current = new QueueItem(1, new TrackRef.File("/music/a.flac")) }));
        h.Platform.Confirm = false;
        await queue.ClearCommand.ExecuteAsync(null);
        Assert.Equal(("Queue_ClearTitle", "Queue_ClearBody", "Queue_ClearConfirm"), h.Platform.Asked[^1]);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.ClearQueue);
        h.Platform.Confirm = true;
        await queue.ClearCommand.ExecuteAsync(null);
        Assert.IsType<CoreCommand.ClearQueue>(h.Core.Sent[^1]);
    }

    [Fact]
    public async Task About_this_song_shows_year_length_lyrics_and_file()
    {
        using var h = new AppHarness();
        h.Core.TrackRows.Add(Samples.Track(3, "Sample Song"));
        h.Started();
        var p = h.App.Player;
        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(7, new TrackRef.Library(3), "/music/3.flac", "Sample Song", "Artist", "Album", 232_000, null, null)));
        await Poll(() => p.Year.Length > 0);
        Assert.Equal("2024", p.Year);
        Assert.Equal("3:52", p.LengthText);
        Assert.Equal("/music/3.flac", p.FilePath);
        Assert.Equal(string.Empty, p.LyricsText); // not looked up yet

        var doc = new LyricDocument(null, LyricTiming.Word, 0, [], [], "lrc", null);
        h.Emit(new CoreEvent.Lyrics(new LyricsState(7, doc, null, new LyricsOrigin.Sidecar("/music/3.lrc"))));
        Assert.Equal("Lyrics_InfoFrom", p.LyricsText); // "Word by word · 3.lrc"
        h.Emit(new CoreEvent.Lyrics(new LyricsState(7, doc with { Timing = LyricTiming.Unsynced }, null, null)));
        Assert.Equal("Lyrics_InfoUnsynced", p.LyricsText);
        h.Emit(new CoreEvent.Lyrics(new LyricsState(7, null, null, null)));
        Assert.Equal("Lyrics_InfoNone", p.LyricsText);

        // A song from a source has no year or file here.
        var source = new SourceTrack("org.sub", "s1", "Online", "Someone", null, 61_000, null, null);
        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(8, new TrackRef.Source(source), null, "Online", "Someone", null, 61_000, null, null)));
        Assert.Equal(string.Empty, p.Year);
        Assert.Equal(string.Empty, p.FilePath);
        Assert.Equal("1:01", p.LengthText);
    }

    [Fact]
    public async Task Song_menu_finds_the_album_the_artist_and_the_file()
    {
        using var h = new AppHarness();
        h.Core.TrackRows.Add(Samples.Track(1, "First", albumId: 7) with { Album = "Tides", Artist = "Band" });
        h.Core.AlbumRows.Add(new Album(7, "Tides", "Band", 2024, 9, null));
        h.Core.ArtistRows.Add(new Artist(4, "band", 9));
        h.Core.ArtistRows.Add(new Artist(3, "Band", 9));
        h.Started();
        var song = new TrackItem(0);
        song.Load(h.Core.TrackRows[0]);

        Assert.True(TrackPlaces.HasPages(song));
        Assert.Equal(7, (await TrackPlaces.AlbumAsync(h.S, song))?.Id);
        Assert.Equal(3, (await TrackPlaces.ArtistAsync(h.S, song))?.Id); // the exact name before one that differs in case
        Assert.Equal("/music/1.flac", await TrackPlaces.FileAsync(h.S, song));
        await TrackPlaces.ShowInExplorerAsync(h.S, song);
        Assert.Equal(["/music/1.flac"], h.Platform.Shown);

        var online = new TrackItem(1);
        online.Load(new SourceItem("org.sub", ItemKind.Track, "s1", "Online", null, null, null, null, true, null, new TrackRef.Source(new SourceTrack("org.sub", "s1", "Online", null, null, null, null, null)), []));
        Assert.False(TrackPlaces.HasPages(online));
        Assert.False(TrackPlaces.HasFile(online));
        Assert.Null(await TrackPlaces.AlbumAsync(h.S, online));
        Assert.Null(await TrackPlaces.FileAsync(h.S, online));
    }

    [Fact]
    public void Installed_sources_offer_a_sign_in_button_for_each_method()
    {
        using var h = new AppHarness();
        var plugin = Plugin("org.jellyfesh.jellyfin", account: null) with { AuthMethods = [AuthMethod.Qr, AuthMethod.PasswordExchange, AuthMethod.ApiKey] };
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [plugin], [], false) };
        h.Started();
        var settings = h.App.CreateSettings(_ => null);
        var vm = Assert.Single(settings.Plugins);
        Assert.Equal(["SignIn_MethodQr", "SignIn_MethodPassword", "SignIn_MethodApiKey"], vm.SignInActions.Select(a => a.Title));
        Assert.Equal("Source_NotSignedIn", vm.Status);
        Assert.Equal("Source_Official", vm.BadgeLabel);
        Assert.Equal("Source_Details", vm.Details);
        Assert.True(vm.CanSignIn);

        SignInViewModel? flow = null;
        settings.SignInRequested += f => flow = f;
        vm.SignInActions[1].Command.Execute(null); // not always the first method
        Assert.Equal(new CoreCommand.BeginLogin("org.jellyfesh.jellyfin", AuthMethod.PasswordExchange), h.Core.Sent[^1]);
        Assert.Equal(AuthMethod.PasswordExchange, flow?.Method);
        vm.SignInActions[2].Command.Execute(null);
        Assert.Equal(new CoreCommand.BeginLogin("org.jellyfesh.jellyfin", AuthMethod.ApiKey), h.Core.Sent[^1]);

        // A method the source does not offer is never started.
        var sent = h.Core.Sent.Count;
        vm.SignInCommand.Execute(AuthMethod.CookieImport);
        Assert.Equal(sent, h.Core.Sent.Count);
    }

    [Fact]
    public async Task Uninstall_asks_first_and_the_status_names_the_account()
    {
        using var h = new AppHarness();
        var plugin = Plugin("org.x", account: "ana") with { Badge = PluginBadge.Community };
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [plugin], [], false) };
        h.Started();
        var settings = h.App.CreateSettings(_ => null);
        var vm = Assert.Single(settings.Plugins);
        Assert.Equal("Source_SignedInAs", vm.Status);
        Assert.Equal("Source_Community", vm.BadgeLabel);
        Assert.False(vm.CanSignIn);

        h.Platform.Confirm = false;
        await vm.UninstallCommand.ExecuteAsync(null);
        Assert.Equal(("Source_UninstallTitle", "Source_UninstallBody", "Action_Uninstall"), h.Platform.Asked[^1]);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.UninstallPlugin);
        h.Platform.Confirm = true;
        await vm.UninstallCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.UninstallPlugin("org.x"), h.Core.Sent[^1]);
    }

    [Fact]
    public void Sources_page_lists_add_a_source_and_your_sources_then_opens_one()
    {
        using var h = new AppHarness();
        var subsonic = Plugin("org.jellyfesh.subsonic", account: "ana") with { Name = "Subsonic", Enabled = true };
        var off = Plugin("org.jellyfesh.jellyfin", account: null) with { Name = "Jellyfin", Enabled = false };
        var navidrome = new SourcePreset("navidrome", "Navidrome", "org.jellyfesh.subsonic", [AuthMethod.PasswordExchange], true, null, "http://192.168.1.20:4533", null);
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [subsonic, off], [navidrome], false) };
        h.Started();
        var sources = h.App.CreateSources();
        sources.Activate();
        var opened = new List<SourceTarget>();
        sources.BrowseRequested += opened.Add;

        Assert.Equal(["Navidrome"], sources.Presets.Select(p => p.Name));
        Assert.Equal(["Subsonic", "Jellyfin"], sources.Choices.Select(c => c.Name)); // every installed source, on or off
        Assert.Equal(["Source_SignedInAs", "Source_StatusOff"], sources.Choices.Select(c => c.Status));

        // A source that is off is not opened.
        sources.OpenCommand.Execute(sources.Choices[1]);
        Assert.Empty(opened);

        // Choosing a source opens its catalogue on a page of its own, at its top level.
        sources.OpenCommand.Execute(sources.Choices[0]);
        var root = Assert.Single(opened);
        Assert.Equal(new SourceTarget("org.jellyfesh.subsonic", null, "Subsonic", null), root);
        sources.Deactivate();

        var page = h.App.CreateSourceBrowse(root);
        page.Activate();
        Assert.Equal(new CoreCommand.Browse("org.jellyfesh.subsonic", null, 0), h.Core.Sent[^1]);
        Assert.Equal(string.Empty, page.KindLabel);
        Assert.False(page.IsCollection);
        var request = (ulong)h.Core.Sent.Count;
        var folder = new SourceItem("org.jellyfesh.subsonic", ItemKind.Folder, "f1", "Jazz", null, null, null, null, false, null, null, []);
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate(request, "org.jellyfesh.subsonic", null, [folder], null, null)));
        Assert.True(page.IsComplete);
        Assert.False(page.CanPlay); // a top level is no collection

        // A folder opens its own page (Back is the frame's).
        var next = new List<SourceTarget>();
        page.OpenRequested += next.Add;
        page.OpenCommand.Execute(page.Rows[0]);
        Assert.Equal("f1", Assert.Single(next).Node);
        Assert.Equal(ItemKind.Folder, next[0].Kind);
        page.Deactivate();
    }

    [Fact]
    public void Sources_page_connects_a_service_from_add_a_source()
    {
        using var h = new AppHarness();
        var subsonic = Plugin("org.jellyfesh.subsonic", account: null) with { Name = "Subsonic" };
        var navidrome = new SourcePreset("navidrome", "Navidrome", "org.jellyfesh.subsonic", [AuthMethod.PasswordExchange], true, null, "http://192.168.1.20:4533", null);
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [subsonic], [navidrome], false) };
        h.Started();
        var sources = h.App.CreateSources();
        sources.Activate();
        SignInViewModel? flow = null;
        sources.SignInRequested += f => flow = f;

        sources.Connect(sources.Presets[0], "http://192.168.1.20:4533", AuthMethod.PasswordExchange);
        Assert.Equal(new CoreCommand.ConnectSource("navidrome", "http://192.168.1.20:4533", AuthMethod.PasswordExchange), h.Core.Sent[^1]);
        Assert.Equal("Navidrome", flow?.Name);
        sources.Deactivate();
    }

    private static TrackQuery[] Queries(AppHarness h)
    {
        lock (h.Core.TrackQueries)
        {
            return [.. h.Core.TrackQueries];
        }
    }

    private static PluginInfo Plugin(string id, string? account) =>
        new(id, "Sub", "1.0.0", "Someone", PluginBadge.Official, false, ["search", "browse"], [AuthMethod.Qr, AuthMethod.PasswordExchange], true, account, false, "{}", "{}", null);

    private static async Task Poll(Func<bool> condition)
    {
        for (var i = 0; i < 500 && !condition(); i++)
        {
            await Task.Delay(5);
        }

        Assert.True(condition());
    }
}

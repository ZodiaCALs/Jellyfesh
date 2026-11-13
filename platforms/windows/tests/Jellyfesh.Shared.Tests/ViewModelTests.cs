using Jellyfesh.Collections;
using Jellyfesh.Core;
using Jellyfesh.Services;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

public class ViewModelTests
{
    [Fact]
    public void Player_mirrors_slices_and_only_sends_commands()
    {
        using var h = new AppHarness().Started();
        var p = h.App.Player;
        p.Progress.IsActive = true; // the view sets this while the seek bar is visible
        h.Emit(
            new CoreEvent.NowPlaying(new NowPlayingInfo(7, new TrackRef.Library(3), "/m/3.flac", "Sample Song", "Placeholder Artist", "Sample Album", 232_000, new ArtworkRef.File("/cache/a.jpg"), new TrackFormat("flac", 44_100, 2, 232_000_000, true, null))),
            new CoreEvent.Playback(Samples.Playing(84_000_000, h.Clock.Now, durationUs: 232_000_000) with { Volume = 0.64f }),
            new CoreEvent.Queue(QueueState.Initial with { Version = 1, Length = 3, Position = 0, Shuffle = true, Repeat = RepeatMode.All, UpNext = [new QueueItem(8, new TrackRef.Library(4))] }));

        Assert.True(p.HasTrack);
        Assert.Equal("Sample Song", p.Title);
        Assert.Equal("Player_ArtistAlbum", p.Subtitle); // the key localizer returns keys
        Assert.True(p.IsPlaying);
        Assert.Equal(64, p.Volume);
        Assert.True(p.Shuffle);
        Assert.Equal(RepeatMode.All, p.Repeat);
        Assert.True(p.CanGoNext);
        Assert.Equal("Format_Gapless", p.FormatText);
        Assert.StartsWith("file://", p.ArtworkUri, StringComparison.Ordinal);
        Assert.Equal("1:24", p.Progress.PositionText);
        Assert.NotNull(h.Timer.Pending); // playing: the next second is scheduled

        p.PlayPauseCommand.Execute(null);
        p.CycleRepeatCommand.Execute(null);
        p.ToggleShuffleCommand.Execute(null);
        Assert.IsType<CoreCommand.TogglePlayPause>(h.Core.Sent[^3]);
        Assert.Equal(new CoreCommand.SetRepeat(RepeatMode.One), h.Core.Sent[^2]);
        Assert.Equal(new CoreCommand.SetShuffle(false), h.Core.Sent[^1]);
        Assert.True(p.Shuffle); // unchanged until the slice says so
    }

    [Fact]
    public void Volume_echoes_do_not_move_the_slider_back()
    {
        using var h = new AppHarness().Started();
        var p = h.App.Player;
        p.SetVolumeFromUser(30);
        Assert.Equal(new CoreCommand.SetVolume(0.3f), h.Core.Sent[^1]);
        h.Emit(new CoreEvent.Playback(Samples.Paused(0, 0) with { Volume = 0.8f })); // an older value arriving late
        Assert.Equal(30, p.Volume);
    }

    [Fact]
    public void Seeking_clamps_to_the_track()
    {
        using var h = new AppHarness().Started();
        var p = h.App.Player;
        h.Emit(new CoreEvent.Playback(Samples.Paused(10_000_000, h.Clock.Now, durationUs: 100_000_000)));
        p.SeekToFraction(0.5);
        Assert.Equal(new CoreCommand.Seek(50_000), h.Core.Sent[^1]);
        p.SeekBy(-30);
        Assert.Equal(new CoreCommand.Seek(0), h.Core.Sent[^1]);
        p.SeekToUs(500_000_000);
        Assert.Equal(new CoreCommand.Seek(100_000), h.Core.Sent[^1]);
    }

    [Fact]
    public void Devices_offer_the_system_default_first_and_select_by_id()
    {
        using var h = new AppHarness().Started();
        var p = h.App.Player;
        h.Emit(new CoreEvent.Devices(new DevicesState(2, [new DeviceInfo("wasapi:1", "Speakers", true, [48_000]), new DeviceInfo("wasapi:2", "Headphones", false, [44_100])], "wasapi:2")));
        Assert.Equal(3, p.Devices.Count);
        Assert.Null(p.Devices[0].Id);
        Assert.Equal("Headphones", p.SelectedDevice!.Name);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.SetOutputDevice); // syncing is not a choice
        p.SelectedDevice = p.Devices[0];
        Assert.Equal(new CoreCommand.SetOutputDevice(null), h.Core.Sent[^1]);
    }

    [Fact]
    public async Task Queue_resolves_titles_and_moves_by_playback_order()
    {
        using var h = new AppHarness();
        h.Core.TrackRows.Add(Samples.Track(4, "Second"));
        h.Core.TrackRows.Add(Samples.Track(5, "Third"));
        h.Started();
        var q = h.App.Queue;
        h.Emit(new CoreEvent.Queue(QueueState.Initial with
        {
            Version = 2,
            Length = 4,
            Position = 1,
            Current = new QueueItem(10, new TrackRef.File("/music/First Song.mp3")),
            UpNext = [new QueueItem(11, new TrackRef.Library(4)), new QueueItem(12, new TrackRef.Library(5))],
        }));
        await Poll(() => q.UpNext.All(i => i.Title.Length > 0));
        Assert.Equal("First Song", q.Current!.Title);
        Assert.Equal(new[] { "Second", "Third" }, q.UpNext.Select(i => i.Title));

        var third = q.UpNext[1];
        q.UpNext.Move(1, 0); // what a drag in the list does
        q.CommitDrag(third);
        Assert.Equal(new CoreCommand.MoveInQueue(12, 2), h.Core.Sent[^1]); // after the current entry (index 1)

        // The next slice reorders the same row objects rather than rebuilding them.
        var before = q.UpNext.ToList();
        h.Emit(new CoreEvent.Queue(QueueState.Initial with
        {
            Version = 3,
            Length = 4,
            Position = 1,
            Current = new QueueItem(10, new TrackRef.File("/music/First Song.mp3")),
            UpNext = [new QueueItem(12, new TrackRef.Library(5)), new QueueItem(11, new TrackRef.Library(4))],
        }));
        Assert.Same(before[0], q.UpNext[0]);
        Assert.Same(before[1], q.UpNext[1]);
    }

    [Fact]
    public async Task Albums_page_in_as_rows_are_realised()
    {
        using var h = new AppHarness();
        for (var i = 0; i < 250; i++)
        {
            h.Core.AlbumRows.Add(new Album(i, $"Album {i}", "Artist", 2000 + (i % 20), 10, null));
        }

        h.Core.StatsValue = new LibraryStats(1, 2_500, 250, 1, 0, 0, 0);
        h.Started();
        var albums = h.App.CreateAlbums();
        await albums.LoadAsync();
        var list = albums.Albums!;
        Assert.Equal(250, list.Items.Count); // every placeholder at once
        h.Ui.RunUntil(() => list.Items[0].IsLoaded);
        Assert.Equal("Album 0", list.Items[0].Title);
        Assert.False(list.Items[150].IsLoaded);
        list.Realize(150);
        h.Ui.RunUntil(() => list.Items[150].IsLoaded);
        Assert.Equal("Album 150", list.Items[150].Title);
        Assert.False(list.Items[249].IsLoaded); // page 3 is untouched

        list.Items[3].IsLoaded.ToString();
        albums.PlayCommand.Execute(list.Items[3]);
        Assert.Equal(new CoreCommand.PlayCollection(new Collection.Album(3), 0, false), h.Core.Sent[^1]);
    }

    [Fact]
    public async Task Unknown_counts_grow_a_page_at_a_time()
    {
        var core = new FakeCore();
        for (var i = 0; i < 130; i++)
        {
            core.FavoriteRows.Add(new TrackRow(new TrackRef.Library(i), $"Song {i}", null, null, 1000, null, null, true, false));
        }

        var ui = new QueueDispatcher();
        var list = new PagedList<TrackItem, TrackRow>(null, i => new TrackItem(i), (o, l) => core.Favorites((uint)o, (uint)l), ui, pageSize: 50);
        list.Start();
        ui.RunUntil(() => list.Items.Count == 50);
        list.Realize(30); // near the end of what is loaded
        ui.RunUntil(() => list.Items.Count == 100);
        list.Realize(80);
        ui.RunUntil(() => list.Items.Count == 130);
        Assert.False(list.HasMore);
        await Task.CompletedTask;
    }

    [Fact]
    public async Task Features_not_in_this_build_show_as_not_available()
    {
        using var h = new AppHarness();
        h.Core.FailQueriesWith = new CoreQueryException(CoreQueryError.NotAvailable, "playlists are not available in this build yet");
        h.Started();
        var playlists = h.App.CreatePlaylists();
        await playlists.LoadAsync();
        Assert.True(playlists.IsNotAvailable);
        Assert.False(playlists.IsEmpty);
        Assert.Empty(h.App.Notifications.Items); // not an error
    }

    [Fact]
    public async Task Thumbnails_load_once_and_skip_rows_that_scrolled_away()
    {
        var core = new FakeCore();
        var thumbnails = new ThumbnailService(core, concurrency: 1);
        var first = await thumbnails.GetAsync("abc", 128, TestContext.Current.CancellationToken);
        var again = await thumbnails.GetAsync("abc", 128, TestContext.Current.CancellationToken);
        Assert.Equal("/cache/abc/128.jpg", first);
        Assert.Equal(first, again);
        Assert.Equal(1, core.QueryCount);
        Assert.Equal(256u, ThumbnailService.SizeFor(200));
        Assert.Equal(1024u, ThumbnailService.SizeFor(5000));

        core.FailQueriesWith = new CoreQueryException(CoreQueryError.NotAvailable, "artwork thumbnails are not available in this build yet");
        Assert.Null(await thumbnails.GetAsync("def", 64, TestContext.Current.CancellationToken));
        Assert.True(thumbnails.Unavailable);
        Assert.Null(await thumbnails.GetAsync("ghi", 64, TestContext.Current.CancellationToken));
        Assert.Equal(2, core.QueryCount); // no further calls once unavailable
    }

    [Fact]
    public async Task A_row_realised_before_its_page_loads_its_thumbnail_once_the_page_arrives()
    {
        var core = new FakeCore();
        var thumbnails = new ThumbnailService(core);
        var placeholder = new AlbumItem(0);
        placeholder.RequestArtwork(thumbnails, 120);
        Assert.Null(placeholder.ArtworkUri);
        Assert.Equal(0, core.QueryCount);

        placeholder.Load(new Album(7, "Tides", "Band", 2024, 9, "c0ffee"));
        await Poll(() => placeholder.ArtworkUri is not null);
        Assert.Equal(new Uri("/cache/c0ffee/128.jpg").AbsoluteUri, placeholder.ArtworkUri);

        // A recycled row forgets the wish, so a late page does not load for it.
        var recycled = new AlbumItem(1);
        recycled.RequestArtwork(thumbnails, 120);
        recycled.CancelArtwork();
        recycled.Load(new Album(8, "Shoals", "Band", 2023, 9, "beef"));
        await Task.Delay(50, TestContext.Current.CancellationToken);
        Assert.Null(recycled.ArtworkUri);
        Assert.Equal(1, core.QueryCount);
    }

    [Fact]
    public async Task Settings_send_modified_copies_and_confirm_the_community_disclosure()
    {
        using var h = new AppHarness();
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [Plugin("org.x", PluginBadge.Community, needsDisclosure: true)], false) };
        h.Started();
        var settings = h.App.CreateSettings(_ => null);
        Assert.Equal(11, settings.Icons.Count);
        Assert.Equal("automatic", settings.SelectedIcon!.Id);
        Assert.Empty(h.Core.Sent); // reading the slice sends nothing

        settings.ReduceEffects = true;
        Assert.True(h.Core.LastSent<CoreCommand.SetSettings>().Settings.ReduceEffects);
        settings.SelectedIcon = settings.Icons.First(i => i.Id == "lagoon");
        Assert.Equal("lagoon", h.Core.LastSent<CoreCommand.SetSettings>().Settings.AppIcon);

        settings.ApplyPresetCommand.Execute(EqPreset.All[1]);
        var eq = h.Core.LastSent<CoreCommand.SetEq>().Eq;
        Assert.True(eq.Enabled);
        Assert.Equal(6f, eq.Bands[0].GainDb);

        settings.CrossfadeSeconds = 40; // clamped to the core's range
        Assert.Equal(12_000u, h.Core.LastSent<CoreCommand.SetCrossfade>().Crossfade.DurationMs);

        h.Platform.Confirm = false;
        settings.AllowCommunity = true;
        await Poll(() => !settings.AllowCommunity);
        Assert.Single(h.Platform.Confirmations);
        Assert.DoesNotContain(h.Core.Sent.OfType<CoreCommand.SetSettings>(), s => s.Settings.AllowCommunityPlugins);

        h.Platform.Confirm = true;
        var plugin = settings.Plugins.Single();
        plugin.IsEnabled = true;
        await Poll(() => h.Core.Sent.OfType<CoreCommand.SetPluginEnabled>().Any());
        Assert.Contains(new CoreCommand.AcknowledgeUnofficial("org.x"), h.Core.Sent);
    }

    [Fact]
    public async Task Library_folders_and_rescans()
    {
        using var h = new AppHarness();
        h.Core.RootRows.Add("/music");
        h.Started();
        var settings = h.App.CreateSettings(_ => null);
        await settings.LoadAsync();
        Assert.Equal(new[] { "/music" }, settings.Folders);
        await settings.AddFolderCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.AddRoot("/music/new"), h.Core.Sent[^1]);
        settings.RescanCommand.Execute(null);
        Assert.IsType<CoreCommand.Rescan>(h.Core.Sent[^1]);
    }

    [Fact]
    public void Sign_in_shows_a_qr_code_then_fields_and_never_keeps_secrets()
    {
        using var h = new AppHarness().Started();
        var plugin = Plugin("org.sub", PluginBadge.Official, needsDisclosure: false);
        var flow = new SignInViewModel(h.S, plugin, AuthMethod.Qr);
        SignInViewModel? finished = null;
        flow.Finished += f => finished = f;
        flow.Begin();
        Assert.Equal(new CoreCommand.BeginLogin("org.sub", AuthMethod.Qr), h.Core.Sent[^1]);

        h.Emit(new CoreEvent.Auth(new AuthUpdate("org.sub", 5, new AuthStep.ShowQr("https://login.example/q?c=1", null))));
        Assert.Equal(SignInStage.ShowQr, flow.Stage);
        Assert.NotNull(flow.Qr);

        h.Emit(new CoreEvent.Auth(new AuthUpdate("org.sub", 5, new AuthStep.AskFields([new AuthField("user", "User", false, null), new AuthField("password", "Password", true, null)]))));
        flow.Fields[0].Value = "ana";
        flow.Fields[1].Value = "secret";
        flow.SubmitCommand.Execute(null);
        var submit = h.Core.LastSent<CoreCommand.SubmitLogin>();
        Assert.Equal(5ul, submit.Flow);
        Assert.Contains(new KeyValuePair<string, string>("password", "secret"), submit.Fields);
        Assert.Equal(string.Empty, flow.Fields[1].Value);

        h.Emit(new CoreEvent.Auth(new AuthUpdate("org.sub", 6, new AuthStep.Done("other flow")))); // not ours
        Assert.Null(finished);
        h.Emit(new CoreEvent.Auth(new AuthUpdate("org.sub", 5, new AuthStep.Done("ana@music.example"))));
        Assert.Same(flow, finished);
        Assert.Equal("ana@music.example", flow.Account);
    }

    [Fact]
    public async Task Search_answers_from_the_library_then_shows_the_merged_source_results()
    {
        using var h = new AppHarness();
        h.Core.TrackRows.Add(Samples.Track(1, "Harbour Lights"));
        var sub = Plugin("org.sub", PluginBadge.Official, false) with { Enabled = true };
        var jelly = Plugin("org.jelly", PluginBadge.Official, false) with { Name = "Jelly", Enabled = true };
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [sub, jelly], false) };
        h.Started();
        var search = h.App.CreateSearch();
        search.Activate();
        search.Query = "harbour";
        await search.SubmitCommand.ExecuteAsync(null);
        Assert.Single(search.Tracks);
        var request = h.Core.LastSent<CoreCommand.Search>();
        Assert.Equal("harbour", request.Query);
        Assert.True(search.IsSearchingSources);
        Assert.Equal(["Sub", "Jelly"], search.SourceStates.Select(s => s.Name));
        Assert.All(search.SourceStates, s => Assert.False(s.IsDone));
        var id = (ulong)h.Core.Sent.Count;

        // The library answers first; its rows stay in the library section.
        var library = new SourceItem("library", ItemKind.Track, "1", "Harbour Lights", "Band", null, 180_000, null, true, null,
            new TrackRef.Library(1), []);
        h.Emit(new CoreEvent.SearchResults(new SearchUpdate(id, "harbour", "library", [library], null, false)));
        Assert.Empty(search.OnlineTracks);
        Assert.False(search.HasOnlineResults);

        // Each source's update carries the whole merged list so far, so it replaces the last one.
        var live = Live("org.sub", []);
        h.Emit(new CoreEvent.SearchResults(new SearchUpdate(id, "harbour", "org.sub", [library, live], null, false)));
        var first = Assert.Single(search.OnlineTracks);
        Assert.Equal("Sub", first.Origin);
        Assert.True(search.SourceStates[0].IsDone);
        Assert.False(search.SourceStates[1].IsDone);

        var merged = Live("org.sub", ["org.jelly"]);
        var withLibrary = library with { AlsoIn = ["org.jelly"] };
        var album = new SourceItem("org.jelly", ItemKind.Album, "a7", "Harbour", "Band", null, null, null, true, null, null, []);
        h.Emit(new CoreEvent.SearchResults(new SearchUpdate(id, "harbour", "org.jelly", [withLibrary, merged, album], "slow", true)));
        var only = Assert.Single(search.OnlineTracks);
        Assert.Equal("Sub · Jelly", only.Origin);
        Assert.Single(search.OnlineEntries);
        Assert.Equal("Jelly", search.Tracks[0].Origin);
        Assert.Equal("slow", search.SourceStates[1].Error);
        Assert.False(search.IsSearchingSources);

        // A failure of the search command is the page's to show, not a generic error.
        search.Query = "tide";
        await search.SubmitCommand.ExecuteAsync(null);
        var next = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.SearchResults(new SearchUpdate(id, "harbour", "org.sub", [live], null, true))); // an old request
        Assert.Empty(search.OnlineTracks);
        h.Emit(new CoreEvent.CommandFailed(next, "sources are not available in this build yet"));
        Assert.True(search.SourcesNotAvailable);
        Assert.Empty(h.App.Notifications.Items);
        search.Deactivate();

        static SourceItem Live(string source, string[] alsoIn) =>
            new(source, ItemKind.Track, "t9", "Harbour (live)", "Band", null, 200_000, null, true, null,
                new TrackRef.Source(new SourceTrack(source, "t9", "Harbour (live)", "Band", null, 200_000, null, null)), alsoIn);
    }

    [Fact]
    public void Notifications_show_failures_and_scan_results_once()
    {
        using var h = new AppHarness().Started();
        h.Emit(new CoreEvent.CommandFailed(42, "nothing to play"), new CoreEvent.CommandFailed(42, "nothing to play"));
        Assert.Single(h.App.Notifications.Items);
        h.Emit(new CoreEvent.Scan(new ScanStatus.Finished(new ScanReport(3, 1, 0, 9, []))));
        Assert.Equal(NotificationSeverity.Success, h.App.Notifications.Items[^1].Severity);
    }

    [Fact]
    public void Appearance_follows_the_setting_and_the_system()
    {
        using var h = new AppHarness().Started();
        var changes = 0;
        h.App.ThemeChanged += () => changes++;
        h.App.SetSystemPreferences(isDark: true, reducesMotion: false);
        Assert.Equal(Core.Appearance.Dark, h.App.Appearance);
        Assert.Equal("steel", h.App.AppIcon);
        h.Emit(new CoreEvent.Settings(AppSettings.Default with { Appearance = AppearancePreference.Light, ReduceEffects = true }));
        Assert.Equal(Core.Appearance.Light, h.App.Appearance);
        Assert.Equal("ice", h.App.AppIcon);
        Assert.True(h.App.ReduceMotion);
        Assert.True(h.App.Lyrics.ReduceMotion);
        Assert.Equal(2, changes);
    }

    private static PluginInfo Plugin(string id, PluginBadge badge, bool needsDisclosure) =>
        new(id, "Sub", "1.0.0", "Someone", badge, false, ["search", "browse"], [AuthMethod.Qr, AuthMethod.PasswordExchange], false, null, needsDisclosure, "{}", "{}", null);

    private static async Task Poll(Func<bool> condition)
    {
        for (var i = 0; i < 500 && !condition(); i++)
        {
            await Task.Delay(5);
        }

        Assert.True(condition());
    }
}

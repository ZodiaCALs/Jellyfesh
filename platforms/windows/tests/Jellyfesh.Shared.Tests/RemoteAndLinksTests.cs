using System.Globalization;
using System.Text.Json;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Formatting;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>
/// Links and remote playback (docs/ui.md, "Links and remote playback"; ADR-0018) as the view models
/// carry them out, and the Luminous numbers the views draw with. The key localizer returns keys.
/// </summary>
public class RemoteAndLinksTests
{
    private const string Spotify = "org.example.spotify";

    [Fact]
    public void A_remote_session_shows_its_track_instead_of_the_engines()
    {
        using var h = Started();
        var p = h.App.Player;
        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(7, new TrackRef.Library(3), "/music/3.flac", "Engine Song", "Artist", "Album", 200_000, null, null)));
        Assert.Equal("Engine Song", p.Title);
        Assert.False(p.IsRemote);

        h.Emit(new CoreEvent.Remote(Session()));
        Assert.True(p.IsRemote);
        Assert.True(p.HasTrack);
        Assert.Equal(("Lantern", "Sample Artist", "Night Swim"), (p.Title, p.Artist, p.Album));
        Assert.Null(p.ArtworkUri); // until the core has the picture (CoversTests)
        Assert.Equal("Remote_PlayingOn", p.RemoteDeviceText);
        Assert.Equal("the service asked to slow down", p.RemoteError);
        Assert.Equal(("Remote_Lyrics", "Remote_Queue"), (p.RemoteLyricsText, p.RemoteQueueText));
        Assert.Equal(("Spotify", "https://open.example/track/t9", "Link_OpenIn"), (p.ServiceName, p.Link, p.OpenLinkLabel));
        Assert.True(p.IsPlaying);
        Assert.Equal((true, RepeatMode.One), (p.Shuffle, p.Repeat));
        Assert.True(p.CanGoNext && p.CanGoPrevious);
        Assert.Equal(70, p.Volume);
        Assert.True(p.ShowsVolume);
        Assert.False(p.CanMute);
        Assert.True(p.HasMore);
        Assert.Equal(184_000_000, p.Progress.DurationUs);
        Assert.Equal(("Phone", "IconDevicePhone", "Remote_Devices"), (p.OutputName, p.OutputGlyph, p.OutputLabel));
        Assert.Equal(string.Empty, p.FilePath);
        Assert.False(h.App.Lyrics.HasLyrics); // the service plays its own

        // The transport sends the usual commands (the core routes them to the service).
        p.PlayPauseCommand.Execute(null);
        Assert.IsType<CoreCommand.TogglePlayPause>(h.Core.Sent[^1]);
        p.ToggleShuffleCommand.Execute(null);
        Assert.Equal(new CoreCommand.SetShuffle(false), h.Core.Sent[^1]);
        p.CycleRepeatCommand.Execute(null);
        Assert.Equal(new CoreCommand.SetRepeat(RepeatMode.Off), h.Core.Sent[^1]);

        // The volume is the playing device's; the engine's mute does not apply.
        p.SetVolumeFromUser(40.2);
        Assert.Equal(new CoreCommand.Remote(new RemoteAction.SetVolume(40)), h.Core.Sent[^1]);
        var sent = h.Core.Sent.Count;
        p.ToggleMuteCommand.Execute(null);
        Assert.Equal(sent, h.Core.Sent.Count);

        // The engine's slices do not take over while the session runs.
        h.Emit(new CoreEvent.Playback(Samples.Paused(0, 1, version: 9)));
        Assert.True(p.IsPlaying);
        Assert.Equal("Lantern", p.Title);

        // The session ends: the engine's song shows again, and nothing is said.
        h.Emit(new CoreEvent.Remote(Ended(version: 6, error: null)));
        Assert.False(p.IsRemote);
        Assert.Equal("Engine Song", p.Title);
        Assert.Equal(string.Empty, p.RemoteDeviceText);
        Assert.True(p.ShowsVolume && p.CanMute);
        Assert.False(p.IsPlaying);
        Assert.Null(p.Link);
        Assert.Empty(h.App.Notifications.Items);
    }

    [Fact]
    public void Devices_refresh_when_opened_and_choosing_one_transfers_playback()
    {
        using var h = Started();
        var p = h.App.Player;
        h.Emit(new CoreEvent.Remote(Session()));
        Assert.Equal([("desk", false), ("phone", true)], p.RemoteDevices.Select(d => (d.Id, d.IsPlaying)));
        Assert.Equal(["IconSpeaker", "IconDevicePhone"], p.RemoteDevices.Select(d => d.Glyph));

        p.RefreshRemoteDevices();
        Assert.Equal(new CoreCommand.Remote(new RemoteAction.Refresh()), h.Core.Sent[^1]);
        p.TransferToCommand.Execute(p.RemoteDevices[0]);
        Assert.Equal(new CoreCommand.Remote(new RemoteAction.Transfer("desk")), h.Core.Sent[^1]);
        var sent = h.Core.Sent.Count;
        p.TransferToCommand.Execute(p.RemoteDevices[1]); // already playing there
        Assert.Equal(sent, h.Core.Sent.Count);

        // A playing device the list does not have yet comes first, checked; one without a volume hides it.
        var tv = new RemoteDevice("tv", "Living Room", RemoteDeviceKind.Tv, true, null);
        h.Emit(new CoreEvent.Remote(Session(version: 6) with { Device = tv }));
        Assert.Equal([("tv", true), ("desk", false), ("phone", false)], p.RemoteDevices.Select(d => (d.Id, d.IsPlaying)));
        Assert.False(p.ShowsVolume);
        sent = h.Core.Sent.Count;
        p.SetVolumeFromUser(80);
        p.VolumeUpCommand.Execute(null);
        Assert.Equal(sent, h.Core.Sent.Count);

        p.StopControllingCommand.Execute(null);
        Assert.Equal(new CoreCommand.Remote(new RemoteAction.End()), h.Core.Sent[^1]);
    }

    [Fact]
    public void A_session_that_ends_with_an_error_says_so_once()
    {
        using var h = Started();
        h.Emit(new CoreEvent.Remote(Session()));
        Assert.Empty(h.App.Notifications.Items); // errors while it runs stay in the quieter line

        h.Emit(new CoreEvent.Remote(Ended(version: 6, error: "Sign in to Spotify again.")));
        var notice = Assert.Single(h.App.Notifications.Items);
        Assert.Equal((NotificationSeverity.Warning, "Remote_Ended", "Sign in to Spotify again."), (notice.Severity, notice.Title, notice.Message));

        h.Emit(new CoreEvent.Remote(Ended(version: 7, error: "Sign in to Spotify again.")));
        Assert.Single(h.App.Notifications.Items);
    }

    [Fact]
    public void The_remote_line_names_the_device_and_the_panes_name_the_service()
    {
        var l = new TableLocalizer(new Dictionary<string, string>
        {
            ["Remote_PlayingOn"] = "Playing on {0}",
            ["Remote_Lyrics"] = "Lyrics aren't shown while {0} plays.",
            ["Remote_Queue"] = "Up Next is in {0}.",
            ["Link_OpenIn"] = "Open in {0}",
        });
        using var h = new AppHarness(l).Started();
        h.Emit(new CoreEvent.Remote(Session()));
        var p = h.App.Player;
        Assert.Equal("Playing on Phone", p.RemoteDeviceText);
        Assert.Equal("Lyrics aren't shown while Spotify plays.", p.RemoteLyricsText);
        Assert.Equal("Up Next is in Spotify.", p.RemoteQueueText);
        Assert.Equal("Open in Spotify", p.OpenLinkLabel);
    }

    [Fact]
    public void Now_playing_opens_its_song_in_the_service()
    {
        using var h = new AppHarness();
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [Plugin("org.qq", "QQ Music", remote: false)], [], false) };
        h.Started();
        var p = h.App.Player;
        var song = new TrackRef.Source(new SourceTrack("org.qq", "s1", "Song", "Singer", null, 61_000, null, null));
        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(8, song, null, "Song", "Singer", null, 61_000, null, null, "https://y.example/song/s1")));
        Assert.Equal(("QQ Music", "https://y.example/song/s1", "Link_OpenIn"), (p.ServiceName, p.Link, p.OpenLinkLabel));
        Assert.True(p.HasLink && p.HasMore);
        p.OpenLinkCommand.Execute(null);
        Assert.Equal(["https://y.example/song/s1"], h.Platform.Opened);

        // Only web addresses open.
        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(9, song, null, "Song", "Singer", null, 61_000, null, null, "file:///C:/Windows/notepad.exe")));
        Assert.Null(p.Link);
        Assert.False(p.HasMore);
        p.OpenLinkCommand.Execute(null);
        Assert.Single(h.Platform.Opened);
    }

    [Fact]
    public void Items_that_cannot_play_here_open_their_link_instead()
    {
        using var h = new AppHarness();
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [Plugin("org.qq", "QQ Music", remote: false)], [], false) };
        h.Started();
        var search = h.App.CreateSearch();
        var track = new TrackRef.Source(new SourceTrack("org.qq", "s1", "Song", null, null, null, null, null));

        var locked = Row(new SourceItem("org.qq", ItemKind.Track, "s1", "Song", "Singer", null, null, null, false, "VIP only", null, [], "https://y.example/song/s1"));
        Assert.True(locked.OpensLink);
        Assert.Equal("VIP only", locked.UnplayableNote);
        Assert.Equal("org.qq", locked.SourceId);
        search.PlayTrackCommand.Execute(locked);
        Assert.Equal(["https://y.example/song/s1"], h.Platform.Opened);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.PlayTracks);

        var playable = Row(new SourceItem("org.qq", ItemKind.Track, "s1", "Song", "Singer", null, null, null, true, null, track, [], "https://y.example/song/s1"));
        Assert.False(playable.OpensLink);
        Assert.Null(playable.UnplayableNote);
        search.PlayTrackCommand.Execute(playable);
        Assert.Equal([track], Assert.IsType<CoreCommand.PlayTracks>(h.Core.Sent[^1]).Tracks);

        // A source's artist, album or playlist opens its page (docs/parity.md N6), never plays at once.
        var pages = new List<SourceTarget>();
        search.OpenRequested += pages.Add;
        var artist = new SourceEntryItem(new SourceItem("org.qq", ItemKind.Artist, "a1", "Singer", null, null, null, null, false, null, null, [], "https://y.example/singer/a1"), KeyLocalizer.Instance);
        search.OpenEntryCommand.Execute(artist);
        var album = new SourceEntryItem(new SourceItem("org.qq", ItemKind.Album, "al1", "Album", null, null, null, null, true, null, null, [], "https://y.example/album/al1"), KeyLocalizer.Instance);
        search.OpenEntryCommand.Execute(album);
        Assert.Equal([("a1", ItemKind.Artist), ("al1", ItemKind.Album)], pages.Select(t => (t.Node!, t.Kind!.Value)));
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.PlayCollection);

        // An album that cannot play here opens its link instead.
        var locked2 = new SourceEntryItem(new SourceItem("org.qq", ItemKind.Album, "al2", "VIP Album", null, null, null, null, false, "VIP only", null, [], "https://y.example/album/al2"), KeyLocalizer.Instance);
        search.OpenEntryCommand.Execute(locked2);
        Assert.Equal("https://y.example/album/al2", h.Platform.Opened[^1]);
        Assert.Equal(2, pages.Count);
    }

    [Fact]
    public void A_source_page_opens_links_and_offers_its_artists_page()
    {
        using var h = new AppHarness();
        var qq = Plugin("org.qq", "QQ Music", remote: false);
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [qq], [], false) };
        h.Started();
        var page = h.App.CreateSourceBrowse(SourceTarget.Root(qq));
        page.Activate();
        var request = (ulong)h.Core.Sent.Count;
        var artist = new SourceItem("org.qq", ItemKind.Artist, "a1", "Singer", null, null, null, null, false, null, null, [], "https://y.example/singer/a1");
        var locked = new SourceItem("org.qq", ItemKind.Track, "s1", "Song", null, null, null, null, false, "VIP only", null, [], "https://y.example/song/s1");
        h.Emit(new CoreEvent.BrowsePage(new BrowseUpdate(request, "org.qq", null, [artist, locked], null, null)));
        Assert.False(page.HasLink);
        Assert.Equal("Link_OpenIn", page.OpenInLabel);

        // A song that cannot play here opens its link.
        page.OpenCommand.Execute(page.Rows[1]);
        Assert.Equal(["https://y.example/song/s1"], h.Platform.Opened);

        // An artist opens its page, whose More offers Open in {Service}.
        SourceTarget? opened = null;
        page.OpenRequested += t => opened = t;
        page.OpenCommand.Execute(page.Rows[0]);
        Assert.NotNull(opened);
        var artistPage = h.App.CreateSourceBrowse(opened!);
        Assert.True(artistPage.HasLink);
        Assert.True(artistPage.HasMore);
        artistPage.OpenLinkCommand.Execute(null);
        Assert.Equal("https://y.example/singer/a1", h.Platform.Opened[^1]);
        page.Deactivate();
    }

    [Fact]
    public void Remote_sources_songs_are_never_queued()
    {
        using var h = new AppHarness();
        h.Core.State = CoreSnapshot.Initial with
        {
            Sources = new SourcesState(1, [Plugin(Spotify, "Spotify", remote: true), Plugin("org.qq", "QQ Music", remote: false)], [], false),
        };
        h.Started();
        var remote = Row(new SourceItem(Spotify, ItemKind.Track, "t9", "Lantern", null, null, null, null, true, null, SourceRef(Spotify, "t9"), []));
        var local = Row(new SourceItem("org.qq", ItemKind.Track, "s1", "Song", null, null, null, null, true, null, SourceRef("org.qq", "s1"), []));
        Assert.False(TrackActions.CanQueue(h.S, remote));
        Assert.True(TrackActions.CanQueue(h.S, local));

        TrackActions.PlayNext(h.S, [remote]);
        TrackActions.PlayLast(h.S, [remote]);
        TrackActions.AddToPlaylist(h.S, 5, [remote]);
        Assert.Empty(h.Core.Sent);

        TrackActions.PlayNext(h.S, [remote, local]);
        var enqueue = Assert.IsType<CoreCommand.Enqueue>(h.Core.Sent[^1]);
        Assert.True(enqueue.PlayNext);
        Assert.Equal([SourceRef("org.qq", "s1")], enqueue.Tracks);
    }

    [Fact]
    public void Remote_slices_coalesce_and_older_ones_are_ignored()
    {
        using var h = Started();
        h.Core.Emit(new CoreEvent.Remote(Session(version: 8)));
        h.Core.Emit(new CoreEvent.Remote(Session(version: 7) with { Error = null }));
        h.Ui.RunAll();
        Assert.Equal(8ul, h.S.Hub.State.Remote.Version);
        Assert.Equal("the service asked to slow down", h.App.Player.RemoteError);
    }

    [Fact]
    public void The_field_has_four_glows_as_strong_as_the_token()
    {
        using var tokens = JsonDocument.Parse(File.ReadAllText(Contract.PathOf("design", "tokens.json")));
        var strength = tokens.RootElement.GetProperty("field").GetProperty("aurora-strength").GetProperty("$value").GetDouble();
        Assert.Equal(0.75, strength);
        Assert.Equal(4, AuroraField.Glows.Count);
        Assert.Equal([0.80, 0.72, 0.66, 0.46], AuroraField.Glows.Select(g => Math.Round(g.Alpha(strength), 2)));
        Assert.Equal(["field.aurora-1", "field.aurora-2", "field.aurora-3", "field.aurora-1"], AuroraField.Glows.Select(g => g.Color));
        Assert.All(AuroraField.Glows, g => Assert.InRange(g.FadeEnd, 0.6, 0.8));
        Assert.Equal(1, AuroraField.Glows[0].Alpha(1)); // never more than opaque
    }

    [Fact]
    public void Lyric_lines_reach_forty_pixels_in_a_wide_pane()
    {
        Assert.Equal(40, NowPlayingMotion.LyricLineSize(640, 28, 40));
        Assert.Equal(40, NowPlayingMotion.LyricLineSize(1200, 28, 40));
        Assert.Equal(30, NowPlayingMotion.LyricLineSize(480, 28, 40));
        Assert.Equal(28, NowPlayingMotion.LyricLineSize(200, 28, 40));
        Assert.Equal(28, NowPlayingMotion.LyricLineSize(double.NaN, 28, 40));
    }

    private static AppHarness Started()
    {
        var h = new AppHarness();
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [Plugin(Spotify, "Spotify", remote: true)], [], false) };
        return h.Started();
    }

    private static RemoteState Session(ulong version = 5)
    {
        var phone = new RemoteDevice("phone", "Phone", RemoteDeviceKind.Smartphone, true, 70);
        var item = new SourceItem(Spotify, ItemKind.Track, "t9", "Lantern", "Sample Artist", "Night Swim", 184_000,
            new ArtworkRef.Url("https://art.example/7.jpg"), true, null, SourceRef(Spotify, "t9"), [], "https://open.example/track/t9");
        return new RemoteState(
            version, Spotify, "Spotify", PlaybackStatus.Playing, item, null,
            new PlaybackAnchor(3, 12_345_678, 1_000_000_000, 1f, 184_000_000), true, RepeatMode.One, phone,
            [new RemoteDevice("desk", "Desk Speaker", RemoteDeviceKind.Speaker, false, null), phone],
            "the service asked to slow down");
    }

    private static RemoteState Ended(ulong version, string? error) => RemoteState.Initial with { Version = version, Error = error };

    private static TrackRef SourceRef(string plugin, string id) => new TrackRef.Source(new SourceTrack(plugin, id, "Title", null, null, null, null, null));

    private static TrackItem Row(SourceItem item)
    {
        var row = new TrackItem(0) { Localizer = KeyLocalizer.Instance };
        row.Load(item);
        return row;
    }

    private static PluginInfo Plugin(string id, string name, bool remote) =>
        new(id, name, "1.0.0", "Someone", PluginBadge.Official, false, ["search", "browse"], [AuthMethod.Oauth], true, "ana", false, "{}", "{}", null, remote);

    private sealed class TableLocalizer(Dictionary<string, string> table) : ILocalizer
    {
        public CultureInfo Culture => CultureInfo.InvariantCulture;

        public string Get(string key) => table.TryGetValue(key, out var value) ? value : key;
    }
}

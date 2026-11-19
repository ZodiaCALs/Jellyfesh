using System.Text.Json;
using Jellyfesh.Interop;
using Jellyfesh.Tests.Support;
using Xunit;
using F = Jellyfesh.Ffi;
using M = Jellyfesh.Core;

namespace Jellyfesh.Tests;

/// <summary>The v0.0.7 contract (docs/core-api.md): the new slices, answers, commands and renamed members.</summary>
public class V007MappingTests
{
    private static IEnumerable<M.CoreEvent> Events(string type)
    {
        using var contract = Contract.Read("ffi-types.json");
        return contract.RootElement.GetProperty("samples").EnumerateArray()
            .Where(s => s.GetProperty("name").GetString() == "Event")
            .Select(s => s.GetProperty("json"))
            .Where(j => j.GetProperty("type").GetString() == type)
            .Select(j => FfiMapping.ToModel(j.Deserialize(F.JellyfeshJsonContext.Default.Event)!))
            .ToList();
    }

    private static string Json(M.CoreCommand c) => JsonSerializer.Serialize(FfiMapping.ToFfi(c), F.JellyfeshJsonContext.Default.Command);

    [Fact]
    public void Ai_lyrics_maps_its_plan_need_jobs_models_and_consents()
    {
        var state = Assert.IsType<M.CoreEvent.AiLyrics>(Assert.Single(Events("AiLyrics"))).State;
        Assert.Equal((3ul, true, "org.jellyfesh.ai-lyrics"), (state.Version, state.Available, state.Plugin));
        Assert.Equal(new M.AiLyricsPlan(M.AiLyricsMode.Online, "whisper-1 at api.example.com", "https://api.example.com", true), state.Plan);
        Assert.Equal(new M.AiLyricsNeed.Consent("https://api.example.com"), state.Need);
        Assert.Equal(
            [typeof(M.LyricsJobStage.Waiting), typeof(M.LyricsJobStage.ReadingAudio), typeof(M.LyricsJobStage.Sending),
             typeof(M.LyricsJobStage.Recognising), typeof(M.LyricsJobStage.TimingWords), typeof(M.LyricsJobStage.Done),
             typeof(M.LyricsJobStage.Failed), typeof(M.LyricsJobStage.Cancelled)],
            state.Jobs.Select(j => j.Stage.GetType()));
        Assert.Equal(0.42f, Assert.IsType<M.LyricsJobStage.Recognising>(state.Jobs[3].Stage).Fraction);
        Assert.Equal("the provider refused the key", Assert.IsType<M.LyricsJobStage.Failed>(state.Jobs[6].Stage).Message);
        Assert.Equal([true, true, true, true, true, false, false, false], state.Jobs.Select(j => j.Stage.IsActive));
        Assert.Equal(7ul, state.JobFor(2)!.Id); // the newest job for the song playing
        Assert.Null(state.JobFor(9));
        Assert.NotEmpty(state.Models);
        Assert.Equal("tiny", state.Models[0].Id);
        Assert.NotEmpty(state.Consents);
    }

    [Fact]
    public void The_look_maps_every_value_and_the_default_is_luminous()
    {
        var looks = Events("Look").Cast<M.CoreEvent.Look>().Select(l => l.State).ToList();
        var custom = looks.Single(l => l.Customized);
        Assert.Equal((1.5f, 0.25f, 0.5f, M.Typeface.Rounded, 1.15f), (custom.CornerScale, custom.Glass, custom.Aurora, custom.Typeface, custom.TextScale));
        Assert.Equal((M.Density.Roomy, M.LyricsWeight.Heavy, 0.8f), (custom.Density, custom.LyricsWeight, custom.LyricsGlow));
        Assert.Equal(new M.LyricsHighlight.Custom(0xFF8800), custom.LyricsHighlight);
        Assert.Equal([M.Place.Songs, M.Place.Home, M.Place.Search], custom.Sidebar);
        Assert.Equal([M.HomeSection.Favourites, M.HomeSection.Summary], custom.Home);
        Assert.Equal((M.NowPlayingLayout.Lyrics, M.MiniPlayerStyle.Floating, "org.jellyfesh.customize"), (custom.NowPlaying, custom.MiniPlayer, custom.Plugin));
        Assert.Equal(1.2, custom.Spacing);

        // The core's defaults are this app's defaults.
        var defaults = looks.Single(l => !l.Customized);
        Assert.Equal(M.LookState.Default with { Version = defaults.Version, Sidebar = defaults.Sidebar, Home = defaults.Home }, defaults);
        Assert.Equal(M.LookState.Default.Sidebar, defaults.Sidebar);
        Assert.Equal(M.LookState.Default.Home, defaults.Home);
    }

    [Fact]
    public void Pro_maps_every_source_and_its_licence_error()
    {
        var pro = Events("Pro").Cast<M.CoreEvent.Pro>().Select(p => p.State).ToList();
        Assert.Contains(pro, p => p.Source == new M.ProSource.Licence("ABCD", 1_763_337_600_000) && p.LicenceError == M.LicenceError.Revoked);
        Assert.Contains(pro, p => p.Source == new M.ProSource.Purchased(M.ProStore.AppStore) && p.Stores.SequenceEqual([M.ProStore.AppStore, M.ProStore.GooglePlay]));
        Assert.Contains(pro, p => p.Source is M.ProSource.Preview && p.Preview && p.LicenceError == M.LicenceError.Malformed);
        Assert.Contains(pro, p => p.Source is M.ProSource.None && !p.Active && p.LicenceError == M.LicenceError.BadSignature);
        Assert.All(pro, p => Assert.False(p.LicenceKeys));
        Assert.True(M.ProState.Initial.LicenceKeys);
    }

    [Fact]
    public void The_update_slice_arrives_as_update_status()
    {
        var updates = Events("Update").Cast<M.CoreEvent.UpdateStatus>().Select(u => u.State).ToList();
        var found = updates.Single(u => u.Available is not null);
        Assert.Equal(new M.UpdateInfo("0.0.8-beta.1", "https://github.com/ZodiaCALs/Jellyfesh/releases/tag/v0.0.8-beta.1"), found.Available);
        Assert.Equal((true, false, true, 1_763_400_000_000L, "the release list could not be read"), (found.Supported, found.Checking, found.Auto, found.LastCheckedMs, found.Error));
        Assert.Contains(updates, u => u.Checking && u.Available is null);
    }

    [Fact]
    public void Discover_details_comments_and_artwork_answers_map()
    {
        var discover = Assert.IsType<M.CoreEvent.Discover>(Assert.Single(Events("Discover"))).Page;
        Assert.Equal((30ul, "org.x", "one shelf could not be loaded"), (discover.Request, discover.Source, discover.Error));
        Assert.Equal([M.ShelfStyle.Row, M.ShelfStyle.Grid, M.ShelfStyle.Chart], discover.Shelves.Select(s => s.Style));
        Assert.Equal(("chart:new", "ar:1", "al:1"), (discover.Shelves[0].More, discover.Shelves[0].Items[0].ArtistId, discover.Shelves[0].Items[0].AlbumId));

        var details = Events("Details").Cast<M.CoreEvent.Details>().Select(d => d.Page).ToList();
        var track = details.Single(d => d.Item is M.ItemRef.Track);
        Assert.Equal(new M.ItemRef.Track(42), track.Item);
        Assert.Equal(15, track.Facts.Count);
        Assert.Equal(Enum.GetValues<M.FactKind>(), track.Facts.Select(f => f.Kind));
        Assert.Equal(new M.Fact(M.FactKind.Other, "Sample", "Mood"), track.Facts[^1]);
        var technical = track.Technical!;
        Assert.Equal(new M.AudioFormat("flac", 96_000, 24, 2, 3_100, true, M.QualityLabel.HiResLossless), technical.Format);
        Assert.Equal((71_234_567ul, -6.5f), (technical.FileSizeBytes!.Value, technical.ReplayGain!.TrackGainDb!.Value));
        var album = details.Single(d => d.Item is M.ItemRef.Source);
        Assert.Equal(new M.ItemRef.Source("org.x", M.ItemKind.Album, "al:1"), album.Item);
        Assert.Equal((3ul, "Example Service", "the source gave no details"), (album.CommentCount!.Value, album.Attribution, album.Error));

        var comments = Assert.IsType<M.CoreEvent.Comments>(Assert.Single(Events("Comments"))).Page;
        Assert.Equal((32ul, 3ul, "2"), (comments.Request, comments.Total!.Value, comments.Next));
        Assert.Single(comments.Hot);
        Assert.Equal(2, comments.Comments.Count);
        Assert.Null(comments.Comments[1].Avatar);

        var artwork = Events("Artwork").Cast<M.CoreEvent.Artwork>().Select(a => a.Ready).ToList();
        Assert.Contains(new M.ArtworkReady("https://music.example/art/7.jpg", 256, "/cache/source-artwork/ab12cd-256.jpg"), artwork);
        Assert.Contains(new M.ArtworkReady("https://music.example/art/8.jpg", 512, null), artwork);
    }

    [Fact]
    public void Plugins_say_their_kind_pro_permissions_and_offers_and_store_builds_install_nothing()
    {
        var sources = Assert.IsType<M.CoreEvent.Sources>(Assert.Single(Events("Sources"))).State;
        Assert.False(sources.CanInstallPlugins);
        var plugin = Assert.Single(sources.Plugins);
        Assert.Equal((M.PluginKind.Extension, true, true, true, true), (plugin.Kind, plugin.RequiresPro, plugin.Discover, plugin.Details, plugin.Comments));
        Assert.Equal(["audio", "speech"], plugin.Permissions!);
        Assert.Equal("Music from an example service", plugin.Description);
        Assert.Empty(sources.Sources);
        Assert.Same(plugin, Assert.Single(sources.Extensions));
        Assert.True(M.SourcesState.Initial.CanInstallPlugins);
    }

    [Fact]
    public void Now_playing_carries_the_decoded_format_and_lyrics_their_maker()
    {
        var np = Events("NowPlaying").Cast<M.CoreEvent.NowPlaying>().Single(n => n.Info is not null).Info!;
        Assert.Equal(16u, np.Format!.BitsPerSample);
        Assert.Equal(new M.AudioFormat("flac", 96_000, 24, 2, 3_100, true, M.QualityLabel.HiResLossless), np.Quality);

        var origins = Events("Lyrics").Cast<M.CoreEvent.Lyrics>().Select(l => l.State.Source).ToList();
        Assert.Contains(new M.LyricsOrigin.Made("org.jellyfesh.ai-lyrics", "Whisper Base on this device"), origins);
        Assert.Contains(new M.LyricsOrigin.Plugin("org.x"), origins);
    }

    [Fact]
    public void The_new_commands_serialise_as_the_core_expects()
    {
        Assert.Equal("""{"type":"MakeLyrics","song":{"type":"Current"}}""", Json(new M.CoreCommand.MakeLyrics(new M.SongRef.Current())));
        Assert.Equal("""{"type":"DeleteMadeLyrics","song":{"type":"Library","id":7}}""", Json(new M.CoreCommand.DeleteMadeLyrics(new M.SongRef.Library(7))));
        Assert.Equal("""{"type":"ActivateLicence","key":"JFP1-AAAA"}""", Json(new M.CoreCommand.ActivateLicence("JFP1-AAAA")));
        Assert.Equal("""{"type":"Discover","source":"library"}""", Json(new M.CoreCommand.Discover("library")));
        Assert.Equal(
            """{"type":"Details","item":{"type":"Source","plugin":"org.x","item_kind":"Artist","id":"ar:1"}}""",
            Json(new M.CoreCommand.Details(new M.ItemRef.Source("org.x", M.ItemKind.Artist, "ar:1"))));
        Assert.Equal(
            """{"type":"Comments","item":{"type":"Track","id":3},"page":null}""",
            Json(new M.CoreCommand.Comments(new M.ItemRef.Track(3), null)));
        Assert.Equal(
            """{"type":"PlayCollection","collection":{"type":"Songs","sort":"Album","descending":true},"start":0,"shuffle":true}""",
            Json(new M.CoreCommand.PlayCollection(new M.Collection.Songs(M.TrackSort.Album, true), 0, true)));
    }

    [Fact]
    public void Settings_carry_updates_the_lyrics_bar_and_desktop_lyrics()
    {
        Assert.Equal((true, true, false), (M.AppSettings.Default.CheckForUpdates, M.AppSettings.Default.ShowLyricsBar, M.AppSettings.Default.DesktopLyrics));
        var settings = M.AppSettings.Default with { CheckForUpdates = false, ShowLyricsBar = false, DesktopLyrics = true };
        var json = Json(new M.CoreCommand.SetSettings(settings));
        Assert.Contains("\"check_for_updates\":false", json, StringComparison.Ordinal);
        Assert.Contains("\"show_lyrics_bar\":false", json, StringComparison.Ordinal);
        Assert.Contains("\"desktop_lyrics\":true", json, StringComparison.Ordinal);
        Assert.Equal(settings, FfiMapping.ToModel(FfiMapping.ToFfi(settings)) with { Eq = settings.Eq });
    }

    [Fact]
    public void A_source_build_tells_the_core_so()
    {
        // Tests build with JfDistribution=source (Directory.Build.props): the release workflow sets direct.
        Assert.Equal(F.Distribution.Source, FfiCoreClient.Distribution);
        var json = JsonSerializer.Serialize(FfiCoreClient.Config("/data", "/cache", []), F.JellyfeshJsonContext.Default.CoreConfig);
        Assert.Contains("\"distribution\":\"Source\"", json, StringComparison.Ordinal);
        Assert.Equal(F.Store.MicrosoftStore, FfiMapping.ToFfi(new M.StoreEntitlement(M.ProStore.MicrosoftStore, "org.jellyfesh.pro", true, "order-1")).Store);
    }

    [Fact]
    public void An_item_reference_comes_from_a_track_reference()
    {
        Assert.Equal(new M.ItemRef.Track(5), M.ItemRef.Of(new M.TrackRef.Library(5)));
        Assert.Equal(new M.ItemRef.File("/a.flac"), M.ItemRef.Of(new M.TrackRef.File("/a.flac")));
        Assert.Equal(
            new M.ItemRef.Source("org.x", M.ItemKind.Track, "t1"),
            M.ItemRef.Of(new M.TrackRef.Source(new M.SourceTrack("org.x", "t1", "T", null, null, null, null, null))));
    }
}

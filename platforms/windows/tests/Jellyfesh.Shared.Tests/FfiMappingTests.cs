using System.Text.Json;
using Jellyfesh.Interop;
using Jellyfesh.Tests.Support;
using Xunit;
using F = Jellyfesh.Ffi;
using M = Jellyfesh.Core;

namespace Jellyfesh.Tests;

/// <summary>The adapter maps every shape the core sends (tests/contract/ffi-types.json) and every command back.</summary>
public class FfiMappingTests
{
    [Fact]
    public void Every_contract_sample_maps_to_the_model()
    {
        using var contract = Contract.Read("ffi-types.json");
        var mapped = new Dictionary<string, int>();
        foreach (var sample in contract.RootElement.GetProperty("samples").EnumerateArray())
        {
            var name = sample.GetProperty("name").GetString()!;
            var json = sample.GetProperty("json");
            object model = name switch
            {
                "Command" => RoundTripCommand(json),
                "Event" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.Event)!),
                "Snapshot" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.Snapshot)!),
                "TrackRef" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.TrackRef)!),
                "Track" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.Track)!),
                "Album" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.Album)!),
                "Artist" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.Artist)!),
                "SearchResults" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.SearchResults)!),
                "LibraryStats" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.LibraryStats)!),
                "TrackRow" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.TrackRow)!),
                "PlaylistSummary" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.PlaylistSummary)!),
                "PlaylistEntry" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.PlaylistEntry)!),
                "QueueItem" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.QueueItem)!),
                "HistoryEntry" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.HistoryEntry)!),
                "DeviceInfo" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.DeviceInfo)!),
                "TrackQuery" => FfiMapping.ToFfi(new M.TrackQuery()),
                "CoreConfig" => json.Deserialize(F.JellyfeshJsonContext.Default.CoreConfig)!,
                _ => throw new Xunit.Sdk.XunitException($"unmapped sample {name}"),
            };
            Assert.NotNull(model);
            Assert.IsNotType<M.CoreEvent.Error>(model is M.CoreEvent e && e is M.CoreEvent.Error err && err.Message.StartsWith("unknown", StringComparison.Ordinal) ? model : null);
            mapped[name] = mapped.GetValueOrDefault(name) + 1;
        }

        Assert.True(mapped["Event"] >= 17, "every event variant has a sample");
        Assert.True(mapped["Command"] >= 40, "every command variant has a sample");
    }

    [Fact]
    public void Core_config_carries_the_bundled_plugins_when_the_binding_has_them()
    {
        var config = FfiCoreClient.Config("/data", "/cache", ["/app/Plugins/org.jellyfesh.jellyfin.jfpkg"]);
        Assert.Equal("/data", config.DataDir);
        Assert.Equal("/cache", config.CacheDir);
        Assert.IsType<F.OutputKind.Device>(config.Output);
        var json = JsonSerializer.Serialize(config, F.JellyfeshJsonContext.Default.CoreConfig);
        Assert.Equal(FfiCoreClient.PassesBundledPlugins, json.Contains("org.jellyfesh.jellyfin.jfpkg", StringComparison.Ordinal));
    }

    [Fact]
    public void Theme_colours_unpack_as_rrggbbaa()
    {
        var theme = new F.Theme(250f, F.Appearance.Dark, [new F.ThemeColor("ink.ink-90", 0xF6F9FCEB)]);
        var palette = FfiMapping.ToModel(theme);
        Assert.Equal(M.Appearance.Dark, palette.Appearance);
        Assert.Equal(new M.Rgba(0xF6, 0xF9, 0xFC, 0xEB), palette["ink.ink-90"]);
        Assert.Null(palette["missing"]);
    }

    [Fact]
    public void Renamed_members_map_to_the_model()
    {
        var failed = FfiMapping.ToModel(new F.Event.CommandFailed(3, "nothing to play"));
        Assert.Equal(new M.CoreEvent.CommandFailed(3, "nothing to play"), failed);
        var buffering = FfiMapping.ToModel(new F.Event.Buffering(5, true));
        Assert.Equal(new M.CoreEvent.Buffering(5, true), buffering);
        var url = FfiMapping.ToModel(new F.ArtworkRef.Url("https://example.org/a.jpg"));
        Assert.Equal("https://example.org/a.jpg", Assert.IsType<M.ArtworkRef.Url>(url).Address);
        var tracks = Assert.IsType<F.Collection.Tracks>(FfiMapping.ToFfi(new M.Collection.Tracks([new M.TrackRef.Library(1)])));
        Assert.Equal(new F.TrackRef.Library(1), tracks.Value[0]);
    }

    [Fact]
    public void Commands_serialise_as_the_core_expects()
    {
        string Json(M.CoreCommand c) => JsonSerializer.Serialize(FfiMapping.ToFfi(c), F.JellyfeshJsonContext.Default.Command);
        Assert.Equal("""{"type":"SetRepeat","mode":"One"}""", Json(new M.CoreCommand.SetRepeat(M.RepeatMode.One)));
        Assert.Equal(
            """{"type":"PlayCollection","collection":{"type":"Album","id":5},"start":2,"shuffle":true}""",
            Json(new M.CoreCommand.PlayCollection(new M.Collection.Album(5), 2, true)));
        Assert.Equal("""{"type":"SetVolume","volume":1}""", Json(new M.CoreCommand.SetVolume(1.7f)));
        Assert.Contains("\"fields\":{\"user\":\"b\"}", Json(new M.CoreCommand.SubmitLogin("p", 1, [new("user", "a"), new("user", "b")])), StringComparison.Ordinal);
    }

    [Fact]
    public void Settings_round_trip_through_the_binding()
    {
        var settings = M.AppSettings.Default with
        {
            Appearance = M.AppearancePreference.Dark,
            Style = M.BackdropStyle.Misty,
            AppIcon = "lagoon",
            HueSource = new M.HueSource.Fixed(120f),
            Transparency = 0.25f,
            LyricsSize = M.LyricsSize.Large,
            LyricsAlign = M.LyricsAlign.Center,
            Crossfade = new M.CrossfadeSettings(true, 4_000, false),
            Eq = M.EqSettings.Graphic([6, 5, 4]),
            OutputDevice = "wasapi:speakers",
        };
        var back = FfiMapping.ToModel(FfiMapping.ToFfi(settings));
        Assert.Equal(settings with { Eq = back.Eq }, back);
        Assert.True(settings.Eq.ContentEquals(back.Eq));
    }

    [Theory]
    [InlineData(M.LyricsSize.Small, M.LyricsAlign.Start, "Small", "Start")]
    [InlineData(M.LyricsSize.Medium, M.LyricsAlign.Center, "Medium", "Center")]
    [InlineData(M.LyricsSize.Large, M.LyricsAlign.Start, "Large", "Start")]
    public void Lyrics_size_alignment_and_transparency_reach_the_core(M.LyricsSize size, M.LyricsAlign align, string sizeJson, string alignJson)
    {
        var settings = M.AppSettings.Default with { Transparency = 0.75f, LyricsSize = size, LyricsAlign = align };
        var json = JsonSerializer.Serialize(FfiMapping.ToFfi(new M.CoreCommand.SetSettings(settings)), F.JellyfeshJsonContext.Default.Command);
        Assert.Contains("\"transparency\":0.75", json, StringComparison.Ordinal);
        Assert.Contains($"\"lyrics_size\":\"{sizeJson}\"", json, StringComparison.Ordinal);
        Assert.Contains($"\"lyrics_align\":\"{alignJson}\"", json, StringComparison.Ordinal);
        var command = Assert.IsType<F.Command.SetSettings>(JsonSerializer.Deserialize(json, F.JellyfeshJsonContext.Default.Command));
        var back = FfiMapping.ToModel(command.Settings);
        Assert.Equal((0.75f, size, align), (back.Transparency, back.LyricsSize, back.LyricsAlign));
    }

    [Fact]
    public void Default_settings_match_the_core()
    {
        Assert.Equal(0.5f, M.AppSettings.Default.Transparency);
        Assert.Equal(M.LyricsSize.Medium, M.AppSettings.Default.LyricsSize);
        Assert.Equal(M.LyricsAlign.Start, M.AppSettings.Default.LyricsAlign);
    }

    [Fact]
    public void Query_failures_keep_their_kind()
    {
        var e = FfiCoreClient.Map(new F.CoreFfiException(F.CoreFfiErrorKind.NotAvailable, "playlists are not available in this build yet"));
        Assert.True(e.IsNotYetAvailable);
        Assert.Equal(M.CoreQueryError.ShutDown, FfiCoreClient.Map(new F.CoreFfiException(F.CoreFfiErrorKind.ShutDown, "gone")).Kind);
    }

    private static M.CoreCommand RoundTripCommand(JsonElement json)
    {
        // Ffi → JSON is the binding's job; here the model must be able to express every command
        // the contract holds, and produce the same JSON again.
        var ffi = json.Deserialize(F.JellyfeshJsonContext.Default.Command)!;
        var model = ToModelCommand(ffi);
        var again = JsonSerializer.Serialize(FfiMapping.ToFfi(model), F.JellyfeshJsonContext.Default.Command);
        using var a = JsonDocument.Parse(again);
        Assert.True(JsonEquivalent(json, a.RootElement), $"{json.GetRawText()}\n{again}");
        return model;
    }

    /// <summary>The inverse of FfiMapping.ToFfi(CoreCommand), for the round trip only.</summary>
    private static M.CoreCommand ToModelCommand(F.Command c) => c switch
    {
        F.Command.PlayTracks p => new M.CoreCommand.PlayTracks(p.Tracks.Select(FfiMapping.ToModel).ToArray(), p.Start),
        F.Command.Enqueue e => new M.CoreCommand.Enqueue(e.Tracks.Select(FfiMapping.ToModel).ToArray(), e.PlayNext),
        F.Command.RemoveFromQueue r => new M.CoreCommand.RemoveFromQueue(r.Uid),
        F.Command.MoveInQueue m => new M.CoreCommand.MoveInQueue(m.Uid, m.To),
        F.Command.JumpTo j => new M.CoreCommand.JumpTo(j.Uid),
        F.Command.ClearQueue => new M.CoreCommand.ClearQueue(),
        F.Command.SetShuffle s => new M.CoreCommand.SetShuffle(s.On),
        F.Command.SetRepeat r => new M.CoreCommand.SetRepeat(FfiMapping.ToModel(r.Mode)),
        F.Command.Play => new M.CoreCommand.Play(),
        F.Command.Pause => new M.CoreCommand.Pause(),
        F.Command.TogglePlayPause => new M.CoreCommand.TogglePlayPause(),
        F.Command.Stop => new M.CoreCommand.Stop(),
        F.Command.Next => new M.CoreCommand.Next(),
        F.Command.Previous => new M.CoreCommand.Previous(),
        F.Command.Seek s => new M.CoreCommand.Seek(s.PositionMs),
        F.Command.SetVolume v => new M.CoreCommand.SetVolume(v.Volume),
        F.Command.SetEq e => new M.CoreCommand.SetEq(FfiMapping.ToModel(e.Eq)),
        F.Command.SetReplayGain r => new M.CoreCommand.SetReplayGain(FfiMapping.ToModel(r.Mode), r.PreampDb),
        F.Command.AddRoot a => new M.CoreCommand.AddRoot(a.Path),
        F.Command.RemoveRoot r => new M.CoreCommand.RemoveRoot(r.Path),
        F.Command.Rescan => new M.CoreCommand.Rescan(),
        F.Command.PlayCollection p => new M.CoreCommand.PlayCollection(ToModel(p.Collection), p.Start, p.Shuffle),
        F.Command.EnqueueCollection e => new M.CoreCommand.EnqueueCollection(ToModel(e.Collection), e.PlayNext),
        F.Command.SetSettings s => new M.CoreCommand.SetSettings(FfiMapping.ToModel(s.Settings)),
        F.Command.SetMuted m => new M.CoreCommand.SetMuted(m.Muted),
        F.Command.SetCrossfade x => new M.CoreCommand.SetCrossfade(new M.CrossfadeSettings(x.Crossfade.Enabled, x.Crossfade.DurationMs, x.Crossfade.AlbumContinuity)),
        F.Command.SetOutputDevice d => new M.CoreCommand.SetOutputDevice(d.Name),
        F.Command.RefreshDevices => new M.CoreCommand.RefreshDevices(),
        F.Command.CreatePlaylist p => new M.CoreCommand.CreatePlaylist(p.Name, p.Tracks.Select(FfiMapping.ToModel).ToArray()),
        F.Command.RenamePlaylist r => new M.CoreCommand.RenamePlaylist(r.Id, r.Name),
        F.Command.DeletePlaylist d => new M.CoreCommand.DeletePlaylist(d.Id),
        F.Command.AddToPlaylist a => new M.CoreCommand.AddToPlaylist(a.Id, a.Tracks.Select(FfiMapping.ToModel).ToArray()),
        F.Command.RemoveFromPlaylist r => new M.CoreCommand.RemoveFromPlaylist(r.Id, r.Entries.ToArray()),
        F.Command.MovePlaylistEntry m => new M.CoreCommand.MovePlaylistEntry(m.Id, m.Entry, m.To),
        F.Command.ImportPlaylist i => new M.CoreCommand.ImportPlaylist(i.Path),
        F.Command.SetFavorite f => new M.CoreCommand.SetFavorite(FfiMapping.ToModel(f.Track), f.Favorite),
        F.Command.InstallPlugin i => new M.CoreCommand.InstallPlugin(i.Path),
        F.Command.UninstallPlugin u => new M.CoreCommand.UninstallPlugin(u.Id),
        F.Command.SetPluginEnabled p => new M.CoreCommand.SetPluginEnabled(p.Id, p.Enabled),
        F.Command.SetPluginSettings p => new M.CoreCommand.SetPluginSettings(p.Id, p.SettingsJson),
        F.Command.AcknowledgeUnofficial a => new M.CoreCommand.AcknowledgeUnofficial(a.Id),
        F.Command.BeginLogin b => new M.CoreCommand.BeginLogin(b.Plugin, FfiMapping.ToModel(b.Method)),
        F.Command.SubmitLogin s => new M.CoreCommand.SubmitLogin(s.Plugin, s.Flow, s.Fields.ToArray()),
        F.Command.CancelLogin x => new M.CoreCommand.CancelLogin(x.Plugin, x.Flow),
        F.Command.Logout l => new M.CoreCommand.Logout(l.Plugin),
        F.Command.Search s => new M.CoreCommand.Search(s.Query, s.Scope switch
        {
            F.SearchScope.Library => new M.SearchScope.Library(),
            F.SearchScope.Plugin p => new M.SearchScope.Plugin(p.Id),
            _ => new M.SearchScope.All(),
        }),
        F.Command.Browse b => new M.CoreCommand.Browse(b.Plugin, b.Node, b.Page),
        _ => throw new Xunit.Sdk.XunitException($"no model command for {c.GetType().Name}"),
    };

    private static M.Collection ToModel(F.Collection c) => c switch
    {
        F.Collection.Album a => new M.Collection.Album(a.Id),
        F.Collection.Artist a => new M.Collection.Artist(a.Id),
        F.Collection.Playlist p => new M.Collection.Playlist(p.Id),
        F.Collection.Favorites => new M.Collection.Favorites(),
        F.Collection.Tracks t => new M.Collection.Tracks(t.Value.Select(FfiMapping.ToModel).ToArray()),
        F.Collection.Source s => new M.Collection.Source(s.Plugin, s.Id),
        _ => throw new Xunit.Sdk.XunitException("unknown collection"),
    };

    /// <summary>JSON equality that ignores property order and number formatting.</summary>
    private static bool JsonEquivalent(JsonElement a, JsonElement b)
    {
        if (a.ValueKind != b.ValueKind)
        {
            return false;
        }

        switch (a.ValueKind)
        {
            case JsonValueKind.Object:
                var ap = a.EnumerateObject().ToDictionary(p => p.Name, p => p.Value);
                var bp = b.EnumerateObject().ToDictionary(p => p.Name, p => p.Value);
                return ap.Count == bp.Count && ap.All(kv => bp.TryGetValue(kv.Key, out var v) && JsonEquivalent(kv.Value, v));
            case JsonValueKind.Array:
                var aa = a.EnumerateArray().ToArray();
                var ba = b.EnumerateArray().ToArray();
                return aa.Length == ba.Length && aa.Zip(ba).All(p => JsonEquivalent(p.First, p.Second));
            case JsonValueKind.Number:
                return a.GetDouble().Equals(b.GetDouble());
            case JsonValueKind.String:
                return a.GetString() == b.GetString();
            default:
                return a.GetRawText() == b.GetRawText();
        }
    }
}

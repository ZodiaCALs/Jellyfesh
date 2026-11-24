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
                "EqPreset" => FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.EqPreset)!),
                "TrackQuery" => FfiMapping.ToFfi(new M.TrackQuery()),
                "CoreConfig" => json.Deserialize(F.JellyfeshJsonContext.Default.CoreConfig)!,
                _ => throw new Xunit.Sdk.XunitException($"unmapped sample {name}"),
            };
            Assert.NotNull(model);
            Assert.IsNotType<M.CoreEvent.Error>(model is M.CoreEvent e && e is M.CoreEvent.Error err && err.Message.StartsWith("unknown", StringComparison.Ordinal) ? model : null);
            mapped[name] = mapped.GetValueOrDefault(name) + 1;
        }

        Assert.True(mapped["Event"] >= 18, "every event variant has a sample");
        Assert.True(mapped["Command"] >= 41, "every command variant has a sample");
    }

    [Fact]
    public void The_remote_slice_maps_with_its_track_devices_and_error()
    {
        using var contract = Contract.Read("ffi-types.json");
        var events = contract.RootElement.GetProperty("samples").EnumerateArray()
            .Where(s => s.GetProperty("name").GetString() == "Event")
            .Select(s => s.GetProperty("json"))
            .Where(j => j.GetProperty("type").GetString() == "Remote")
            .Select(j => Assert.IsType<M.CoreEvent.Remote>(FfiMapping.ToModel(j.Deserialize(F.JellyfeshJsonContext.Default.Event)!)).State)
            .ToList();
        Assert.Equal(2, events.Count);

        var session = events.Single(r => r.HasSession);
        Assert.Equal(("org.x", "Example Source", M.PlaybackStatus.Playing), (session.Source, session.SourceName, session.Status));
        Assert.Equal(("Lantern", "Sample Artist", "https://music.example.com/song/42"), (session.Item!.Title, session.Item.Subtitle, session.Item.Link));
        Assert.Equal(M.ItemKind.Album, session.Context!.Kind);
        Assert.Equal(new M.PlaybackAnchor(3, 12_345_678, 987_654_321_012, 1f, 184_000_000), session.Anchor);
        Assert.Equal((true, M.RepeatMode.One), (session.Shuffle, session.Repeat));
        Assert.Equal(new M.RemoteDevice("phone", "Phone", M.RemoteDeviceKind.Smartphone, true, 70), session.Device);
        Assert.Equal([new M.RemoteDevice("desk", "Desk Speaker", M.RemoteDeviceKind.Speaker, false, null), session.Device!], session.Devices);
        Assert.Equal("the service asked to slow down", session.Error);

        var ended = events.Single(r => !r.HasSession);
        Assert.Null(ended.Device);
        Assert.Empty(ended.Devices);

        // The snapshot carries the slice too; links and the remote-playback flag come through.
        var snapshot = contract.RootElement.GetProperty("samples").EnumerateArray()
            .First(s => s.GetProperty("name").GetString() == "Snapshot").GetProperty("json");
        var state = FfiMapping.ToModel(snapshot.Deserialize(F.JellyfeshJsonContext.Default.Snapshot)!);
        Assert.Equal("org.x", state.Remote.Source);
        Assert.Equal(M.RemoteState.Initial, FfiMapping.ToModel(snapshot.Deserialize(F.JellyfeshJsonContext.Default.Snapshot)! with { Remote = null }).Remote);
        var plugin = FfiMapping.ToModel(new F.PluginSummary("org.spotify", "Spotify", "1.0.0", "Someone", null, F.PluginBadge.Community, null, false, [], [], true, null, null, false, null, "{}", "{}", null, RemotePlayback: true));
        Assert.True(plugin.RemotePlayback);
        var np = FfiMapping.ToModel(new F.NowPlaying(1, new F.TrackRef.Library(2), null, "T", null, null, null, null, null, "https://music.example.com/song/2"));
        Assert.Equal("https://music.example.com/song/2", np.Link);
    }

    [Fact]
    public void Remote_actions_serialise_as_the_core_expects()
    {
        string Json(M.CoreCommand c) => JsonSerializer.Serialize(FfiMapping.ToFfi(c), F.JellyfeshJsonContext.Default.Command);
        Assert.Equal("""{"type":"Remote","action":{"type":"Refresh"}}""", Json(new M.CoreCommand.Remote(new M.RemoteAction.Refresh())));
        Assert.Equal("""{"type":"Remote","action":{"type":"Transfer","device":"phone"}}""", Json(new M.CoreCommand.Remote(new M.RemoteAction.Transfer("phone"))));
        Assert.Equal("""{"type":"Remote","action":{"type":"SetVolume","percent":100}}""", Json(new M.CoreCommand.Remote(new M.RemoteAction.SetVolume(140))));
        Assert.Equal("""{"type":"Remote","action":{"type":"End"}}""", Json(new M.CoreCommand.Remote(new M.RemoteAction.End())));
    }

    [Fact]
    public void Eq_presets_map_from_the_binding_with_their_ids_and_bands()
    {
        using var contract = Contract.Read("ffi-types.json");
        var json = contract.RootElement.GetProperty("samples").EnumerateArray()
            .First(s => s.GetProperty("name").GetString() == "EqPreset")
            .GetProperty("json");
        var preset = FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.EqPreset)!);
        Assert.Equal("bass_boost", preset.Id);
        Assert.True(preset.Settings.Enabled);
        Assert.Equal(0f, preset.Settings.PreampDb);
        Assert.Equal(10, preset.Settings.Bands.Count);
        Assert.Equal(new M.EqBand(M.EqBandKind.Peaking, 31f, 6f, 1.41f, true), preset.Settings.Bands[0]);
        Assert.Equal(16_000f, preset.Settings.Bands[^1].FreqHz);
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
    public void Presets_map_from_the_sources_slice_and_connect_source_goes_back()
    {
        using var contract = Contract.Read("ffi-types.json");
        var json = contract.RootElement.GetProperty("samples").EnumerateArray()
            .Where(s => s.GetProperty("name").GetString() == "Event")
            .Select(s => s.GetProperty("json"))
            .First(j => j.GetProperty("type").GetString() == "Sources");
        var sources = Assert.IsType<M.CoreEvent.Sources>(FfiMapping.ToModel(json.Deserialize(F.JellyfeshJsonContext.Default.Event)!)).State;
        var preset = Assert.Single(sources.Presets);
        Assert.Equal(("navidrome", "Navidrome", "org.jellyfesh.subsonic"), (preset.Id, preset.Name, preset.Plugin));
        Assert.Equal([M.AuthMethod.PasswordExchange, M.AuthMethod.ApiKey], preset.AuthMethods);
        Assert.True(preset.NeedsAddress);
        Assert.Equal(("https://music.example.com", "http://192.168.1.20:4533"), (preset.Address, preset.AddressExample));
        Assert.Null(preset.Account);
        Assert.Single(sources.Plugins);

        string Json(M.CoreCommand c) => JsonSerializer.Serialize(FfiMapping.ToFfi(c), F.JellyfeshJsonContext.Default.Command);
        Assert.Equal(
            """{"type":"ConnectSource","preset":"navidrome","address":"http://192.168.1.20:4533","method":"PasswordExchange"}""",
            Json(new M.CoreCommand.ConnectSource("navidrome", "http://192.168.1.20:4533", M.AuthMethod.PasswordExchange)));
        Assert.Equal(
            """{"type":"ConnectSource","preset":"jellyfin","address":null,"method":"Qr"}""",
            Json(new M.CoreCommand.ConnectSource("jellyfin", null, M.AuthMethod.Qr)));
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
            Eq = Samples.Eq(true, -1.5f, 6, 5, 4),
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
        Assert.Equal(M.StreamingQuality.Auto, M.AppSettings.Default.StreamingQuality);

        // The binding's defaults are the core's (tests/contract/ffi-types.json's default SetSettings).
        Assert.Equal(new F.DownloadSettings(), FfiMapping.ToFfi(M.DownloadSettings.Default));
        Assert.Equal(new F.MiniPlayerSettings(), FfiMapping.ToFfi(M.MiniPlayerSettings.Default));
    }

    [Fact]
    public void Settings_never_send_the_core_a_null_downloads_or_mini_player()
    {
        // The core refuses "downloads": null; a record built without them sends its defaults.
        Assert.Equal((M.DownloadSettings.Default, M.MiniPlayerSettings.Default), (M.AppSettings.Default.Downloads, M.AppSettings.Default.MiniPlayer));
        var without = M.AppSettings.Default with { Downloads = null, MiniPlayer = null };
        var json = JsonSerializer.Serialize(FfiMapping.ToFfi(new M.CoreCommand.SetSettings(without)), F.JellyfeshJsonContext.Default.Command);
        Assert.DoesNotContain("\"downloads\":null", json, StringComparison.Ordinal);
        Assert.DoesNotContain("\"mini_player\":null", json, StringComparison.Ordinal);
        var sent = Assert.IsType<F.Command.SetSettings>(JsonSerializer.Deserialize(json, F.JellyfeshJsonContext.Default.Command)).Settings;
        Assert.Equal((new F.DownloadSettings(), new F.MiniPlayerSettings()), (sent.Downloads, sent.MiniPlayer));

        var chosen = M.AppSettings.Default with
        {
            StreamingQuality = M.StreamingQuality.Low,
            Downloads = M.DownloadSettings.Default with { WifiOnly = false, AtOnce = 9 },
            MiniPlayer = M.MiniPlayerSettings.Default with { KeepOnTop = false },
        };
        var back = FfiMapping.ToModel(FfiMapping.ToFfi(chosen));
        Assert.Equal(M.StreamingQuality.Low, back.StreamingQuality);
        Assert.Equal(M.DownloadSettings.Default with { WifiOnly = false, AtOnce = M.DownloadSettings.MaxAtOnce }, back.Downloads);
        Assert.Equal(M.MiniPlayerSettings.Default with { KeepOnTop = false }, back.MiniPlayer);
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
        F.Command.ConnectSource cs => new M.CoreCommand.ConnectSource(cs.Preset, cs.Address, FfiMapping.ToModel(cs.Method)),
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
        F.Command.Remote r => new M.CoreCommand.Remote(r.Action switch
        {
            F.RemoteAction.Refresh => new M.RemoteAction.Refresh(),
            F.RemoteAction.Transfer t => new M.RemoteAction.Transfer(t.Device),
            F.RemoteAction.SetVolume v => new M.RemoteAction.SetVolume(v.Percent),
            F.RemoteAction.End => new M.RemoteAction.End(),
            _ => throw new Xunit.Sdk.XunitException("unknown remote action"),
        }),
        F.Command.ResetPluginSettings r => new M.CoreCommand.ResetPluginSettings(r.Plugin),
        F.Command.MakeLyrics m => new M.CoreCommand.MakeLyrics(ToModel(m.Song)),
        F.Command.CancelLyrics x => new M.CoreCommand.CancelLyrics(x.Job),
        F.Command.DeleteMadeLyrics d => new M.CoreCommand.DeleteMadeLyrics(ToModel(d.Song)),
        F.Command.AllowAudioUpload a => new M.CoreCommand.AllowAudioUpload(a.Plugin, a.Origin),
        F.Command.RevokeAudioUpload r => new M.CoreCommand.RevokeAudioUpload(r.Plugin, r.Origin),
        F.Command.DownloadSpeechModel d => new M.CoreCommand.DownloadSpeechModel(d.Id),
        F.Command.CancelSpeechModelDownload x => new M.CoreCommand.CancelSpeechModelDownload(x.Id),
        F.Command.DeleteSpeechModel d => new M.CoreCommand.DeleteSpeechModel(d.Id),
        F.Command.ActivateLicence a => new M.CoreCommand.ActivateLicence(a.Key),
        F.Command.RemoveLicence => new M.CoreCommand.RemoveLicence(),
        F.Command.CheckForUpdates => new M.CoreCommand.CheckForUpdates(),
        F.Command.Discover d => new M.CoreCommand.Discover(d.Source),
        F.Command.Details d => new M.CoreCommand.Details(FfiMapping.ToModel(d.Item)),
        F.Command.Comments x => new M.CoreCommand.Comments(FfiMapping.ToModel(x.Item), x.Page),
        F.Command.SetNetworkState n => new M.CoreCommand.SetNetworkState(n.Online, n.Metered),
        F.Command.SearchMore m => new M.CoreCommand.SearchMore(m.Plugin, FfiMapping.ToModel(m.Kind), m.Cursor),
        F.Command.RefreshSourceLibrary r => new M.CoreCommand.RefreshSourceLibrary(r.Plugin),
        F.Command.SourceLibraryMore m => new M.CoreCommand.SourceLibraryMore(m.Plugin, FfiMapping.ToModel(m.Part), m.Cursor),
        F.Command.SavePlaylistCopy s => new M.CoreCommand.SavePlaylistCopy(s.Plugin, s.Id, s.Name),
        F.Command.Download d => new M.CoreCommand.Download(d.Target switch
        {
            F.DownloadTarget.Track track => new M.DownloadTarget.Track(FfiMapping.ToModel(track.Value)),
            F.DownloadTarget.Collection col => new M.DownloadTarget.Collection(col.Plugin, col.Id, FfiMapping.ToModel(col.ItemKind)),
            _ => throw new Xunit.Sdk.XunitException("unknown download target"),
        }),
        F.Command.CancelDownload x => new M.CoreCommand.CancelDownload(x.Id),
        F.Command.RetryDownload r => new M.CoreCommand.RetryDownload(r.Id),
        F.Command.RemoveDownload r => new M.CoreCommand.RemoveDownload(r.Id),
        F.Command.RemoveAllDownloads => new M.CoreCommand.RemoveAllDownloads(),
        _ => throw new Xunit.Sdk.XunitException($"no model command for {c.GetType().Name}"),
    };

    private static M.Collection ToModel(F.Collection c) => c switch
    {
        F.Collection.Album a => new M.Collection.Album(a.Id),
        F.Collection.Artist a => new M.Collection.Artist(a.Id),
        F.Collection.Playlist p => new M.Collection.Playlist(p.Id),
        F.Collection.Favorites => new M.Collection.Favorites(),
        F.Collection.Songs s => new M.Collection.Songs(FfiMapping.ToModel(s.Sort), s.Descending),
        F.Collection.Tracks t => new M.Collection.Tracks(t.Value.Select(FfiMapping.ToModel).ToArray()),
        F.Collection.Source s => new M.Collection.Source(s.Plugin, s.Id, FfiMapping.ToModel(s.ItemKind)),
        _ => throw new Xunit.Sdk.XunitException("unknown collection"),
    };

    private static M.SongRef ToModel(F.SongRef s) => s switch
    {
        F.SongRef.Library l => new M.SongRef.Library(l.Id),
        F.SongRef.Current => new M.SongRef.Current(),
        _ => throw new Xunit.Sdk.XunitException("unknown song"),
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

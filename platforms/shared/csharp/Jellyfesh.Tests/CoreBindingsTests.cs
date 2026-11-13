using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization.Metadata;
using System.Threading;
using Jellyfesh.Ffi;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Jellyfesh.Ffi against the jf-ffi library built here: tests/contract/core-scenario.json.</summary>
public sealed class CoreBindingsTests : IDisposable
{
    private readonly string _dir = Directory.CreateTempSubdirectory("jf-csharp-").FullName;

    public void Dispose()
    {
        try
        {
            Directory.Delete(_dir, recursive: true);
        }
        catch (IOException)
        {
            // A file still open on a slow machine: the temp folder is cleaned later.
        }
    }

    [Fact]
    public void TheSharedScenario()
    {
        using JsonDocument document = Contract.Read("core-scenario.json");
        JsonElement scenario = document.RootElement;
        JsonElement expect = scenario.GetProperty("expect");
        TimeSpan timeout = TimeSpan.FromMilliseconds(scenario.GetProperty("timeout_ms").GetInt32());
        TimeSpan quiet = TimeSpan.FromMilliseconds(scenario.GetProperty("quiet_ms").GetInt32());
        TimeSpan every = TimeSpan.FromMilliseconds(scenario.GetProperty("poll_ms").GetInt32());
        string music = WriteTones(Path.Combine(_dir, "music"), scenario);
        JsonElement config = scenario.GetProperty("config");

        // 1. Start with the scenario's config and temporary folders.
        using var core = new JellyfeshCore(new CoreConfig(
            DataDir: Path.Combine(_dir, "data"),
            CacheDir: Path.Combine(_dir, "cache"),
            Output: Output(config.GetProperty("output")),
            BufferMs: config.GetProperty("buffer_ms").GetUInt32(),
            RestoreSession: config.GetProperty("restore_session").GetBoolean(),
            PersistSession: config.GetProperty("persist_session").GetBoolean(),
            WatchFolders: config.GetProperty("watch_folders").GetBoolean()));
        var events = new BlockingCollection<Event>();
        int inCall = 0;
        bool overlapped = false;
        core.SetListener(new DelegateListener(e =>
        {
            if (Interlocked.Increment(ref inCall) > 1)
            {
                overlapped = true;
            }
            events.Add(e);
            Interlocked.Decrement(ref inCall);
        }));

        // 2. AddRoot scans the folder.
        Assert.True(core.Send(new Command.AddRoot(music)) > 0);
        List<Event> scan = WaitFor(events, timeout, e => e is Event.Scan { Status: ScanStatus.Finished });
        ScanReport report = ((ScanStatus.Finished)((Event.Scan)scan[^1]).Status).Report;
        Assert.Equal(expect.GetProperty("scan_added").GetUInt64(), report.Added);

        // 3. Query and search the library.
        IReadOnlyList<Track> tracks = core.Tracks(new TrackQuery(Sort: TrackSort.Path));
        JsonElement search = scenario.GetProperty("search");
        SearchResults found = core.SearchLibrary(search.GetProperty("text").GetString()!, search.GetProperty("limit").GetUInt32());
        Assert.Equal(Strings(search.GetProperty("titles")), found.Tracks.Select(t => t.Title));
        Assert.Equal(new[] { music }, core.Roots());
        List<TrackRef> refs = tracks.Select(t => (TrackRef)new TrackRef.Library(t.Id)).ToList();

        // 4. A playlist of every track.
        string name = scenario.GetProperty("playlist").GetProperty("name").GetString()!;
        ulong request = core.Send(new Command.CreatePlaylist(name, refs));
        List<Event> created = WaitFor(events, timeout, e => e is Event.PlaylistCreated p && p.Request == request);
        long playlist = ((Event.PlaylistCreated)created[^1]).Id;
        PlaylistSummary summary = core.Playlist(playlist)!;
        Assert.Equal((name, 2u), (summary.Name, summary.EntryCount));
        Assert.Equal(Strings(expect.GetProperty("playlist_titles")), core.PlaylistEntries(playlist, 0, 50).Select(e => e.Row.Title));
        Assert.Contains(core.Playlists(), p => p.Id == playlist);

        // 5. A favourite.
        string favoriteTitle = scenario.GetProperty("favorite").GetString()!;
        TrackRef favorite = refs[tracks.ToList().FindIndex(t => t.Title == favoriteTitle)];
        core.Send(new Command.SetFavorite(favorite, true));
        Poll(timeout, every, "the favourite", () => core.IsFavorite(favorite));
        Assert.Equal(Strings(expect.GetProperty("favorite_titles")), core.Favorites(0, 50).Select(r => r.Title));

        // 6. Export.
        string export = Path.Combine(_dir, scenario.GetProperty("playlist").GetProperty("export_file").GetString()!);
        core.ExportPlaylist(playlist, export);
        string text = File.ReadAllText(export);
        foreach (string needle in Strings(scenario.GetProperty("playlist").GetProperty("export_contains")))
        {
            Assert.Contains(needle, text);
        }

        // 7. Play the playlist.
        Assert.True(core.Send(new Command.PlayCollection(new Collection.Playlist(playlist), 0, false)) > 0);
        Status final = Enum.Parse<Status>(expect.GetProperty("final_status").GetString()!);
        List<Event> seen = WaitFor(events, timeout, e => e is Event.Playback p && p.Snapshot.Status == final);
        seen.AddRange(Drain(events, quiet));
        // Consecutive announcements of the same entry count once (see the scenario's "about").
        List<NowPlaying> announced = seen.OfType<Event.NowPlaying>().Select(e => e.Snapshot).OfType<NowPlaying>().ToList();
        IEnumerable<string> titles = announced.Where((np, i) => i == 0 || announced[i - 1].Uid != np.Uid).Select(np => np.Title);
        Assert.Equal(Strings(expect.GetProperty("now_playing_titles")), titles);
        Snapshot snapshot = core.Snapshot();
        Assert.Equal(final, snapshot.Playback.Status);
        Assert.Equal(expect.GetProperty("queue_len").GetUInt32(), snapshot.Queue.Len);
        AssertSameJson(seen.OfType<Event.Playback>().Last().Snapshot, snapshot.Playback, JellyfeshJsonContext.Default.PlaybackSnapshot);
        AssertSameJson(seen.OfType<Event.Queue>().Last().Snapshot, snapshot.Queue, JellyfeshJsonContext.Default.QueueSnapshot);
        Assert.Equal(seen.OfType<Event.NowPlaying>().Last().Snapshot, snapshot.NowPlaying);
        Assert.False(overlapped, "listener calls overlapped");

        // 8. Play statistics.
        List<string> played = Strings(expect.GetProperty("played_titles"));
        Poll(timeout, every, "the plays", () =>
            core.RecentlyPlayed(50).Select(r => r.Title).Distinct().Order(StringComparer.Ordinal).SequenceEqual(played));

        // 9. Sorted albums and thumbnails.
        Assert.Empty(core.AlbumsSorted(AlbumSort.Title, false, 0, 50));
        Assert.Null(core.ArtworkThumbnail(scenario.GetProperty("unknown_artwork_hash").GetString()!, 128));

        // 10. Shutdown; errors are CoreFfiExceptions.
        core.Shutdown();
        CoreFfiException error = Assert.Throws<CoreFfiException>(() => core.Stats());
        Assert.Equal(CoreFfiErrorKind.ShutDown, error.Kind);
        Assert.Equal(0ul, core.Send(new Command.Play()));
        AssertSameJson(snapshot, core.Snapshot(), JellyfeshJsonContext.Default.Snapshot);
        Drain(events, TimeSpan.FromMilliseconds(100));
        Assert.False(events.TryTake(out _, TimeSpan.FromMilliseconds(200)), "no events after shutdown");
    }

    [Fact]
    public void ListenersErrorsAndFreeFunctions()
    {
        // A core that cannot start throws.
        string file = Path.Combine(_dir, "a file");
        File.WriteAllText(file, "");
        CoreFfiException failed = Assert.Throws<CoreFfiException>(() => new JellyfeshCore(
            new CoreConfig(Path.Combine(file, "data"), Path.Combine(_dir, "cache"), new OutputKind.Null(false))));
        Assert.Equal(CoreFfiErrorKind.Core, failed.Kind);

        using var core = new JellyfeshCore(new CoreConfig(
            Path.Combine(_dir, "data"), Path.Combine(_dir, "cache"), new OutputKind.Null(false),
            RestoreSession: false, PersistSession: false, WatchFolders: false));

        // A throwing listener is reported and does not stop the stream.
        var errors = new BlockingCollection<Exception>();
        Action<Exception> report = errors.Add;
        JellyfeshCore.ListenerFailed += report;
        try
        {
            var events = new BlockingCollection<Event>();
            int calls = 0;
            core.SetListener(new DelegateListener(e =>
            {
                if (Interlocked.Increment(ref calls) == 1)
                {
                    throw new InvalidOperationException("listener bug");
                }
                events.Add(e);
            }));
            core.Send(new Command.SetVolume(0.5f));
            core.Send(new Command.SetVolume(0.25f));
            WaitFor(events, TimeSpan.FromSeconds(10), e => e is Event.Playback p && p.Snapshot.Volume == 0.25f);
            Assert.True(errors.TryTake(out Exception? caught, TimeSpan.FromSeconds(10)));
            Assert.Equal("listener bug", caught!.Message);

            // After SetListener(null) returns, the old listener is not called again.
            core.SetListener(null);
            int before = Volatile.Read(ref calls);
            core.Send(new Command.SetVolume(0.75f));
            Thread.Sleep(300);
            Assert.Equal(before, Volatile.Read(ref calls));
        }
        finally
        {
            JellyfeshCore.ListenerFailed -= report;
        }

        // A failed command comes back as an event with the id Send returned.
        var failures = new BlockingCollection<Event>();
        core.SetListener(new DelegateListener(failures.Add));
        ulong id = core.Send(new Command.PlayTracks(Array.Empty<TrackRef>(), 0));
        WaitFor(failures, TimeSpan.FromSeconds(10), e => e is Event.CommandFailed f && f.Id == id && f.Message.Length > 0);

        Assert.Equal(new List<PlaylistSummary>(), core.Playlists());
        Assert.Null(core.Playlist(99));
        Assert.Null(core.Track(12345));
        Assert.False(string.IsNullOrEmpty(JellyfeshMethods.LibraryVersion()));
        ulong a = JellyfeshMethods.MonotonicNowNs();
        Thread.Sleep(2);
        Assert.True(JellyfeshMethods.MonotonicNowNs() >= a + 1_000_000);
        try
        {
            Assert.NotNull(JellyfeshMethods.OutputDevices());
        }
        catch (CoreFfiException e)
        {
            Assert.Equal(CoreFfiErrorKind.Core, e.Kind); // no audio system on this machine
        }
        if (Environment.GetEnvironmentVariable("JF_DATA_DIR") is null
            && (Environment.GetEnvironmentVariable("HOME") ?? Environment.GetEnvironmentVariable("APPDATA")) is not null)
        {
            Assert.Contains("jellyfesh", JellyfeshMethods.DefaultDataDir()!, StringComparison.OrdinalIgnoreCase);
            Assert.NotNull(JellyfeshMethods.DefaultCacheDir());
        }
        core.Dispose();
        Assert.Throws<ObjectDisposedException>(() => core.Stats());
    }

    /// <summary>The app's secret store: records each key asked for; every call throws when locked.</summary>
    private sealed class Keychain(bool locked) : SecretStore
    {
        public ConcurrentQueue<string> Asked { get; } = new();

        public string? Get(string key)
        {
            Answer(key);
            return null;
        }

        public void Set(string key, string value) => Answer(key);

        public void Delete(string key) => Answer(key);

        private void Answer(string key)
        {
            Asked.Enqueue(key);
            if (locked)
            {
                throw new SecretStoreException("the keychain is locked");
            }
        }
    }

    [Theory]
    [InlineData(false, SecretStorage.Keystore)]
    [InlineData(true, SecretStorage.SessionOnly)]
    public void TheAppsSecretStoreKeepsTheSignIns(bool locked, SecretStorage storage)
    {
        var keychain = new Keychain(locked);
        using var core = new JellyfeshCore(
            new CoreConfig(
                Path.Combine(_dir, "data"), Path.Combine(_dir, "cache"), new OutputKind.Null(false),
                RestoreSession: false, PersistSession: false, WatchFolders: false),
            keychain);
        // The core checks the store once at start by reading an entry no plugin can name.
        Poll(TimeSpan.FromSeconds(10), TimeSpan.FromMilliseconds(10), "the check", () => !keychain.Asked.IsEmpty);
        Assert.Equal("jellyfesh/~probe", keychain.Asked.First());
        Poll(TimeSpan.FromSeconds(10), TimeSpan.FromMilliseconds(10), "the slice",
            () => core.Snapshot().Sources.SecretStorage == storage);
        Assert.Null(core.Snapshot().Sources.Unavailable);
        core.Dispose();
        int asked = keychain.Asked.Count;
        Thread.Sleep(100);
        Assert.Equal(asked, keychain.Asked.Count);
    }

    private static OutputKind Output(JsonElement json) => json.GetProperty("type").GetString() switch
    {
        "Null" => new OutputKind.Null(json.GetProperty("realtime").GetBoolean()),
        "Device" => new OutputKind.Device(json.TryGetProperty("name", out JsonElement n) ? n.GetString() : null),
        var other => throw new InvalidOperationException($"unsupported output {other}"),
    };

    private static List<string> Strings(JsonElement array) => array.EnumerateArray().Select(e => e.GetString()!).ToList();

    private static List<Event> WaitFor(BlockingCollection<Event> events, TimeSpan timeout, Func<Event, bool> done)
    {
        DateTime end = DateTime.UtcNow + timeout;
        var seen = new List<Event>();
        while (true)
        {
            TimeSpan left = end - DateTime.UtcNow;
            if (left <= TimeSpan.Zero || !events.TryTake(out Event? ev, left))
            {
                throw new Xunit.Sdk.XunitException("timed out; events seen: " + string.Join("\n", seen));
            }
            seen.Add(ev);
            if (done(ev))
            {
                return seen;
            }
        }
    }

    private static List<Event> Drain(BlockingCollection<Event> events, TimeSpan quiet)
    {
        var seen = new List<Event>();
        while (events.TryTake(out Event? ev, quiet))
        {
            seen.Add(ev);
        }
        return seen;
    }

    private static void Poll(TimeSpan timeout, TimeSpan every, string what, Func<bool> probe)
    {
        DateTime end = DateTime.UtcNow + timeout;
        while (!probe())
        {
            if (DateTime.UtcNow > end)
            {
                throw new Xunit.Sdk.XunitException($"timed out waiting for {what}");
            }
            Thread.Sleep(every);
        }
    }

    /// <summary>Records holding lists compare those by reference, so compare their JSON.</summary>
    private static void AssertSameJson<T>(T expected, T actual, JsonTypeInfo<T> info)
    {
        using JsonDocument a = JsonDocument.Parse(JsonSerializer.Serialize(expected, info));
        using JsonDocument b = JsonDocument.Parse(JsonSerializer.Serialize(actual, info));
        Assert.True(Contract.Same(a.RootElement, b.RootElement), $"{a.RootElement.GetRawText()}\n!=\n{b.RootElement.GetRawText()}");
    }

    /// <summary>Writes the scenario's tones as 16-bit stereo WAV files.</summary>
    private static string WriteTones(string folder, JsonElement scenario)
    {
        Directory.CreateDirectory(folder);
        int rate = scenario.GetProperty("sample_rate").GetInt32();
        foreach (JsonElement tone in scenario.GetProperty("tones").EnumerateArray())
        {
            double frequency = tone.GetProperty("frequency_hz").GetDouble();
            int frames = (int)(rate * tone.GetProperty("seconds").GetDouble());
            using var writer = new BinaryWriter(File.Create(Path.Combine(folder, tone.GetProperty("file").GetString()!)));
            writer.Write("RIFF"u8);
            writer.Write(36 + frames * 4);
            writer.Write("WAVEfmt "u8);
            writer.Write(16);
            writer.Write((short)1);
            writer.Write((short)2);
            writer.Write(rate);
            writer.Write(rate * 4);
            writer.Write((short)4);
            writer.Write((short)16);
            writer.Write("data"u8);
            writer.Write(frames * 4);
            for (int i = 0; i < frames; i++)
            {
                short sample = (short)(Math.Sin(2 * Math.PI * frequency * i / rate) * 0.25 * short.MaxValue);
                writer.Write(sample);
                writer.Write(sample);
            }
        }
        return Path.GetFullPath(folder);
    }
}

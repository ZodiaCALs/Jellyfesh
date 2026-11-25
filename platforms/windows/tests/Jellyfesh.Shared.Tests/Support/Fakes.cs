using System.Reflection;
using System.Text.Json;
using Jellyfesh.Core;
using Jellyfesh.Drawing;

namespace Jellyfesh.Tests.Support;

/// <summary>A scriptable core: records commands, answers queries from fields, emits events on demand.</summary>
internal sealed class FakeCore : ICoreClient
{
    private Action<CoreEvent>? _listener;
    private ulong _nextId = 1;

    public List<CoreCommand> Sent { get; } = [];

    public CoreSnapshot State { get; set; } = CoreSnapshot.Initial;

    public List<Album> AlbumRows { get; } = [];

    public List<Artist> ArtistRows { get; } = [];

    public List<Track> TrackRows { get; } = [];

    public List<TrackRow> FavoriteRows { get; } = [];

    public List<PlaylistSummary> PlaylistRows { get; } = [];

    public Dictionary<long, List<PlaylistEntry>> PlaylistEntryRows { get; } = [];

    public List<QueueItem> QueueRows { get; } = [];

    public List<string> RootRows { get; } = [];

    public LibraryStats StatsValue { get; set; } = new(0, 0, 0, 0, 0, 0, 0);

    /// <summary>What EqPresets answers: the core's four presets, as tests/contract/ffi-types.json has them.</summary>
    public List<EqPreset> EqPresetRows { get; } = Samples.EqPresets().ToList();

    /// <summary>The order the last AlbumsSorted asked for.</summary>
    public (AlbumSort Sort, bool Descending)? LastAlbumSort { get; private set; }

    /// <summary>Every Tracks query, in order (pages load in the background, so tests look for theirs).</summary>
    public List<TrackQuery> TrackQueries { get; } = [];

    public Func<string, uint, string?> Thumbnail { get; set; } = (hash, size) => $"/cache/{hash}/{size}.jpg";

    public CoreQueryException? FailQueriesWith { get; set; }

    public int QueryCount { get; private set; }

    public int SnapshotCount { get; private set; }

    public ulong NowNs { get; set; } = 1_000_000_000;

    public bool HasListener => _listener is not null;

    public ulong Send(CoreCommand command)
    {
        Sent.Add(command);
        return _nextId++;
    }

    public void Emit(CoreEvent e) => _listener?.Invoke(e);

    public T LastSent<T>()
        where T : CoreCommand => Sent.OfType<T>().Last();

    /// <summary>The id the last command of type <typeparamref name="T"/> was sent with (ids count from 1).</summary>
    public ulong LastSentId<T>()
        where T : CoreCommand => (ulong)(Sent.FindLastIndex(c => c is T) + 1);

    public CoreSnapshot Snapshot()
    {
        SnapshotCount++;
        return State;
    }

    public void SetListener(Action<CoreEvent>? listener) => _listener = listener;

    public IReadOnlyList<Album> Albums(uint offset, uint limit) => Page(AlbumRows, offset, limit);

    public IReadOnlyList<Album> AlbumsSorted(AlbumSort sort, bool descending, uint offset, uint limit)
    {
        LastAlbumSort = (sort, descending);
        return Page(AlbumRows, offset, limit);
    }

    public IReadOnlyList<Artist> Artists(uint offset, uint limit) => Page(ArtistRows, offset, limit);

    public IReadOnlyList<Track> Tracks(TrackQuery query)
    {
        lock (TrackQueries)
        {
            TrackQueries.Add(query);
        }

        var rows = TrackRows.Where(t =>
            (query.AlbumId is null || t.AlbumId == query.AlbumId)).ToList();
        return Page(rows, query.Offset, query.Limit);
    }

    public Track? Track(long id)
    {
        Count();
        return TrackRows.FirstOrDefault(t => t.Id == id);
    }

    public IReadOnlyList<Track> AlbumTracks(long albumId)
    {
        Count();
        return TrackRows.Where(t => t.AlbumId == albumId).ToArray();
    }

    public IReadOnlyList<Album> ArtistAlbums(long artistId)
    {
        Count();
        return AlbumRows.ToArray();
    }

    public SearchResults SearchLibrary(string text, uint limit)
    {
        Count();
        return new SearchResults(
            TrackRows.Where(t => t.Title.Contains(text, StringComparison.OrdinalIgnoreCase)).Take((int)limit).ToArray(),
            AlbumRows.Where(a => a.Title.Contains(text, StringComparison.OrdinalIgnoreCase)).Take((int)limit).ToArray(),
            ArtistRows.Where(a => a.Name.Contains(text, StringComparison.OrdinalIgnoreCase)).Take((int)limit).ToArray());
    }

    public IReadOnlyList<TrackRef> TracksForQuery(string text, uint limit) =>
        SearchLibrary(text, limit).Tracks.Select(t => (TrackRef)new TrackRef.Library(t.Id)).ToArray();

    public IReadOnlyList<Track> RecentlyAdded(uint limit) => Page(TrackRows, 0, limit);

    public IReadOnlyList<TrackRow> RecentlyPlayed(uint limit) => Page(FavoriteRows, 0, limit);

    public IReadOnlyList<TrackRow> MostPlayed(uint limit) => Page(FavoriteRows, 0, limit);

    public IReadOnlyList<PlaylistSummary> Playlists() => Page(PlaylistRows, 0, uint.MaxValue);

    public PlaylistSummary? Playlist(long id)
    {
        Count();
        return PlaylistRows.FirstOrDefault(p => p.Id == id);
    }

    public IReadOnlyList<PlaylistEntry> PlaylistEntries(long id, uint offset, uint limit) =>
        Page(PlaylistEntryRows.GetValueOrDefault(id) ?? [], offset, limit);

    public List<(long Id, string Path)> Exports { get; } = [];

    public void ExportPlaylist(long id, string path)
    {
        Count();
        Exports.Add((id, path));
    }

    public IReadOnlyList<TrackRow> Favorites(uint offset, uint limit) => Page(FavoriteRows, offset, limit);

    public bool IsFavorite(TrackRef track)
    {
        Count();
        return FavoriteRows.Any(r => r.Track == track);
    }

    public string? ArtworkThumbnail(string hash, uint sizePx)
    {
        Count();
        return Thumbnail(hash, sizePx);
    }

    public IReadOnlyList<QueueItem> QueueWindow(uint offset, uint limit) => Page(QueueRows, offset, limit);

    public IReadOnlyList<HistoryEntry> History() => [];

    public IReadOnlyList<string> Roots() => Page(RootRows, 0, uint.MaxValue);

    public LibraryStats Stats()
    {
        Count();
        return StatsValue;
    }

    public IReadOnlyList<DeviceInfo> OutputDevices() => [];

    public IReadOnlyList<EqPreset> EqPresets()
    {
        Count();
        return EqPresetRows.ToArray();
    }

    /// <summary>What SourceArtwork answers: a cached path, or null while "fetching".</summary>
    public Func<string, uint, string?> SourceArtworkPath { get; set; } = (url, size) => null;

    /// <summary>Every SourceArtwork call, in order.</summary>
    public List<(string Url, uint SizePx)> ArtworkRequests { get; } = [];

    /// <summary>What LocalizedSettingsSchema answers, by plugin id.</summary>
    public Dictionary<string, string> LocalizedSchemas { get; } = [];

    /// <summary>The languages LocalizedSettingsSchema was asked for.</summary>
    public List<string> SchemaLanguages { get; } = [];

    public List<StoreEntitlement> Entitlements { get; } = [];

    public string? SourceArtwork(string url, uint sizePx)
    {
        Count();
        ArtworkRequests.Add((url, sizePx));
        return SourceArtworkPath(url, sizePx);
    }

    public string? LocalizedSettingsSchema(string plugin, string language)
    {
        Count();
        SchemaLanguages.Add(language);
        return LocalizedSchemas.GetValueOrDefault(plugin);
    }

    public void SetStoreEntitlement(StoreEntitlement entitlement)
    {
        Count();
        Entitlements.Add(entitlement);
    }

    public ulong MonotonicNowNs() => NowNs;

    public void Shutdown()
    {
    }

    public void Dispose()
    {
    }

    private IReadOnlyList<T> Page<T>(List<T> rows, uint offset, uint limit)
    {
        Count();
        return rows.Skip((int)Math.Min(offset, int.MaxValue)).Take((int)Math.Min(limit, int.MaxValue)).ToArray();
    }

    private void Count()
    {
        QueryCount++;
        if (FailQueriesWith is { } e)
        {
            throw e;
        }
    }
}

/// <summary>Runs posted work when asked, like a UI thread between frames.</summary>
internal sealed class QueueDispatcher : IUiDispatcher
{
    private readonly Queue<Action> _work = new();
    private readonly object _lock = new();

    public bool HasThreadAccess => true;

    public int Pending
    {
        get
        {
            lock (_lock)
            {
                return _work.Count;
            }
        }
    }

    public void Post(Action action)
    {
        lock (_lock)
        {
            _work.Enqueue(action);
        }
    }

    /// <summary>Runs everything queued, including work queued while running.</summary>
    public int RunAll()
    {
        var ran = 0;
        while (true)
        {
            Action? next;
            lock (_lock)
            {
                if (!_work.TryDequeue(out next))
                {
                    return ran;
                }
            }

            next();
            ran++;
        }
    }

    /// <summary>Waits for background work that posts here (paged loads), then runs it.</summary>
    public void RunUntil(Func<bool> done, int timeoutMs = 5_000)
    {
        var deadline = Environment.TickCount64 + timeoutMs;
        while (!done())
        {
            if (RunAll() == 0)
            {
                Thread.Sleep(2);
            }

            if (Environment.TickCount64 > deadline)
            {
                throw new TimeoutException("the condition did not become true");
            }
        }
    }
}

/// <summary>A clock the test moves by hand.</summary>
internal sealed class ManualClock : IMonotonicClock
{
    public ulong Now { get; set; } = 10_000_000_000;

    public ulong NowNs() => Now;

    public void AdvanceMs(double ms) => Now += (ulong)(ms * 1_000_000);
}

/// <summary>A timer the test fires by hand.</summary>
internal sealed class ManualTimer : IOneShotTimer
{
    public event EventHandler? Tick;

    public TimeSpan? Pending { get; private set; }

    public int Starts { get; private set; }

    public void Start(TimeSpan delay)
    {
        Pending = delay;
        Starts++;
    }

    public void Stop() => Pending = null;

    public void Fire()
    {
        Pending = null;
        Tick?.Invoke(this, EventArgs.Empty);
    }
}

/// <summary>The shared contract files in tests/contract/.</summary>
internal static class Contract
{
    public static string RepoRoot { get; } = FindRoot();

    public static JsonDocument Read(string name) =>
        JsonDocument.Parse(File.ReadAllText(Path.Combine(RepoRoot, "tests", "contract", name)));

    public static string PathOf(params string[] parts) => Path.Combine([RepoRoot, .. parts]);

    private static string FindRoot()
    {
        var root = typeof(Contract).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>()
            .FirstOrDefault(a => a.Key == "JfRepoRoot")?.Value;
        if (root is not null && Directory.Exists(Path.Combine(root, "tests", "contract")))
        {
            return root;
        }

        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            if (Directory.Exists(Path.Combine(dir.FullName, "tests", "contract")))
            {
                return dir.FullName;
            }
        }

        throw new InvalidOperationException("tests/contract not found");
    }
}

internal static class Samples
{
    /// <summary>The core's ten one-octave bands (jf-engine's preset layout), for building EQ settings in tests.</summary>
    public static readonly float[] BandFrequencies = [31f, 62f, 125f, 250f, 500f, 1_000f, 2_000f, 4_000f, 8_000f, 16_000f];

    /// <summary>Ten peaking bands at <see cref="BandFrequencies"/> with these gains (missing ones at 0 dB).</summary>
    public static EqSettings Eq(bool enabled, float preampDb, params float[] gains) =>
        new(enabled, preampDb, BandFrequencies.Select((f, i) => new EqBand(EqBandKind.Peaking, f, i < gains.Length ? gains[i] : 0f, 1.41f, true)).ToArray());

    /// <summary>The presets eq_presets() returns (core/jf-engine/src/dsp/eq.rs), as the fake core answers them.</summary>
    public static IEnumerable<EqPreset> EqPresets() =>
    [
        new("flat", Eq(true, 0f)),
        new("bass_boost", Eq(true, 0f, 6, 5, 4, 2, 0.5f)),
        new("vocal", Eq(true, 0f, -2, -2, -1, 0, 1.5f, 3, 3, 2, 0, -1)),
        new("treble", Eq(true, 0f, 0, 0, 0, 0, 0, 0, 1.5f, 3, 4.5f, 6)),
    ];

    public static Track Track(long id, string title, long? albumId = 1, ulong? durationMs = 200_000, string? hash = null) =>
        new(id, $"/music/{id}.flac", title, "Artist", "Album", "Artist", albumId, (uint)id, 1, 2024, null, durationMs, "flac", hash);

    public static PlaybackState Playing(long positionUs, ulong atNs, ulong generation = 1, long? durationUs = 200_000_000, ulong version = 1) =>
        PlaybackState.Initial with
        {
            Version = version,
            Status = PlaybackStatus.Playing,
            Anchor = new PlaybackAnchor(generation, positionUs, atNs, 1f, durationUs),
        };

    public static PlaybackState Paused(long positionUs, ulong atNs, ulong generation = 1, long? durationUs = 200_000_000, ulong version = 2) =>
        PlaybackState.Initial with
        {
            Version = version,
            Status = PlaybackStatus.Paused,
            Anchor = new PlaybackAnchor(generation, positionUs, atNs, 0f, durationUs),
        };
}

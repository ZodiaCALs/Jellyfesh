using Jellyfesh.Core;

namespace Jellyfesh.Services;

/// <summary>
/// Pictures through the core, off the UI thread. Library artwork by content hash: the core writes
/// a JPEG the first time a size is asked for; this caches the paths, runs at most two requests at
/// once, and drops a request when every row that wanted it scrolled away before it started.
/// Source pictures by URL (docs/core-api.md, "Covers"): the core fetches them, never the app;
/// <see cref="GetSourceAsync"/> answers the cached file or waits for the core's Artwork event.
/// </summary>
public sealed class ThumbnailService
{
    /// <summary>The sizes the core makes.</summary>
    public static readonly uint[] Sizes = [64, 128, 256, 512, 1024];

    /// <summary>How long a source picture is waited for before the row keeps its placeholder.</summary>
    public static readonly TimeSpan SourceWait = TimeSpan.FromMinutes(1);

    /// <summary>How long a picture the core cannot have is not asked for again.</summary>
    public static readonly TimeSpan SourceRetryAfter = TimeSpan.FromMinutes(10);

    private readonly ICoreClient _core;
    private readonly SemaphoreSlim _gate;
    private readonly Dictionary<(string Hash, uint Size), Entry> _cache = [];
    private readonly Dictionary<(string Url, uint Size), SourceEntry> _sources = [];
    private readonly object _lock = new();
    private readonly Func<long> _nowMs;
    private volatile bool _unavailable;
    private volatile bool _sourcesUnavailable;

    public ThumbnailService(ICoreClient core, int concurrency = 2, Func<long>? nowMs = null)
    {
        _core = core;
        _gate = new SemaphoreSlim(concurrency);
        _nowMs = nowMs ?? (() => Environment.TickCount64);
    }

    /// <summary>True once the core said thumbnails are not in this build; requests then return null at once.</summary>
    public bool Unavailable => _unavailable;

    /// <summary>The smallest core size that covers <paramref name="pixels"/> device pixels.</summary>
    public static uint SizeFor(double pixels)
    {
        foreach (var s in Sizes)
        {
            if (s >= pixels)
            {
                return s;
            }
        }

        return Sizes[^1];
    }

    /// <summary>
    /// The thumbnail's file path, or null without artwork. Cancelling stops waiting; the work itself
    /// is dropped only when nobody else waits for it and it has not started.
    /// </summary>
    public async Task<string?> GetAsync(string? hash, uint size, CancellationToken cancel = default)
    {
        if (string.IsNullOrEmpty(hash) || _unavailable)
        {
            return null;
        }

        Entry entry;
        lock (_lock)
        {
            if (!_cache.TryGetValue((hash, size), out entry!) || entry.Dropped)
            {
                entry = new Entry();
                entry.Task = LoadAsync(hash, size, entry);
                _cache[(hash, size)] = entry;
            }

            entry.Waiters++;
        }

        try
        {
            return await entry.Task.WaitAsync(cancel).ConfigureAwait(false);
        }
        finally
        {
            lock (_lock)
            {
                entry.Waiters--;
                if (entry.Waiters == 0 && !entry.Started && !entry.Task.IsCompleted)
                {
                    entry.Dropped = true;
                    _cache.Remove((hash, size));
                }
            }
        }
    }

    /// <summary>
    /// A source picture's cached file at about <paramref name="size"/> pixels: the core's answer
    /// when it has it, else the path its Artwork event brings (<see cref="OnArtwork"/>). Null when
    /// the picture cannot be had (not asked again for a while), when it did not come within
    /// <see cref="SourceWait"/>, or when this build fetches no source pictures.
    /// </summary>
    public async Task<string?> GetSourceAsync(string? url, uint size, CancellationToken cancel = default)
    {
        if (string.IsNullOrEmpty(url) || _sourcesUnavailable)
        {
            return null;
        }

        SourceEntry entry;
        var ask = false;
        lock (_lock)
        {
            if (_sources.TryGetValue((url, size), out var known)
                && !(known.Missing && _nowMs() - known.MissingSinceMs >= SourceRetryAfter.TotalMilliseconds))
            {
                entry = known;
            }
            else
            {
                entry = new SourceEntry();
                _sources[(url, size)] = entry;
                ask = true;
            }
        }

        if (ask)
        {
            _ = AskAsync(url, size, entry);
        }

        return await entry.Ready.Task.WaitAsync(cancel).ConfigureAwait(false);
    }

    /// <summary>The core's Artwork event (UI thread): a source picture is cached, or cannot be had.</summary>
    public void OnArtwork(ArtworkReady ready)
    {
        List<SourceEntry> done = [];
        lock (_lock)
        {
            if (_sources.TryGetValue((ready.Url, ready.SizePx), out var exact))
            {
                done.Add(exact);
            }
            else
            {
                // The core answered another size than was asked: it serves every wait for the picture.
                done.AddRange(_sources.Where(kv => kv.Key.Url == ready.Url && !kv.Value.Ready.Task.IsCompleted).Select(kv => kv.Value));
            }

            foreach (var entry in done)
            {
                if (ready.Path is null)
                {
                    entry.Missing = true;
                    entry.MissingSinceMs = _nowMs();
                }
            }
        }

        foreach (var entry in done)
        {
            entry.Ready.TrySetResult(ready.Path);
        }
    }

    /// <summary>Forgets cached paths (a cleared cache folder, a new library).</summary>
    public void Clear()
    {
        lock (_lock)
        {
            _cache.Clear();
            _sources.Clear();
        }
    }

    private async Task AskAsync(string url, uint size, SourceEntry entry)
    {
        string? path;
        try
        {
            path = await Task.Run(() => _core.SourceArtwork(url, size)).ConfigureAwait(false);
        }
        catch (CoreQueryException e)
        {
            if (e.IsNotYetAvailable)
            {
                _sourcesUnavailable = true;
            }

            Forget(url, size, entry);
            entry.Ready.TrySetResult(null);
            return;
        }

        if (path is not null)
        {
            entry.Ready.TrySetResult(path);
            return;
        }

        // The core is fetching it: its Artwork event completes the entry, or the wait ends.
        if (await Task.WhenAny(entry.Ready.Task, Task.Delay(SourceWait)).ConfigureAwait(false) != entry.Ready.Task)
        {
            Forget(url, size, entry);
            entry.Ready.TrySetResult(null);
        }
    }

    /// <summary>Drops an entry that did not come to anything, so a later request asks again.</summary>
    private void Forget(string url, uint size, SourceEntry entry)
    {
        lock (_lock)
        {
            if (_sources.TryGetValue((url, size), out var current) && ReferenceEquals(current, entry))
            {
                _sources.Remove((url, size));
            }
        }
    }

    private async Task<string?> LoadAsync(string hash, uint size, Entry entry)
    {
        await _gate.WaitAsync().ConfigureAwait(false);
        try
        {
            lock (_lock)
            {
                if (entry.Dropped)
                {
                    return null;
                }

                entry.Started = true;
            }

            return await Task.Run(() => _core.ArtworkThumbnail(hash, size)).ConfigureAwait(false);
        }
        catch (CoreQueryException e)
        {
            if (e.IsNotYetAvailable)
            {
                _unavailable = true;
            }

            return null;
        }
        finally
        {
            _gate.Release();
        }
    }

    private sealed class SourceEntry
    {
        public readonly TaskCompletionSource<string?> Ready = new(TaskCreationOptions.RunContinuationsAsynchronously);
        public bool Missing;
        public long MissingSinceMs;
    }

    private sealed class Entry
    {
        public Task<string?> Task = System.Threading.Tasks.Task.FromResult<string?>(null);
        public int Waiters;
        public bool Started;
        public bool Dropped;
    }
}

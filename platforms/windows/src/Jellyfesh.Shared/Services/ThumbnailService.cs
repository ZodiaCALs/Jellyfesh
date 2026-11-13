using Jellyfesh.Core;

namespace Jellyfesh.Services;

/// <summary>
/// Artwork thumbnails by content hash, off the UI thread. The core writes a JPEG the first time a
/// size is asked for; this caches the paths, runs at most two requests at once, and drops a
/// request when every row that wanted it scrolled away before it started.
/// </summary>
public sealed class ThumbnailService
{
    /// <summary>The sizes the core makes.</summary>
    public static readonly uint[] Sizes = [64, 128, 256, 512, 1024];

    private readonly ICoreClient _core;
    private readonly SemaphoreSlim _gate;
    private readonly Dictionary<(string Hash, uint Size), Entry> _cache = [];
    private readonly object _lock = new();
    private volatile bool _unavailable;

    public ThumbnailService(ICoreClient core, int concurrency = 2)
    {
        _core = core;
        _gate = new SemaphoreSlim(concurrency);
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

    /// <summary>Forgets cached paths (a cleared cache folder, a new library).</summary>
    public void Clear()
    {
        lock (_lock)
        {
            _cache.Clear();
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

    private sealed class Entry
    {
        public Task<string?> Task = System.Threading.Tasks.Task.FromResult<string?>(null);
        public int Waiters;
        public bool Started;
        public bool Dropped;
    }
}

namespace Jellyfesh.Core;

/// <summary>Runs work on the UI thread (DispatcherQueue in the app).</summary>
public interface IUiDispatcher
{
    bool HasThreadAccess { get; }

    /// <summary>Queues <paramref name="action"/> on the UI thread.</summary>
    void Post(Action action);
}

/// <summary>Which slices changed in one batch of events.</summary>
[Flags]
public enum SliceChange
{
    None = 0,
    Playback = 1 << 0,
    Queue = 1 << 1,
    NowPlaying = 1 << 2,
    Lyrics = 1 << 3,
    Theme = 1 << 4,
    Scan = 1 << 5,
    Library = 1 << 6,
    Settings = 1 << 7,
    Devices = 1 << 8,
    Sources = 1 << 9,
    Muted = 1 << 10,
    All = (1 << 11) - 1,
}

/// <summary>
/// The shell's mirror of the core: it receives events on the core's thread, keeps only the newest
/// copy of each slice, and applies them on the UI thread in one batch per dispatcher turn. A scan
/// that reports thousands of files a second therefore costs one UI update per frame at most.
/// The mirror is never changed by the shell itself (architecture §1, "one direction").
/// </summary>
public sealed class CoreHub : IDisposable
{
    private readonly IUiDispatcher _ui;
    private readonly object _gate = new();
    private Pending _pending = new();
    private bool _posted;
    private bool _refreshMuted;
    private bool _disposed;

    public CoreHub(ICoreClient core, IUiDispatcher ui)
    {
        Core = core;
        _ui = ui;
    }

    public ICoreClient Core { get; }

    /// <summary>The state as of the last applied batch (UI thread).</summary>
    public CoreSnapshot State { get; private set; } = CoreSnapshot.Initial;

    /// <summary>The latest scan progress while a scan runs.</summary>
    public ScanProgress? ScanProgress { get; private set; }

    /// <summary>How the last scan of this session ended (finished or failed), for Settings › Library's Last Scan.</summary>
    public ScanStatus? LastScan { get; private set; }

    /// <summary>Raised on the UI thread after a batch of slices was applied.</summary>
    public event Action<SliceChange>? Changed;

    /// <summary>
    /// Raised on the UI thread, in order, for events that are not slices: sign-in steps, search and
    /// browse results, buffering, command failures, errors, and a scan's end.
    /// </summary>
    public event Action<CoreEvent>? Notification;

    /// <summary>Subscribes, then reads the snapshot; call once on the UI thread.</summary>
    public void Start()
    {
        Core.SetListener(OnCoreEvent);
        var snapshot = Core.Snapshot();
        lock (_gate)
        {
            // Slices that arrived before the snapshot are older than it; versioned ones are
            // filtered again when applied.
            _pending.DropUnversionedSlices();
        }

        State = snapshot;
        Changed?.Invoke(SliceChange.All);
        Drain();
    }

    /// <summary>Sends a command. Results come back as slices or a failure notification.</summary>
    public ulong Send(CoreCommand command)
    {
        if (command is CoreCommand.SetMuted)
        {
            // The core publishes `muted` only in the snapshot; read it after the next anchor.
            _refreshMuted = true;
        }

        return Core.Send(command);
    }

    /// <summary>Called by the core on its own thread.</summary>
    internal void OnCoreEvent(CoreEvent e)
    {
        bool post;
        lock (_gate)
        {
            if (_disposed)
            {
                return;
            }

            _pending.Add(e);
            post = !_posted;
            _posted = true;
        }

        if (post)
        {
            _ui.Post(Drain);
        }
    }

    private void Drain()
    {
        Pending batch;
        lock (_gate)
        {
            batch = _pending;
            _pending = new Pending();
            _posted = false;
        }

        var (state, changes) = batch.ApplyTo(State);
        if (batch.Scan is ScanStatus.Progress p)
        {
            ScanProgress = p.Value;
        }
        else if (batch.Scan is not null)
        {
            ScanProgress = null;
            LastScan = batch.Scan;
        }

        State = state;
        if (changes != SliceChange.None)
        {
            Changed?.Invoke(changes);
        }

        foreach (var e in batch.Notifications)
        {
            Notification?.Invoke(e);
        }

        if (_refreshMuted && changes.HasFlag(SliceChange.Playback))
        {
            _refreshMuted = false;
            _ = RefreshMutedAsync();
        }
    }

    private async Task RefreshMutedAsync()
    {
        bool muted;
        try
        {
            muted = await Task.Run(() => Core.Snapshot().Muted).ConfigureAwait(false);
        }
        catch (CoreQueryException)
        {
            return;
        }

        _ui.Post(() =>
        {
            if (State.Muted != muted)
            {
                State = State with { Muted = muted };
                Changed?.Invoke(SliceChange.Muted);
            }
        });
    }

    public void Dispose()
    {
        lock (_gate)
        {
            _disposed = true;
        }

        Core.SetListener(null);
    }

    /// <summary>Newest copy of each slice since the last drain, plus ordered notifications.</summary>
    private sealed class Pending
    {
        public PlaybackState? Playback;
        public QueueState? Queue;
        public bool HasNowPlaying;
        public NowPlayingInfo? NowPlaying;
        public LyricsState? Lyrics;
        public ThemeState? Theme;
        public ScanStatus? Scan;
        public ulong? LibraryRevision;
        public AppSettings? Settings;
        public DevicesState? Devices;
        public SourcesState? Sources;
        public readonly List<CoreEvent> Notifications = [];

        public void Add(CoreEvent e)
        {
            switch (e)
            {
                case CoreEvent.Playback p when Playback is null || p.State.Version >= Playback.Version:
                    Playback = p.State;
                    break;
                case CoreEvent.Queue q when Queue is null || q.State.Version >= Queue.Version:
                    Queue = q.State;
                    break;
                case CoreEvent.NowPlaying n:
                    HasNowPlaying = true;
                    NowPlaying = n.Info;
                    break;
                case CoreEvent.Lyrics l:
                    Lyrics = l.State;
                    break;
                case CoreEvent.Theme t:
                    Theme = t.State;
                    break;
                case CoreEvent.Scan s:
                    // A finished or failed scan is also news; progress only replaces progress.
                    if (s.Status is not ScanStatus.Progress)
                    {
                        Notifications.Add(e);
                    }

                    Scan = s.Status;
                    break;
                case CoreEvent.LibraryChanged lc:
                    LibraryRevision = Math.Max(LibraryRevision ?? 0, lc.Revision);
                    break;
                case CoreEvent.Settings st:
                    Settings = st.Value;
                    break;
                case CoreEvent.Devices d when Devices is null || d.State.Version >= Devices.Version:
                    Devices = d.State;
                    break;
                case CoreEvent.Sources so when Sources is null || so.State.Version >= Sources.Version:
                    Sources = so.State;
                    break;
                case CoreEvent.Playback or CoreEvent.Queue or CoreEvent.Devices or CoreEvent.Sources:
                    break; // an older version than one already pending
                default:
                    Notifications.Add(e);
                    break;
            }
        }

        public void DropUnversionedSlices()
        {
            HasNowPlaying = false;
            NowPlaying = null;
            Lyrics = null;
            Theme = null;
            Settings = null;
        }

        public (CoreSnapshot State, SliceChange Changes) ApplyTo(CoreSnapshot s)
        {
            var changes = SliceChange.None;
            if (Playback is { } p && p.Version >= s.Playback.Version)
            {
                s = s with { Playback = p };
                changes |= SliceChange.Playback;
            }

            if (Queue is { } q && q.Version >= s.Queue.Version)
            {
                s = s with { Queue = q };
                changes |= SliceChange.Queue;
            }

            if (HasNowPlaying)
            {
                s = s with { NowPlaying = NowPlaying };
                changes |= SliceChange.NowPlaying;
            }

            if (Lyrics is { } l)
            {
                s = s with { Lyrics = l };
                changes |= SliceChange.Lyrics;
            }

            if (Theme is { } t)
            {
                s = s with { Theme = t };
                changes |= SliceChange.Theme;
            }

            if (Scan is { } scan)
            {
                s = s with { Scanning = scan is ScanStatus.Progress };
                changes |= SliceChange.Scan;
            }

            if (LibraryRevision is { } rev && rev > s.LibraryRevision)
            {
                s = s with { LibraryRevision = rev };
                changes |= SliceChange.Library;
            }

            if (Settings is { } st)
            {
                s = s with { Settings = st };
                changes |= SliceChange.Settings;
            }

            if (Devices is { } d && d.Version >= s.Devices.Version)
            {
                s = s with { Devices = d };
                changes |= SliceChange.Devices;
            }

            if (Sources is { } so && so.Version >= s.Sources.Version)
            {
                s = s with { Sources = so };
                changes |= SliceChange.Sources;
            }

            return (s, changes);
        }
    }
}

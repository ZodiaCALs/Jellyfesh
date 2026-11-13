namespace Jellyfesh.Drawing;

/// <summary>A one-shot timer on the UI thread (DispatcherQueueTimer in the app).</summary>
public interface IOneShotTimer
{
    event EventHandler? Tick;

    /// <summary>Fires <see cref="Tick"/> once after <paramref name="delay"/>; replaces a pending tick.</summary>
    void Start(TimeSpan delay);

    void Stop();
}

/// <summary>
/// What a timed view draws from. <see cref="AnchorSerial"/> changes with every anchor (play, pause,
/// seek, drift correction), so a view can restart an animation it runs on its own.
/// </summary>
public readonly record struct TimedFrame(long PositionUs, bool IsRunning, ulong NowNs, long AnchorSerial);

/// <summary>A view that draws from the playback position: a progress bar, a time label, lyrics.</summary>
public interface ITimedView
{
    /// <summary>Visible and wanting updates. Hidden views neither draw nor wake the ticker.</summary>
    bool IsActive { get; }

    void Render(in TimedFrame frame);

    /// <summary>The next position (µs) at which this view looks different, or null for never.</summary>
    long? NextChangeUs(long positionUs);
}

/// <summary>
/// Drawing time without waste (architecture §5): views are drawn when an anchor arrives and when
/// the next visible change is due, and the ticker sleeps in between. While paused, or when no view
/// is active (minimised, hidden, Now Playing closed), nothing is scheduled at all (Still mode).
/// </summary>
public sealed class PlaybackTicker
{
    /// <summary>Never wake sooner than this, even when a change is due now.</summary>
    public static readonly TimeSpan MinWait = TimeSpan.FromMilliseconds(2);

    /// <summary>Wake this much after a change is due, so the draw lands past it.</summary>
    public static readonly TimeSpan WakeLate = TimeSpan.FromMilliseconds(1);

    private readonly IMonotonicClock _mono;
    private readonly IOneShotTimer _timer;
    private readonly List<ITimedView> _views = [];
    private long _anchorSerial;

    public PlaybackTicker(IDisplayClock clock, IMonotonicClock mono, IOneShotTimer timer)
    {
        Clock = clock;
        _mono = mono;
        _timer = timer;
        _timer.Tick += (_, _) => Draw();
    }

    public IDisplayClock Clock { get; }

    /// <summary>Draws so far (a measure of how idle Still mode is).</summary>
    public long Draws { get; private set; }

    /// <summary>The wait last scheduled, or null when sleeping until the next event.</summary>
    public TimeSpan? ScheduledWait { get; private set; }

    public void Attach(ITimedView view)
    {
        if (!_views.Contains(view))
        {
            _views.Add(view);
        }

        Invalidate();
    }

    public void Detach(ITimedView view)
    {
        _views.Remove(view);
        Invalidate();
    }

    /// <summary>Feeds a playback anchor and redraws.</summary>
    public void OnAnchor(Core.PlaybackAnchor anchor)
    {
        Clock.Update(anchor, _mono.NowNs());
        _anchorSerial++;
        Invalidate();
    }

    /// <summary>Redraws now and reschedules: after an anchor, a seek, a size or visibility change.</summary>
    public void Invalidate() => Draw();

    /// <summary>The drawn position now, for one-off reads (e.g. a seek by keyboard).</summary>
    public long PositionUsNow() => Clock.PositionUs(_mono.NowNs());

    private void Draw()
    {
        var now = _mono.NowNs();
        var pos = Clock.PositionUs(now);
        var frame = new TimedFrame(pos, Clock.IsRunning, now, _anchorSerial);
        long? target = null;
        var any = false;
        foreach (var view in _views.ToArray())
        {
            if (!view.IsActive)
            {
                continue;
            }

            any = true;
            view.Render(frame);
            if (view.NextChangeUs(pos) is { } next && (target is null || next < target))
            {
                target = next;
            }
        }

        if (any)
        {
            Draws++;
        }

        var wait = target is { } t && Clock.IsRunning ? Clock.WaitUntil(t, now) : null;
        ScheduledWait = wait is { } w ? Max(w, MinWait) + WakeLate : null;
        if (ScheduledWait is { } delay)
        {
            _timer.Start(delay);
        }
        else
        {
            _timer.Stop();
        }
    }

    private static TimeSpan Max(TimeSpan a, TimeSpan b) => a > b ? a : b;
}

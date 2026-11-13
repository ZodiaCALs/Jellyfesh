using CommunityToolkit.Mvvm.ComponentModel;
using Jellyfesh.Formatting;

namespace Jellyfesh.Drawing;

/// <summary>
/// One progress display (the seek bar with its times, or the mini-player's ring). It redraws at
/// the next whole second (for its times) and at the next of <see cref="Steps"/> visible steps
/// (for the bar), and not in between.
/// </summary>
public sealed partial class ProgressPresenter : ObservableObject, ITimedView
{
    private readonly ILocalizer _l;
    private readonly IDrawingTime _time;
    private long _durationUs;

    [ObservableProperty]
    private string _positionText = "0:00";

    [ObservableProperty]
    private string _remainingText = TimeFormat.Minus + "0:00";

    [ObservableProperty]
    private string _durationText = "0:00";

    [ObservableProperty]
    private double _fraction;

    [ObservableProperty]
    private string _accessibleText = string.Empty;

    public ProgressPresenter(ILocalizer localizer, IDrawingTime time, bool showsTime)
    {
        _l = localizer;
        _time = time;
        ShowsTime = showsTime;
    }

    /// <summary>Set by the view while it is visible.</summary>
    public bool IsActive { get; set; }

    /// <summary>Visibly different positions (the bar's length in pixels); 0 for none.</summary>
    public int Steps { get; set; }

    /// <summary>Whether position and remaining times are shown (they change every second).</summary>
    public bool ShowsTime { get; }

    /// <summary>While the person drags the thumb, the drawn position is theirs, not the clock's.</summary>
    public bool IsScrubbing { get; set; }

    public long DurationUs => _durationUs;

    public long PositionUs { get; private set; }

    public void SetDuration(long? durationUs)
    {
        _durationUs = Math.Max(0, durationUs ?? 0);
        DurationText = TimeFormat.Clock(_durationUs);
    }

    public void Render(in TimedFrame frame)
    {
        if (IsScrubbing)
        {
            return;
        }

        Show(frame.PositionUs);
    }

    /// <summary>Shows <paramref name="positionUs"/> (the clock's, or the scrub position).</summary>
    public void Show(long positionUs)
    {
        PositionUs = Math.Max(0, positionUs);
        Fraction = _durationUs > 0 ? Math.Clamp(PositionUs / (double)_durationUs, 0, 1) : 0;
        if (ShowsTime)
        {
            PositionText = TimeFormat.Clock(PositionUs);
            RemainingText = _durationUs > 0 ? TimeFormat.Remaining(PositionUs, _durationUs) : string.Empty;
            AccessibleText = _durationUs > 0
                ? _l.Format("Player_PositionOf", PositionText, DurationText)
                : PositionText;
        }
    }

    public long? NextChangeUs(long positionUs)
    {
        long? next = ShowsTime ? _time.NextSecondUs(positionUs) : null;
        if (Steps > 0 && _durationUs > 0)
        {
            var step = Math.Max(1, _durationUs / Steps);
            if (_time.NextStepUs(positionUs, step) is { } s && (next is null || s < next))
            {
                next = s;
            }
        }

        // Nothing changes past the end of the track.
        return next is { } n && _durationUs > 0 && n > _durationUs ? null : next;
    }
}

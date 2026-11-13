namespace Jellyfesh.Drawing;

/// <summary>What the person is doing with a seek bar or a slider in its style.</summary>
public enum SeekBarState
{
    /// <summary>Nothing: a thin line, no thumb.</summary>
    Rest,

    /// <summary>Hovered or focused: the line thickens and the thumb shows (pointer screens).</summary>
    Hover,

    /// <summary>Being dragged: the line thickens further and the times follow the thumb.</summary>
    Drag,
}

/// <summary>The seek bar's keyboard steps (docs/design-controls.md, "v0.0.4").</summary>
public enum SeekKey
{
    /// <summary>Left (or Down): five seconds back.</summary>
    Back,

    /// <summary>Right (or Up): five seconds on.</summary>
    Forward,

    /// <summary>Page Down: thirty seconds back.</summary>
    PageBack,

    /// <summary>Page Up: thirty seconds on.</summary>
    PageForward,

    /// <summary>Home: the start.</summary>
    Start,

    /// <summary>End: the end.</summary>
    End,
}

/// <summary>
/// The straight seek bar of v0.0.4 (docs/design-controls.md): a line with round caps that thickens
/// on hover and while dragging, a 14 px thumb on pointer screens only, the track inset so the thumb
/// and the caps stay inside the control; the mini-player's 2 px progress line; and the width from
/// which the player bar replaces the capsule. Sizes are the player tokens (design/tokens.json; the
/// tests read them from there). Pure geometry: the views only draw it.
/// </summary>
public static class SeekGeometry
{
    /// <summary>player.scrubber-track: the line at rest on pointer screens.</summary>
    public const double Track = 4;

    /// <summary>player.scrubber-track-hover.</summary>
    public const double TrackHover = 6;

    /// <summary>player.scrubber-track-drag.</summary>
    public const double TrackDrag = 8;

    /// <summary>player.scrubber-track-touch: the line at rest on touch screens.</summary>
    public const double TrackTouch = 6;

    /// <summary>player.scrubber-track-touch-drag.</summary>
    public const double TrackTouchDrag = 12;

    /// <summary>player.scrubber-thumb: the hover thumb's diameter.</summary>
    public const double Thumb = 14;

    /// <summary>player.mini-progress: the mini-player's progress line.</summary>
    public const double MiniProgress = 2;

    /// <summary>player.mini-radius: the capsule's corners, which inset its progress line.</summary>
    public const double MiniRadius = 16;

    /// <summary>player.bar-height: the desktop player bar.</summary>
    public const double BarHeight = 80;

    /// <summary>Windows this wide (effective pixels) and wider show the player bar instead of the capsule.</summary>
    public const double PlayerBarMinWidth = 900;

    /// <summary>Left and Right seek this far (seconds).</summary>
    public const double SmallStep = 5;

    /// <summary>Page Up and Page Down seek this far (seconds).</summary>
    public const double LargeStep = 30;

    /// <summary>The line's thickness for <paramref name="state"/>.</summary>
    public static double Thickness(SeekBarState state, bool touch) => touch
        ? (state == SeekBarState.Drag ? TrackTouchDrag : TrackTouch)
        : state switch
        {
            SeekBarState.Drag => TrackDrag,
            SeekBarState.Hover => TrackHover,
            _ => Track,
        };

    /// <summary>Pointer screens show the thumb while hovered, focused or dragged; touch screens never.</summary>
    public static bool ShowsThumb(SeekBarState state, bool touch) => !touch && state != SeekBarState.Rest;

    /// <summary>The space at each end, so the thumb (or the thickest line's cap) stays inside the control.</summary>
    public static double Inset(bool touch) => (touch ? TrackTouchDrag : Thumb) / 2;

    /// <summary>The length of the line inside a control <paramref name="width"/> wide.</summary>
    public static double TrackLength(double width, bool touch) => Math.Max(0, width - (2 * Inset(touch)));

    /// <summary>The fraction of the track at <paramref name="x"/> (control coordinates), clamped to 0..1.</summary>
    public static double FractionAt(double x, double width, bool touch, bool rightToLeft)
    {
        var length = TrackLength(width, touch);
        if (length <= 0 || double.IsNaN(x))
        {
            return 0;
        }

        var fraction = Math.Clamp((x - Inset(touch)) / length, 0, 1);
        return rightToLeft ? 1 - fraction : fraction;
    }

    /// <summary>How long the played part is at <paramref name="fraction"/>.</summary>
    public static double PlayedLength(double fraction, double width, bool touch) =>
        Clamp01(fraction) * TrackLength(width, touch);

    /// <summary>Where the thumb's centre is (control coordinates).</summary>
    public static double ThumbCentre(double fraction, double width, bool touch, bool rightToLeft)
    {
        var played = PlayedLength(fraction, width, touch);
        return rightToLeft ? width - Inset(touch) - played : Inset(touch) + played;
    }

    /// <summary>
    /// How many visibly different positions the line has: one per device pixel of its length. The
    /// bar redraws only when its fill moves by that much (the drawing-time DisplayClock).
    /// </summary>
    public static int Steps(double width, double rasterizationScale, bool touch) =>
        DevicePixels(TrackLength(width, touch), rasterizationScale);

    /// <summary>The position (seconds) a key moves to from <paramref name="position"/>, within 0..<paramref name="duration"/>.</summary>
    public static double KeyTarget(SeekKey key, double position, double duration)
    {
        var target = key switch
        {
            SeekKey.Back => position - SmallStep,
            SeekKey.Forward => position + SmallStep,
            SeekKey.PageBack => position - LargeStep,
            SeekKey.PageForward => position + LargeStep,
            SeekKey.Start => 0,
            _ => duration,
        };
        return Math.Clamp(target, 0, Math.Max(0, duration));
    }

    /// <summary>The mini-player's progress line at <paramref name="fraction"/>: along the bottom edge, inset by the corners.</summary>
    public static double MiniProgressLength(double fraction, double width) =>
        Clamp01(fraction) * MiniTrackLength(width);

    /// <summary>The full length of the mini-player's progress line.</summary>
    public static double MiniTrackLength(double width) => Math.Max(0, width - (2 * MiniRadius));

    /// <summary>Visibly different positions of the mini-player's line (one per device pixel).</summary>
    public static int MiniSteps(double width, double rasterizationScale) =>
        DevicePixels(MiniTrackLength(width), rasterizationScale);

    /// <summary>A straight progress line <paramref name="length"/> long, drawn to <paramref name="fraction"/> (the compact player's).</summary>
    public static double LineLength(double fraction, double length) => Clamp01(fraction) * Math.Max(0, length);

    /// <summary>Visibly different positions of a straight line <paramref name="length"/> long (one per device pixel).</summary>
    public static int LineSteps(double length, double rasterizationScale) =>
        DevicePixels(Math.Max(0, length), rasterizationScale);

    /// <summary>The player bar replaces the capsule from <see cref="PlayerBarMinWidth"/> on.</summary>
    public static bool UsesPlayerBar(double width) => width >= PlayerBarMinWidth;

    private static int DevicePixels(double length, double scale) =>
        (int)Math.Max(0, Math.Round(length * (double.IsFinite(scale) && scale > 0 ? scale : 1)));

    private static double Clamp01(double value) => double.IsNaN(value) ? 0 : Math.Clamp(value, 0, 1);
}

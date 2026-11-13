// Reference port of DisplayClock, next_second_us and next_step_us (core/jf-core/src/timing.rs) and
// PlaybackAnchor::position_at (core/jf-engine/src/anchor.rs). Tested against
// tests/contract/display-clock.json; keep it in step with the Rust original.

using System;
using Jellyfesh.Ffi;

namespace Jellyfesh.Shared;

/// <summary>
/// A smoothed, monotonic playback position for drawing progress bars and lyrics. Feed it every
/// PlaybackSnapshot.Anchor and draw from PositionUs(JellyfeshMethods.MonotonicNowNs()). It never
/// steps backwards while playing, and spreads the small corrections the engine makes when it
/// re-anchors (up to 250 ms) over a short slew; a new track, a seek, a pause or a resume shows at
/// once. WaitUntil turns "the next visible change is at position X" into a delay. Not
/// thread-safe: use it from the thread that draws.
/// </summary>
public sealed class DisplayClock
{
    /// <summary>Corrections up to this size are slewed; larger ones jump.</summary>
    public const long MaxSlewUs = 250_000;

    /// <summary>Shortest slew; longer corrections take four times their size.</summary>
    public const long MinSlewUs = 100_000;

    // Correction added to the anchor's position, fading linearly to zero.
    private long _offsetUs;
    private ulong _offsetSinceNs;
    private long _slewUs;

    // Last position handed out, so it never decreases while playing.
    private long? _lastUs;

    /// <summary>The latest anchor.</summary>
    public PlaybackAnchor? Anchor { get; private set; }

    /// <summary>True while the position moves.</summary>
    public bool IsRunning => Anchor is { Rate: > 0f };

    /// <summary>Feeds an anchor from the playback slice.</summary>
    public void Update(PlaybackAnchor anchor, ulong nowNs)
    {
        ArgumentNullException.ThrowIfNull(anchor);
        PlaybackAnchor? old = Anchor;
        bool continuous = old is not null
            && old.TrackGeneration == anchor.TrackGeneration
            && old.Rate > 0f
            && anchor.Rate > 0f;
        long shown = Drawn(nowNs);
        Anchor = anchor;
        long error = unchecked(shown - anchor.PositionAt(nowNs));
        long size = error < 0 ? unchecked(-error) : error;
        if (continuous && error != 0 && size <= MaxSlewUs)
        {
            _offsetUs = error;
            _offsetSinceNs = nowNs;
            _slewUs = Math.Max(size * 4, MinSlewUs);
        }
        else
        {
            // Show the new anchor exactly, backwards too.
            _offsetUs = 0;
            _lastUs = null;
        }
    }

    /// <summary>The position to draw at nowNs, in microseconds within the track.</summary>
    public long PositionUs(ulong nowNs)
    {
        if (Anchor is not { } anchor)
        {
            return 0;
        }
        long pos = Drawn(nowNs);
        if (anchor.Rate > 0f && _lastUs is long last)
        {
            pos = Math.Max(pos, last);
        }
        if (anchor.DurationUs is long duration)
        {
            pos = Math.Min(pos, Math.Max(duration, 0));
        }
        pos = Math.Max(pos, 0);
        _lastUs = pos;
        return pos;
    }

    /// <summary>
    /// How long until the drawn position reaches targetUs: zero when it already has, null while
    /// stopped or paused. During a slew the result is off by up to a quarter, so re-ask on waking.
    /// </summary>
    public TimeSpan? WaitUntil(long targetUs, ulong nowNs)
    {
        if (Anchor is not { Rate: > 0f } anchor)
        {
            return null;
        }
        long pos = Math.Max(Drawn(nowNs), _lastUs ?? long.MinValue);
        long ahead = DrawingTime.SaturatingSub(targetUs, pos);
        if (ahead <= 0)
        {
            return TimeSpan.Zero;
        }
        double us = Math.Ceiling(ahead / (double)anchor.Rate);
        return us >= long.MaxValue / TimeSpan.TicksPerMicrosecond
            ? TimeSpan.MaxValue
            : TimeSpan.FromTicks((long)us * TimeSpan.TicksPerMicrosecond);
    }

    private long Drawn(ulong nowNs) =>
        Anchor is { } anchor ? DrawingTime.SaturatingAdd(anchor.PositionAt(nowNs), FadingOffset(nowNs)) : 0;

    private long FadingOffset(ulong nowNs)
    {
        if (_offsetUs == 0)
        {
            return 0;
        }
        ulong elapsedNs = nowNs > _offsetSinceNs ? nowNs - _offsetSinceNs : 0;
        ulong elapsed = elapsedNs / 1_000;
        long elapsedUs = elapsed > long.MaxValue ? long.MaxValue : (long)elapsed;
        return elapsedUs >= _slewUs ? 0 : _offsetUs * (_slewUs - elapsedUs) / _slewUs;
    }
}

/// <summary>The redraw helpers, and the anchor extrapolation the clock uses.</summary>
public static class DrawingTime
{
    /// <summary>
    /// The extrapolated audible position at nowNs in microseconds:
    /// PositionUs + (nowNs − AnchorMonoNs) × Rate / 1000, rounded half away from zero. A paused
    /// anchor (rate 0) always returns PositionUs.
    /// </summary>
    public static long PositionAt(this PlaybackAnchor anchor, ulong nowNs)
    {
        ArgumentNullException.ThrowIfNull(anchor);
        if (anchor.Rate == 0f)
        {
            return anchor.PositionUs;
        }
        double elapsedNs = nowNs >= anchor.AnchorMonoNs
            ? nowNs - anchor.AnchorMonoNs
            : -(double)(anchor.AnchorMonoNs - nowNs);
        double advance = Math.Round(elapsedNs * anchor.Rate / 1000.0, MidpointRounding.AwayFromZero);
        return SaturatingAdd(anchor.PositionUs, SaturatingToLong(advance));
    }

    /// <summary>The next whole second after positionUs, when an m:ss display changes.</summary>
    public static long NextSecond(long positionUs) => (FloorDiv(positionUs, 1_000_000) + 1) * 1_000_000;

    /// <summary>
    /// The next multiple of stepUs after positionUs, when a progress bar drawn in steps of stepUs
    /// changes; null for a step of zero or less.
    /// </summary>
    public static long? NextStep(long positionUs, long stepUs) =>
        stepUs > 0 ? (FloorDiv(positionUs, stepUs) + 1) * stepUs : null;

    internal static long SaturatingAdd(long a, long b)
    {
        long sum = unchecked(a + b);
        return ((a ^ sum) & (b ^ sum)) < 0 ? (a < 0 ? long.MinValue : long.MaxValue) : sum;
    }

    internal static long SaturatingSub(long a, long b)
    {
        long difference = unchecked(a - b);
        return ((a ^ b) & (a ^ difference)) < 0 ? (a < 0 ? long.MinValue : long.MaxValue) : difference;
    }

    /// <summary>Rust's `as i64`: saturating, NaN to 0.</summary>
    private static long SaturatingToLong(double x) =>
        double.IsNaN(x) ? 0 : x >= 9.2233720368547758E18 ? long.MaxValue : x <= -9.2233720368547758E18 ? long.MinValue : (long)x;

    /// <summary>Division rounding towards negative infinity, for a positive divisor (Rust's div_euclid).</summary>
    private static long FloorDiv(long a, long b)
    {
        long q = a / b;
        return a % b != 0 && a < 0 ? q - 1 : q;
    }
}

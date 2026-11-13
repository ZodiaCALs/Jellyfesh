using System.Diagnostics;

namespace Jellyfesh.Drawing;

/// <summary>A monotonic clock in nanoseconds on the core's time base.</summary>
public interface IMonotonicClock
{
    ulong NowNs();
}

/// <summary>
/// The core's anchor clock read without an FFI call: Stopwatch (QueryPerformanceCounter) plus an
/// offset measured once against <see cref="Core.ICoreClient.MonotonicNowNs"/>. Both count the same
/// QPC ticks on Windows, so the offset holds; the narrowest of a few bracketed samples wins, which
/// keeps the error well under the 1 ms the clock contract allows.
/// </summary>
public sealed class CoreClock : IMonotonicClock
{
    private readonly Func<long> _ticks;
    private readonly long _frequency;
    private readonly long _offsetNs;

    private CoreClock(Func<long> ticks, long frequency, long offsetNs)
    {
        _ticks = ticks;
        _frequency = frequency;
        _offsetNs = offsetNs;
    }

    /// <summary>The measured offset: core ns minus Stopwatch ns.</summary>
    public long OffsetNs => _offsetNs;

    /// <summary>Half the narrowest sample's width: the worst-case error of the offset, in ns.</summary>
    public long UncertaintyNs { get; private init; }

    public static CoreClock Calibrate(
        Func<ulong> coreNowNs,
        Func<long>? ticks = null,
        long? frequency = null,
        int samples = 9)
    {
        ArgumentNullException.ThrowIfNull(coreNowNs);
        ticks ??= Stopwatch.GetTimestamp;
        var freq = frequency ?? Stopwatch.Frequency;
        var bestWidth = long.MaxValue;
        var bestOffset = 0L;
        for (var i = 0; i < Math.Max(1, samples); i++)
        {
            var t0 = ticks();
            var core = coreNowNs();
            var t1 = ticks();
            var width = t1 - t0;
            if (width < bestWidth)
            {
                bestWidth = width;
                var mid = TicksToNs(t0, freq) + ((TicksToNs(t1, freq) - TicksToNs(t0, freq)) / 2);
                bestOffset = (long)core - mid;
            }
        }

        return new CoreClock(ticks, freq, bestOffset) { UncertaintyNs = TicksToNs(bestWidth, freq) / 2 };
    }

    public ulong NowNs()
    {
        var ns = TicksToNs(_ticks(), _frequency) + _offsetNs;
        return ns < 0 ? 0 : (ulong)ns;
    }

    private static long TicksToNs(long ticks, long frequency) =>
        (long)((Int128)ticks * 1_000_000_000 / frequency);
}

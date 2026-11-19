using Jellyfesh.Core;

namespace Jellyfesh.Drawing;

/// <summary>
/// The playback position to draw, fed by every playback anchor (never by asking the core per
/// frame): the shared port of jf-core's <c>DisplayClock</c>. It never steps backwards while
/// playing and slews small corrections; seeks, pauses and new tracks show at once.
/// </summary>
public interface IDisplayClock
{
    bool IsRunning { get; }

    long? DurationUs { get; }

    void Update(PlaybackAnchor anchor, ulong nowNs);

    long PositionUs(ulong nowNs);

    /// <summary>How long until the drawn position reaches <paramref name="targetUs"/>; null while paused or stopped.</summary>
    TimeSpan? WaitUntil(long targetUs, ulong nowNs);
}

/// <summary>Where the playhead is in the lyrics.</summary>
public readonly record struct LyricsCursor(int? Line, int? Word, float WordProgress);

/// <summary>The lyrics cursor over one timeline: the shared port of jf-lyrics' cursor and next_change.</summary>
public interface ILyricsCursor
{
    bool IsEmpty { get; }

    LyricsCursor Cursor(uint tMs);

    /// <summary>The next time (ms) the line or word changes or the current word finishes; null when nothing changes.</summary>
    uint? NextChange(uint tMs);

    /// <summary>What the lyrics bar shows at <paramref name="tMs"/> (<see cref="LyricsBars.At"/> with this cursor's line).</summary>
    LyricsBarState Bar(uint tMs);
}

/// <summary>
/// The drawing-time helpers (docs/core-api.md, "Drawing time"). Jellyfesh.Interop implements them
/// with the C# ports in platforms/shared/csharp, which replay the contract files.
/// </summary>
public interface IDrawingTime
{
    IDisplayClock CreateClock();

    ILyricsCursor CreateCursor(LyricTimelineData timeline);

    /// <summary>The next whole second after <paramref name="positionUs"/>, when an m:ss display changes.</summary>
    long NextSecondUs(long positionUs);

    /// <summary>The next multiple of <paramref name="stepUs"/> after <paramref name="positionUs"/>; null for a zero step.</summary>
    long? NextStepUs(long positionUs, long stepUs);
}

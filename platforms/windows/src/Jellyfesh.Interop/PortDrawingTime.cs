// IDrawingTime over the C# ports in platforms/shared/csharp (namespace Jellyfesh.Shared), which
// replay tests/contract/lyrics-cursor.json and display-clock.json, and the lyrics bar's port
// (Jellyfesh.Drawing.LyricsBars, tests/contract/lyrics-bar.json).

using System;
using Jellyfesh.Drawing;
using Jellyfesh.Shared;
using F = Jellyfesh.Ffi;
using M = Jellyfesh.Core;

namespace Jellyfesh.Interop;

public sealed class PortDrawingTime : IDrawingTime
{
    public static PortDrawingTime Instance { get; } = new();

    public IDisplayClock CreateClock() => new PortClock();

    public ILyricsCursor CreateCursor(M.LyricTimelineData timeline) => new PortCursor(timeline, FfiMapping.ToFfi(timeline));

    public long NextSecondUs(long positionUs) => DrawingTime.NextSecond(positionUs);

    public long? NextStepUs(long positionUs, long stepUs) => DrawingTime.NextStep(positionUs, stepUs);

    private sealed class PortClock : IDisplayClock
    {
        private readonly DisplayClock _clock = new();

        public bool IsRunning => _clock.IsRunning;

        public long? DurationUs => _clock.Anchor?.DurationUs;

        public void Update(M.PlaybackAnchor anchor, ulong nowNs) => _clock.Update(FfiMapping.ToFfi(anchor), nowNs);

        public long PositionUs(ulong nowNs) => _clock.PositionUs(nowNs);

        public TimeSpan? WaitUntil(long targetUs, ulong nowNs) => _clock.WaitUntil(targetUs, nowNs);
    }

    private sealed class PortCursor(M.LyricTimelineData model, F.LyricTimeline timeline) : ILyricsCursor
    {
        public bool IsEmpty => timeline.SegmentStarts.Count == 0;

        public LyricsCursor Cursor(uint tMs)
        {
            var c = timeline.Cursor(tMs);
            return new LyricsCursor(c.Line, c.Word, c.WordProgress);
        }

        public uint? NextChange(uint tMs) => timeline.NextChange(tMs);

        public LyricsBarState Bar(uint tMs) => LyricsBars.At(model, timeline.Cursor(tMs).Line, tMs);
    }
}

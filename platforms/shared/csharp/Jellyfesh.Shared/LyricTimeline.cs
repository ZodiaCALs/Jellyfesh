// Reference port of LyricTimeline::cursor and ::next_change (core/jf-lyrics/src/timeline.rs).
// Tested against tests/contract/lyrics-cursor.json; keep it in step with the Rust original.

using System;
using System.Collections.Generic;
using Jellyfesh.Ffi;

namespace Jellyfesh.Shared;

/// <summary>Position of the playhead in the lyrics.</summary>
/// <param name="Line">Index of the active line in LyricDoc.Lines; null before the first line, in gaps and after the last.</param>
/// <param name="Word">Index of the current word in that line's Words; null before its first word.</param>
/// <param name="WordProgress">How much of the current word has been sung, 0 to 1.</param>
public readonly record struct LyricCursor(int? Line, int? Word, float WordProgress)
{
    public static LyricCursor None => new(null, null, 0f);
}

/// <summary>The per-frame lyric lookups: two binary searches each.</summary>
public static class LyricTimelineExtensions
{
    /// <summary>"No active line" in SegmentLines and "no end" in LineEnds.</summary>
    public const uint NoLine = uint.MaxValue;

    /// <summary>
    /// The cursor at tMs. A line is active when start ≤ t &lt; end (a line without an end stays
    /// active once started); the active line that started last wins. The word is the last word of
    /// that line with start ≤ t; its progress is (t − start) / (end − start) clamped to 0..1
    /// (1 for zero-length words).
    /// </summary>
    public static LyricCursor Cursor(this LyricTimeline timeline, uint tMs)
    {
        ArgumentNullException.ThrowIfNull(timeline);
        if (ActiveLine(timeline, tMs) is not long line)
        {
            return LyricCursor.None;
        }
        uint? from = At(timeline.WordOffsets, line);
        uint? to = At(timeline.WordOffsets, line + 1);
        if (from is null || to is null)
        {
            return new LyricCursor((int)line, null, 0f);
        }
        (int lo, int hi) = Slice(timeline.WordStarts, from.Value, to.Value);
        int count = CountAtMost(timeline.WordStarts, lo, hi, tMs);
        if (count == 0)
        {
            return new LyricCursor((int)line, null, 0f);
        }
        int word = count - 1;
        uint start = timeline.WordStarts[lo + word];
        uint end = At(timeline.WordEnds, (long)from.Value + word) ?? start;
        float progress = end <= start ? 1f : Math.Clamp((float)(tMs - start) / (float)(end - start), 0f, 1f);
        return new LyricCursor((int)line, word, progress);
    }

    /// <summary>
    /// The earliest time after tMs at which the cursor's line or word changes, or the current word
    /// finishes; null when nothing changes after tMs. A view that draws no word sweep can sleep
    /// until then.
    /// </summary>
    public static uint? NextChange(this LyricTimeline timeline, uint tMs)
    {
        ArgumentNullException.ThrowIfNull(timeline);
        int k = CountAtMost(timeline.SegmentStarts, 0, timeline.SegmentStarts.Count, tMs);
        uint? next = At(timeline.SegmentStarts, k);
        if (ActiveLine(timeline, tMs) is not long line
            || At(timeline.WordOffsets, line) is not uint from
            || At(timeline.WordOffsets, line + 1) is not uint to)
        {
            return next;
        }
        (int lo, int hi) = Slice(timeline.WordStarts, from, to);
        int count = CountAtMost(timeline.WordStarts, lo, hi, tMs);
        if (lo + count < hi)
        {
            uint start = timeline.WordStarts[lo + count];
            next = next is uint n ? Math.Min(n, start) : start;
        }
        if (count > 0 && At(timeline.WordEnds, (long)from + count - 1) is uint end && end > tMs && end != NoLine)
        {
            next = next is uint n ? Math.Min(n, end) : end;
        }
        return next;
    }

    /// <summary>The active line at tMs from the segment arrays, or null for none.</summary>
    private static long? ActiveLine(LyricTimeline timeline, uint tMs)
    {
        int k = CountAtMost(timeline.SegmentStarts, 0, timeline.SegmentStarts.Count, tMs);
        if (k == 0 || At(timeline.SegmentLines, k - 1) is not uint line || line == NoLine)
        {
            return null;
        }
        return line;
    }

    private static uint? At(IReadOnlyList<uint> list, long index) =>
        index >= 0 && index < list.Count ? list[(int)index] : null;

    /// <summary>The bounds of from..to in the list, or an empty range when they do not fit.</summary>
    private static (int Lo, int Hi) Slice(IReadOnlyList<uint> list, uint from, uint to) =>
        from <= to && to <= list.Count ? ((int)from, (int)to) : (0, 0);

    /// <summary>How many of the ascending elements in [lo, hi) are at most t (Rust's partition_point).</summary>
    private static int CountAtMost(IReadOnlyList<uint> list, int lo, int hi, uint t)
    {
        int low = lo;
        int high = hi;
        while (low < high)
        {
            int mid = (int)((uint)(low + high) >> 1);
            if (list[mid] <= t)
            {
                low = mid + 1;
            }
            else
            {
                high = mid;
            }
        }
        return low - lo;
    }
}

using Jellyfesh.Core;

namespace Jellyfesh.Drawing;

/// <summary>
/// What the one-line lyrics bar (and Desktop Lyrics) shows at a time (docs/core-api.md, "The
/// lyrics bar"): the line, the line after it, how much of each word is sung, and for lines without
/// word timing how much of the line is over.
/// </summary>
/// <param name="Line">The active line, or in a gap between lines the last one that started; null
/// before the first line and for unsynced lyrics.</param>
/// <param name="Next">The next line by start (before the first line, the first one).</param>
/// <param name="Words">Each word of <paramref name="Line"/> from 0 (to come) to 1 (sung); empty for
/// lines without word timing.</param>
/// <param name="LineProgress">0–1 through <paramref name="Line"/>, for lines without word timing.</param>
public sealed record LyricsBarState(int? Line, int? Next, IReadOnlyList<float> Words, float LineProgress)
{
    public static LyricsBarState Empty { get; } = new(null, null, [], 0f);
}

/// <summary>
/// The port of jf_core::lyrics_bar (core/jf-core/src/timing.rs) over the timeline's flat arrays,
/// replayed against tests/contract/lyrics-bar.json. It costs a binary search and one pass over a
/// line's words, so it may run every frame while words are swept.
/// </summary>
public static class LyricsBars
{
    private const uint Untimed = uint.MaxValue;

    /// <summary>
    /// The bar at <paramref name="tMs"/>; <paramref name="activeLine"/> is the lyrics cursor's line at
    /// that time (<see cref="ILyricsCursor.Cursor"/>), which the bar keeps through gaps.
    /// </summary>
    public static LyricsBarState At(LyricTimelineData timeline, int? activeLine, uint tMs)
    {
        var starts = timeline.LineStarts;
        var count = starts.Length;
        bool Timed(int i) => i >= 0 && i < count && starts[i] != Untimed;

        var active = activeLine is not null;
        var line = activeLine;
        if (line is null)
        {
            // The last line that started, if any: it stays through the gap.
            for (var i = PartitionPoint(starts, tMs) - 1; i >= 0; i--)
            {
                if (Timed(i))
                {
                    line = i;
                    break;
                }
            }
        }

        int? next = null;
        for (var i = line is { } from ? from + 1 : 0; i < count; i++)
        {
            if (Timed(i))
            {
                next = i;
                break;
            }
        }

        if (line is not { } l)
        {
            return new LyricsBarState(null, next, [], 0f);
        }

        var offsets = timeline.WordOffsets;
        var (first, end) = l + 1 < offsets.Length ? ((int)offsets[l], (int)offsets[l + 1]) : (0, 0);
        var words = new float[Math.Max(0, end - first)];
        for (var w = first; w < end; w++)
        {
            var start = timeline.WordStarts[w];
            var stop = Math.Max(timeline.WordEnds[w], start);
            words[w - first] = tMs < start ? 0f
                : tMs >= stop ? 1f
                : (float)(tMs - start) / (float)(stop - start);
        }

        var lineStart = starts[l];
        uint? lineEnd = l < timeline.LineEnds.Length && timeline.LineEnds[l] != Untimed
            ? timeline.LineEnds[l]
            : next is { } n ? starts[n] : null;
        float progress;
        if (!active && tMs >= lineStart)
        {
            // In the gap after the line: it is over.
            progress = words.Length == 0 ? 0f : 1f;
        }
        else if (lineEnd is { } e && e > lineStart)
        {
            var into = tMs > lineStart ? tMs - lineStart : 0u;
            progress = Math.Clamp((float)into / (float)(e - lineStart), 0f, 1f);
        }
        else
        {
            progress = 0f;
        }

        return new LyricsBarState(l, next, words, progress);
    }

    /// <summary>The number of leading starts at or before <paramref name="tMs"/> (starts are in order).</summary>
    private static int PartitionPoint(uint[] starts, uint tMs)
    {
        int lo = 0, hi = starts.Length;
        while (lo < hi)
        {
            var mid = lo + ((hi - lo) / 2);
            if (starts[mid] <= tMs)
            {
                lo = mid + 1;
            }
            else
            {
                hi = mid;
            }
        }

        return lo;
    }
}

using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Interop;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>The lyrics bar: which line it shows and how its words fill (docs/core-api.md, "The lyrics bar").</summary>
public class LyricsBarTests
{
    private static LyricsState Lyrics(ulong uid = 7)
    {
        // Two word-timed lines with a gap between them, then a line without words.
        var lines = new[]
        {
            new LyricLine(0, 1_000, 2_000, null, "First line", [new LyricWord(1_000, 1_500, "First", false), new LyricWord(1_500, 2_000, "line", false)], [], TextDirection.Ltr),
            new LyricLine(1, 4_000, 5_000, null, "Second line", [new LyricWord(4_000, 4_400, "Second", false), new LyricWord(4_400, 5_000, "line", false)], [], TextDirection.Ltr),
            new LyricLine(2, 8_000, null, null, "Plain", [], [], TextDirection.Ltr),
        };
        var translation = new AuxTrack(AuxKind.Translation, "de", new Dictionary<uint, AuxLine> { [0] = new("Erste Zeile", []) });
        var doc = new LyricDocument("en", LyricTiming.Word, 0, lines, [translation], "EnhancedLrc", null);
        var timeline = new LyricTimelineData(
            [1_000, 4_000, 8_000], [2_000, 5_000, uint.MaxValue], [1_000, 2_000, 4_000, 5_000, 8_000], [0, uint.MaxValue, 1, uint.MaxValue, 2],
            [0, 2, 4, 4], [1_000, 1_500, 4_000, 4_400], [1_500, 2_000, 4_400, 5_000]);
        return new LyricsState(uid, doc, timeline, new LyricsOrigin.Sidecar("/a.lrc"));
    }

    private static LyricsBarPresenter Bar(out List<SweepState?> sweeps)
    {
        var bar = new LyricsBarPresenter(PortDrawingTime.Instance);
        var seen = new List<SweepState?>();
        bar.SweepChanged += s => seen.Add(s);
        sweeps = seen;
        bar.Load(Lyrics(), 7);
        bar.Show(true);
        return bar;
    }

    private static TimedFrame At(uint ms, bool running = true, long serial = 1) => new(ms * 1_000L, running, 0, serial);

    [Fact]
    public void Before_the_first_line_nothing_shows_and_the_first_line_is_next()
    {
        var bar = Bar(out _);
        Assert.True(bar.IsActive);
        bar.Render(At(500));
        Assert.Null(bar.Line);
        Assert.False(bar.HasLine);
        Assert.Equal(0, bar.Next!.Index);
    }

    [Fact]
    public void The_word_being_sung_sweeps_over_the_time_it_has_left()
    {
        var bar = Bar(out var sweeps);
        bar.Render(At(1_250));
        Assert.Equal((0, 1), (bar.Line!.Index, bar.Next!.Index));
        Assert.True(bar.Line.IsActive); // the bar's own copy is always lit
        var sweep = Assert.IsType<SweepState>(sweeps[^1]);
        Assert.Equal((0, 0, 0.5f, TimeSpan.FromMilliseconds(250), true), (sweep.Line, sweep.Word, sweep.Progress, sweep.Remaining, sweep.Running));

        // The same word on another wake (a progress label's second) is not told again.
        var told = sweeps.Count;
        bar.Render(At(1_300));
        Assert.Equal(told, sweeps.Count);

        bar.Render(At(1_600));
        Assert.Equal(1, sweeps[^1]!.Value.Word);
    }

    [Fact]
    public void In_a_gap_the_last_line_stays_fully_sung_until_the_next_starts()
    {
        var bar = Bar(out var sweeps);
        bar.Render(At(3_000));
        Assert.Equal(0, bar.Line!.Index);
        var sweep = sweeps[^1]!.Value;
        Assert.Equal((2, false), (sweep.Word, sweep.Running)); // past the last word: all sung
        bar.Render(At(4_100));
        Assert.Equal((1, 2), (bar.Line!.Index, bar.Next!.Index));
    }

    [Fact]
    public void A_line_without_words_shows_whole_and_translation_follows_the_setting()
    {
        var bar = Bar(out _);
        bar.Render(At(9_000));
        Assert.Equal(2, bar.Line!.Index);
        Assert.Null(bar.Next);
        Assert.Null(bar.Sweep); // shown whole

        bar.Render(At(1_100));
        Assert.Equal("Erste Zeile", bar.Line!.Translation);
        Assert.True(bar.Line.TranslationVisible);
        bar.ShowTranslation = false;
        Assert.False(bar.Line.TranslationVisible);
        Assert.False(bar.Line.RomanizationVisible);
    }

    [Fact]
    public void Lyrics_of_another_song_or_none_clear_the_bar()
    {
        var bar = Bar(out _);
        bar.Render(At(1_250));
        bar.Load(Lyrics(uid: 8), 7); // not the song playing: ignored
        Assert.NotNull(bar.Line);
        bar.Load(null, 7);
        Assert.Null(bar.Line);
        Assert.False(bar.IsActive);
        bar.Render(At(1_250));
        Assert.Null(bar.Line);
    }
}

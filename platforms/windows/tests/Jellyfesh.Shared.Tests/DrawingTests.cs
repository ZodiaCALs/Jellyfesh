using System.Text.Json;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Formatting;
using Jellyfesh.Interop;
using Jellyfesh.Tests.Support;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Drawing time: the ports through IDrawingTime, and the ticker that sleeps between changes.</summary>
public class DrawingTests
{
    private static readonly IDrawingTime Time = PortDrawingTime.Instance;

    [Fact]
    public void Lyrics_cursor_contract_through_the_adapter()
    {
        using var contract = Contract.Read("lyrics-cursor.json");
        var queries = 0;
        foreach (var c in contract.RootElement.GetProperty("cases").EnumerateArray())
        {
            var t = c.GetProperty("timeline");
            uint[] A(string name) => t.GetProperty(name).EnumerateArray().Select(e => e.GetUInt32()).ToArray();
            var cursor = Time.CreateCursor(new LyricTimelineData(
                A("line_starts"), A("line_ends"), A("segment_starts"), A("segment_lines"),
                A("word_offsets"), A("word_starts"), A("word_ends")));
            foreach (var q in c.GetProperty("queries").EnumerateArray())
            {
                var at = q.GetProperty("t").GetUInt32();
                var got = cursor.Cursor(at);
                Assert.Equal(Nullable(q, "line"), got.Line);
                Assert.Equal(Nullable(q, "word"), got.Word);
                Assert.InRange(got.WordProgress, q.GetProperty("word_progress").GetSingle() - 1e-6f, q.GetProperty("word_progress").GetSingle() + 1e-6f);
                var next = q.GetProperty("next_change");
                Assert.Equal(next.ValueKind == JsonValueKind.Null ? null : next.GetUInt32(), cursor.NextChange(at));
                queries++;
            }
        }

        Assert.True(queries > 100);
    }

    [Fact]
    public void Display_clock_contract_through_the_adapter()
    {
        using var contract = Contract.Read("display-clock.json");
        foreach (var c in contract.RootElement.GetProperty("cases").EnumerateArray())
        {
            var clock = Time.CreateClock();
            foreach (var step in c.GetProperty("steps").EnumerateArray())
            {
                switch (step.GetProperty("op").GetString())
                {
                    case "update":
                        var a = step.GetProperty("anchor");
                        var duration = a.GetProperty("duration_us");
                        clock.Update(
                            new PlaybackAnchor(
                                a.GetProperty("track_generation").GetUInt64(),
                                a.GetProperty("position_us").GetInt64(),
                                a.GetProperty("anchor_mono_ns").GetUInt64(),
                                a.GetProperty("rate").GetSingle(),
                                duration.ValueKind == JsonValueKind.Null ? null : duration.GetInt64()),
                            step.GetProperty("now_ns").GetUInt64());
                        break;
                    case "position":
                        Assert.Equal(step.GetProperty("expect").GetInt64(), clock.PositionUs(step.GetProperty("now_ns").GetUInt64()));
                        break;
                    case "running":
                        Assert.Equal(step.GetProperty("expect").GetBoolean(), clock.IsRunning);
                        break;
                    case "wait_until":
                        var expect = step.GetProperty("expect_us");
                        var wait = clock.WaitUntil(step.GetProperty("target_us").GetInt64(), step.GetProperty("now_ns").GetUInt64());
                        long? expected = expect.ValueKind == JsonValueKind.Null ? null : expect.GetInt64();
                        long? actual = wait is { } w ? w.Ticks / TimeSpan.TicksPerMicrosecond : null;
                        Assert.Equal(expected, actual);
                        break;
                }
            }
        }
    }

    [Fact]
    public void Glass_alpha_contract_through_the_app_s_copy_of_the_port()
    {
        // The app compiles platforms/shared/csharp into Jellyfesh.Interop; its glass follows this.
        using var contract = Contract.Read("glass-alpha.json");
        var tolerance = contract.RootElement.GetProperty("tolerance").GetDouble();
        var cases = 0;
        foreach (var c in contract.RootElement.GetProperty("cases").EnumerateArray())
        {
            double got = Jellyfesh.Shared.Glass.Alpha(c[0].GetSingle(), c[1].GetSingle());
            var expected = c[2].GetDouble();
            Assert.InRange(got, expected - tolerance, expected + tolerance);
            cases++;
        }

        Assert.True(cases >= 50);
    }

    [Fact]
    public void Ticker_wakes_at_the_next_second_and_step_and_sleeps_while_paused()
    {
        var mono = new ManualClock();
        var timer = new ManualTimer();
        var ticker = new PlaybackTicker(Time.CreateClock(), mono, timer);
        var progress = new ProgressPresenter(KeyLocalizer.Instance, Time, showsTime: true) { IsActive = true, Steps = 400 };
        progress.SetDuration(200_000_000);
        ticker.Attach(progress);
        Assert.Null(timer.Pending); // no anchor yet: nothing moves

        ticker.OnAnchor(new PlaybackAnchor(1, 1_250_000, mono.Now, 1f, 200_000_000));
        // 200 s over 400 px is a step every 0.5 s: the next is at 1.5 s, before the next second.
        Assert.Equal(TimeSpan.FromMilliseconds(250) + PlaybackTicker.WakeLate, timer.Pending);
        Assert.Equal("0:01", progress.PositionText);

        mono.AdvanceMs(251);
        timer.Fire();
        Assert.Equal(0.0075, progress.Fraction, 4);
        Assert.Equal(TimeSpan.FromMilliseconds(499) + PlaybackTicker.WakeLate, timer.Pending); // next: 2.0 s

        ticker.OnAnchor(new PlaybackAnchor(1, 1_600_000, mono.Now, 0f, 200_000_000));
        Assert.Null(timer.Pending); // paused: nothing scheduled at all
        Assert.Equal("0:01", progress.PositionText);
    }

    [Fact]
    public void Hidden_views_neither_draw_nor_wake_the_ticker()
    {
        var mono = new ManualClock();
        var timer = new ManualTimer();
        var ticker = new PlaybackTicker(Time.CreateClock(), mono, timer);
        var progress = new ProgressPresenter(KeyLocalizer.Instance, Time, showsTime: true) { IsActive = false };
        ticker.Attach(progress);
        ticker.OnAnchor(new PlaybackAnchor(1, 0, mono.Now, 1f, 200_000_000));
        Assert.Null(timer.Pending);
        Assert.Equal(0, ticker.Draws);

        progress.IsActive = true;
        ticker.Invalidate();
        Assert.NotNull(timer.Pending);
        Assert.Equal(1, ticker.Draws);
    }

    [Fact]
    public void A_ring_without_times_steps_only_by_pixels()
    {
        var ring = new ProgressPresenter(KeyLocalizer.Instance, Time, showsTime: false) { Steps = 100 };
        ring.SetDuration(100_000_000);
        Assert.Equal(1_000_000, ring.NextChangeUs(0)); // one pixel per second
        Assert.Null(ring.NextChangeUs(100_000_000)); // nothing after the end
        var none = new ProgressPresenter(KeyLocalizer.Instance, Time, showsTime: false);
        Assert.Null(none.NextChangeUs(5));
    }

    [Fact]
    public void Remaining_time_adds_up_to_the_duration()
    {
        var p = new ProgressPresenter(KeyLocalizer.Instance, Time, showsTime: true);
        p.SetDuration(232_500_000);
        p.Show(84_200_000);
        Assert.Equal("1:24", p.PositionText);
        Assert.Equal("−2:28", p.RemainingText);
        Assert.Equal("3:52", p.DurationText);
    }

    [Fact]
    public void Scrubbing_holds_the_drawn_position()
    {
        var p = new ProgressPresenter(KeyLocalizer.Instance, Time, showsTime: true);
        p.SetDuration(100_000_000);
        p.IsScrubbing = true;
        p.Render(new TimedFrame(50_000_000, true, 0, 0));
        Assert.Equal(0, p.Fraction);
        p.Show(25_000_000);
        Assert.Equal(0.25, p.Fraction);
    }

    [Fact]
    public void Lyrics_light_the_active_line_and_sweep_the_current_word()
    {
        var lyrics = new LyricsPresenter(Time) { IsActive = true };
        lyrics.Load(WordTimedLyrics(uid: 7), currentUid: 7);
        Assert.True(lyrics.HasLyrics);
        Assert.True(lyrics.IsSynced);
        Assert.True(lyrics.HasTranslation);
        Assert.Equal(2, lyrics.Lines.Count);
        Assert.Equal("Morning folds", lyrics.Lines[0].Text);
        Assert.Equal("清晨", lyrics.Lines[0].Translation);
        Assert.True(lyrics.Lines[1].IsCjk);

        var sweeps = new List<SweepState?>();
        lyrics.SweepChanged += s => sweeps.Add(s);
        lyrics.Render(new TimedFrame(1_250_000, true, 0, 1));
        Assert.Equal(0, lyrics.ActiveIndex);
        Assert.True(lyrics.Lines[0].IsActive);
        var sweep = Assert.IsType<SweepState>(sweeps.Last());
        Assert.Equal((0, 0), (sweep.Line, sweep.Word));
        Assert.Equal(0.5f, sweep.Progress, 3);
        Assert.Equal(TimeSpan.FromMilliseconds(250), sweep.Remaining);
        Assert.Equal(1_500_000, lyrics.NextChangeUs(1_250_000)); // the first word ends

        // The same word at a later draw (a progress tick) does not restart the sweep.
        lyrics.Render(new TimedFrame(1_300_000, true, 0, 1));
        Assert.Single(sweeps);

        // Between lines nothing is active and nothing sweeps.
        lyrics.Render(new TimedFrame(2_500_000, true, 0, 1));
        Assert.Equal(-1, lyrics.ActiveIndex);
        Assert.Equal(0, lyrics.ScrollIndex); // keeps its place
        Assert.Null(sweeps.Last());

        lyrics.Render(new TimedFrame(3_100_000, true, 0, 1));
        Assert.Equal(1, lyrics.ActiveIndex);
        Assert.False(lyrics.Lines[0].IsActive);
    }

    [Fact]
    public void Reduced_motion_lights_whole_lines_without_a_sweep()
    {
        var lyrics = new LyricsPresenter(Time) { ReduceMotion = true };
        lyrics.Load(WordTimedLyrics(uid: 1), currentUid: 1);
        var sweeps = 0;
        lyrics.SweepChanged += _ => sweeps++;
        lyrics.Render(new TimedFrame(1_250_000, true, 0, 1));
        Assert.Equal(0, lyrics.ActiveIndex);
        Assert.Null(lyrics.Sweep);
        Assert.Equal(0, sweeps);
    }

    [Fact]
    public void Lyrics_for_another_track_are_ignored_and_toggles_reach_every_line()
    {
        var lyrics = new LyricsPresenter(Time);
        lyrics.Load(WordTimedLyrics(uid: 3), currentUid: 4);
        Assert.False(lyrics.HasLyrics);
        lyrics.Load(WordTimedLyrics(uid: 4), currentUid: 4);
        lyrics.ShowTranslation = false;
        Assert.All(lyrics.Lines, l => Assert.False(l.TranslationVisible));
        lyrics.ShowTranslation = true;
        Assert.True(lyrics.Lines[0].TranslationVisible);
        Assert.False(lyrics.Lines[1].TranslationVisible); // it has none
    }

    internal static LyricsState WordTimedLyrics(ulong uid)
    {
        var lines = new[]
        {
            new LyricLine(0, 1_000, 2_000, null, "Morning folds", [new LyricWord(1_000, 1_500, "Morning", false), new LyricWord(1_500, 2_000, "folds", false)], [], TextDirection.Ltr),
            new LyricLine(1, 3_000, 4_000, null, "港湾", [new LyricWord(3_000, 3_500, "港", true), new LyricWord(3_500, 4_000, "湾", false)], [], TextDirection.Auto),
        };
        var doc = new LyricDocument("en", LyricTiming.Word, 0, lines,
            [new AuxTrack(AuxKind.Translation, "zh-Hans", new Dictionary<uint, AuxLine> { [0] = new("清晨", []) })], "Lrc", null);
        var timeline = new LyricTimelineData(
            LineStarts: [1_000, 3_000],
            LineEnds: [2_000, 4_000],
            SegmentStarts: [1_000, 2_000, 3_000, 4_000],
            SegmentLines: [0, uint.MaxValue, 1, uint.MaxValue],
            WordOffsets: [0, 2, 4],
            WordStarts: [1_000, 1_500, 3_000, 3_500],
            WordEnds: [1_500, 2_000, 3_500, 4_000]);
        return new LyricsState(uid, doc, timeline, new LyricsOrigin.Embedded());
    }

    private static int? Nullable(JsonElement e, string name)
    {
        var v = e.GetProperty(name);
        return v.ValueKind == JsonValueKind.Null ? null : v.GetInt32();
    }
}

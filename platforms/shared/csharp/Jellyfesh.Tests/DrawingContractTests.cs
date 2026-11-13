using System.Linq;
using System.Text.Json;
using Jellyfesh.Ffi;
using Jellyfesh.Shared;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>The ports replay the contracts recorded from the Rust originals.</summary>
public class DrawingContractTests
{
    [Fact]
    public void LyricsCursorContract()
    {
        using JsonDocument contract = Contract.Read("lyrics-cursor.json");
        Assert.Equal(1, contract.RootElement.GetProperty("version").GetInt32());
        int checkedQueries = 0;
        foreach (JsonElement testCase in contract.RootElement.GetProperty("cases").EnumerateArray())
        {
            string name = testCase.GetProperty("name").GetString()!;
            JsonElement tl = testCase.GetProperty("timeline");
            var timeline = new LyricTimeline(
                LineStarts: UInts(tl.GetProperty("line_starts")),
                LineEnds: UInts(tl.GetProperty("line_ends")),
                SegmentStarts: UInts(tl.GetProperty("segment_starts")),
                SegmentLines: UInts(tl.GetProperty("segment_lines")),
                WordOffsets: UInts(tl.GetProperty("word_offsets")),
                WordStarts: UInts(tl.GetProperty("word_starts")),
                WordEnds: UInts(tl.GetProperty("word_ends")));
            foreach (JsonElement query in testCase.GetProperty("queries").EnumerateArray())
            {
                uint t = query.GetProperty("t").GetUInt32();
                LyricCursor cursor = timeline.Cursor(t);
                Assert.True(Nullable(query.GetProperty("line")) == cursor.Line, $"{name}: line at {t}");
                Assert.True(Nullable(query.GetProperty("word")) == cursor.Word, $"{name}: word at {t}");
                double progress = query.GetProperty("word_progress").GetDouble();
                Assert.True(System.Math.Abs(progress - cursor.WordProgress) <= 1e-6, $"{name}: word progress at {t}");
                JsonElement next = query.GetProperty("next_change");
                uint? expected = next.ValueKind == JsonValueKind.Null ? null : next.GetUInt32();
                Assert.True(expected == timeline.NextChange(t), $"{name}: next change after {t}");
                checkedQueries++;
            }
        }
        Assert.True(checkedQueries > 100, $"only {checkedQueries} queries");
    }

    [Fact]
    public void DisplayClockContract()
    {
        using JsonDocument contract = Contract.Read("display-clock.json");
        Assert.Equal(1, contract.RootElement.GetProperty("version").GetInt32());
        int checks = 0;
        foreach (JsonElement testCase in contract.RootElement.GetProperty("cases").EnumerateArray())
        {
            string name = testCase.GetProperty("name").GetString()!;
            var clock = new DisplayClock();
            int i = 0;
            foreach (JsonElement step in testCase.GetProperty("steps").EnumerateArray())
            {
                string at = $"{name}, step {i++}";
                switch (step.GetProperty("op").GetString())
                {
                    case "update":
                        clock.Update(Anchor(step.GetProperty("anchor")), step.GetProperty("now_ns").GetUInt64());
                        break;
                    case "position":
                        Assert.True(
                            step.GetProperty("expect").GetInt64() == clock.PositionUs(step.GetProperty("now_ns").GetUInt64()),
                            at);
                        break;
                    case "wait_until":
                        JsonElement expect = step.GetProperty("expect_us");
                        long? wait = clock.WaitUntil(step.GetProperty("target_us").GetInt64(), step.GetProperty("now_ns").GetUInt64())
                            ?.Ticks / System.TimeSpan.TicksPerMicrosecond;
                        Assert.True((expect.ValueKind == JsonValueKind.Null ? null : expect.GetInt64()) == wait, at);
                        break;
                    case "running":
                        Assert.True(step.GetProperty("expect").GetBoolean() == clock.IsRunning, at);
                        break;
                    default:
                        Assert.Fail($"unknown op in {at}");
                        break;
                }
                checks++;
            }
        }
        foreach (JsonElement row in contract.RootElement.GetProperty("next_second").EnumerateArray())
        {
            long position = row[0].GetInt64();
            Assert.True(row[1].GetInt64() == DrawingTime.NextSecond(position), $"NextSecond({position})");
            checks++;
        }
        foreach (JsonElement row in contract.RootElement.GetProperty("next_step").EnumerateArray())
        {
            long position = row[0].GetInt64();
            long step = row[1].GetInt64();
            long? expected = row[2].ValueKind == JsonValueKind.Null ? null : row[2].GetInt64();
            Assert.True(expected == DrawingTime.NextStep(position, step), $"NextStep({position}, {step})");
            checks++;
        }
        Assert.True(checks > 1000, $"only {checks} checks");
    }

    private static uint[] UInts(JsonElement array) => array.EnumerateArray().Select(e => e.GetUInt32()).ToArray();

    private static int? Nullable(JsonElement value) => value.ValueKind == JsonValueKind.Null ? null : value.GetInt32();

    private static PlaybackAnchor Anchor(JsonElement json) => new(
        TrackGeneration: json.GetProperty("track_generation").GetUInt64(),
        PositionUs: json.GetProperty("position_us").GetInt64(),
        AnchorMonoNs: json.GetProperty("anchor_mono_ns").GetUInt64(),
        Rate: json.GetProperty("rate").GetSingle(),
        DurationUs: json.GetProperty("duration_us").ValueKind == JsonValueKind.Null ? null : json.GetProperty("duration_us").GetInt64());
}

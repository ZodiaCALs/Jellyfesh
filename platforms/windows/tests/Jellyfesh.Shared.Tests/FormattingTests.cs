using Jellyfesh.Formatting;
using Xunit;

namespace Jellyfesh.Tests;

public class FormattingTests
{
    [Theory]
    [InlineData(0, "0:00")]
    [InlineData(-5_000_000, "0:00")]
    [InlineData(7_999_999, "0:07")]
    [InlineData(232_000_000, "3:52")]
    [InlineData(3_723_000_000, "1:02:03")]
    public void Clock_positions(long us, string text) => Assert.Equal(text, TimeFormat.Clock(us));

    [Fact]
    public void Remaining_never_goes_below_zero()
    {
        Assert.Equal("−0:00", TimeFormat.Remaining(300_000_000, 200_000_000));
        Assert.Equal("−1:00:00", TimeFormat.Remaining(0, 3_600_000_000));
    }

    [Fact]
    public void Totals_and_spoken_times_use_localised_formats()
    {
        var l = new TableLocalizer(new()
        {
            ["Time_Minutes"] = "{0} min",
            ["Time_HoursMinutes"] = "{0} h {1} min",
            ["Time_SpokenMinutes"] = "{0} minutes {1} seconds",
            ["Time_SpokenHours"] = "{0} hours {1} minutes {2} seconds",
            ["Count_Songs_One"] = "{0} song",
            ["Count_Songs_Other"] = "{0} songs",
        });
        Assert.Equal("52 min", TimeFormat.Total(52 * 60_000, l));
        Assert.Equal("1 h 08 min", TimeFormat.Total(68 * 60_000, l));
        Assert.Equal("1 minutes 24 seconds", TimeFormat.Spoken(84_500_000, l));
        Assert.Equal("1 song", l.Plural("Count_Songs", 1));
        Assert.Equal("1,234 songs", l.Plural("Count_Songs", 1234));
    }

    [Fact]
    public void Cjk_detection()
    {
        Assert.True(Script.HasCjk("港湾"));
        Assert.True(Script.HasCjk("こんにちは"));
        Assert.True(Script.HasCjk("안녕"));
        Assert.False(Script.HasCjk("Morning"));
        Assert.True(Script.IsCjkLanguage("zh-Hant"));
        Assert.False(Script.IsCjkLanguage("en"));
    }

    private sealed class TableLocalizer(Dictionary<string, string> table) : ILocalizer
    {
        public System.Globalization.CultureInfo Culture => System.Globalization.CultureInfo.GetCultureInfo("en-US");

        public string Get(string key) => table.GetValueOrDefault(key, key);
    }
}

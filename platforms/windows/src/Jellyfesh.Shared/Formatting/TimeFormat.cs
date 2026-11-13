using System.Globalization;

namespace Jellyfesh.Formatting;

/// <summary>Times as the player shows them: m:ss, h:mm:ss, a remaining time with a true minus.</summary>
public static class TimeFormat
{
    /// <summary>U+2212 MINUS SIGN, as in the mock-ups ("−2:28").</summary>
    public const string Minus = "−";

    /// <summary>A clock position: 0:07, 3:52, 1:02:03. Negative times show as 0:00.</summary>
    public static string Clock(long microseconds) => FromSeconds(WholeSeconds(microseconds));

    public static string ClockMs(ulong milliseconds) => FromSeconds((long)(milliseconds / 1_000));

    /// <summary>
    /// The remaining time, so that position plus remaining is always the whole duration:
    /// <c>Remaining(84.2 s, 232.5 s)</c> is "−2:28" next to "1:24" for a 3:52 track.
    /// </summary>
    public static string Remaining(long positionUs, long durationUs)
    {
        var left = Math.Max(0, WholeSeconds(durationUs) - WholeSeconds(positionUs));
        return Minus + FromSeconds(left);
    }

    /// <summary>A track length for list rows; empty when unknown.</summary>
    public static string Duration(ulong? milliseconds) =>
        milliseconds is { } ms ? ClockMs(ms) : string.Empty;

    /// <summary>A long total for albums and playlists: "52 min", "1 h 08 min" (localised units).</summary>
    public static string Total(ulong milliseconds, ILocalizer l)
    {
        var minutes = (long)Math.Round(milliseconds / 60_000.0);
        if (minutes < 60)
        {
            return l.Format("Time_Minutes", minutes);
        }

        return l.Format("Time_HoursMinutes", minutes / 60, (minutes % 60).ToString("00", CultureInfo.InvariantCulture));
    }

    /// <summary>A total in words for an album's footer: "48 minutes", "1 hour 12 minutes" (localised, plural-aware).</summary>
    public static string LongTotal(ulong milliseconds, ILocalizer l)
    {
        var minutes = (long)Math.Round(milliseconds / 60_000.0);
        if (minutes < 60)
        {
            return l.Plural("Count_Minutes", minutes);
        }

        var hours = l.Plural("Count_Hours", minutes / 60);
        return minutes % 60 == 0 ? hours : l.Format("Time_HoursAndMinutes", hours, l.Plural("Count_Minutes", minutes % 60));
    }

    /// <summary>For Narrator: "1 minute 24 seconds" style text, localised.</summary>
    public static string Spoken(long microseconds, ILocalizer l)
    {
        var s = WholeSeconds(microseconds);
        var h = s / 3600;
        var m = (s / 60) % 60;
        var sec = s % 60;
        return h > 0
            ? l.Format("Time_SpokenHours", h, m, sec)
            : l.Format("Time_SpokenMinutes", m, sec);
    }

    public static long WholeSeconds(long microseconds) =>
        microseconds <= 0 ? 0 : microseconds / 1_000_000;

    private static string FromSeconds(long seconds)
    {
        var h = seconds / 3600;
        var m = (seconds / 60) % 60;
        var s = seconds % 60;
        return h > 0
            ? string.Create(CultureInfo.InvariantCulture, $"{h}:{m:00}:{s:00}")
            : string.Create(CultureInfo.InvariantCulture, $"{m}:{s:00}");
    }
}

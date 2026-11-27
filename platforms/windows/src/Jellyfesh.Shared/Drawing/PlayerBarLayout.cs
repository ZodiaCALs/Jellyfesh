namespace Jellyfesh.Drawing;

/// <summary>
/// The player bar's three columns at any width (docs/ui.md, "The player bar and the mini player",
/// v0.0.9; Jellyfesh Web's PlayerBar): what is playing, the transport above the scrubber and its
/// times, then the tools. Each part keeps to its own column, so nothing in the bar overlaps:
/// <list type="bullet">
/// <item>the tools' column is as wide as what it holds, and a narrower bar holds less: no volume
/// line below 900 epx (Ctrl+Up and Ctrl+Down still change the volume), no Lyrics and Queue below
/// 760 (Now Playing has both), no mute button below 640;</item>
/// <item>the middle column grows to 340 epx once the title's column has 112, and never shrinks
/// below the transport's 244;</item>
/// <item>the title's column takes the rest, and its lines end with "…" or fade out within it.</item>
/// </list>
/// Pure: the bar gives it its width and the tools' width, and sizes its columns as it says.
/// </summary>
/// <param name="VolumeLine">The volume line beside the mute button.</param>
/// <param name="Panes">The Lyrics and Queue buttons.</param>
/// <param name="Mute">The mute button.</param>
public sealed record PlayerBarLayout(bool VolumeLine, bool Panes, bool Mute)
{
    /// <summary>The bar's padding, start and end together (8 + 16 epx).</summary>
    public const double Padding = 24;

    /// <summary>space.s4 between the columns.</summary>
    public const double Gap = 16;

    /// <summary>The title's column keeps this much while the bar has room for it (the web's 112 px).</summary>
    public const double TitleMin = 112;

    /// <summary>The transport: shuffle and repeat (44 epx each), previous, play and next (44 each) and their gaps.</summary>
    public const double MiddleMin = 244;

    /// <summary>The middle column's widest (the web's 340 px).</summary>
    public const double MiddleMax = 340;

    /// <summary>The volume line shows in bars at least this wide (the web's container query, 900 px).</summary>
    public const double VolumeLineMinWidth = 900;

    /// <summary>Lyrics and Queue show in bars at least this wide (760 px).</summary>
    public const double PanesMinWidth = 760;

    /// <summary>The mute button shows in bars at least this wide (640 px).</summary>
    public const double MuteMinWidth = 640;

    /// <summary>What a bar <paramref name="width"/> epx wide holds.</summary>
    public static PlayerBarLayout For(double width) =>
        new(width >= VolumeLineMinWidth, width >= PanesMinWidth, width >= MuteMinWidth);

    /// <summary>
    /// The middle column's width in a bar <paramref name="width"/> epx wide whose tools' column is
    /// <paramref name="tools"/> epx wide: up to <see cref="MiddleMax"/> once the title's column has
    /// <see cref="TitleMin"/>, never below the transport's <see cref="MiddleMin"/>.
    /// </summary>
    public static double MiddleWidth(double width, double tools)
    {
        var free = Free(width, tools);
        return Math.Clamp(free - TitleMin, MiddleMin, MiddleMax);
    }

    /// <summary>The title's column: what the padding, the gaps, the middle column and the tools leave.</summary>
    public static double TitleWidth(double width, double tools) =>
        Math.Max(0, Free(width, tools) - MiddleWidth(width, tools));

    /// <summary>The room the title's and the middle columns share.</summary>
    private static double Free(double width, double tools) =>
        (double.IsFinite(width) ? Math.Max(0, width) : 0) - Padding - (2 * Gap) - (double.IsFinite(tools) ? Math.Max(0, tools) : 0);
}

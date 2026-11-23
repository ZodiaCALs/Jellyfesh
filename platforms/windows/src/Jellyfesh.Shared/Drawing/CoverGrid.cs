namespace Jellyfesh.Drawing;

/// <summary>
/// The library's album grid, as Jellyfesh Web draws it (ItemList's grid, Luminous): columns at least
/// 196 epx wide (150 on phones) and 12 epx apart (4 on phones), filling the width, so a 1280 epx
/// window has four large covers to a row. Each web card reaches 8 epx beyond its cover on every side
/// and the grid steps out by as much, so the first cover lines up with the page's title.
/// </summary>
/// <param name="Columns">How many covers a row holds.</param>
/// <param name="Cover">Each cover's side.</param>
/// <param name="Spacing">From one cover to the next: the gap and the two cards' edges.</param>
public readonly record struct CoverGrid(int Columns, double Cover, double Spacing)
{
    /// <summary>The web's card padding around a cover.</summary>
    public const double CardPadding = 8;

    /// <summary>The narrowest column on wide windows (a 180 epx cover in its card).</summary>
    public const double MinColumn = 196;

    /// <summary>The narrowest column in the phone layout.</summary>
    public const double MinPhoneColumn = 150;

    /// <summary>space.s3: the gap between columns on wide windows.</summary>
    public const double Gap = 12;

    /// <summary>space.s1: the gap between columns on phones.</summary>
    public const double PhoneGap = 4;

    /// <summary>The grid for <paramref name="width"/> epx of content.</summary>
    public static CoverGrid For(double width, bool phone)
    {
        var min = phone ? MinPhoneColumn : MinColumn;
        var gap = phone ? PhoneGap : Gap;
        var span = Math.Max(0, width) + (2 * CardPadding);
        var columns = Math.Max(1, (int)Math.Floor((span + gap) / (min + gap)));
        var column = (span - ((columns - 1) * gap)) / columns;
        return new CoverGrid(columns, Math.Max(1, Math.Floor(column - (2 * CardPadding))), gap + (2 * CardPadding));
    }

    /// <summary>
    /// The layout slot of one cover: the cover and the spacing after it. A row of slots is
    /// <see cref="Spacing"/> wider than the content, which the grid gives up on its trailing side.
    /// </summary>
    public double Slot => Cover + Spacing;
}

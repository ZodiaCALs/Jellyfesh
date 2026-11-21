using Jellyfesh.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Windows.UI;
using Windows.UI.Text;

namespace Jellyfesh.App.Controls;

/// <summary>
/// The lyrics' part of the Look slice (ADR-0020 §2) as the lyric controls draw it: the lines'
/// weight, the colour the current line lights up in, and its glow (a soft shadow in that colour).
/// The rules are LookRules'; this only turns them into brushes and weights.
/// </summary>
internal sealed record LyricsLook(FontWeight Weight, Brush Highlight, double Glow)
{
    /// <summary>The glow's blur radius at full strength (epx).</summary>
    public const double GlowRadius = 18;

    /// <summary>The Luminous lyrics (v0.0.6): bold, lit in ink-90, no glow.</summary>
    public static LyricsLook Default => From(LookState.Default);

    public static LyricsLook From(LookState look)
    {
        var res = Application.Current.Resources;
        var (ink, rgb) = LookRules.Highlight(look);
        var highlight = ink switch
        {
            HighlightInk.Accent => (Brush)res["JfAccentText"],
            HighlightInk.Custom => new SolidColorBrush(Rgb(rgb)),
            _ => (Brush)res["JfInk90"],
        };
        return new LyricsLook(new FontWeight { Weight = LookRules.LyricsFontWeight(look) }, highlight, LookRules.LyricsGlow(look));
    }

    /// <summary>The glow's colour: the highlight's as it is now (the theme's brushes change colour in place).</summary>
    public Color GlowColor => Highlight is SolidColorBrush solid ? solid.Color : Rgb(0xFFFFFF);

    /// <summary>The colour the lyrics bar lights words in: its accent unless the look chose a colour of its own.</summary>
    public static Brush BarHighlight(LookState look) => LookRules.Highlight(look) is (HighlightInk.Custom, var rgb)
        ? new SolidColorBrush(Rgb(rgb))
        : (Brush)Application.Current.Resources["JfAccentText"];

    private static Color Rgb(uint rgb) => Color.FromArgb(255, (byte)(rgb >> 16), (byte)(rgb >> 8), (byte)rgb);
}

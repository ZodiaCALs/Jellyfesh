// The only file that names members of the generated Misty tokens (Generated/Tokens.cs, namespace
// Jellyfesh.Design, from `cargo xtask tokens --target csharp`). XAML reads them through
// {th:Token Key=...}; code reads them here. A renamed token is fixed in this file only.

using Jellyfesh.Design;
using Jellyfesh.Drawing;

namespace Jellyfesh.App.Theming;

internal static class TokenValues
{
    /// <summary>Numbers by key: dimensions in epx, durations in ms.</summary>
    public static IReadOnlyDictionary<string, double> Numbers { get; } = new Dictionary<string, double>(StringComparer.Ordinal)
    {
        ["Radius.Xs"] = Tokens.Radius.RXs,
        ["Radius.Sm"] = Tokens.Radius.RSm,
        ["Radius.Md"] = Tokens.Radius.RMd,
        ["Radius.Lg"] = Tokens.Radius.RLg,
        ["Radius.Xl"] = Tokens.Radius.RXl,
        ["Radius.2xl"] = Tokens.Radius.R2xl,
        ["Radius.Full"] = Tokens.Radius.RFull,
        ["Space.S1"] = Tokens.Space.S1,
        ["Space.S2"] = Tokens.Space.S2,
        ["Space.S3"] = Tokens.Space.S3,
        ["Space.S4"] = Tokens.Space.S4,
        ["Space.S5"] = Tokens.Space.S5,
        ["Space.S6"] = Tokens.Space.S6,
        ["Space.S7"] = Tokens.Space.S7,
        ["Space.S8"] = Tokens.Space.S8,
        ["Space.S9"] = Tokens.Space.S9,
        ["Space.S10"] = Tokens.Space.S10,
        ["Artwork.RadiusList"] = Tokens.Artwork.RadiusList,
        ["Artwork.RadiusGrid"] = Tokens.Artwork.RadiusGrid,
        ["Artwork.RadiusHero"] = Tokens.Artwork.RadiusHero,
        ["Artwork.SizeMini"] = Tokens.Artwork.SizeMini,
        ["Player.MiniHeight"] = Tokens.Player.MiniHeight,
        ["Player.MiniRadius"] = Tokens.Player.MiniRadius,
        ["Player.RowHeight"] = Tokens.Player.RowHeight,
        ["Player.ScrubberThumb"] = Tokens.Player.ScrubberThumb,
        ["Player.ScrubberTrack"] = Tokens.Player.ScrubberTrack,
        ["Player.ScrubberTrackHover"] = Tokens.Player.ScrubberTrackHover,
        ["Player.ScrubberTrackDrag"] = Tokens.Player.ScrubberTrackDrag,
        ["Player.MiniProgress"] = Tokens.Player.MiniProgress,
        ["Player.BarHeight"] = Tokens.Player.BarHeight,
        ["Player.ArtworkPausedScale"] = Tokens.Player.ArtworkPausedScale,
        ["Lyrics.ScaleInactive"] = Tokens.Lyrics.ScaleInactive,
        ["Lyrics.StaggerMs"] = Tokens.Lyrics.StaggerMs,
        ["EdgeBlur.TopHeight"] = Tokens.EdgeBlur.TopHeight,
        ["EdgeBlur.BottomHeight"] = Tokens.EdgeBlur.BottomHeight,
        ["Lyrics.LineGap"] = Tokens.Lyrics.LineGap,
        ["Lyrics.AnchorPosition"] = Tokens.Lyrics.AnchorPosition,
        ["Lyrics.LineSize"] = Tokens.Lyrics.Line.Size,
        ["Lyrics.LineHeight"] = Tokens.Lyrics.Line.LineHeight,
        ["Lyrics.CjkLineHeight"] = Tokens.Lyrics.LineCjkLineHeight,
        ["Lyrics.LineMin"] = Tokens.Lyrics.LineFluid.Min,
        ["Lyrics.LineMax"] = Tokens.Lyrics.LineFluid.Max,
        ["Lyrics.TranslationSize"] = Tokens.Lyrics.Translation.Size,
        ["Lyrics.RomanizationSize"] = Tokens.Lyrics.Romanization.Size,
        ["Type.NowPlayingTitle"] = Tokens.TypeRoles.NowPlayingTitle.Size,
        ["Type.NowPlayingArtist"] = Tokens.TypeRoles.NowPlayingArtist.Size,
        ["Type.ListTitle"] = Tokens.TypeRoles.ListTitle.Size,
        ["Type.ListSecondary"] = Tokens.TypeRoles.ListSecondary.Size,
        ["Type.Section"] = Tokens.TypeRoles.Section.Size,
        ["Type.H1"] = Tokens.TypeScale.TextH1,
        ["Type.H2"] = Tokens.TypeScale.TextH2,
        ["Type.H3"] = Tokens.TypeScale.TextH3,
        ["Motion.Dur0"] = Tokens.Motion.Dur0Ms,
        ["Motion.Dur1"] = Tokens.Motion.Dur1Ms,
        ["Motion.Dur2"] = Tokens.Motion.Dur2Ms,
        ["Motion.Dur3"] = Tokens.Motion.Dur3Ms,
        ["Motion.Dur4"] = Tokens.Motion.Dur4Ms,
        ["Glass.AlphaClearLight"] = Tokens.Glass.Clear.Alpha.Light,
        ["Glass.AlphaClearDark"] = Tokens.Glass.Clear.Alpha.Dark,
        ["Glass.AlphaMistLight"] = Tokens.Glass.Mist.Alpha.Light,
        ["Glass.AlphaMistDark"] = Tokens.Glass.Mist.Alpha.Dark,
        ["Glass.AlphaVeilLight"] = Tokens.Glass.Veil.Alpha.Light,
        ["Glass.AlphaVeilDark"] = Tokens.Glass.Veil.Alpha.Dark,
    };

    /// <summary>The Fluid curve, for entering and moving (Misty).</summary>
    public static CubicBezierCurve Fluid { get; } = Curve(Tokens.Motion.EaseFluid);

    /// <summary>The Spring curve, for small presses only; overshoots, so only the composition API can run it.</summary>
    public static CubicBezierCurve Spring { get; } = Curve(Tokens.Motion.EaseSpring);

    public static CubicBezierCurve Exit { get; } = Curve(Tokens.Motion.EaseExit);

    public static TimeSpan Duration(int index) => TimeSpan.FromMilliseconds(index switch
    {
        0 => Tokens.Motion.Dur0Ms,
        1 => Tokens.Motion.Dur1Ms,
        2 => Tokens.Motion.Dur2Ms,
        3 => Tokens.Motion.Dur3Ms,
        _ => Tokens.Motion.Dur4Ms,
    });

    /// <summary>App brush resource key → the theme colour it shows (a name in the core's Theme slice).</summary>
    public static IReadOnlyList<(string Key, string Color)> Brushes { get; } =
    [
        ("JfSky", Tokens.Field.Sky.Name),
        ("JfSkyHigh", Tokens.Field.SkyHigh.Name),
        ("JfAurora1", Tokens.Field.Aurora1.Name),
        ("JfAurora2", Tokens.Field.Aurora2.Name),
        ("JfAurora3", Tokens.Field.Aurora3.Name),
        ("JfInk90", Tokens.Ink.Ink90.Name),
        ("JfInk70", Tokens.Ink.Ink70.Name),
        ("JfInk50", Tokens.Ink.Ink50.Name),
        ("JfInk30", Tokens.Ink.Ink30.Name),
        ("JfInk10", Tokens.Ink.Ink10.Name),
        ("JfInk06", Tokens.Ink.Ink06.Name),
        ("JfAccent", Tokens.Accent.AccentColor.Name),
        ("JfAccentHover", Tokens.Accent.AccentHover.Name),
        ("JfAccentPress", Tokens.Accent.AccentPress.Name),
        ("JfAccentText", Tokens.Accent.AccentText.Name),
        ("JfAccentSoft", Tokens.Accent.AccentSoft.Name),
        ("JfOnAccent", Tokens.Accent.OnAccent.Name),
        ("JfLine", Tokens.Line.LineColor.Name),
        ("JfLineSoft", Tokens.Line.LineSoft.Name),
        ("JfLineStrong", Tokens.Line.LineStrong.Name),
        ("JfFill", Tokens.Line.Fill.Name),
        ("JfFillHover", Tokens.Line.FillHover.Name),
        ("JfFillPress", Tokens.Line.FillPress.Name),
        ("JfCloud", Tokens.Glass.Cloud.Name),
        ("JfCloudSoft", Tokens.Glass.CloudSoft.Name),
        ("JfGlassEdge", Tokens.Glass.GlassEdge.Name),
        ("JfGlassEdgeSoft", Tokens.Glass.GlassEdgeSoft.Name),
        ("JfTintClear", Tokens.Glass.TintClear.Name),
        ("JfTintMist", Tokens.Glass.TintMist.Name),
        ("JfTintVeil", Tokens.Glass.TintVeil.Name),
        ("JfScrim", Tokens.Elevation.Scrim.Name),
        ("JfDanger", Tokens.Semantic.Danger.Name),
        ("JfSuccess", Tokens.Semantic.Success.Name),
        ("JfWarning", Tokens.Semantic.Warning.Name),
        ("JfInfo", Tokens.Semantic.Info.Name),
    ];

    public static double Number(string key) =>
        Numbers.TryGetValue(key, out var value) ? value : throw new KeyNotFoundException($"no design token '{key}'");

    private static CubicBezierCurve Curve(CubicBezier b) => new(b.X1, b.Y1, b.X2, b.Y2);
}

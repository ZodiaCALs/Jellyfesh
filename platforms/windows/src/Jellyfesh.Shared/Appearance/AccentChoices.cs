using Jellyfesh.Core;

namespace Jellyfesh.Appearance;

/// <summary>
/// OKLCH to 8-bit sRGB as jf_palette converts (core/jf-palette/src/color.rs: the CSS Color 4 OKLab
/// matrices in double, each channel clipped to 0..1, rounded as floor(v·255 + 0.5)), so a swatch
/// shows the accent the core resolves for its hue. Presentation only: themes still come from the core.
/// </summary>
public static class Oklch
{
    private static readonly double[,] OklabToLms =
    {
        { 1.0, 0.3963377773761749, 0.2158037573099136 },
        { 1.0, -0.1055613458156586, -0.0638541728258133 },
        { 1.0, -0.0894841775298119, -1.2914855480194092 },
    };

    private static readonly double[,] LmsToXyz =
    {
        { 1.2268798758459243, -0.5578149944602171, 0.2813910456659647 },
        { -0.0405757452148008, 1.112286803280317, -0.0717110580655164 },
        { -0.0763729366746601, -0.4214933324022432, 1.5869240198367816 },
    };

    private static readonly double[,] XyzToLinearSrgb =
    {
        { 12831.0 / 3959.0, -329.0 / 214.0, -1974.0 / 3959.0 },
        { -851781.0 / 878810.0, 1648619.0 / 878810.0, 36519.0 / 878810.0 },
        { 705.0 / 12673.0, -2585.0 / 12673.0, 705.0 / 667.0 },
    };

    /// <summary>The colour oklch(<paramref name="l"/> <paramref name="c"/> <paramref name="hue"/>) with <paramref name="alpha"/>.</summary>
    public static Rgba ToRgba(double l, double c, double hue, double alpha = 1)
    {
        var h = hue * Math.PI / 180;
        var lms = Multiply(OklabToLms, [l, c * Math.Cos(h), c * Math.Sin(h)]);
        for (var i = 0; i < 3; i++)
        {
            lms[i] = lms[i] * lms[i] * lms[i];
        }

        var rgb = Multiply(XyzToLinearSrgb, Multiply(LmsToXyz, lms));
        return new Rgba(Quantize(Encode(rgb[0])), Quantize(Encode(rgb[1])), Quantize(Encode(rgb[2])), Quantize(alpha));
    }

    private static double[] Multiply(double[,] m, double[] v) =>
    [
        (m[0, 0] * v[0]) + (m[0, 1] * v[1]) + (m[0, 2] * v[2]),
        (m[1, 0] * v[0]) + (m[1, 1] * v[1]) + (m[1, 2] * v[2]),
        (m[2, 0] * v[0]) + (m[2, 1] * v[1]) + (m[2, 2] * v[2]),
    ];

    private static double Encode(double v)
    {
        var a = Math.Abs(v);
        var encoded = a > 0.0031308 ? (1.055 * Math.Pow(a, 1 / 2.4)) - 0.055 : 12.92 * a;
        return Math.Clamp(Math.CopySign(encoded, v), 0, 1);
    }

    private static byte Quantize(double v) => (byte)Math.Floor((Math.Clamp(v, 0, 1) * 255) + 0.5);
}

/// <summary>
/// Preferences → Appearance → Accent colour (docs/design-controls.md, "v0.0.4"): follow the
/// artwork, one of ten swatches, or a custom hue. All three are the core's hue source.
/// </summary>
public static class AccentChoices
{
    /// <summary>The swatches' hues in the order shown: Misty blue first, then round the wheel.</summary>
    public static IReadOnlyList<float> SwatchHues { get; } = [250, 210, 180, 150, 120, 60, 30, 0, 330, 290];

    /// <summary>
    /// The swatches' names in <see cref="SwatchHues"/> order, as the string keys the app translates
    /// (the names the other apps show; docs/ui.md: the last is Violet, not Purple).
    /// </summary>
    public static IReadOnlyList<string> SwatchNameKeys { get; } =
        ["Accent_Blue", "Accent_Cyan", "Accent_Teal", "Accent_Green", "Accent_Olive", "Accent_Orange", "Accent_Red", "Accent_Rose", "Accent_Magenta", "Accent_Violet"];

    /// <summary>A hue in degrees, normalised to 0 ≤ h &lt; 360 (NaN counts as Misty's 250).</summary>
    public static float Normalize(float hue)
    {
        if (!float.IsFinite(hue))
        {
            return FallbackTheme.Hue;
        }

        var h = hue % 360f;
        if (h < 0)
        {
            h += 360f;
        }

        return h >= 360f ? 0f : h;
    }

    /// <summary>The swatch a hue source shows as chosen: its index, or -1 for the artwork's hue or a custom one.</summary>
    public static int SwatchIndex(HueSource source)
    {
        if (source is not HueSource.Fixed f)
        {
            return -1;
        }

        var hue = Normalize(f.Hue);
        for (var i = 0; i < SwatchHues.Count; i++)
        {
            var d = Math.Abs(hue - SwatchHues[i]);
            if (Math.Min(d, 360 - d) < 0.5)
            {
                return i;
            }
        }

        return -1;
    }

    /// <summary>The accent at <paramref name="hue"/>: accent.accent is oklch(0.46 0.15 h) light, oklch(0.78 0.12 h) dark.</summary>
    public static Rgba Accent(float hue, bool dark) =>
        dark ? Oklch.ToRgba(0.78, 0.12, Normalize(hue)) : Oklch.ToRgba(0.46, 0.15, Normalize(hue));

    /// <summary>Stops for the custom hue slider's track: the accent every 30°, at 0 and 1 the same red.</summary>
    public static IReadOnlyList<(double Offset, Rgba Color)> HueTrack(bool dark) =>
        Enumerable.Range(0, 13).Select(i => (i / 12.0, Accent(i * 30f % 360f, dark))).ToArray();
}

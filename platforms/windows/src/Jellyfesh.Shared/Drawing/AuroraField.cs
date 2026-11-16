namespace Jellyfesh.Drawing;

/// <summary>
/// One glow of the field: a radial gradient in the field's box, its centre and radii as fractions
/// of the box's width and height, fading from the colour at its centre to nothing at
/// <see cref="FadeEnd"/> of its radius.
/// </summary>
/// <param name="Color">The theme colour (a key in the core's Theme slice).</param>
/// <param name="Weight">How strong this glow is relative to field.aurora-strength.</param>
public readonly record struct AuroraGlow(string Color, double X, double Y, double RadiusX, double RadiusY, double Weight, double FadeEnd)
{
    /// <summary>The glow's opacity at its centre for an aurora strength of 0 to 1.</summary>
    public double Alpha(double strength) => Math.Clamp(strength * Weight, 0, 1);
}

/// <summary>
/// The Luminous field (docs/ui.md, v0.0.6): four large glows at the artwork's hue over the sky, on
/// every screen, as strong as field.aurora-strength (0.75; Misty's was about 0.45). The same four
/// glows as the other apps (the design's mock-up and Jellyfesh Web): the first two at 107 % and 96 %
/// of the strength in opposite corners, a third at 88 % bottom left, and a quieter fourth (61 %)
/// top right.
/// </summary>
public static class AuroraField
{
    public static IReadOnlyList<AuroraGlow> Glows { get; } =
    [
        new("field.aurora-1", 0.04, 0.00, 0.62, 0.58, 1.07, 0.72),
        new("field.aurora-2", 1.00, 0.96, 0.58, 0.64, 0.96, 0.72),
        new("field.aurora-3", 0.00, 1.00, 0.44, 0.52, 0.88, 0.70),
        new("field.aurora-1", 0.92, 0.04, 0.38, 0.40, 0.61, 0.70),
    ];
}

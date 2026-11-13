// Reference port of jf_palette::glass_alpha (core/jf-palette/src/theme.rs). Tested against
// tests/contract/glass-alpha.json; keep it in step with the Rust original.

using System;

namespace Jellyfesh.Shared;

/// <summary>Glass for the transparency setting (docs/design-controls.md, "v0.0.4").</summary>
public static class Glass
{
    /// <summary>The transparency setting that keeps the glass tokens' own alphas.</summary>
    public const float DefaultTransparency = 0.5f;

    /// <summary>
    /// The alpha of a glass grade (Clear, Mist, Veil) whose token alpha is <paramref name="alpha"/>,
    /// at the transparency setting <paramref name="transparency"/> (0 solid, 1 clearest). At 0.5 the
    /// token's alpha applies unchanged; toward 0 it rises linearly to 1, so glass becomes opaque;
    /// toward 1 it falls linearly to half the token's alpha. Inputs are clamped to 0..1; a
    /// transparency that is not finite counts as 0.5 and an alpha that is not finite as 1.
    /// </summary>
    public static float Alpha(float alpha, float transparency)
    {
        // In double, so the ends and the default come out exact, as in the Rust original.
        double a = float.IsFinite(alpha) ? Math.Clamp(alpha, 0f, 1f) : 1.0;
        double t = float.IsFinite(transparency) ? Math.Clamp(transparency, 0f, 1f) : 0.5;
        double result = t <= 0.5 ? a + ((1.0 - a) * (1.0 - (t / 0.5))) : a * (1.0 - (t - 0.5));
        return Math.Clamp((float)result, 0f, 1f);
    }
}

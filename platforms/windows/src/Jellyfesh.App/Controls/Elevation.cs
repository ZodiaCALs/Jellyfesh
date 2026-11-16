using System.Numerics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Controls;

/// <summary>
/// Misty's shadows (shadow-1, -2 and -3) as ThemeShadow depths. Outside a popup a ThemeShadow is
/// drawn only on the elements in its Receivers, which cannot be ancestors of the caster, so each
/// layer has one shared shadow cast onto its backdrop (what the ThemeShadow documentation
/// recommends): the window's field for the library, Now Playing's backdrop for the stage.
/// </summary>
internal static class Elevation
{
    /// <summary>shadow-1: grouped surfaces and the player bar.</summary>
    public const float Shadow1 = 8;

    /// <summary>shadow-2: grid covers at rest.</summary>
    public const float Shadow2 = 16;

    /// <summary>shadow-3: Now Playing's artwork, and a cover lifted on hover or focus.</summary>
    public const float Shadow3 = 32;

    /// <summary>The library's shadow, cast onto the window's field.</summary>
    public static ThemeShadow Library { get; } = new();

    /// <summary>Now Playing's shadow, cast onto its backdrop.</summary>
    public static ThemeShadow Stage { get; } = new();

    public static void AddReceiver(ThemeShadow shadow, UIElement receiver)
    {
        if (!shadow.Receivers.Contains(receiver))
        {
            shadow.Receivers.Add(receiver);
        }
    }

    /// <summary>Raises <paramref name="element"/> to depth <paramref name="z"/> with the layer's shadow (none at 0), keeping its x and y offset.</summary>
    public static void Cast(UIElement element, float z, ThemeShadow? layer = null)
    {
        element.Shadow = z > 0 ? layer ?? Library : null;
        var t = element.Translation;
        element.Translation = new Vector3(t.X, t.Y, z);
    }
}

using Jellyfesh.Core;

namespace Jellyfesh.Appearance;

/// <summary>One app icon option (ADR-0017).</summary>
/// <param name="Id">The value stored in <see cref="AppSettings.AppIcon"/> and the asset folder name.</param>
/// <param name="NameKey">The localised display name's resource key.</param>
/// <param name="ShipsAsIco">Ice and Steel ship as full .ico sets; the others are built from PNGs on first selection.</param>
public sealed record AppIconOption(string Id, string NameKey, bool ShipsAsIco);

/// <summary>The ten icons and the Automatic choice: Ice in light appearance, Steel in dark.</summary>
public static class AppIcons
{
    public const string Automatic = "automatic";
    public const string DefaultLight = "ice";
    public const string DefaultDark = "steel";

    /// <summary>The PNG sizes shipped for every icon (16 and 32 use the simplified small master).</summary>
    public static readonly int[] PngSizes = [16, 24, 32, 48, 64, 256];

    public static IReadOnlyList<AppIconOption> All { get; } =
    [
        new("ice", "Icon_Ice", true),
        new("steel", "Icon_Steel", true),
        new("original", "Icon_Original", false),
        new("navy-tide", "Icon_NavyTide", false),
        new("royal", "Icon_Royal", false),
        new("teal-deep", "Icon_TealDeep", false),
        new("sea-glass", "Icon_SeaGlass", false),
        new("electric", "Icon_Electric", false),
        new("abyss", "Icon_Abyss", false),
        new("lagoon", "Icon_Lagoon", false),
    ];

    public static bool IsKnown(string? id) => id == Automatic || All.Any(o => o.Id == id);

    /// <summary>The icon to show for a setting in the given appearance; unknown values act as Automatic.</summary>
    public static string Resolve(string? setting, Core.Appearance appearance) =>
        setting is not null && setting != Automatic && All.Any(o => o.Id == setting)
            ? setting
            : appearance == Core.Appearance.Dark ? DefaultDark : DefaultLight;
}

/// <summary>Light or dark, from the setting and the system's choice.</summary>
public static class AppearanceResolver
{
    public static Core.Appearance Effective(AppearancePreference preference, bool systemIsDark) => preference switch
    {
        AppearancePreference.Light => Core.Appearance.Light,
        AppearancePreference.Dark => Core.Appearance.Dark,
        _ => systemIsDark ? Core.Appearance.Dark : Core.Appearance.Light,
    };

    /// <summary>The theme to draw with: the core's, or the fallback hue's before anything played.</summary>
    public static ThemePalette Palette(ThemeState? theme, Core.Appearance appearance) =>
        (theme ?? FallbackTheme.Value).For(appearance);
}

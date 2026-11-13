using Jellyfesh.Appearance;
using Jellyfesh.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Windows.UI;
using Windows.UI.ViewManagement;

namespace Jellyfesh.App.Theming;

/// <summary>
/// The app's colours as shared brush resources (JfInk90, JfAccent, …) whose colours come from the
/// core's Theme slice (Misty PLT-03): one hue, resolved by the core, identical on every platform.
/// Brushes are created once and their colours changed in place, so a new hue restyles every
/// element without re-templating. A hue change crossfades over dur-4 with the Fluid curve, driven
/// per frame only for those 600 ms; under reduced motion the crossfade is short (dur-1). Glass brushes (in-app acrylic
/// tinted Clear, Mist and Veil) fall back to solid Cloud when transparency is off. The
/// Transparency setting sets each grade's tint opacity through the glass-alpha port
/// (platforms/shared/csharp, tests/contract/glass-alpha.json): the token's own alpha at 50 %,
/// opaque toward Solid (solid Cloud at 0 %), half the alpha toward Clear.
/// </summary>
internal static class ThemeBrushes
{
    private static readonly Dictionary<string, SolidColorBrush> Solid = [];
    private static readonly Dictionary<string, AcrylicBrush> Glass = [];
    private static readonly List<(RadialGradientBrush Brush, string Color)> Auroras = [];
    private static ThemePalette _current = FallbackTheme.Value.Light;
    private static ThemePalette? _from;
    private static ThemePalette? _to;
    private static DateTime _fadeStart;
    private static TimeSpan _fadeDuration = TokenValues.Duration(4);
    private static bool _rendering;
    private static bool _solidGlass;
    private static float _transparency = global::Jellyfesh.Shared.Glass.DefaultTransparency;

    /// <summary>Adds the brushes to the application's resources (before any XAML that uses them loads).</summary>
    public static void Install(ResourceDictionary resources)
    {
        foreach (var (key, color) in TokenValues.Brushes)
        {
            var brush = new SolidColorBrush(ToColor(_current[color]));
            Solid[key] = brush;
            resources[key] = brush;
        }

        foreach (var (key, tint) in new[] { ("JfGlassClear", "JfTintClear"), ("JfGlassMist", "JfTintMist"), ("JfGlassVeil", "JfTintVeil") })
        {
            var brush = new AcrylicBrush { FallbackColor = Solid["JfCloud"].Color };
            Glass[key] = brush;
            resources[key] = brush;
            ApplyGlass(key, tint);
        }

        // The Misty field: three soft aurora glows over the sky (MAT-02), at the theme hue.
        resources["JfAuroraGlow1"] = Aurora("field.aurora-1", 0.10, 0.08, 0.60, 0.55);
        resources["JfAuroraGlow2"] = Aurora("field.aurora-2", 0.92, 0.92, 0.55, 0.55);
        resources["JfAuroraGlow3"] = Aurora("field.aurora-3", 0.04, 0.98, 0.45, 0.50);
    }

    /// <summary>Shows <paramref name="palette"/>; crossfades unless <paramref name="instant"/>.</summary>
    /// <summary>
    /// Restyles every brush for <paramref name="palette"/>: at once, over dur-4, or over dur-1 when
    /// <paramref name="quick"/> (reduced motion).
    /// </summary>
    public static void Apply(ThemePalette palette, bool instant, bool quick = false)
    {
        if (instant || ReferenceEquals(palette, _current))
        {
            StopFade();
            Set(palette, palette, 1);
            _current = palette;
            return;
        }

        _from = _current;
        _to = palette;
        _fadeStart = DateTime.UtcNow;
        _fadeDuration = TokenValues.Duration(quick ? 1 : 4);
        if (!_rendering)
        {
            CompositionTarget.Rendering += OnRendering;
            _rendering = true;
        }
    }

    /// <summary>Solid Cloud instead of glass: transparency effects off, battery saver, high contrast, reduce effects.</summary>
    public static void SetSolidGlass(bool solid)
    {
        _solidGlass = solid;
        ApplyGlass("JfGlassClear", "JfTintClear");
        ApplyGlass("JfGlassMist", "JfTintMist");
        ApplyGlass("JfGlassVeil", "JfTintVeil");
    }

    /// <summary>The Transparency setting (0 solid … 1 clearest): re-tints the three glass grades at once.</summary>
    public static void SetTransparency(float transparency)
    {
        if (transparency == _transparency)
        {
            return;
        }

        _transparency = transparency;
        ApplyGlass("JfGlassClear", "JfTintClear");
        ApplyGlass("JfGlassMist", "JfTintMist");
        ApplyGlass("JfGlassVeil", "JfTintVeil");
    }

    /// <summary>High contrast: the system's colours instead of the theme's.</summary>
    public static void ApplyHighContrast(UISettings ui)
    {
        StopFade();
        var text = ui.UIElementColor(UIElementType.WindowText);
        var window = ui.UIElementColor(UIElementType.Window);
        var highlight = ui.UIElementColor(UIElementType.Highlight);
        var highlightText = ui.UIElementColor(UIElementType.HighlightText);
        foreach (var (key, brush) in Solid)
        {
            brush.Color = key switch
            {
                "JfSky" or "JfSkyHigh" or "JfCloud" or "JfCloudSoft" or "JfAurora1" or "JfAurora2" or "JfAurora3" => window,
                "JfAccent" or "JfAccentHover" or "JfAccentPress" or "JfAccentText" => highlight,
                "JfOnAccent" => highlightText,
                "JfAccentSoft" or "JfFill" or "JfFillHover" or "JfFillPress" or "JfInk06" or "JfInk10" => window,
                _ => text,
            };
        }

        SetSolidGlass(true);
    }

    private static void OnRendering(object? sender, object e)
    {
        if (_from is null || _to is null)
        {
            StopFade();
            return;
        }

        var t = (DateTime.UtcNow - _fadeStart).TotalMilliseconds / _fadeDuration.TotalMilliseconds;
        Set(_from, _to, TokenValues.Fluid.Ease(t));
        if (t >= 1)
        {
            _current = _to;
            StopFade();
        }
    }

    private static void StopFade()
    {
        if (_rendering)
        {
            CompositionTarget.Rendering -= OnRendering;
            _rendering = false;
        }

        _from = _to = null;
    }

    private static void Set(ThemePalette from, ThemePalette to, double t)
    {
        foreach (var (key, color) in TokenValues.Brushes)
        {
            var a = from[color] ?? to[color];
            var b = to[color] ?? a;
            if (a is { } ca && b is { } cb)
            {
                Solid[key].Color = ToColor(Rgba.Lerp(ca, cb, t));
            }
        }

        ApplyGlass("JfGlassClear", "JfTintClear");
        ApplyGlass("JfGlassMist", "JfTintMist");
        ApplyGlass("JfGlassVeil", "JfTintVeil");
        foreach (var (brush, color) in Auroras)
        {
            var a = from[color] ?? to[color];
            var b = to[color] ?? a;
            if (a is { } ca && b is { } cb)
            {
                var c = ToColor(Rgba.Lerp(ca, cb, t));
                brush.GradientStops[0].Color = c;
                brush.GradientStops[1].Color = Color.FromArgb(0, c.R, c.G, c.B);
            }
        }
    }

    private static void ApplyGlass(string key, string tintKey)
    {
        if (!Glass.TryGetValue(key, out var brush))
        {
            return; // during Install, before every grade exists
        }

        var tint = Solid[tintKey].Color;
        var alpha = global::Jellyfesh.Shared.Glass.Alpha(tint.A / 255f, _transparency);
        brush.TintColor = Color.FromArgb(255, tint.R, tint.G, tint.B);
        brush.TintOpacity = alpha;
        brush.TintLuminosityOpacity = alpha;
        brush.FallbackColor = Solid["JfCloud"].Color;
        // Opaque glass is the solid Cloud look (and costs no blur).
        brush.AlwaysUseFallback = _solidGlass || alpha >= 0.999f;
    }

    private static RadialGradientBrush Aurora(string color, double x, double y, double rx, double ry)
    {
        var c = ToColor(_current[color]);
        var brush = new RadialGradientBrush
        {
            Center = new Windows.Foundation.Point(x, y),
            GradientOrigin = new Windows.Foundation.Point(x, y),
            RadiusX = rx,
            RadiusY = ry,
            MappingMode = BrushMappingMode.RelativeToBoundingBox,
        };
        brush.GradientStops.Add(new GradientStop { Color = c, Offset = 0 });
        brush.GradientStops.Add(new GradientStop { Color = Color.FromArgb(0, c.R, c.G, c.B), Offset = 0.7 });
        Auroras.Add((brush, color));
        return brush;
    }

    public static Color ToColor(Rgba? rgba) =>
        rgba is { } c ? Color.FromArgb(c.A, c.R, c.G, c.B) : Color.FromArgb(0, 0, 0, 0);
}

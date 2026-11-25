using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Shell;

/// <summary>Functions for x:Bind (no converters, no reflection): visibility, glyphs and labels.</summary>
public static class Bind
{
    public static Visibility Visible(bool value) => value ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility Collapsed(bool value) => value ? Visibility.Collapsed : Visibility.Visible;

    public static Visibility VisibleIf(string? text) => string.IsNullOrEmpty(text) ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>Visible when it has text and is wanted (Home's summary while the look lists it).</summary>
    public static Visibility VisibleIfTextAnd(string? text, bool wanted) => !string.IsNullOrEmpty(text) && wanted ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility VisibleIfEmpty(string? text) => string.IsNullOrEmpty(text) ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility VisibleIfNull(object? value) => value is null ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility VisibleIfNotNull(object? value) => value is null ? Visibility.Collapsed : Visibility.Visible;

    public static Visibility VisibleIfAll(bool a, bool b) => a && b ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Collapsed when both hold (what something else replaces then).</summary>
    public static Visibility CollapsedIfAll(bool a, bool b) => a && b ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>Visible when neither <paramref name="a"/> nor <paramref name="b"/> holds.</summary>
    public static Visibility VisibleIfNeither(bool a, bool b) => !a && !b ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Visible when <paramref name="a"/> holds and <paramref name="b"/> does not.</summary>
    public static Visibility VisibleIfButNot(bool a, bool b) => a && !b ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Visible when <paramref name="a"/> holds and neither <paramref name="b"/> nor <paramref name="c"/> does.</summary>
    public static Visibility VisibleIfButNeither(bool a, bool b, bool c) => a && !b && !c ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility VisibleIfAny(int count) => count > 0 ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Visible when it has something and is wanted (a Home section the look lists).</summary>
    public static Visibility VisibleIfAnyAnd(int count, bool wanted) => count > 0 && wanted ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility VisibleIfNone(int count) => count == 0 ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>"Nothing found": a search was made, it found nothing, and nothing is still on its way.</summary>
    public static bool Not(bool value) => !value;

    /// <summary>The playing row's fill: accent-soft (Misty COL-06); clear otherwise.</summary>
    public static Brush CurrentRowFill(bool current) =>
        current ? (Brush)Application.Current.Resources["JfAccentSoft"] : new SolidColorBrush(Microsoft.UI.Colors.Transparent);

    /// <summary>The playing row's title in accent text; ink-90 otherwise.</summary>
    public static Brush CurrentRowInk(bool current) =>
        (Brush)Application.Current.Resources[current ? "JfAccentText" : "JfInk90"];

    public static double Dim(bool dim) => dim ? 0.45 : 1.0;

    public static TextWrapping Wrap(bool wrap) => wrap ? TextWrapping.Wrap : TextWrapping.NoWrap;

    /// <summary>A colour well's fill from "#RRGGBB" (a settings field); transparent for anything else.</summary>
    public static Brush ColorBrush(string? hex) => new SolidColorBrush(ColorOf(hex) ?? Microsoft.UI.Colors.Transparent);

    /// <summary>The colour "#RRGGBB" names, or null.</summary>
    public static Windows.UI.Color? ColorOf(string? hex) =>
        hex is { Length: 7 } && hex[0] == '#' && uint.TryParse(hex.AsSpan(1), System.Globalization.NumberStyles.HexNumber, null, out var rgb)
            ? Windows.UI.Color.FromArgb(255, (byte)(rgb >> 16), (byte)(rgb >> 8), (byte)rgb)
            : null;

    /// <summary>The touch keyboard for a sign-in field's kind (e-mail, web address, digits).</summary>
    public static InputScope InputScopeFor(AuthFieldKind kind)
    {
        var scope = new InputScope();
        scope.Names.Add(new InputScopeName(kind switch
        {
            AuthFieldKind.Email => InputScopeNameValue.EmailSmtpAddress,
            AuthFieldKind.Url => InputScopeNameValue.Url,
            AuthFieldKind.Number => InputScopeNameValue.Number,
            _ => InputScopeNameValue.Default,
        }));
        return scope;
    }

    public static string RepeatGlyph(RepeatMode mode) => Glyph(mode == RepeatMode.One ? "IconRepeatOne" : "IconRepeat");

    public static bool RepeatOn(RepeatMode mode) => mode != RepeatMode.Off;

    public static string VolumeGlyph(double volume, bool muted) =>
        Glyph(muted || volume <= 0 ? "IconMuted" : volume < 50 ? "IconVolumeLow" : "IconVolumeHigh");

    /// <summary>" · text" to follow another line's text, or nothing when <paramref name="text"/> is empty.</summary>
    public static string After(string? text) => string.IsNullOrEmpty(text) ? string.Empty : " · " + text;

    /// <summary>A localised string for a key held by a view model (form errors, labels).</summary>
    public static string Text(string? key) => string.IsNullOrEmpty(key) ? string.Empty : AppHost.L.Get(key);

    /// <summary>A localised message with <paramref name="value"/> in it, or nothing without a value ("This Source Did Not Answer: …").</summary>
    public static string Format(string key, string? value) => string.IsNullOrEmpty(value) ? string.Empty : AppHost.L.Format(key, value);

    public static string Glyph(string key) =>
        Application.Current.Resources.TryGetValue(key, out var value) && value is string path ? path : string.Empty;
}

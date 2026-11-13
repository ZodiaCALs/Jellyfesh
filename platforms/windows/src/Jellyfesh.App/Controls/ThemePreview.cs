using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Jellyfesh.Appearance;
using Jellyfesh.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Jellyfesh.App.Controls;

/// <summary>What a preview in Settings → Appearance shows.</summary>
public enum PreviewKind
{
    /// <summary>The light appearance: the field, a Cloud card, ink and the accent.</summary>
    Light,

    /// <summary>The dark appearance.</summary>
    Dark,

    /// <summary>Follows the system: light and dark side by side.</summary>
    System,
}

/// <summary>
/// A small picture of the theme for the Theme choice: a window of the field with a card, two ink
/// lines and the accent, in the core's colours at the current hue (light, dark or both halves).
/// </summary>
public sealed partial class ThemePreview : Grid
{
    public static readonly DependencyProperty KindProperty =
        DependencyProperty.Register(nameof(Kind), typeof(PreviewKind), typeof(ThemePreview), new PropertyMetadata(PreviewKind.Light, (d, _) => ((ThemePreview)d).Redraw()));

    public ThemePreview()
    {
        Width = 64;
        Height = 40;
        CornerRadius = new CornerRadius(TokenValues.Number("Radius.Md"));
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        IsHitTestVisible = false;
        Loaded += (_, _) =>
        {
            AppHost.App.ThemeChanged += Redraw;
            Redraw();
        };
        Unloaded += (_, _) => AppHost.App.ThemeChanged -= Redraw;
    }

    public PreviewKind Kind
    {
        get => (PreviewKind)GetValue(KindProperty);
        set => SetValue(KindProperty, value);
    }

    private void Redraw()
    {
        Children.Clear();
        ColumnDefinitions.Clear();
        var theme = AppHost.Services.Hub.State.Theme;
        var light = AppearanceResolver.Palette(theme, Core.Appearance.Light);
        var dark = AppearanceResolver.Palette(theme, Core.Appearance.Dark);
        if (Kind == PreviewKind.System)
        {
            ColumnDefinitions.Add(new ColumnDefinition());
            ColumnDefinitions.Add(new ColumnDefinition());
            Children.Add(Window(light, 0));
            Children.Add(Window(dark, 1));
        }
        else
        {
            Children.Add(Window(Kind == PreviewKind.Dark ? dark : light, 0));
        }
    }

    private static Grid Window(ThemePalette palette, int column)
    {
        SolidColorBrush Brush(string name) => new(ThemeBrushes.ToColor(palette[name]));
        var window = new Grid { Background = Brush("field.sky"), Padding = new Thickness(6) };
        SetColumn(window, column);
        var card = new StackPanel { Spacing = 4, Padding = new Thickness(6), Background = Brush("glass.cloud"), CornerRadius = new CornerRadius(4) };
        card.Children.Add(new Rectangle { Height = 3, Width = 20, RadiusX = 1.5, RadiusY = 1.5, HorizontalAlignment = HorizontalAlignment.Left, Fill = Brush("ink.ink-90") });
        card.Children.Add(new Rectangle { Height = 3, Width = 14, RadiusX = 1.5, RadiusY = 1.5, HorizontalAlignment = HorizontalAlignment.Left, Fill = Brush("ink.ink-50") });
        window.Children.Add(card);
        window.Children.Add(new Ellipse { Width = 8, Height = 8, Margin = new Thickness(0, 0, 4, 4), HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Bottom, Fill = Brush("accent.accent") });
        return window;
    }
}

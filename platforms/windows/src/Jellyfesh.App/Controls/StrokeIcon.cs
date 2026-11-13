using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Path = Microsoft.UI.Xaml.Shapes.Path;

namespace Jellyfesh.App.Controls;

/// <summary>
/// A Misty glyph (ICO-01: 1.75-unit strokes on a 24-unit grid, outline at rest, filled only for
/// active states such as pause). Takes its colour from the inherited Foreground, so button
/// states recolour it like a FontIcon. Path data lives in Styles/Controls.xaml as Icon* strings.
/// </summary>
public sealed partial class StrokeIcon : ContentControl
{
    public static readonly DependencyProperty DataProperty =
        DependencyProperty.Register(nameof(Data), typeof(string), typeof(StrokeIcon), new PropertyMetadata(string.Empty, (d, _) => ((StrokeIcon)d).UpdateGeometry()));

    public static readonly DependencyProperty FilledProperty =
        DependencyProperty.Register(nameof(Filled), typeof(bool), typeof(StrokeIcon), new PropertyMetadata(false, (d, _) => ((StrokeIcon)d).UpdateBrush()));

    public static readonly DependencyProperty SizeProperty =
        DependencyProperty.Register(nameof(Size), typeof(double), typeof(StrokeIcon), new PropertyMetadata(20.0, (d, _) => ((StrokeIcon)d).UpdateSize()));

    private readonly Path _path = new()
    {
        StrokeThickness = 1.75,
        StrokeStartLineCap = PenLineCap.Round,
        StrokeEndLineCap = PenLineCap.Round,
        StrokeLineJoin = PenLineJoin.Round,
    };

    private readonly Viewbox _box = new() { Stretch = Stretch.Uniform };

    public StrokeIcon()
    {
        IsTabStop = false;
        HorizontalContentAlignment = HorizontalAlignment.Stretch;
        VerticalContentAlignment = VerticalAlignment.Stretch;
        var canvas = new Canvas { Width = 24, Height = 24 };
        canvas.Children.Add(_path);
        _box.Child = canvas;
        Content = _box;
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        RegisterPropertyChangedCallback(ForegroundProperty, (_, _) => UpdateBrush());
        UpdateSize();
    }

    /// <summary>Path markup on the 24-unit grid.</summary>
    public string Data
    {
        get => (string)GetValue(DataProperty);
        set => SetValue(DataProperty, value);
    }

    /// <summary>Filled glyph (play, pause) instead of a stroke.</summary>
    public bool Filled
    {
        get => (bool)GetValue(FilledProperty);
        set => SetValue(FilledProperty, value);
    }

    public double Size
    {
        get => (double)GetValue(SizeProperty);
        set => SetValue(SizeProperty, value);
    }

    private void UpdateGeometry()
    {
        _path.Data = string.IsNullOrWhiteSpace(Data)
            ? null
            : (Geometry)XamlBindingHelper.ConvertValue(typeof(Geometry), Data);
    }

    private void UpdateBrush()
    {
        _path.Stroke = Filled ? null : Foreground;
        _path.Fill = Filled ? Foreground : null;
    }

    private void UpdateSize()
    {
        _box.Width = Size;
        _box.Height = Size;
    }
}

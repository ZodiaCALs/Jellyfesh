using Jellyfesh.Qr;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;
using Path = Microsoft.UI.Xaml.Shapes.Path;

namespace Jellyfesh.App.Controls;

/// <summary>Lays out children left to right, wrapping to new lines (karaoke words; mirrored for RTL by XAML).</summary>
public sealed partial class WrapPanel : Panel
{
    protected override Size MeasureOverride(Size availableSize)
    {
        double x = 0, y = 0, line = 0, width = 0;
        foreach (var child in Children)
        {
            child.Measure(new Size(availableSize.Width, double.PositiveInfinity));
            var s = child.DesiredSize;
            if (x > 0 && x + s.Width > availableSize.Width)
            {
                y += line;
                x = 0;
                line = 0;
            }

            x += s.Width;
            line = Math.Max(line, s.Height);
            width = Math.Max(width, x);
        }

        return new Size(double.IsInfinity(availableSize.Width) ? width : Math.Min(width, availableSize.Width), y + line);
    }

    protected override Size ArrangeOverride(Size finalSize)
    {
        double x = 0, y = 0, line = 0;
        foreach (var child in Children)
        {
            var s = child.DesiredSize;
            if (x > 0 && x + s.Width > finalSize.Width)
            {
                y += line;
                x = 0;
                line = 0;
            }

            child.Arrange(new Rect(x, y, s.Width, s.Height));
            x += s.Width;
            line = Math.Max(line, s.Height);
        }

        return finalSize;
    }
}

/// <summary>A QR code drawn as one vector path (crisp at any scale), with its quiet zone.</summary>
public sealed partial class QrCodeView : Grid
{
    public static readonly DependencyProperty CodeProperty =
        DependencyProperty.Register(nameof(Code), typeof(QrCode), typeof(QrCodeView), new PropertyMetadata(null, (d, _) => ((QrCodeView)d).Redraw()));

    private readonly Path _path = new() { Stretch = Stretch.Uniform };

    public QrCodeView()
    {
        // A QR code needs dark on light whatever the theme: white plate, black modules.
        Background = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 255, 255, 255));
        _path.Fill = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 0, 0, 0));
        Padding = new Thickness(16);
        Children.Add(_path);
    }

    public QrCode? Code
    {
        get => (QrCode?)GetValue(CodeProperty);
        set => SetValue(CodeProperty, value);
    }

    private void Redraw()
    {
        if (Code is not { } code)
        {
            _path.Data = null;
            return;
        }

        var group = new GeometryGroup { FillRule = FillRule.Nonzero };
        for (var y = 0; y < code.Size; y++)
        {
            for (var x = 0; x < code.Size; x++)
            {
                if (!code[x, y])
                {
                    continue;
                }

                var start = x;
                while (x + 1 < code.Size && code[x + 1, y])
                {
                    x++;
                }

                group.Children.Add(new RectangleGeometry { Rect = new Rect(start, y, x - start + 1, 1) });
            }
        }

        // Pin the full extent so Stretch keeps every module square.
        group.Children.Add(new RectangleGeometry { Rect = new Rect(0, 0, code.Size, 0.0001) });
        _path.Data = group;
    }
}

/// <summary>
/// Artwork for an Image: <c>controls:Artwork.Uri="{x:Bind Item.ArtworkUri}"</c>. Decodes at the
/// element's size (never the full JPEG), and clears the image when the row is recycled.
/// </summary>
public static class Artwork
{
    public static readonly DependencyProperty UriProperty =
        DependencyProperty.RegisterAttached("Uri", typeof(string), typeof(Artwork), new PropertyMetadata(null, OnUriChanged));

    public static readonly DependencyProperty DecodeSizeProperty =
        DependencyProperty.RegisterAttached("DecodeSize", typeof(int), typeof(Artwork), new PropertyMetadata(0));

    public static string? GetUri(DependencyObject d) => (string?)d.GetValue(UriProperty);

    public static void SetUri(DependencyObject d, string? value) => d.SetValue(UriProperty, value);

    public static int GetDecodeSize(DependencyObject d) => (int)d.GetValue(DecodeSizeProperty);

    public static void SetDecodeSize(DependencyObject d, int value) => d.SetValue(DecodeSizeProperty, value);

    private static void OnUriChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is not Image image)
        {
            return;
        }

        if (e.NewValue is not string uri || !Uri.TryCreate(uri, UriKind.Absolute, out var parsed))
        {
            image.Source = null;
            return;
        }

        var size = GetDecodeSize(image);
        if (size <= 0)
        {
            size = (int)Math.Ceiling((double.IsNaN(image.Width) ? 256 : image.Width) * (image.XamlRoot?.RasterizationScale ?? 1.0));
        }

        image.Source = new BitmapImage
        {
            UriSource = parsed,
            DecodePixelWidth = size,
            DecodePixelType = DecodePixelType.Physical,
        };
    }
}

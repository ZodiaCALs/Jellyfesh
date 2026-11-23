using System.Numerics;
using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Controls;

/// <summary>
/// An album in the grid (after Apple Music and Media Player; Luminous): the cover at
/// artwork.radius-grid (16) on shadow-2, with its title and artist below. Pointing at it or giving
/// its tile keyboard focus lifts the cover by 4 epx onto a deeper shadow and shows a 36 px glass
/// play button in its bottom start corner that plays the album; a press scales the cover to 97 %.
/// Motion runs on the compositor (dur-1 Fluid) and becomes instant with reduced motion.
/// </summary>
public sealed partial class AlbumTile : StackPanel
{
    public static readonly DependencyProperty AlbumProperty =
        DependencyProperty.Register(nameof(Album), typeof(AlbumItem), typeof(AlbumTile), new PropertyMetadata(null));

    public static readonly DependencyProperty ArtworkUriProperty =
        DependencyProperty.Register(nameof(ArtworkUri), typeof(string), typeof(AlbumTile), new PropertyMetadata(null, (d, e) => Artwork.SetUri(((AlbumTile)d)._image, (string?)e.NewValue)));

    public static readonly DependencyProperty TitleProperty =
        DependencyProperty.Register(nameof(Title), typeof(string), typeof(AlbumTile), new PropertyMetadata(string.Empty, (d, e) => ((AlbumTile)d)._title.Text = (string)e.NewValue));

    public static readonly DependencyProperty SubtitleProperty =
        DependencyProperty.Register(nameof(Subtitle), typeof(string), typeof(AlbumTile), new PropertyMetadata(string.Empty, (d, e) => ((AlbumTile)d)._subtitle.Text = (string)e.NewValue));

    public static readonly DependencyProperty CoverSizeProperty =
        DependencyProperty.Register(nameof(CoverSize), typeof(double), typeof(AlbumTile), new PropertyMetadata(DefaultCoverSize, (d, e) => ((AlbumTile)d).SizeCover((double)e.NewValue)));

    /// <summary>The cover's side in shelves; the Albums grid widens covers to fill its columns (Drawing.AlbumGrid).</summary>
    public const double DefaultCoverSize = 168;
    private readonly Grid _cover;
    private readonly Image _image = new() { Stretch = Stretch.UniformToFill };
    private readonly Button _play;
    private readonly TextBlock _title = new();
    private readonly TextBlock _subtitle = new();
    private bool _hovered;
    private bool _focused;
    private bool _lifted;
    private Microsoft.UI.Xaml.Controls.Primitives.SelectorItem? _container;

    public AlbumTile()
    {
        Width = DefaultCoverSize;
        Spacing = 8;
        var resources = Application.Current.Resources;
        Artwork.SetDecodeSize(_image, DecodeSize(DefaultCoverSize));
        _cover = new Grid
        {
            Width = DefaultCoverSize,
            Height = DefaultCoverSize,
            CenterPoint = new Vector3((float)DefaultCoverSize / 2, (float)DefaultCoverSize / 2, 0),
        };
        _cover.Children.Add(new Border
        {
            CornerRadius = new CornerRadius(TokenValues.Number("Artwork.RadiusGrid")),
            Background = (Brush)resources["JfFill"],
            Child = _image,
        });
        _play = new Button
        {
            Style = (Style)resources["JfGlassPlayButtonStyle"],
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(8),
            Opacity = 0,
            IsTabStop = false,
            Content = new StrokeIcon { Data = Bind.Glyph("IconPlay"), Filled = true, Size = 16 },
        };
        AutomationProperties.SetName(_play, AppHost.L.Get("Album_PlayAlbum"));
        ToolTipService.SetToolTip(_play, AppHost.L.Get("Album_PlayAlbum"));
        _play.Click += OnPlayClick;
        _cover.Children.Add(_play);
        _title.Style = (Style)resources["JfListTitleText"];
        _subtitle.Style = (Style)resources["JfListSecondaryText"];
        Children.Add(_cover);
        Children.Add(new StackPanel { Spacing = 2, Children = { _title, _subtitle } });
        Elevation.Cast(_cover, Elevation.Shadow2);
        PointerEntered += (_, _) => SetHovered(true);
        PointerExited += (_, _) => SetHovered(false);
        PointerCanceled += (_, _) => SetHovered(false);
        Loaded += (_, _) => WatchContainerFocus();
        Unloaded += (_, _) => WatchContainerFocus(stop: true);
        PointerPressed += OnPressed;
        PointerReleased += (_, _) => Press(false);
        PointerCaptureLost += (_, _) => Press(false);
    }

    /// <summary>The album (a placeholder until its page of the core's query arrives).</summary>
    public AlbumItem? Album
    {
        get => (AlbumItem?)GetValue(AlbumProperty);
        set => SetValue(AlbumProperty, value);
    }

    private bool IsReady => Album is { IsLoaded: true };

    public string? ArtworkUri
    {
        get => (string?)GetValue(ArtworkUriProperty);
        set => SetValue(ArtworkUriProperty, value);
    }

    public string Title
    {
        get => (string)GetValue(TitleProperty);
        set => SetValue(TitleProperty, value);
    }

    public string Subtitle
    {
        get => (string)GetValue(SubtitleProperty);
        set => SetValue(SubtitleProperty, value);
    }

    /// <summary>The cover's side (epx); the tile is as wide.</summary>
    public double CoverSize
    {
        get => (double)GetValue(CoverSizeProperty);
        set => SetValue(CoverSizeProperty, value);
    }

    /// <summary>Covers decode at twice their side (crisp up to 200 % scaling), in 64 px steps so resizing rarely decodes again.</summary>
    private static int DecodeSize(double cover) => (int)(Math.Ceiling(cover * 2 / 64) * 64);

    private void SizeCover(double size)
    {
        if (!double.IsFinite(size) || size <= 0)
        {
            return;
        }

        Width = size;
        _cover.Width = size;
        _cover.Height = size;
        _cover.CenterPoint = new Vector3((float)size / 2, (float)size / 2, 0);
        var decode = DecodeSize(size);
        if (decode > Artwork.GetDecodeSize(_image))
        {
            // A larger cover than the picture was decoded for: decode it again at the new size.
            Artwork.SetDecodeSize(_image, decode);
            if (ArtworkUri is { } uri)
            {
                Artwork.SetUri(_image, null);
                Artwork.SetUri(_image, uri);
            }
        }
    }

    private static bool ReduceMotion => AppHost.App.ReduceMotion;

    private TimeSpan Duration => ReduceMotion ? TimeSpan.Zero : TokenValues.Duration(1);

    private void SetHovered(bool hovered)
    {
        if (hovered == _hovered)
        {
            return;
        }

        _hovered = hovered;
        UpdateLift();
        if (!hovered)
        {
            Press(false);
        }
    }

    /// <summary>The tile's container (the grid item) takes keyboard focus: the cover lifts as on hover.</summary>
    private void WatchContainerFocus(bool stop = false)
    {
        if (_container is not null)
        {
            _container.GotFocus -= OnContainerFocus;
            _container.LostFocus -= OnContainerFocus;
            _container = null;
        }

        if (stop)
        {
            return;
        }

        DependencyObject? node = this;
        while (node is not null and not Microsoft.UI.Xaml.Controls.Primitives.SelectorItem)
        {
            node = VisualTreeHelper.GetParent(node);
        }

        _container = node as Microsoft.UI.Xaml.Controls.Primitives.SelectorItem;
        if (_container is not null)
        {
            _container.GotFocus += OnContainerFocus;
            _container.LostFocus += OnContainerFocus;
        }
    }

    private void OnContainerFocus(object sender, RoutedEventArgs e)
    {
        _focused = _container is { FocusState: FocusState.Keyboard or FocusState.Programmatic };
        UpdateLift();
    }

    /// <summary>Lifted 4 epx onto shadow-3 while pointed at or focused, else on shadow-2 (Luminous).</summary>
    private void UpdateLift()
    {
        var lifted = _hovered || _focused;
        if (lifted == _lifted)
        {
            return;
        }

        _lifted = lifted;
        Motion.Translate(_cover, new Vector3(0, lifted ? -4 : 0, lifted ? Elevation.Shadow3 : Elevation.Shadow2), Duration, TokenValues.Fluid);
        Motion.Fade(_play, lifted && IsReady ? 1 : 0, Duration, TokenValues.Fluid);
    }

    private void OnPressed(object sender, PointerRoutedEventArgs e)
    {
        if (!ReferenceEquals(e.OriginalSource, _play))
        {
            Press(true);
        }
    }

    private void Press(bool pressed) => Motion.Scale(_cover, pressed ? 0.97f : 1f, ReduceMotion ? TimeSpan.Zero : TokenValues.Duration(0), TokenValues.Spring);

    private void OnPlayClick(object sender, RoutedEventArgs e)
    {
        if (Album is { IsLoaded: true } album)
        {
            AppHost.Services.Send(new CoreCommand.PlayCollection(new Collection.Album(album.Id), 0, false));
        }
    }
}

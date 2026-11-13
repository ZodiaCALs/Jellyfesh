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
/// An album in the grid (after Apple Music and Media Player): the cover at artwork.radius-grid
/// with its title and artist below. Hovering lifts the cover by 2 px with a shadow and shows a
/// 36 px glass play button in its bottom start corner that plays the album; a press scales the
/// cover to 97 %. Motion runs on the compositor (dur-1 Fluid) and becomes instant with reduced motion.
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

    private const double CoverSize = 168;
    private readonly Grid _cover;
    private readonly Image _image = new() { Stretch = Stretch.UniformToFill };
    private readonly Button _play;
    private readonly TextBlock _title = new();
    private readonly TextBlock _subtitle = new();
    private bool _hovered;

    public AlbumTile()
    {
        Width = CoverSize;
        Spacing = 8;
        var resources = Application.Current.Resources;
        Artwork.SetDecodeSize(_image, (int)CoverSize * 2);
        _cover = new Grid
        {
            Width = CoverSize,
            Height = CoverSize,
            CenterPoint = new Vector3((float)CoverSize / 2, (float)CoverSize / 2, 0),
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
        PointerEntered += (_, _) => SetHovered(true);
        PointerExited += (_, _) => SetHovered(false);
        PointerCanceled += (_, _) => SetHovered(false);
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

    private static bool ReduceMotion => AppHost.App.ReduceMotion;

    private TimeSpan Duration => ReduceMotion ? TimeSpan.Zero : TokenValues.Duration(1);

    private void SetHovered(bool hovered)
    {
        if (hovered == _hovered)
        {
            return;
        }

        _hovered = hovered;
        _cover.Shadow = hovered ? new ThemeShadow() : null;
        Motion.Translate(_cover, new Vector3(0, hovered ? -2 : 0, hovered ? 16 : 0), Duration, TokenValues.Fluid);
        Motion.Fade(_play, hovered && IsReady ? 1 : 0, Duration, TokenValues.Fluid);
        if (!hovered)
        {
            Press(false);
        }
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

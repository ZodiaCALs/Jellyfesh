using System.ComponentModel;
using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Controls;

/// <summary>
/// An album row's first cell (after Apple Music and Media Player): the track number, which turns
/// into a play glyph while the row is hovered; on the row of the song playing, the level glyph,
/// which moves only while music plays. The whole row stays the click target.
/// </summary>
public sealed partial class TrackNumberCell : Grid
{
    public static readonly DependencyProperty NumberProperty =
        DependencyProperty.Register(nameof(Number), typeof(string), typeof(TrackNumberCell), new PropertyMetadata(string.Empty, (d, e) => ((TrackNumberCell)d)._number.Text = (string)e.NewValue));

    public static readonly DependencyProperty IsCurrentProperty =
        DependencyProperty.Register(nameof(IsCurrent), typeof(bool), typeof(TrackNumberCell), new PropertyMetadata(false, (d, _) => ((TrackNumberCell)d).Update()));

    private readonly TextBlock _number = new() { HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center };
    private readonly StrokeIcon _play = new() { Filled = true, Size = 14, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center };
    private readonly LevelGlyph _level = new() { HorizontalAlignment = HorizontalAlignment.Right };
    private UIElement? _row;
    private bool _hovered;

    public TrackNumberCell()
    {
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        _number.Style = (Style)Application.Current.Resources["JfTimeText"];
        _play.Data = Bind.Glyph("IconPlay");
        _play.Foreground = (Brush)Application.Current.Resources["JfInk90"];
        Children.Add(_number);
        Children.Add(_play);
        Children.Add(_level);
        Loaded += OnLoaded;
        Unloaded += OnUnloaded;
        Update();
    }

    public string Number
    {
        get => (string)GetValue(NumberProperty);
        set => SetValue(NumberProperty, value);
    }

    public bool IsCurrent
    {
        get => (bool)GetValue(IsCurrentProperty);
        set => SetValue(IsCurrentProperty, value);
    }

    private static PlayerViewModel Player => AppHost.App.Player;

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        _row = Parent as UIElement;
        if (_row is not null)
        {
            _row.PointerEntered += OnRowEntered;
            _row.PointerExited += OnRowExited;
        }

        Player.PropertyChanged += OnPlayerChanged;
        AppHost.App.PropertyChanged += OnPlayerChanged;
        Update();
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        if (_row is not null)
        {
            _row.PointerEntered -= OnRowEntered;
            _row.PointerExited -= OnRowExited;
            _row = null;
        }

        Player.PropertyChanged -= OnPlayerChanged;
        AppHost.App.PropertyChanged -= OnPlayerChanged;
        _hovered = false;
    }

    private void OnRowEntered(object sender, PointerRoutedEventArgs e)
    {
        _hovered = true;
        Update();
    }

    private void OnRowExited(object sender, PointerRoutedEventArgs e)
    {
        _hovered = false;
        Update();
    }

    private void OnPlayerChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(PlayerViewModel.IsPlaying) or nameof(AppViewModel.ReduceMotion))
        {
            Update();
        }
    }

    private void Update()
    {
        var showLevel = IsCurrent && !_hovered;
        _number.Visibility = !IsCurrent && !_hovered ? Visibility.Visible : Visibility.Collapsed;
        _play.Visibility = _hovered ? Visibility.Visible : Visibility.Collapsed;
        _level.Visibility = showLevel ? Visibility.Visible : Visibility.Collapsed;
        _level.ReduceMotion = AppHost.App.ReduceMotion;
        _level.IsPlaying = showLevel && Player.IsPlaying && IsLoaded;
    }
}

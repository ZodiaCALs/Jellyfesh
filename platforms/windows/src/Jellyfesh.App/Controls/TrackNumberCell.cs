using System.ComponentModel;
using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Controls;

/// <summary>
/// An album row's first cell (after Apple Music and Media Player): the track number, which turns
/// into the row's play button while the row is pointed at or has keyboard focus (docs/ui.md, "Rows
/// and cards"); on the row of the song playing, the level glyph, which moves only while music
/// plays. The whole row stays the click target, so the play button plays from that row as choosing
/// it does; a row that cannot play here keeps its number.
/// </summary>
public sealed partial class TrackNumberCell : Grid
{
    public static readonly DependencyProperty NumberProperty =
        DependencyProperty.Register(nameof(Number), typeof(string), typeof(TrackNumberCell), new PropertyMetadata(string.Empty, (d, e) => ((TrackNumberCell)d)._number.Text = (string)e.NewValue));

    public static readonly DependencyProperty IsCurrentProperty =
        DependencyProperty.Register(nameof(IsCurrent), typeof(bool), typeof(TrackNumberCell), new PropertyMetadata(false, (d, _) => ((TrackNumberCell)d).Update()));

    public static readonly DependencyProperty IsPlayableProperty =
        DependencyProperty.Register(nameof(IsPlayable), typeof(bool), typeof(TrackNumberCell), new PropertyMetadata(true, (d, _) => ((TrackNumberCell)d).Update()));

    private readonly TextBlock _number = new() { HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center };
    private readonly StrokeIcon _play = new() { Filled = true, Size = 14, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center };
    private readonly LevelGlyph _level = new() { HorizontalAlignment = HorizontalAlignment.Right };
    private readonly RowHover _row;

    public TrackNumberCell()
    {
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        _number.Style = (Style)Application.Current.Resources["JfTimeText"];
        _play.Data = Bind.Glyph("IconPlay");
        _play.Foreground = (Brush)Application.Current.Resources["JfInk90"];
        Children.Add(_number);
        Children.Add(_play);
        Children.Add(_level);
        _row = new RowHover(this, Update);
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

    /// <summary>Choosing the row plays it here; a song that cannot keeps its number.</summary>
    public bool IsPlayable
    {
        get => (bool)GetValue(IsPlayableProperty);
        set => SetValue(IsPlayableProperty, value);
    }

    private static PlayerViewModel Player => AppHost.App.Player;

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        Player.PropertyChanged += OnPlayerChanged;
        AppHost.App.PropertyChanged += OnPlayerChanged;
        Update();
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        Player.PropertyChanged -= OnPlayerChanged;
        AppHost.App.PropertyChanged -= OnPlayerChanged;
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
        var hovered = _row is { IsActive: true } && IsPlayable;
        var showLevel = IsCurrent && !hovered;
        _number.Visibility = !IsCurrent && !hovered ? Visibility.Visible : Visibility.Collapsed;
        _play.Visibility = hovered ? Visibility.Visible : Visibility.Collapsed;
        _level.Visibility = showLevel ? Visibility.Visible : Visibility.Collapsed;
        _level.ReduceMotion = AppHost.App.ReduceMotion;
        _level.IsPlaying = showLevel && Player.IsPlaying && IsLoaded;
    }
}

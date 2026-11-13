using System.Numerics;
using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Shapes;

namespace Jellyfesh.App.Controls;

/// <summary>
/// Play and pause morph into each other (docs/design-controls.md, "Now Playing"): the glyph that
/// goes fades and shrinks while the one that comes fades in and springs to full size, dur-2 on the
/// Spring curve, on the compositor. With reduced motion the glyphs swap.
/// </summary>
public sealed partial class PlayPauseGlyph : Grid
{
    public static readonly DependencyProperty IsPlayingProperty =
        DependencyProperty.Register(nameof(IsPlaying), typeof(bool), typeof(PlayPauseGlyph), new PropertyMetadata(false, (d, _) => ((PlayPauseGlyph)d).Morph(animate: true)));

    public static readonly DependencyProperty SizeProperty =
        DependencyProperty.Register(nameof(Size), typeof(double), typeof(PlayPauseGlyph), new PropertyMetadata(22.0, (d, e) => ((PlayPauseGlyph)d).Resize((double)e.NewValue)));

    public static readonly DependencyProperty ReduceMotionProperty =
        DependencyProperty.Register(nameof(ReduceMotion), typeof(bool), typeof(PlayPauseGlyph), new PropertyMetadata(false));

    private readonly StrokeIcon _play = new() { Filled = true };
    private readonly StrokeIcon _pause = new() { Filled = true };

    public PlayPauseGlyph()
    {
        IsHitTestVisible = false;
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        _play.Data = Bind.Glyph("IconPlay");
        _pause.Data = Bind.Glyph("IconPause");
        Children.Add(_play);
        Children.Add(_pause);
        Resize(Size);
        Loaded += (_, _) => Morph(animate: false);
    }

    public bool IsPlaying
    {
        get => (bool)GetValue(IsPlayingProperty);
        set => SetValue(IsPlayingProperty, value);
    }

    public double Size
    {
        get => (double)GetValue(SizeProperty);
        set => SetValue(SizeProperty, value);
    }

    public bool ReduceMotion
    {
        get => (bool)GetValue(ReduceMotionProperty);
        set => SetValue(ReduceMotionProperty, value);
    }

    private void Resize(double size)
    {
        foreach (var icon in new[] { _play, _pause })
        {
            icon.Size = size;
            icon.CenterPoint = new Vector3((float)size / 2, (float)size / 2, 0);
        }

        Width = Height = size;
    }

    private void Morph(bool animate)
    {
        var (shown, hidden) = IsPlaying ? (_pause, _play) : (_play, _pause);
        var duration = animate && !ReduceMotion && IsLoaded ? TokenValues.Duration(2) : TimeSpan.Zero;
        Motion.FadeScale(shown, 1, 1, duration, TokenValues.Spring);
        Motion.FadeScale(hidden, 0, 0.6f, duration, TokenValues.Spring);
    }
}

/// <summary>
/// The playing row's level glyph (after Apple Music): three bars that move only while music plays,
/// on the compositor (a repeating render-transform animation), and stand still when paused, with
/// reduced motion or when the row is not on screen.
/// </summary>
public sealed partial class LevelGlyph : StackPanel
{
    public static readonly DependencyProperty IsPlayingProperty =
        DependencyProperty.Register(nameof(IsPlaying), typeof(bool), typeof(LevelGlyph), new PropertyMetadata(false, (d, _) => ((LevelGlyph)d).Update()));

    public static readonly DependencyProperty ReduceMotionProperty =
        DependencyProperty.Register(nameof(ReduceMotion), typeof(bool), typeof(LevelGlyph), new PropertyMetadata(false, (d, _) => ((LevelGlyph)d).Update()));

    /// <summary>The bars' resting heights and their periods (seconds), so they never move in step.</summary>
    private static readonly (double Height, double Period)[] Bars = [(0.55, 0.42), (0.95, 0.58), (0.7, 0.5)];

    private readonly List<ScaleTransform> _scales = [];
    private Storyboard? _motion;

    public LevelGlyph()
    {
        Orientation = Orientation.Horizontal;
        Spacing = 2;
        Height = 14;
        VerticalAlignment = VerticalAlignment.Center;
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        foreach (var (rest, _) in Bars)
        {
            var scale = new ScaleTransform { ScaleY = rest };
            _scales.Add(scale);
            var bar = new Rectangle
            {
                Width = 3,
                Height = 14,
                RadiusX = 1.5,
                RadiusY = 1.5,
                RenderTransformOrigin = new Windows.Foundation.Point(0.5, 1),
                RenderTransform = scale,
            };
            bar.SetValue(Shape.FillProperty, Application.Current.Resources["JfAccentText"]);
            Children.Add(bar);
        }

        Loaded += (_, _) => Update();
        Unloaded += (_, _) => Stop();
    }

    public bool IsPlaying
    {
        get => (bool)GetValue(IsPlayingProperty);
        set => SetValue(IsPlayingProperty, value);
    }

    public bool ReduceMotion
    {
        get => (bool)GetValue(ReduceMotionProperty);
        set => SetValue(ReduceMotionProperty, value);
    }

    private void Update()
    {
        if (IsPlaying && !ReduceMotion && IsLoaded && Visibility == Visibility.Visible)
        {
            Start();
        }
        else
        {
            Stop();
        }
    }

    private void Start()
    {
        if (_motion is not null)
        {
            return;
        }

        var storyboard = new Storyboard { RepeatBehavior = RepeatBehavior.Forever };
        for (var i = 0; i < _scales.Count; i++)
        {
            var (rest, period) = Bars[i];
            var animation = new DoubleAnimation
            {
                From = 0.25,
                To = rest,
                Duration = TimeSpan.FromSeconds(period),
                AutoReverse = true,
                RepeatBehavior = RepeatBehavior.Forever,
                EasingFunction = new SineEase { EasingMode = EasingMode.EaseInOut },
            };
            Storyboard.SetTarget(animation, _scales[i]);
            Storyboard.SetTargetProperty(animation, "ScaleY");
            storyboard.Children.Add(animation);
        }

        _motion = storyboard;
        storyboard.Begin();
    }

    private void Stop()
    {
        _motion?.Stop();
        _motion = null;
        for (var i = 0; i < _scales.Count; i++)
        {
            _scales[i].ScaleY = Bars[i].Height;
        }
    }
}

using Jellyfesh.App.Theming;
using Jellyfesh.Drawing;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.Foundation;
using Windows.System;

namespace Jellyfesh.App.Controls;

/// <summary>
/// The seek bar of v0.0.4 (docs/design-controls.md): a straight line with round caps, the played
/// part in ink-90 and the rest in ink-30, no thumb at rest. Hovering or focusing thickens it to
/// 6 px and shows a 14 px ringed thumb; dragging thickens it to 8 px (touch: 6 px, 12 px while
/// dragging, never a thumb). Thickness and thumb change over dur-1 Fluid; progress itself is set
/// from the drawing-time presenter at visible steps only, so nothing loops. Geometry comes from
/// <see cref="SeekGeometry"/>. A RangeBase, so Narrator reads it as a slider whose value is
/// "1:23 of 3:45"; Left/Right seek 5 s, Page Up/Page Down 30 s, Home/End the ends. Keyboard focus
/// shows as hover does, thickened with its thumb, never with the system's focus rectangle (docs/ui.md,
/// v0.0.9); so do keys pressed while a click has focused it. A click alone shows nothing once the
/// pointer leaves.
/// </summary>
public sealed partial class SeekBar : RangeBase
{
    public static readonly DependencyProperty ReduceMotionProperty =
        DependencyProperty.Register(nameof(ReduceMotion), typeof(bool), typeof(SeekBar), new PropertyMetadata(false));

    public static readonly DependencyProperty ValueTextProperty =
        DependencyProperty.Register(nameof(ValueText), typeof(string), typeof(SeekBar), new PropertyMetadata(string.Empty, (d, e) =>
            AutomationProperties.SetItemStatus(d, (string)e.NewValue)));

    private FrameworkElement? _root;
    private FrameworkElement? _track;
    private FrameworkElement? _played;
    private FrameworkElement? _thumb;
    private SeekBarState _state = SeekBarState.Rest;
    private bool _hovered;
    private bool _touch;
    private bool _dragging;

    /// <summary>Keys moved it since it was focused: it shows its focus as keyboard focus does.</summary>
    private bool _keyed;
    private Storyboard? _thickness;

    public SeekBar()
    {
        DefaultStyleKey = typeof(SeekBar);
        IsTabStop = true;
        UseSystemFocusVisuals = false;
        SmallChange = SeekGeometry.SmallStep;
        LargeChange = SeekGeometry.LargeStep;
        GotFocus += (_, _) => UpdateState();
        LostFocus += (_, _) =>
        {
            _keyed = false;
            UpdateState();
        };
    }

    /// <summary>Raised when the person releases the thumb or uses the keyboard: seek to this value.</summary>
    public event EventHandler<double>? SeekRequested;

    /// <summary>Raised while dragging, so the times can follow the thumb.</summary>
    public event EventHandler<double>? ScrubMoved;

    /// <summary>Raised when the drawn length of the line changes: the presenter's steps (device pixels).</summary>
    public event EventHandler<int>? StepsChanged;

    public bool ReduceMotion
    {
        get => (bool)GetValue(ReduceMotionProperty);
        set => SetValue(ReduceMotionProperty, value);
    }

    /// <summary>"1:24 of 3:52", read by Narrator.</summary>
    public string ValueText
    {
        get => (string)GetValue(ValueTextProperty);
        set => SetValue(ValueTextProperty, value);
    }

    public bool IsDragging => _dragging;

    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        if (_root is not null)
        {
            _root.SizeChanged -= OnRootSizeChanged;
        }

        _root = GetTemplateChild("Root") as FrameworkElement;
        _track = GetTemplateChild("Track") as FrameworkElement;
        _played = GetTemplateChild("Played") as FrameworkElement;
        _thumb = GetTemplateChild("Thumb") as FrameworkElement;
        if (_root is not null)
        {
            _root.SizeChanged += OnRootSizeChanged;
        }

        if (_thumb is not null)
        {
            var half = (float)(SeekGeometry.Thumb / 2);
            _thumb.CenterPoint = new System.Numerics.Vector3(half, half, 0);
            _thumb.Opacity = 0;
            _thumb.Scale = new System.Numerics.Vector3(0.6f, 0.6f, 1);
        }

        ApplyState(animate: false);
        UpdateVisuals();
    }

    protected override void OnValueChanged(double oldValue, double newValue)
    {
        base.OnValueChanged(oldValue, newValue);
        UpdateVisuals();
    }

    protected override void OnMaximumChanged(double oldMaximum, double newMaximum)
    {
        base.OnMaximumChanged(oldMaximum, newMaximum);
        UpdateVisuals();
    }

    protected override AutomationPeer OnCreateAutomationPeer() => new SeekBarAutomationPeer(this);

    protected override void OnPointerEntered(PointerRoutedEventArgs e)
    {
        base.OnPointerEntered(e);
        _touch = e.Pointer.PointerDeviceType == PointerDeviceType.Touch;
        _hovered = !_touch;
        UpdateState();
    }

    protected override void OnPointerExited(PointerRoutedEventArgs e)
    {
        base.OnPointerExited(e);
        _hovered = false;
        UpdateState();
    }

    protected override void OnPointerPressed(PointerRoutedEventArgs e)
    {
        base.OnPointerPressed(e);
        if (Maximum <= Minimum || !CapturePointer(e.Pointer))
        {
            return;
        }

        _touch = e.Pointer.PointerDeviceType == PointerDeviceType.Touch;
        _dragging = true;
        _keyed = false;
        Focus(FocusState.Pointer);
        UpdateState();
        MoveTo(e.GetCurrentPoint(this).Position);
        e.Handled = true;
    }

    protected override void OnPointerMoved(PointerRoutedEventArgs e)
    {
        base.OnPointerMoved(e);
        if (_dragging)
        {
            MoveTo(e.GetCurrentPoint(this).Position);
            e.Handled = true;
        }
    }

    protected override void OnPointerReleased(PointerRoutedEventArgs e)
    {
        base.OnPointerReleased(e);
        if (_dragging)
        {
            ReleasePointerCapture(e.Pointer);
            EndDrag();
            e.Handled = true;
        }
    }

    protected override void OnPointerCaptureLost(PointerRoutedEventArgs e)
    {
        base.OnPointerCaptureLost(e);
        EndDrag();
    }

    protected override void OnKeyDown(KeyRoutedEventArgs e)
    {
        SeekKey? key = e.Key switch
        {
            VirtualKey.Left or VirtualKey.Down => SeekKey.Back,
            VirtualKey.Right or VirtualKey.Up => SeekKey.Forward,
            VirtualKey.PageDown => SeekKey.PageBack,
            VirtualKey.PageUp => SeekKey.PageForward,
            VirtualKey.Home => SeekKey.Start,
            VirtualKey.End => SeekKey.End,
            _ => null,
        };
        if (key is { } k && Maximum > Minimum)
        {
            var target = Minimum + SeekGeometry.KeyTarget(k, Value - Minimum, Maximum - Minimum);
            Value = target;
            SeekRequested?.Invoke(this, target);
            e.Handled = true;
            if (!_keyed)
            {
                _keyed = true;
                UpdateState();
            }
        }

        base.OnKeyDown(e);
    }

    private void EndDrag()
    {
        if (!_dragging)
        {
            return;
        }

        _dragging = false;
        UpdateState();
        SeekRequested?.Invoke(this, Value);
    }

    private void MoveTo(Point p)
    {
        var fraction = SeekGeometry.FractionAt(p.X, ActualWidth, _touch, FlowDirection == FlowDirection.RightToLeft);
        Value = Minimum + (fraction * (Maximum - Minimum));
        ScrubMoved?.Invoke(this, Value);
    }

    private void OnRootSizeChanged(object sender, SizeChangedEventArgs e)
    {
        UpdateVisuals();
        StepsChanged?.Invoke(this, SeekGeometry.Steps(e.NewSize.Width, XamlRoot?.RasterizationScale ?? 1, _touch));
    }

    private void UpdateState()
    {
        var focused = FocusState is FocusState.Keyboard or FocusState.Programmatic || (_keyed && FocusState != FocusState.Unfocused);
        var state = _dragging ? SeekBarState.Drag : _hovered || focused ? SeekBarState.Hover : SeekBarState.Rest;
        if (state != _state)
        {
            _state = state;
            ApplyState(animate: !ReduceMotion);
        }
    }

    /// <summary>The line's thickness and the thumb for the current state, over dur-1 Fluid (Misty MOT-06).</summary>
    private void ApplyState(bool animate)
    {
        if (_track is null || _played is null)
        {
            return;
        }

        var thickness = SeekGeometry.Thickness(_state, _touch);
        var duration = animate ? TokenValues.Duration(1) : TimeSpan.Zero;
        _thickness?.Stop();
        if (animate)
        {
            // Height is a layout property: a short dependent animation, only when the state changes.
            var storyboard = new Storyboard();
            foreach (var part in new[] { _track, _played })
            {
                var animation = new DoubleAnimationUsingKeyFrames { EnableDependentAnimation = true };
                animation.KeyFrames.Add(Motion.Key(thickness, duration, TokenValues.Fluid));
                Storyboard.SetTarget(animation, part);
                Storyboard.SetTargetProperty(animation, "Height");
                storyboard.Children.Add(animation);
            }

            _thickness = storyboard;
            storyboard.Begin();
        }
        else
        {
            _track.Height = _played.Height = thickness;
        }

        if (_thumb is not null)
        {
            var show = SeekGeometry.ShowsThumb(_state, _touch) && Maximum > Minimum;
            Motion.FadeScale(_thumb, show ? 1 : 0, show ? 1 : 0.6f, duration, TokenValues.Fluid);
        }
    }

    private void UpdateVisuals()
    {
        if (_root is null || _played is null)
        {
            return;
        }

        var width = _root.ActualWidth;
        var fraction = Maximum > Minimum ? (Value - Minimum) / (Maximum - Minimum) : 0;
        var inset = SeekGeometry.Inset(_touch);
        _played.Margin = new Thickness(inset, 0, 0, 0);
        _played.Width = SeekGeometry.PlayedLength(fraction, width, _touch);
        if (_track is not null)
        {
            _track.Margin = new Thickness(inset, 0, inset, 0);
        }

        if (_thumb is not null)
        {
            var x = SeekGeometry.ThumbCentre(fraction, width, _touch, rightToLeft: false) - (SeekGeometry.Thumb / 2);
            _thumb.Translation = new System.Numerics.Vector3((float)x, 0, 0);
        }
    }

    /// <summary>A slider whose value Narrator reads as "1:23 of 3:45" (the Value pattern beside RangeValue).</summary>
    private sealed partial class SeekBarAutomationPeer(SeekBar owner) : RangeBaseAutomationPeer(owner), IValueProvider
    {
        bool IValueProvider.IsReadOnly => true;

        string IValueProvider.Value => owner.ValueText;

        void IValueProvider.SetValue(string value)
        {
            // Read only: the RangeValue pattern sets the position.
        }

        protected override string GetClassNameCore() => nameof(SeekBar);

        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Slider;

        protected override object? GetPatternCore(PatternInterface patternInterface) =>
            patternInterface == PatternInterface.Value ? this : base.GetPatternCore(patternInterface);
    }
}

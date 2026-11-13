using System.ComponentModel;
using System.Numerics;
using Jellyfesh.App.Controls;
using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.ViewModels;
using Microsoft.UI.Composition;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Jellyfesh.App.Views;

public sealed partial class LyricsView : UserControl
{
    /// <summary>A hand scroll pauses following the active line for this long (lyrics model).</summary>
    private static readonly TimeSpan FollowPause = TimeSpan.FromSeconds(4);

    private readonly AppViewModel _app;
    private readonly DispatcherQueueTimer _resume;
    private DateTime _pausedUntil;
    private double _lineSize = 30;
    private double _widthSize = 30;

    public LyricsView()
    {
        _app = AppHost.App;
        Presenter = _app.Lyrics;
        InitializeComponent();
        Presenter.PropertyChanged += OnPresenterChanged;
        Presenter.SweepChanged += OnSweepChanged;
        _app.PropertyChanged += OnAppChanged;
        _resume = DispatcherQueue.GetForCurrentThread().CreateTimer();
        _resume.IsRepeating = false;
        _resume.Tick += (_, _) =>
        {
            _resume.Stop();
            Follow(animate: !_app.ReduceMotion);
        };
        Scroller.AddHandler(PointerWheelChangedEvent, new PointerEventHandler((_, _) => PauseFollow()), handledEventsToo: true);
    }

    public LyricsPresenter Presenter { get; }

    /// <summary>The lyrics came into view: jump to the line being sung once layout has run.</summary>
    public void Refresh()
    {
        _pausedUntil = default;
        DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () => Follow(animate: false));
    }

    private void OnPresenterChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(LyricsPresenter.ScrollIndex):
                Follow(animate: !_app.ReduceMotion);
                break;
            case nameof(LyricsPresenter.HasLyrics):
                _pausedUntil = default;
                Scroller.ScrollTo(0, 0, new ScrollingScrollOptions(ScrollingAnimationMode.Disabled));
                break;
        }
    }

    private void OnAppChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(AppViewModel.ReduceMotion))
        {
            ForEachLine(view => view.ReduceMotion = _app.ReduceMotion);
        }
        else if (e.PropertyName == nameof(AppViewModel.Settings))
        {
            ApplyLyricsSettings();
        }
    }

    /// <summary>Brings the active line to 38 % of the viewport, unless the person scrolled away a moment ago.</summary>
    private void Follow(bool animate)
    {
        var index = Presenter.ScrollIndex;
        if (index < 0 || index >= Presenter.Lines.Count || !Presenter.IsActive || !Lines.IsLoaded || XamlRoot is null)
        {
            return; // not on screen: Refresh catches up when it is
        }

        var wait = _pausedUntil - DateTime.UtcNow;
        if (wait > TimeSpan.Zero)
        {
            _resume.Interval = wait;
            _resume.Start();
            return;
        }

        try
        {
            if (Lines.GetOrCreateElement(index) is FrameworkElement element)
            {
                element.UpdateLayout();
                element.StartBringIntoView(new BringIntoViewOptions
                {
                    VerticalAlignmentRatio = TokenValues.Number("Lyrics.AnchorPosition"),
                    AnimationDesired = animate,
                });
            }
        }
        catch (Exception e) when (e is System.Runtime.InteropServices.COMException or ArgumentException or InvalidOperationException)
        {
            // The repeater was between layouts; the next line change scrolls again.
        }
    }

    private void PauseFollow()
    {
        _pausedUntil = DateTime.UtcNow + FollowPause;
    }

    private void OnScrollStateChanged(ScrollView sender, object args)
    {
        if (sender.State == ScrollingInteractionState.Interaction)
        {
            PauseFollow();
        }
    }

    /// <summary>The line advance runs 380 ms on the Fluid curve (lyrics tokens), not the system's default.</summary>
    private void OnScrollAnimationStarting(ScrollView sender, ScrollingScrollAnimationStartingEventArgs args)
    {
        if (args.Animation is not Vector3KeyFrameAnimation animation)
        {
            return;
        }

        var fluid = TokenValues.Fluid;
        var easing = animation.Compositor.CreateCubicBezierEasingFunction(
            new Vector2((float)fluid.X1, (float)fluid.Y1),
            new Vector2((float)fluid.X2, (float)fluid.Y2));
        animation.InsertKeyFrame(1f, new Vector3((float)args.EndPosition.X, (float)args.EndPosition.Y, 0f), easing);
        animation.Duration = TokenValues.Duration(3);
        Cascade(args.EndPosition.Y - args.StartPosition.Y);
    }

    /// <summary>
    /// The lines below the new active line follow the scroll one after another, each starting
    /// lyrics.stagger (30 ms) after the one above, so they settle in a cascade (Apple Music). Each
    /// gets a compositor keyframe animation of its Translation; nothing runs per frame here.
    /// </summary>
    private void Cascade(double distance)
    {
        var active = Presenter.ActiveIndex;
        if (distance <= 0 || active < 0 || _app.ReduceMotion)
        {
            return;
        }

        var duration = TokenValues.Duration(3);
        var last = Math.Min(Presenter.Lines.Count - 1, active + NowPlayingMotion.MaxStaggeredLines + 4);
        for (var i = active + 1; i <= last; i++)
        {
            if (Lines.TryGetElement(i) is UIElement element)
            {
                var delay = NowPlayingMotion.CascadeDelay(i, active);
                Motion.Lag(element, NowPlayingMotion.CascadeLag(distance, delay, duration, TokenValues.Fluid), duration + delay);
            }
        }
    }

    private void OnScrollerSizeChanged(object sender, SizeChangedEventArgs e)
    {
        // Room above and below so the first and last lines can reach the anchor.
        var anchor = TokenValues.Number("Lyrics.AnchorPosition");
        Lines.Margin = new Thickness(0, e.NewSize.Height * anchor, 0, e.NewSize.Height * (1 - anchor));

        // The line size follows the width, 24 to 40 (lyrics tokens: 30 at a typical width), times
        // the Lyrics size setting (lyrics.scale-small, 1, lyrics.scale-large).
        var min = TokenValues.Number("Lyrics.LineMin");
        var max = TokenValues.Number("Lyrics.LineMax");
        _widthSize = Math.Clamp(min + ((e.NewSize.Width - 400) * (max - min) / 800), min, max);
        ApplyLyricsSettings();
    }

    /// <summary>The Lyrics size and alignment settings, applied live.</summary>
    private void ApplyLyricsSettings()
    {
        _lineSize = Math.Round(_widthSize * NowPlayingMotion.LyricsScale(_app.Settings.LyricsSize));
        var align = _app.Settings.LyricsAlign;
        ForEachLine(view =>
        {
            view.LineFontSize = _lineSize;
            view.Align = align;
        });
    }

    private void OnElementPrepared(ItemsRepeater sender, ItemsRepeaterElementPreparedEventArgs args)
    {
        if (args.Element is not LyricLineView view)
        {
            return;
        }

        view.LineFontSize = _lineSize;
        view.ReduceMotion = _app.ReduceMotion;
        view.Align = _app.Settings.LyricsAlign;
        view.SeekRequested += OnLineSeekRequested;
        if (args.Index == Presenter.ActiveIndex && Presenter.Sweep is { } sweep)
        {
            view.ApplySweep(sweep);
        }
    }

    private void OnElementClearing(ItemsRepeater sender, ItemsRepeaterElementClearingEventArgs args)
    {
        if (args.Element is LyricLineView view)
        {
            view.SeekRequested -= OnLineSeekRequested;
            view.ApplySweep(null);
        }
    }

    private void OnSweepChanged(SweepState? sweep)
    {
        var index = sweep?.Line ?? Presenter.ActiveIndex;
        if (index >= 0 && Lines.TryGetElement(index) is LyricLineView view)
        {
            view.ApplySweep(sweep);
        }
    }

    private void OnLineSeekRequested(object? sender, uint startMs) => _app.Player.SeekToUs(startMs * 1_000L);

    private void ForEachLine(Action<LyricLineView> action)
    {
        for (var i = 0; i < Presenter.Lines.Count; i++)
        {
            if (Lines.TryGetElement(i) is LyricLineView view)
            {
                action(view);
            }
        }
    }
}

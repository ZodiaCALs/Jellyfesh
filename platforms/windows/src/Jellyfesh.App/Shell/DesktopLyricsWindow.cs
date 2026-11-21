using Jellyfesh.App.Controls;
using Jellyfesh.App.Platform;
using Jellyfesh.Drawing;
using Jellyfesh.Formatting;
using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using WinRT.Interop;

namespace Jellyfesh.App.Shell;

/// <summary>
/// Desktop Lyrics (docs/ui.md, "Discover, details, comments and the lyrics bar"): the line being
/// sung and the next one in a small window of its own that stays above other apps, on acrylic,
/// at the bottom of the screen. It is dragged anywhere on its glass; Lock lets clicks pass through
/// to what lies beneath (a layered window with WS_EX_TRANSPARENT), unlocked again from the
/// player's More menu or by turning Desktop Lyrics off; its close button turns the setting off.
/// What it shows is the lyrics bar's (LyricsBarPresenter); it draws only while open.
/// </summary>
internal sealed partial class DesktopLyricsWindow : Window
{
    private readonly LyricsBarPresenter _bar;
    private readonly LyricsBarLine _current = new() { LineFontSize = 30 };
    private readonly LyricsBarLine _next = new() { LineFontSize = 20, Dim = true };
    private readonly StackPanel _tools = new() { Orientation = Orientation.Horizontal, Spacing = 4, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Top, Opacity = 0 };
    private readonly nint _hwnd;

    /// <summary>The colour the current line's words light up in (the look's, or the accent).</summary>
    public Brush? Highlight
    {
        get => _current.Highlight;
        set => _current.Highlight = value;
    }

    public DesktopLyricsWindow(LyricsBarPresenter bar, ILocalizer l, Action close)
    {
        _bar = bar;
        Title = l.Get("DesktopLyrics_Title");
        _hwnd = WindowNative.GetWindowHandle(this);

        var lines = new StackPanel { Spacing = 6, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(24, 12, 24, 12) };
        lines.Children.Add(_current);
        lines.Children.Add(_next);
        var lockButton = Tool("", l.Get("DesktopLyrics_Lock"), () => Lock(true));
        var closeButton = Tool("", l.Get("DesktopLyrics_Close"), close);
        _tools.Children.Add(lockButton);
        _tools.Children.Add(closeButton);
        var root = new Grid { Background = new SolidColorBrush(Colors.Transparent) };
        root.Children.Add(lines);
        root.Children.Add(_tools);
        AutomationProperties.SetName(root, Title);

        // The tools show while the pointer is over the window; the glass drags it.
        root.PointerEntered += (_, _) => _tools.Opacity = 1;
        root.PointerExited += (_, _) => _tools.Opacity = 0;
        root.PointerPressed += OnPointerPressed;
        Content = root;
        SystemBackdrop = new DesktopAcrylicBackdrop();

        var presenter = OverlappedPresenter.CreateForToolWindow();
        presenter.IsAlwaysOnTop = true;
        presenter.IsMaximizable = false;
        presenter.IsMinimizable = false;
        presenter.SetBorderAndTitleBar(false, false);
        AppWindow.SetPresenter(presenter);
        AppWindow.IsShownInSwitchers = false;
        Place();

        _bar.PropertyChanged += OnBarChanged;
        _bar.SweepChanged += OnSweep;
        _bar.Show(true);
        ShowLines();
        Closed += (_, _) =>
        {
            _bar.PropertyChanged -= OnBarChanged;
            _bar.SweepChanged -= OnSweep;
            _bar.Show(false);
        };
    }

    /// <summary>Clicks pass through to the apps beneath while locked.</summary>
    public bool IsLocked { get; private set; }

    /// <summary>Raised when locked or unlocked (the More menu offers Unlock while locked).</summary>
    public event Action? LockChanged;

    /// <summary>Locks (clicks pass through) or unlocks the window.</summary>
    public void Lock(bool locked)
    {
        var style = Win32.GetWindowLongPtr(_hwnd, Win32.GWL_EXSTYLE);
        if (locked)
        {
            Win32.SetWindowLongPtr(_hwnd, Win32.GWL_EXSTYLE, style | Win32.WS_EX_LAYERED | Win32.WS_EX_TRANSPARENT);
            Win32.SetLayeredWindowAttributes(_hwnd, 0, 255, Win32.LWA_ALPHA); // fully drawn, only hit testing changes
            _tools.Opacity = 0;
        }
        else
        {
            Win32.SetWindowLongPtr(_hwnd, Win32.GWL_EXSTYLE, style & ~(Win32.WS_EX_LAYERED | Win32.WS_EX_TRANSPARENT));
        }

        IsLocked = locked;
        LockChanged?.Invoke();
    }

    /// <summary>The bar's lines now, then as they change.</summary>
    private void ShowLines()
    {
        _current.Line = _bar.Line;
        _next.Line = _bar.Next;
        _current.ApplySweep(_bar.Sweep);
    }

    private void OnBarChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(LyricsBarPresenter.Line) or nameof(LyricsBarPresenter.Next))
        {
            ShowLines();
        }
    }

    private void OnSweep(SweepState? sweep) => _current.ApplySweep(sweep);

    /// <summary>Bottom centre of the screen the main window is on, above the taskbar.</summary>
    private void Place()
    {
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var scale = Win32.GetDpiForWindow(_hwnd) / 96.0;
        var width = (int)Math.Min(area.Width * 0.8, 900 * scale);
        var height = (int)(150 * scale);
        AppWindow.MoveAndResize(new RectInt32(area.X + ((area.Width - width) / 2), area.Y + area.Height - height - (int)(48 * scale), width, height));
    }

    /// <summary>Drags the window by its glass: a window without a title bar asks Windows to move it.</summary>
    private void OnPointerPressed(object sender, PointerRoutedEventArgs e)
    {
        if (e.OriginalSource is DependencyObject source && IsInside(source, _tools))
        {
            return;
        }

        Win32.ReleaseCapture();
        Win32.SendMessage(_hwnd, Win32.WM_NCLBUTTONDOWN, Win32.HTCAPTION, 0);
    }

    private static bool IsInside(DependencyObject element, DependencyObject container)
    {
        for (var e = element; e is not null; e = VisualTreeHelper.GetParent(e))
        {
            if (ReferenceEquals(e, container))
            {
                return true;
            }
        }

        return false;
    }

    private static Button Tool(string glyph, string name, Action click)
    {
        var button = new Button { Content = new FontIcon { Glyph = glyph, FontSize = 14 }, Padding = new Thickness(8) };
        if (Application.Current.Resources.TryGetValue("JfIconButtonStyle", out var style) && style is Style s)
        {
            button.Style = s;
        }

        AutomationProperties.SetName(button, name);
        ToolTipService.SetToolTip(button, name);
        button.Click += (_, _) => click();
        return button;
    }
}

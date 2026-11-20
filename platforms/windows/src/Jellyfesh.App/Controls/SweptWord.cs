using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.Foundation;

namespace Jellyfesh.App.Controls;

/// <summary>
/// A word that fills as it is sung (Now Playing's lines and the lyrics bar): an unsung copy and a
/// sung copy whose clip slides open, swept by a linear compositor animation over the time the word
/// has left (Misty MOT-06), so no app code runs per frame.
/// </summary>
internal sealed class SweptWord
{
    private readonly TextBlock _base;
    private readonly TextBlock _lit;
    private readonly TranslateTransform _slide = new();
    private readonly RectangleGeometry _clip;
    private readonly bool _rtl;
    private double _progress;

    public SweptWord(string text, Brush unsung, Brush sung, bool rtl)
    {
        _rtl = rtl;
        _base = new TextBlock { Text = text, Foreground = unsung, FontWeight = FontWeights.Bold, LineStackingStrategy = LineStackingStrategy.BlockLineHeight };
        _lit = new TextBlock { Text = text, Foreground = sung, FontWeight = FontWeights.Bold, LineStackingStrategy = LineStackingStrategy.BlockLineHeight };
        _clip = new RectangleGeometry { Transform = _slide };
        _lit.Clip = _clip;
        Root = new Grid { Children = { _base, _lit } };
        AutomationProperties.SetAccessibilityView(Root, AccessibilityView.Raw);
        _lit.SizeChanged += (_, e) =>
        {
            _clip.Rect = new Rect(0, 0, e.NewSize.Width, e.NewSize.Height);
            Show(_progress);
        };
    }

    public Grid Root { get; }

    public void Apply(double size, double lineHeight)
    {
        _base.FontSize = _lit.FontSize = size;
        _base.LineHeight = _lit.LineHeight = size * lineHeight;
    }

    /// <summary>Shows the word sung up to <paramref name="progress"/> (0 to 1), without animation.</summary>
    public void Show(double progress)
    {
        _progress = Math.Clamp(progress, 0, 1);
        _slide.X = Offset(_progress);
    }

    /// <summary>A linear sweep from the current progress to the end over <paramref name="remaining"/>.</summary>
    public Storyboard? Animate(TimeSpan remaining)
    {
        var width = _lit.ActualWidth;
        if (width <= 0)
        {
            return null;
        }

        var animation = new DoubleAnimation
        {
            From = Offset(_progress),
            To = 0,
            Duration = new Duration(remaining),
            EnableDependentAnimation = false,
        };
        Storyboard.SetTarget(animation, _slide);
        Storyboard.SetTargetProperty(animation, "X");
        var storyboard = new Storyboard();
        storyboard.Children.Add(animation);
        storyboard.Completed += (_, _) => Show(1);
        return storyboard;
    }

    private double Offset(double progress)
    {
        var hidden = (1 - progress) * _lit.ActualWidth;
        return _rtl ? hidden : -hidden;
    }
}

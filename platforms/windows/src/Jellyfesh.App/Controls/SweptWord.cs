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
/// has left (Misty MOT-06), so no app code runs per frame. The look's glow (ADR-0020) sits behind
/// the sung copy, inside the same clip, so it follows the sweep; the clip leaves it room.
/// </summary>
internal sealed class SweptWord
{
    private readonly TextBlock _base;
    private readonly TextBlock _lit;
    private readonly TranslateTransform _slide = new();
    private readonly RectangleGeometry _clip;
    private readonly TextGlow _glow;
    private readonly bool _rtl;
    private double _progress;
    private double _pad;

    public SweptWord(string text, Brush unsung, Brush sung, bool rtl)
    {
        _rtl = rtl;
        _base = new TextBlock { Text = text, Foreground = unsung, FontWeight = FontWeights.Bold, LineStackingStrategy = LineStackingStrategy.BlockLineHeight };
        _lit = new TextBlock { Text = text, Foreground = sung, FontWeight = FontWeights.Bold, LineStackingStrategy = LineStackingStrategy.BlockLineHeight };
        var glowHost = new Canvas();
        var litLayer = new Grid { Children = { glowHost, _lit } };
        _clip = new RectangleGeometry { Transform = _slide };
        litLayer.Clip = _clip;
        _glow = new TextGlow(_lit, glowHost);
        Root = new Grid { Children = { _base, litLayer } };
        AutomationProperties.SetAccessibilityView(Root, AccessibilityView.Raw);
        _lit.SizeChanged += (_, _) =>
        {
            UpdateClip();
            Show(_progress);
        };
    }

    public Grid Root { get; }

    public void Apply(double size, double lineHeight)
    {
        _base.FontSize = _lit.FontSize = size;
        _base.LineHeight = _lit.LineHeight = size * lineHeight;
    }

    /// <summary>The look's weight for both copies.</summary>
    public void SetWeight(Windows.UI.Text.FontWeight weight) => _base.FontWeight = _lit.FontWeight = weight;

    /// <summary>The colour the word lights up in.</summary>
    public void SetSung(Brush sung) => _lit.Foreground = sung;

    /// <summary>The glow behind the sung copy (0 for none); the clip grows by its radius so it is not cut off.</summary>
    public void SetGlow(Windows.UI.Color color, double amount)
    {
        var pad = amount > 0 ? LyricsLook.GlowRadius * amount : 0;
        _glow.Set(color, amount);
        if (pad != _pad)
        {
            _pad = pad;
            UpdateClip();
            Show(_progress);
        }
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

    /// <summary>The sung copy's box, grown by the glow's radius on every side.</summary>
    private void UpdateClip() => _clip.Rect = new Rect(-_pad, -_pad, _lit.ActualWidth + (2 * _pad), _lit.ActualHeight + (2 * _pad));

    /// <summary>How far the clip is slid away at <paramref name="progress"/>: all of it (word and glow room) at 0, none at 1.</summary>
    private double Offset(double progress)
    {
        var hidden = (1 - progress) * (_lit.ActualWidth + (2 * _pad));
        return _rtl ? hidden : -hidden;
    }
}

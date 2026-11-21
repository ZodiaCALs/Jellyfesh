using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Windows.UI;

namespace Jellyfesh.App.Controls;

/// <summary>
/// A soft glow behind a TextBlock in one colour: a composition DropShadow shaped by the text's
/// alpha mask, on a sprite in a host element behind the text, sized with the host (the drop
/// shadow recipe of "Using the Visual Layer with XAML" for the Windows App SDK). Nothing is
/// created while the glow is off.
/// </summary>
internal sealed class TextGlow(TextBlock text, UIElement host)
{
    private SpriteVisual? _sprite;
    private DropShadow? _shadow;

    /// <summary>Shows the glow at <paramref name="amount"/> (0–1; 0 removes it).</summary>
    public void Set(Color color, double amount)
    {
        if (amount <= 0)
        {
            Clear();
            return;
        }

        if (_sprite is null || _shadow is null)
        {
            var hostVisual = ElementCompositionPreview.GetElementVisual(host);
            var compositor = hostVisual.Compositor;
            _shadow = compositor.CreateDropShadow();
            _shadow.Offset = Vector3.Zero;
            _shadow.Mask = text.GetAlphaMask();
            _sprite = compositor.CreateSpriteVisual();
            _sprite.Shadow = _shadow;
            ElementCompositionPreview.SetElementChildVisual(host, _sprite);
            var size = compositor.CreateExpressionAnimation("hostVisual.Size");
            size.SetReferenceParameter("hostVisual", hostVisual);
            _sprite.StartAnimation("Size", size);
        }

        _shadow.Color = color;
        _shadow.BlurRadius = (float)(LyricsLook.GlowRadius * amount);
        _shadow.Opacity = (float)Math.Clamp(amount, 0, 1);
    }

    public void Clear()
    {
        if (_sprite is null)
        {
            return;
        }

        ElementCompositionPreview.SetElementChildVisual(host, null);
        _sprite.Dispose();
        _shadow?.Dispose();
        _sprite = null;
        _shadow = null;
    }
}

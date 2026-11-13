using System.Numerics;
using Jellyfesh.Drawing;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.Foundation;

namespace Jellyfesh.App.Controls;

/// <summary>
/// Misty's motion on the compositor (docs/design-controls.md, "Motion"): animations of an element's
/// Opacity, Scale and Translation (UIElement.StartAnimation) with the token curves, so no app code
/// runs per frame. A zero duration (reduced motion) sets the value at once.
/// </summary>
internal static class Motion
{
    private static Compositor Compositor => CompositionTarget.GetCompositorForCurrentThread();

    public static CompositionEasingFunction Easing(CubicBezierCurve curve) =>
        Compositor.CreateCubicBezierEasingFunction(new Vector2((float)curve.X1, (float)curve.Y1), new Vector2((float)curve.X2, (float)curve.Y2));

    /// <summary>Animates the element's Opacity.</summary>
    public static void Fade(UIElement element, float to, TimeSpan duration, CubicBezierCurve curve, TimeSpan delay = default)
    {
        if (duration <= TimeSpan.Zero)
        {
            element.Opacity = to;
            return;
        }

        var animation = Compositor.CreateScalarKeyFrameAnimation();
        animation.Target = "Opacity";
        animation.InsertKeyFrame(1f, to, Easing(curve));
        Run(element, animation, duration, delay);
    }

    /// <summary>Animates the element's uniform Scale about its CenterPoint.</summary>
    public static void Scale(UIElement element, float to, TimeSpan duration, CubicBezierCurve curve, TimeSpan delay = default)
    {
        if (duration <= TimeSpan.Zero)
        {
            element.Scale = new Vector3(to, to, 1);
            return;
        }

        var animation = Compositor.CreateVector3KeyFrameAnimation();
        animation.Target = "Scale";
        animation.InsertKeyFrame(1f, new Vector3(to, to, 1), Easing(curve));
        Run(element, animation, duration, delay);
    }

    /// <summary>Animates the element's Translation.</summary>
    public static void Translate(UIElement element, Vector3 to, TimeSpan duration, CubicBezierCurve curve, TimeSpan delay = default)
    {
        if (duration <= TimeSpan.Zero)
        {
            element.Translation = to;
            return;
        }

        var animation = Compositor.CreateVector3KeyFrameAnimation();
        animation.Target = "Translation";
        animation.InsertKeyFrame(1f, to, Easing(curve));
        Run(element, animation, duration, delay);
    }

    /// <summary>Fades and scales together (a thumb appearing, a glyph morphing).</summary>
    public static void FadeScale(UIElement element, float opacity, float scale, TimeSpan duration, CubicBezierCurve curve)
    {
        Fade(element, opacity, duration, curve);
        Scale(element, scale, duration, curve);
    }

    /// <summary>
    /// Enters from <paramref name="from"/> (offset and opacity 0) to rest: pages rising 8 px, sheets
    /// and the play glass appearing. With a zero duration it simply shows.
    /// </summary>
    public static void Enter(UIElement element, Vector3 from, TimeSpan duration, CubicBezierCurve curve, TimeSpan delay = default)
    {
        if (duration <= TimeSpan.Zero)
        {
            element.Opacity = 1;
            element.Translation = Vector3.Zero;
            return;
        }

        var fade = Compositor.CreateScalarKeyFrameAnimation();
        fade.Target = "Opacity";
        fade.InsertKeyFrame(0f, 0f);
        fade.InsertKeyFrame(1f, 1f, Easing(curve));
        Run(element, fade, duration, delay);
        var move = Compositor.CreateVector3KeyFrameAnimation();
        move.Target = "Translation";
        move.InsertKeyFrame(0f, from);
        move.InsertKeyFrame(1f, Vector3.Zero, Easing(curve));
        Run(element, move, duration, delay);
    }

    /// <summary>
    /// The lyrics cascade (NowPlayingMotion.CascadeLag): the line is drawn this much lower while
    /// the list scrolls, so it starts later than the line above it. Linear between the samples.
    /// </summary>
    public static void Lag(UIElement element, IReadOnlyList<CascadeKeyframe> frames, TimeSpan duration)
    {
        if (duration <= TimeSpan.Zero || frames.Count < 2)
        {
            return;
        }

        var animation = Compositor.CreateVector3KeyFrameAnimation();
        animation.Target = "Translation";
        var linear = Compositor.CreateLinearEasingFunction();
        foreach (var frame in frames)
        {
            animation.InsertKeyFrame((float)frame.Progress, new Vector3(0, (float)frame.Offset, 0), linear);
        }

        Run(element, animation, duration, TimeSpan.Zero);
    }

    /// <summary>A XAML key frame on <paramref name="curve"/> (for Storyboards of layout properties).</summary>
    public static SplineDoubleKeyFrame Key(double value, TimeSpan at, CubicBezierCurve curve) => new()
    {
        KeyTime = KeyTime.FromTimeSpan(at),
        Value = value,
        KeySpline = new KeySpline { ControlPoint1 = new Point(curve.X1, curve.Y1), ControlPoint2 = new Point(curve.X2, curve.Y2) },
    };

    private static void Run(UIElement element, KeyFrameAnimation animation, TimeSpan duration, TimeSpan delay)
    {
        animation.Duration = duration < TimeSpan.FromMilliseconds(1) ? TimeSpan.FromMilliseconds(1) : duration;
        if (delay > TimeSpan.Zero)
        {
            animation.DelayTime = delay;
            animation.DelayBehavior = AnimationDelayBehavior.SetInitialValueBeforeDelay;
        }

        element.StartAnimation(animation);
    }
}

namespace Jellyfesh.Drawing;

/// <summary>
/// A CSS cubic-bezier easing curve (Misty's motion tokens: Fluid, Spring, Exit), evaluated in code
/// for colour crossfades and for the keyframes of the lyrics cascade.
/// </summary>
public sealed record CubicBezierCurve(double X1, double Y1, double X2, double Y2)
{
    /// <summary>The eased progress at time fraction <paramref name="x"/> (0 to 1).</summary>
    public double Ease(double x)
    {
        if (x <= 0)
        {
            return 0;
        }

        if (x >= 1)
        {
            return 1;
        }

        // Solve Bx(t) = x by bisection (robust; a crossfade evaluates a few dozen times).
        double lo = 0, hi = 1, t = x;
        for (var i = 0; i < 24; i++)
        {
            t = (lo + hi) / 2;
            if (Sample(X1, X2, t) < x)
            {
                lo = t;
            }
            else
            {
                hi = t;
            }
        }

        return Sample(Y1, Y2, t);
    }

    private static double Sample(double p1, double p2, double t)
    {
        var u = 1 - t;
        return (3 * u * u * t * p1) + (3 * u * t * t * p2) + (t * t * t);
    }
}

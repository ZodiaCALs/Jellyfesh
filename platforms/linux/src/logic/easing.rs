//! Misty's motion curves as functions of progress.
//!
//! libadwaita's animations take a fixed set of easings, so the app runs them
//! linearly and maps progress through these curves instead: Fluid for
//! entering and moving, Spring for small presses, Exit for leaving, Linear for
//! progress (design tokens `motion.ease-*`).

/// A CSS `cubic-bezier(x1, y1, x2, y2)` timing function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl CubicBezier {
    pub const fn new(points: [f32; 4]) -> Self {
        CubicBezier {
            x1: points[0] as f64,
            y1: points[1] as f64,
            x2: points[2] as f64,
            y2: points[3] as f64,
        }
    }

    /// The eased value at progress `t` (0 to 1). Values may overshoot 1 for
    /// curves like Spring.
    pub fn ease(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        let s = self.solve_x(t);
        sample(self.y1, self.y2, s)
    }

    /// The curve parameter whose x is `x`: Newton steps, then bisection.
    fn solve_x(&self, x: f64) -> f64 {
        let mut s = x;
        for _ in 0..8 {
            let err = sample(self.x1, self.x2, s) - x;
            if err.abs() < 1e-7 {
                return s;
            }
            let d = derivative(self.x1, self.x2, s);
            if d.abs() < 1e-6 {
                break;
            }
            s -= err / d;
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        s = x;
        for _ in 0..40 {
            let v = sample(self.x1, self.x2, s);
            if (v - x).abs() < 1e-7 {
                break;
            }
            if v < x {
                lo = s;
            } else {
                hi = s;
            }
            s = (lo + hi) / 2.0;
        }
        s
    }
}

/// One coordinate of a cubic Bézier from (0,0) to (1,1) at parameter `s`.
fn sample(p1: f64, p2: f64, s: f64) -> f64 {
    let inv = 1.0 - s;
    3.0 * inv * inv * s * p1 + 3.0 * inv * s * s * p2 + s * s * s
}

fn derivative(p1: f64, p2: f64, s: f64) -> f64 {
    let inv = 1.0 - s;
    3.0 * inv * inv * p1 + 6.0 * inv * s * (p2 - p1) + 3.0 * s * s * (1.0 - p2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLUID: CubicBezier = CubicBezier::new([0.22, 1.0, 0.36, 1.0]);
    const SPRING: CubicBezier = CubicBezier::new([0.34, 1.36, 0.64, 1.0]);
    const LINEAR: CubicBezier = CubicBezier::new([0.0, 0.0, 1.0, 1.0]);

    #[test]
    fn ends_are_fixed() {
        for c in [FLUID, SPRING, LINEAR] {
            assert_eq!(c.ease(0.0), 0.0);
            assert_eq!(c.ease(1.0), 1.0);
            assert_eq!(c.ease(-3.0), 0.0);
            assert_eq!(c.ease(7.0), 1.0);
        }
    }

    #[test]
    fn linear_is_identity() {
        for i in 0..=20 {
            let t = f64::from(i) / 20.0;
            assert!((LINEAR.ease(t) - t).abs() < 1e-5, "{t}");
        }
    }

    #[test]
    fn fluid_is_fast_then_calm_and_monotonic() {
        assert!(FLUID.ease(0.2) > 0.5, "{}", FLUID.ease(0.2));
        let mut last = 0.0;
        for i in 0..=100 {
            let v = FLUID.ease(f64::from(i) / 100.0);
            assert!(v >= last - 1e-9);
            last = v;
        }
    }

    #[test]
    fn spring_overshoots() {
        let peak = (1..100)
            .map(|i| SPRING.ease(f64::from(i) / 100.0))
            .fold(0.0, f64::max);
        assert!(peak > 1.0, "{peak}");
        assert!(peak < 1.2);
    }

    #[test]
    fn matches_known_css_values() {
        // ease-in-out (0.42, 0, 0.58, 1) is symmetric: 0.5 maps to 0.5.
        let c = CubicBezier::new([0.42, 0.0, 0.58, 1.0]);
        assert!((c.ease(0.5) - 0.5).abs() < 1e-6);
    }
}

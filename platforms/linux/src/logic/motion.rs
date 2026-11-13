//! Motion plans (design-controls.md, "Motion"): the duration, curve and
//! distance of every transition, from Misty's motion tokens, and how each
//! collapses under reduced motion. With the app's "Reduce motion and
//! transparency" everything becomes a short fade and nothing moves or
//! scales; with the system's animations off (`gtk-enable-animations`)
//! nothing animates at all.

use jf_tokens::{lyrics, motion, player};

use crate::logic::easing::CubicBezier;

/// Entering and moving (Misty Fluid).
pub const FLUID: CubicBezier = CubicBezier::new(motion::EASE_FLUID);
/// Small presses and the play button's shape change (Misty Spring).
pub const SPRING: CubicBezier = CubicBezier::new(motion::EASE_SPRING);
/// Leaving (Misty Exit).
pub const EXIT: CubicBezier = CubicBezier::new(motion::EASE_EXIT);
/// Progress (Misty Linear).
pub const LINEAR: CubicBezier = CubicBezier::new([0.0, 0.0, 1.0, 1.0]);

/// A page rises this far as it fades in.
pub const PAGE_RISE_PX: f64 = 8.0;
/// Now Playing slides this far as it opens and closes.
pub const NOW_PLAYING_SLIDE_PX: f64 = 32.0;
/// Popovers and sheets scale up from this.
pub const POPUP_SCALE_FROM: f64 = 0.96;

/// How much motion the person allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowed {
    /// Everything moves.
    Full,
    /// The app's "Reduce motion and transparency": short fades only.
    Reduced,
    /// The system's animations are off: nothing animates.
    Nothing,
}

impl Allowed {
    pub fn new(system_animations: bool, reduce_effects: bool) -> Allowed {
        match (system_animations, reduce_effects) {
            (false, _) => Allowed::Nothing,
            (true, true) => Allowed::Reduced,
            (true, false) => Allowed::Full,
        }
    }

    /// Whether things may move and scale (not only fade).
    pub fn moves(self) -> bool {
        self == Allowed::Full
    }
}

/// One transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub duration_ms: u32,
    pub curve: CubicBezier,
    /// Distance moved, in px: up from below while entering (positive), down
    /// while leaving.
    pub distance: f64,
    /// Scale at the hidden end (1: no scaling).
    pub scale_from: f64,
}

impl Plan {
    /// No animation: the change shows at once.
    pub const NONE: Plan = Plan {
        duration_ms: 0,
        curve: LINEAR,
        distance: 0.0,
        scale_from: 1.0,
    };

    pub fn animates(&self) -> bool {
        self.duration_ms > 0
    }

    /// The eased progress at `t` (0 to 1 of the duration).
    pub fn ease(&self, t: f64) -> f64 {
        self.curve.ease(t)
    }

    fn fade(duration_ms: u32) -> Plan {
        Plan {
            duration_ms,
            curve: FLUID,
            ..Plan::NONE
        }
    }
}

/// The transitions the app runs itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// A page appears (a section, an album): fades in and rises 8 px.
    PageEnter,
    /// Back to the page underneath: a quick fade.
    PageBack,
    /// Now Playing opens: slides up and fades in ("Deeper").
    NowPlayingOpen,
    /// Now Playing closes: slides down and fades out on Exit ("Back").
    NowPlayingClose,
    /// Popovers and sheets: scale up from 96 % and fade in.
    Popup,
}

/// The plan for `transition` under `allowed`.
pub fn plan(transition: Transition, allowed: Allowed) -> Plan {
    let short = Plan::fade(motion::DUR_1_MS);
    match (allowed, transition) {
        (Allowed::Nothing, _) => Plan::NONE,
        (Allowed::Reduced, _) => short,
        (Allowed::Full, Transition::PageEnter) => Plan {
            duration_ms: motion::DUR_2_MS,
            curve: FLUID,
            distance: PAGE_RISE_PX,
            scale_from: 1.0,
        },
        (Allowed::Full, Transition::PageBack) => short,
        (Allowed::Full, Transition::NowPlayingOpen) => Plan {
            duration_ms: token_ms(player::OPEN_NOW_PLAYING).unwrap_or(motion::DUR_3_MS),
            curve: FLUID,
            distance: NOW_PLAYING_SLIDE_PX,
            scale_from: 1.0,
        },
        (Allowed::Full, Transition::NowPlayingClose) => Plan {
            duration_ms: token_ms(player::CLOSE_NOW_PLAYING).unwrap_or(motion::DUR_3_MS),
            curve: EXIT,
            distance: NOW_PLAYING_SLIDE_PX,
            scale_from: 1.0,
        },
        (Allowed::Full, Transition::Popup) => Plan {
            duration_ms: motion::DUR_2_MS,
            curve: FLUID,
            distance: 0.0,
            scale_from: POPUP_SCALE_FROM,
        },
    }
}

/// The milliseconds in a motion token written as text ("Deeper 320 ms").
pub fn token_ms(text: &str) -> Option<u32> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let unit = words.iter().position(|w| *w == "ms")?;
    words.get(unit.checked_sub(1)?)?.parse().ok()
}

/// The Now Playing artwork's scale: 1 while playing, 0.88 while paused
/// (`player.artwork-paused-scale`); it never scales with reduced motion.
pub fn artwork_scale(playing: bool, allowed: Allowed) -> f64 {
    if playing || !allowed.moves() {
        1.0
    } else {
        f64::from(player::ARTWORK_PAUSED_SCALE)
    }
}

/// The artwork springing between its scales: `dur-3` on the Spring curve.
pub fn artwork_plan(allowed: Allowed) -> Plan {
    if allowed.moves() {
        Plan {
            duration_ms: motion::DUR_3_MS,
            curve: SPRING,
            ..Plan::NONE
        }
    } else {
        Plan::NONE
    }
}

/// Play and pause morphing into each other: `dur-2` Spring; with reduced
/// motion the glyph changes at once.
pub fn morph_plan(allowed: Allowed) -> Plan {
    if allowed.moves() {
        Plan {
            duration_ms: motion::DUR_2_MS,
            curve: SPRING,
            ..Plan::NONE
        }
    } else {
        Plan::NONE
    }
}

/// A slider thickening and its thumb appearing: `dur-1` Fluid.
pub fn slider_plan(allowed: Allowed) -> Plan {
    match allowed {
        Allowed::Nothing => Plan::NONE,
        _ => Plan::fade(motion::DUR_1_MS),
    }
}

/// The lyrics moving to a new line: `dur-3` Fluid, each following line
/// `lyrics.stagger` later; with reduced motion they jump (`None`).
pub fn lyrics_cascade(allowed: Allowed) -> Option<(f64, f64)> {
    allowed
        .moves()
        .then(|| (f64::from(motion::DUR_3_MS), f64::from(lyrics::STAGGER_MS)))
}

/// The theme's colours crossfading on a hue change: `dur-4`, `dur-1` with
/// reduced motion (it is a fade), none with animations off.
pub fn theme_crossfade_ms(allowed: Allowed) -> u32 {
    match allowed {
        Allowed::Full => motion::DUR_4_MS,
        Allowed::Reduced => motion::DUR_1_MS,
        Allowed::Nothing => 0,
    }
}

/// The Now Playing backdrop crossfading between songs
/// (`backdrop.crossfade`: `dur-4` Fluid), shortened like the theme's.
pub fn backdrop_crossfade_ms(allowed: Allowed) -> u32 {
    theme_crossfade_ms(allowed)
}

/// Whether the level glyph on the playing row moves: only while playing,
/// and it stands still with reduced motion.
pub fn level_moves(playing: bool, allowed: Allowed) -> bool {
    playing && allowed.moves()
}

/// The progress (0 to 1) of an animation started `elapsed_ms` ago that
/// waits `delay_ms`, then runs `duration_ms`.
pub fn delayed_progress(elapsed_ms: f64, delay_ms: f64, duration_ms: f64) -> f64 {
    if duration_ms <= 0.0 {
        return if elapsed_ms >= delay_ms { 1.0 } else { 0.0 };
    }
    ((elapsed_ms - delay_ms) / duration_ms).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_motion_follows_the_system_then_the_app() {
        assert_eq!(Allowed::new(true, false), Allowed::Full);
        assert_eq!(Allowed::new(true, true), Allowed::Reduced);
        assert_eq!(Allowed::new(false, false), Allowed::Nothing);
        assert_eq!(Allowed::new(false, true), Allowed::Nothing);
        assert!(Allowed::Full.moves());
        assert!(!Allowed::Reduced.moves());
    }

    #[test]
    fn pages_fade_in_and_rise() {
        let p = plan(Transition::PageEnter, Allowed::Full);
        assert_eq!((p.duration_ms, p.distance), (240, 8.0));
        assert_eq!(p.curve, FLUID);
        let back = plan(Transition::PageBack, Allowed::Full);
        assert_eq!((back.duration_ms, back.distance), (160, 0.0));
    }

    #[test]
    fn now_playing_takes_its_player_tokens() {
        assert_eq!(token_ms("Deeper 320 ms"), Some(320));
        assert_eq!(token_ms("Back 300 ms"), Some(300));
        assert_eq!(token_ms("dur-3 fluid"), None);
        assert_eq!(token_ms("ms"), None);
        let open = plan(Transition::NowPlayingOpen, Allowed::Full);
        assert_eq!((open.duration_ms, open.curve), (320, FLUID));
        let close = plan(Transition::NowPlayingClose, Allowed::Full);
        assert_eq!((close.duration_ms, close.curve), (300, EXIT));
        assert!(close.distance > 0.0);
        let popup = plan(Transition::Popup, Allowed::Full);
        assert_eq!((popup.duration_ms, popup.scale_from), (240, 0.96));
    }

    #[test]
    fn reduced_motion_becomes_short_fades() {
        for t in [
            Transition::PageEnter,
            Transition::PageBack,
            Transition::NowPlayingOpen,
            Transition::NowPlayingClose,
            Transition::Popup,
        ] {
            let p = plan(t, Allowed::Reduced);
            assert_eq!(p.duration_ms, 160, "{t:?}");
            assert_eq!(p.distance, 0.0, "{t:?}");
            assert_eq!(p.scale_from, 1.0, "{t:?}");
            assert!(!plan(t, Allowed::Nothing).animates(), "{t:?}");
        }
        assert_eq!(theme_crossfade_ms(Allowed::Full), 600);
        assert_eq!(theme_crossfade_ms(Allowed::Reduced), 160);
        assert_eq!(backdrop_crossfade_ms(Allowed::Nothing), 0);
    }

    #[test]
    fn the_artwork_breathes_only_with_full_motion() {
        assert_eq!(artwork_scale(true, Allowed::Full), 1.0);
        assert!((artwork_scale(false, Allowed::Full) - 0.88).abs() < 1e-6);
        assert_eq!(artwork_scale(false, Allowed::Reduced), 1.0);
        assert_eq!(artwork_scale(false, Allowed::Nothing), 1.0);
        let spring = artwork_plan(Allowed::Full);
        assert_eq!((spring.duration_ms, spring.curve), (380, SPRING));
        assert!(!artwork_plan(Allowed::Reduced).animates());
        assert_eq!(morph_plan(Allowed::Full).duration_ms, 240);
        assert!(!morph_plan(Allowed::Reduced).animates());
        assert_eq!(slider_plan(Allowed::Reduced).duration_ms, 160);
        assert!(!slider_plan(Allowed::Nothing).animates());
    }

    #[test]
    fn lyrics_cascade_and_level_glyph() {
        assert_eq!(lyrics_cascade(Allowed::Full), Some((380.0, 30.0)));
        assert_eq!(lyrics_cascade(Allowed::Reduced), None);
        assert!(level_moves(true, Allowed::Full));
        assert!(!level_moves(false, Allowed::Full));
        assert!(!level_moves(true, Allowed::Reduced));
    }

    #[test]
    fn delays_hold_progress_at_zero() {
        assert_eq!(delayed_progress(10.0, 30.0, 380.0), 0.0);
        assert_eq!(delayed_progress(220.0, 30.0, 380.0), 0.5);
        assert_eq!(delayed_progress(1_000.0, 30.0, 380.0), 1.0);
        assert_eq!(delayed_progress(40.0, 30.0, 0.0), 1.0);
        assert_eq!(delayed_progress(20.0, 30.0, 0.0), 0.0);
    }
}

//! Stylesheets and the run-time theme.
//!
//! * The static stylesheets are generated at build time from the design
//!   tokens (`build.rs`); control style D lives in its own sheet so another
//!   control style can replace it.
//! * Theme colours come from the core's theme slice (Misty at the artwork
//!   hue, or a fixed hue from Appearance), in the light or dark variant that
//!   `AdwStyleManager` reports, with the glass grades' alpha set for the
//!   Transparency setting (`Theme::with_transparency`), and are installed as
//!   GTK named colours. A hue change crossfades on `dur-4` with the Fluid
//!   curve.
//! * Live glass (`backdrop-filter`) needs GTK 4.22; older GTK, high contrast
//!   and reduced effects get solid Cloud surfaces (ADR-0006, Misty MAT-08),
//!   and "Reduce motion and transparency" makes every glass grade opaque.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use jf_core::AppearancePref;

use crate::logic::appearance::glass_transparency;
use crate::logic::easing::CubicBezier;
use crate::logic::motion::{self, Allowed};
use crate::logic::theme::{self, Colors, Glass};
use crate::ui::state::{App, try_app};

const STYLE_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/css/style.css"));
const CONTROLS_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/css/controls-d.css"));
const GLASS_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/css/glass-live.css"));

/// Misty's Fluid curve, for animations driven in code (CSS transitions use
/// the token curves directly).
pub const FLUID: CubicBezier = motion::FLUID;

/// Colour crossfade steps: the theme stylesheet is rebuilt this many times
/// during a hue change, not every frame.
const CROSSFADE_STEPS: f64 = 12.0;

pub struct Style {
    theme: gtk::CssProvider,
    glass: gtk::CssProvider,
    glass_on: Cell<bool>,
    shown: RefCell<Colors>,
    from: RefCell<Colors>,
    target: RefCell<Colors>,
    step: Cell<i32>,
    animation: RefCell<Option<adw::TimedAnimation>>,
}

impl std::fmt::Debug for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Style").finish_non_exhaustive()
    }
}

thread_local! {
    static STYLE: RefCell<Option<Rc<Style>>> = const { RefCell::new(None) };
}

pub fn style() -> Rc<Style> {
    STYLE
        .with(|s| s.borrow().clone())
        .expect("styles are loaded at startup")
}

/// Loads the stylesheets for the default display.
pub fn load() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let priority = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION;
    for (name, css) in [("style.css", STYLE_CSS), ("controls-d.css", CONTROLS_CSS)] {
        let provider = gtk::CssProvider::new();
        provider.connect_parsing_error(move |_, section, error| {
            tracing::warn!("{name}: {} at {}", error, section.to_str());
        });
        provider.load_from_string(css);
        gtk::style_context_add_provider_for_display(&display, &provider, priority);
    }
    let theme_provider = gtk::CssProvider::new();
    // The theme's colours must win over libadwaita's own definitions.
    gtk::style_context_add_provider_for_display(&display, &theme_provider, priority + 1);
    let glass = gtk::CssProvider::new();
    glass.connect_parsing_error(|_, section, error| {
        tracing::warn!("glass-live.css: {} at {}", error, section.to_str());
    });
    let s = Rc::new(Style {
        theme: theme_provider,
        glass,
        glass_on: Cell::new(false),
        shown: RefCell::new(Colors::new()),
        from: RefCell::new(Colors::new()),
        target: RefCell::new(Colors::new()),
        step: Cell::new(-1),
        animation: RefCell::new(None),
    });
    STYLE.with(|st| *st.borrow_mut() = Some(s.clone()));
    // The first frame uses the fallback hue until a theme arrives.
    s.set_colors(fallback_theme(false).colors, false);

    let manager = adw::StyleManager::default();
    manager.connect_dark_notify(|_| apply_theme(false));
    manager.connect_high_contrast_notify(|_| {
        apply_glass();
        apply_theme(false);
    });
    if let Some(settings) = gtk::Settings::default() {
        settings.connect_gtk_enable_animations_notify(|_| apply_glass());
    }
}

/// True when the system lets apps animate (reduce motion is off).
pub fn animations_enabled() -> bool {
    gtk::Settings::default().is_none_or(|s| s.is_gtk_enable_animations())
}

/// Whether "Reduce motion and transparency" is on.
pub fn reduce_effects() -> bool {
    try_app().is_some_and(|a| a.settings.borrow().reduce_effects)
}

/// How much motion the person allows: the system's animation setting, then
/// the app's "Reduce motion and transparency" (Misty MOT-12).
pub fn allowed() -> Allowed {
    Allowed::new(animations_enabled(), reduce_effects())
}

/// A fade's duration from the tokens: as is, at most `dur-1` with reduced
/// motion (everything becomes a short fade), and zero when the system asks
/// for no animations.
pub fn duration(ms: u32) -> u32 {
    match allowed() {
        Allowed::Full => ms,
        Allowed::Reduced => ms.min(jf_tokens::motion::DUR_1_MS),
        Allowed::Nothing => 0,
    }
}

/// Fades a popover in as it opens (design-controls.md, "Motion": popovers
/// scale from 96 % and fade, `dur-2`). GTK draws a popover as its own
/// surface, which cannot be scaled, so it only fades; without a compositor
/// it simply appears.
pub fn animate_popup(popover: &(impl IsA<gtk::Popover> + IsA<gtk::Widget>)) {
    let running: Rc<RefCell<Option<adw::TimedAnimation>>> = Rc::new(RefCell::new(None));
    popover.connect_map(move |p| {
        let plan = motion::plan(motion::Transition::Popup, allowed());
        let composited = gdk::Display::default().is_some_and(|d| d.is_composited());
        if !plan.animates() || !composited {
            p.set_opacity(1.0);
            return;
        }
        p.set_opacity(0.0);
        let weak = p.downgrade();
        let target = adw::CallbackAnimationTarget::new(move |t| {
            if let Some(p) = weak.upgrade() {
                p.set_opacity(plan.ease(t).clamp(0.0, 1.0));
            }
        });
        let anim = adw::TimedAnimation::new(p, 0.0, 1.0, plan.duration_ms, target);
        anim.set_easing(adw::Easing::Linear);
        anim.play();
        *running.borrow_mut() = Some(anim);
    });
}

/// The transparency glass is drawn with now.
fn transparency(app: &App) -> f32 {
    let setting = app.settings.borrow().transparency;
    let high_contrast = adw::StyleManager::default().is_high_contrast();
    glass_transparency(setting, reduce_effects(), high_contrast)
}

fn fallback_theme(dark: bool) -> jf_core::Theme {
    let t = jf_core::theme::theme_for_artwork(None);
    if dark { t.dark } else { t.light }
}

/// The theme of an appearance as the core resolved it (the hue setting is
/// the core's), or the fallback theme before one arrives.
pub fn theme_for(app: &App, dark: bool) -> jf_core::Theme {
    match app.theme.borrow().as_ref() {
        Some(theme) => {
            if dark {
                theme.dark.clone()
            } else {
                theme.light.clone()
            }
        }
        None => fallback_theme(dark),
    }
}

/// An appearance's colours with the glass grades set for the Transparency
/// setting (also for previews of the other appearance in Settings).
pub fn colors_for(app: &App, dark: bool) -> Colors {
    theme_for(app, dark).with_transparency(transparency(app)).colors
}

/// The colours for the current settings, theme slice and appearance.
pub fn wanted_colors(app: &App) -> Colors {
    colors_for(app, adw::StyleManager::default().is_dark())
}

/// Applies the appearance setting (light, dark or system).
pub fn apply_appearance() {
    let Some(app) = try_app() else { return };
    let scheme = match app.settings.borrow().appearance {
        AppearancePref::System => adw::ColorScheme::Default,
        AppearancePref::Light => adw::ColorScheme::ForceLight,
        AppearancePref::Dark => adw::ColorScheme::ForceDark,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

/// Installs the theme colours, crossfading from the current ones when
/// `animate` and motion is allowed.
pub fn apply_theme(animate: bool) {
    let Some(app) = try_app() else { return };
    let colors = wanted_colors(&app);
    style().set_colors(colors, animate);
}

/// Turns live glass on or off for the GTK version and the accessibility
/// settings.
pub fn apply_glass() {
    let reduce = try_app().is_some_and(|a| a.settings.borrow().reduce_effects);
    let mode = theme::glass(
        gtk::minor_version(),
        adw::StyleManager::default().is_high_contrast(),
        reduce,
        animations_enabled(),
    );
    let s = style();
    let on = mode == Glass::Live;
    if on == s.glass_on.get() {
        return;
    }
    let Some(display) = gdk::Display::default() else {
        return;
    };
    if on {
        s.glass.load_from_string(GLASS_CSS);
        gtk::style_context_add_provider_for_display(
            &display,
            &s.glass,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    } else {
        gtk::style_context_remove_provider_for_display(&display, &s.glass);
    }
    s.glass_on.set(on);
}

impl Style {
    /// The colour shown now for a theme colour name (`ink.ink-90`).
    pub fn rgba(&self, name: &str) -> gdk::RGBA {
        let [r, g, b, a] = self.shown.borrow().get(name).copied().unwrap_or([0, 0, 0, 255]);
        gdk::RGBA::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        )
    }

    fn set_colors(&self, colors: Colors, animate: bool) {
        if colors == *self.target.borrow() {
            return;
        }
        if let Some(a) = self.animation.borrow_mut().take() {
            a.skip();
        }
        let window = try_app().and_then(|a| a.window.borrow().upgrade());
        let fade_ms = motion::theme_crossfade_ms(allowed());
        let can_animate = animate && !self.shown.borrow().is_empty() && fade_ms > 0 && window.is_some();
        *self.target.borrow_mut() = colors.clone();
        if !can_animate {
            self.show(colors);
            return;
        }
        *self.from.borrow_mut() = self.shown.borrow().clone();
        self.step.set(-1);
        let Some(window) = window else { return };
        let target = adw::CallbackAnimationTarget::new(|t| {
            let s = style();
            let eased = FLUID.ease(t);
            let step = (eased * CROSSFADE_STEPS).floor() as i32;
            if step == s.step.get() && t < 1.0 {
                return;
            }
            s.step.set(step);
            let mixed = if t >= 1.0 {
                s.target.borrow().clone()
            } else {
                theme::mix_maps(&s.from.borrow(), &s.target.borrow(), eased)
            };
            s.show(mixed);
        });
        let anim = adw::TimedAnimation::new(&window, 0.0, 1.0, fade_ms, target);
        anim.set_easing(adw::Easing::Linear);
        anim.connect_done(|_| {
            let s = style();
            let target = s.target.borrow().clone();
            s.show(target);
        });
        anim.play();
        *self.animation.borrow_mut() = Some(anim);
    }

    fn show(&self, colors: Colors) {
        if colors == *self.shown.borrow() {
            return;
        }
        self.theme.load_from_string(&theme::theme_css(&colors));
        *self.shown.borrow_mut() = colors;
    }
}

//! The line sliders (design-controls.md, "Seek bar and sliders: a straight
//! line"): the seek bar and the volume slider. A straight line with round
//! caps, played part `ink-90`, the rest `ink-30`; no thumb at rest; hovering
//! or focusing thickens the line to 6 px and shows a 14 px ringed thumb,
//! dragging thickens it to 8 px (12 px and no thumb on touch screens). The
//! changes of thickness and the thumb's appearance take `dur-1` Fluid; the
//! fill itself never animates.
//!
//! The seek bar draws its times under the line's ends (Now Playing) or
//! beside them (the player bar). It redraws only when its fill moves by a
//! device pixel or a time label changes (the clock scheduler asks
//! [`SeekBar::next_change`]); a drag redraws per pointer event.

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, pango};
use jf_core::timing::next_second_us;

use crate::logic::motion;
use crate::logic::seek::{self, Input, Interaction, Line, SliderKey, Times};
use crate::logic::{format, schedule};
use crate::ui::i18n::trf;
use crate::ui::style;

type SeekFn = Box<dyn Fn(u64)>;
type VolumeFn = Box<dyn Fn(f64)>;

/// What every line slider shares: how it is being used, and the look it
/// shows on the way between states.
#[derive(Default)]
pub struct LineState {
    hover: Cell<bool>,
    focus: Cell<bool>,
    dragging: Cell<bool>,
    input: Cell<Input>,
    /// The thickness and the thumb's appearance (0 to 1) shown now.
    shown: Cell<Option<(f64, f64)>>,
    from: Cell<(f64, f64)>,
    animation: RefCell<Option<adw::TimedAnimation>>,
}

impl std::fmt::Debug for LineState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineState")
            .field("interaction", &self.interaction())
            .field("input", &self.input.get())
            .finish_non_exhaustive()
    }
}

impl LineState {
    fn interaction(&self) -> Interaction {
        if self.dragging.get() {
            Interaction::Drag
        } else if self.hover.get() || self.focus.get() {
            Interaction::Hover
        } else {
            Interaction::Rest
        }
    }

    fn target(&self) -> (f64, f64) {
        let (state, input) = (self.interaction(), self.input.get());
        let thumb = if seek::thumb_shown(state, input) { 1.0 } else { 0.0 };
        (seek::thickness(state, input), thumb)
    }

    fn shown(&self) -> (f64, f64) {
        self.shown.get().unwrap_or_else(|| self.target())
    }
}

/// The widgets with a [`LineState`].
trait HasLine: IsA<gtk::Widget> {
    fn line_state(&self) -> &LineState;
}

/// Moves the slider's look towards its state: `dur-1` Fluid, or at once.
fn retarget<W: HasLine>(widget: &W) {
    let state = widget.line_state();
    if let Some(a) = state.animation.borrow_mut().take() {
        a.pause();
    }
    let target = state.target();
    let from = state.shown();
    let plan = motion::slider_plan(style::allowed());
    if from == target || !plan.animates() || !widget.is_mapped() {
        state.shown.set(Some(target));
        widget.queue_draw();
        return;
    }
    state.from.set(from);
    let weak = widget.downgrade();
    let callback = adw::CallbackAnimationTarget::new(move |t| {
        let Some(w) = weak.upgrade() else { return };
        let s = w.line_state();
        let (f, to) = (s.from.get(), s.target());
        let e = plan.ease(t);
        s.shown
            .set(Some((seek::lerp(f.0, to.0, e), seek::lerp(f.1, to.1, e))));
        w.queue_draw();
    });
    let anim = adw::TimedAnimation::new(widget.as_ref(), 0.0, 1.0, plan.duration_ms, callback);
    anim.set_easing(adw::Easing::Linear);
    anim.play();
    *state.animation.borrow_mut() = Some(anim);
}

/// The kind of pointer behind a gesture.
fn input_of(gesture: &impl IsA<gtk::EventController>) -> Input {
    let touch = gesture
        .current_event_device()
        .is_some_and(|d| d.source() == gdk::InputSource::Touchscreen);
    if touch { Input::Touch } else { Input::Pointer }
}

fn slider_key(key: gdk::Key, vertical_too: bool) -> Option<SliderKey> {
    Some(match key {
        gdk::Key::Left | gdk::Key::KP_Left => SliderKey::Back,
        gdk::Key::Right | gdk::Key::KP_Right => SliderKey::Forward,
        gdk::Key::Down | gdk::Key::KP_Down if vertical_too => SliderKey::Back,
        gdk::Key::Up | gdk::Key::KP_Up if vertical_too => SliderKey::Forward,
        gdk::Key::Page_Down | gdk::Key::KP_Page_Down => SliderKey::PageBack,
        gdk::Key::Page_Up | gdk::Key::KP_Page_Up => SliderKey::PageForward,
        gdk::Key::Home | gdk::Key::KP_Home => SliderKey::Home,
        gdk::Key::End | gdk::Key::KP_End => SliderKey::End,
        _ => return None,
    })
}

/// Hover and focus controllers that keep the line state and its look.
fn track_hover_and_focus<W: HasLine>(widget: &W) {
    let motion_ctl = gtk::EventControllerMotion::new();
    let weak = widget.downgrade();
    motion_ctl.connect_enter(move |c, _, _| {
        let Some(w) = weak.upgrade() else { return };
        // Touch screens have no hover.
        if input_of(c) == Input::Touch {
            return;
        }
        let s = w.line_state();
        s.hover.set(true);
        s.input.set(Input::Pointer);
        retarget(&w);
    });
    let weak = widget.downgrade();
    motion_ctl.connect_leave(move |_| {
        if let Some(w) = weak.upgrade() {
            w.line_state().hover.set(false);
            retarget(&w);
        }
    });
    widget.add_controller(motion_ctl);
    let focus = gtk::EventControllerFocus::new();
    let weak = widget.downgrade();
    focus.connect_enter(move |_| {
        if let Some(w) = weak.upgrade() {
            let s = w.line_state();
            s.focus.set(true);
            s.input.set(Input::Pointer);
            retarget(&w);
        }
    });
    let weak = widget.downgrade();
    focus.connect_leave(move |_| {
        if let Some(w) = weak.upgrade() {
            w.line_state().focus.set(false);
            retarget(&w);
        }
    });
    widget.add_controller(focus);
}

/// The colours of a line slider: the played part is the widget's CSS
/// colour; the rest is `ink-30`, or `line-strong` on plain surfaces.
struct Paint {
    played: gdk::RGBA,
    rest: gdk::RGBA,
    ring: gdk::RGBA,
    shadow: gdk::RGBA,
}

impl Paint {
    fn of(widget: &impl IsA<gtk::Widget>) -> Paint {
        let s = style::style();
        let plain = widget.has_css_class("plain");
        let mut played = widget.color();
        let mut rest = s.rgba(if plain { "line.line-strong" } else { "ink.ink-30" });
        if !widget.is_sensitive() {
            played = gdk::RGBA::new(played.red(), played.green(), played.blue(), played.alpha() * 0.4);
            rest = gdk::RGBA::new(rest.red(), rest.green(), rest.blue(), rest.alpha() * 0.6);
        }
        Paint {
            played,
            rest,
            ring: s.rgba("glass.glass-edge"),
            shadow: s.rgba("elevation.shadow-1.1"),
        }
    }
}

fn rounded(rect: graphene::Rect, radius: f32) -> gsk::RoundedRect {
    let r = graphene::Size::new(radius, radius);
    gsk::RoundedRect::new(rect, r, r, r, r)
}

/// Draws the line with its played part up to `fraction`, `thickness` px
/// thick, and the thumb at `thumb` of its appearance (0: none).
fn paint_line(
    snapshot: &gtk::Snapshot,
    line: &Line,
    fraction: f64,
    thickness: f64,
    thumb: f64,
    paint: &Paint,
) {
    let t = thickness as f32;
    let (x0, x1, y) = (line.x0 as f32, line.x1 as f32, line.y as f32);
    let track = graphene::Rect::new(x0 - t / 2.0, y - t / 2.0, (x1 - x0) + t, t);
    snapshot.push_rounded_clip(&rounded(track, t / 2.0));
    snapshot.append_color(&paint.rest, &track);
    snapshot.pop();
    let xp = line.x_at(fraction) as f32;
    if xp > x0 {
        let played = graphene::Rect::new(x0 - t / 2.0, y - t / 2.0, (xp - x0) + t, t);
        snapshot.push_rounded_clip(&rounded(played, t / 2.0));
        snapshot.append_color(&paint.played, &played);
        snapshot.pop();
    }
    if thumb <= 0.01 {
        return;
    }
    // The thumb grows in from 60 % as it fades in.
    let size = (seek::thumb_size() * seek::lerp(0.6, 1.0, thumb)) as f32;
    let disc_rect = graphene::Rect::new(xp - size / 2.0, y - size / 2.0, size, size);
    let disc = rounded(disc_rect, size / 2.0);
    snapshot.push_opacity(thumb);
    snapshot.append_outset_shadow(&disc, &paint.shadow, 0.0, 1.0, 0.0, 3.0);
    snapshot.push_rounded_clip(&disc);
    let face = gdk::RGBA::new(paint.played.red(), paint.played.green(), paint.played.blue(), 1.0);
    snapshot.append_color(&face, &disc_rect);
    snapshot.pop();
    let ring = [paint.ring; 4];
    snapshot.append_border(&disc, &[2.0; 4], &ring);
    snapshot.pop();
}

// ---------------------------------------------------------------------------
// The seek bar

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SeekBar {
        pub line: LineState,
        pub times: Cell<Option<Times>>,
        pub position_us: Cell<i64>,
        pub duration_us: Cell<Option<i64>>,
        pub playing: Cell<bool>,
        pub drag_x: Cell<f64>,
        pub drag_start: Cell<f64>,
        pub step_index: Cell<i64>,
        pub second: Cell<i64>,
        pub on_seek: RefCell<Option<SeekFn>>,
        /// Laid-out time labels: (text, layout), elapsed and remaining.
        pub labels: RefCell<[(String, Option<pango::Layout>); 2]>,
        /// The widest remaining time for the track's length, the font it
        /// was measured in, and its width.
        pub widest: RefCell<(String, Option<pango::FontDescription>, f64)>,
        /// The time labels' line height, for the font it was measured in.
        pub text_height: RefCell<Option<(Option<pango::FontDescription>, f64)>>,
    }

    impl std::fmt::Debug for SeekBar {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("SeekBar").finish_non_exhaustive()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SeekBar {
        const NAME: &'static str = "JfSeekBar";
        type Type = super::SeekBar;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("seekbar");
            klass.set_accessible_role(gtk::AccessibleRole::Slider);
        }
    }

    impl ObjectImpl for SeekBar {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_focusable(true);
            obj.set_hexpand(true);
            obj.set_cursor_from_name(Some("pointer"));
            self.second.set(-1);
            obj.setup_controllers();
        }
    }

    impl WidgetImpl for SeekBar {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let obj = self.obj();
            match orientation {
                gtk::Orientation::Horizontal => {
                    let label = if obj.times() == Times::Beside {
                        2.0 * (obj.time_width() + seek::BESIDE_GAP)
                    } else {
                        0.0
                    };
                    let min = (80.0 + label) as i32;
                    (min, min.max(300), -1, -1)
                }
                _ => {
                    let h = seek::seek_height(obj.times(), obj.text_height()).ceil() as i32;
                    (h, h, -1, -1)
                }
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }

    impl HasLine for super::SeekBar {
        fn line_state(&self) -> &LineState {
            &self.imp().line
        }
    }
}

glib::wrapper! {
    /// A seek bar for the playback position.
    pub struct SeekBar(ObjectSubclass<imp::SeekBar>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for SeekBar {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl SeekBar {
    /// A seek bar with its times under the line or beside it.
    pub fn new(times: Times) -> SeekBar {
        let bar = Self::default();
        bar.imp().times.set(Some(times));
        bar
    }

    fn times(&self) -> Times {
        self.imp().times.get().unwrap_or(Times::Below)
    }

    /// Moves the times under the line or beside it (Now Playing's dock in
    /// narrow and wide windows).
    pub fn set_times(&self, times: Times) {
        if self.imp().times.replace(Some(times)) != Some(times) {
            self.queue_resize();
        }
    }

    /// Called with the new position in ms when the person seeks.
    pub fn connect_seek(&self, f: impl Fn(u64) + 'static) {
        *self.imp().on_seek.borrow_mut() = Some(Box::new(f));
    }

    fn layout_for(&self, text: &str) -> pango::Layout {
        let layout = self.create_pango_layout(Some(text));
        // Tabular figures, so the times do not jiggle (12 px, CSS).
        let attrs = pango::AttrList::new();
        attrs.insert(pango::AttrFontFeatures::new("tnum 1"));
        layout.set_attributes(Some(&attrs));
        layout
    }

    /// The time labels' line height, measured once per font (the bar draws
    /// often; a layout per frame would be waste).
    fn text_height(&self) -> f64 {
        let font = self.pango_context().font_description();
        let mut cached = self.imp().text_height.borrow_mut();
        if let Some((f, h)) = cached.as_ref()
            && *f == font
        {
            return *h;
        }
        let h = f64::from(self.layout_for("0:00").pixel_size().1);
        *cached = Some((font, h));
        h
    }

    /// The width of the widest remaining time for the track's length.
    fn time_width(&self) -> f64 {
        let widest = format::widest_remaining(self.duration_ms());
        let font = self.pango_context().font_description();
        let mut cached = self.imp().widest.borrow_mut();
        if cached.0 != widest || cached.1 != font {
            let w = f64::from(self.layout_for(&widest).pixel_size().0);
            *cached = (widest, font, w);
        }
        cached.2
    }

    fn geometry(&self) -> seek::SeekLayout {
        let (w, h) = (f64::from(self.width()), f64::from(self.height()));
        let times = self.times();
        let text_height = if times == Times::None {
            0.0
        } else {
            self.text_height()
        };
        seek::seek_layout(w, h, times, self.time_width(), text_height)
    }

    /// The time one device pixel of the line stands for.
    fn step_us(&self, duration_us: Option<i64>) -> Option<i64> {
        let length = self.geometry().line.length();
        schedule::bar_step_device_us(duration_us, length, f64::from(self.scale_factor()))
    }

    /// Updates the bar for a position; redraws only when the fill moved by a
    /// device pixel, a time label changed, or the state changed.
    pub fn set_position(&self, position_us: i64, duration_us: Option<i64>, playing: bool) {
        let imp = self.imp();
        let step = self.step_us(duration_us).unwrap_or(i64::MAX);
        let index = schedule::bar_step_index(position_us, step);
        let second = position_us.max(0) / 1_000_000;
        let new_second = second != imp.second.get() || imp.duration_us.get() != duration_us;
        let changed = index != imp.step_index.get() || new_second || imp.playing.get() != playing;
        imp.position_us.set(position_us);
        imp.duration_us.set(duration_us);
        imp.playing.set(playing);
        imp.step_index.set(index);
        imp.second.set(second);
        if new_second {
            self.update_accessible();
        }
        if changed && !imp.line.dragging.get() {
            self.queue_draw();
        }
    }

    /// When the bar next changes while playing: its next device pixel, or
    /// the next second of its times.
    pub fn next_change(&self, position_us: i64) -> Option<i64> {
        let step = self.step_us(self.imp().duration_us.get());
        let bar = schedule::next_bar_change(position_us, step, false);
        if self.times() == Times::None {
            bar
        } else {
            schedule::earliest([bar, Some(next_second_us(position_us))])
        }
    }

    fn duration_ms(&self) -> u64 {
        self.imp()
            .duration_us
            .get()
            .map_or(0, |d| u64::try_from(d.max(0) / 1_000).unwrap_or(0))
    }

    /// The position shown: the dragged one while dragging.
    fn shown_ms(&self) -> u64 {
        let imp = self.imp();
        if imp.line.dragging.get() {
            let f = self.geometry().line.fraction_at(imp.drag_x.get());
            (f * self.duration_ms() as f64).round() as u64
        } else {
            format::us_to_ms(imp.position_us.get())
        }
    }

    fn shown_fraction(&self) -> f64 {
        let imp = self.imp();
        if imp.line.dragging.get() {
            self.geometry().line.fraction_at(imp.drag_x.get())
        } else {
            seek::fraction(imp.position_us.get(), imp.duration_us.get())
        }
    }

    fn update_accessible(&self) {
        let pos_ms = self.shown_ms();
        let dur = self.duration_ms();
        let text = trf(
            "{position} of {duration}",
            &[
                ("position", &format::clock(pos_ms)),
                ("duration", &format::clock(dur)),
            ],
        );
        self.update_property(&[
            gtk::accessible::Property::ValueMin(0.0),
            gtk::accessible::Property::ValueMax(dur as f64 / 1_000.0),
            gtk::accessible::Property::ValueNow(pos_ms as f64 / 1_000.0),
            gtk::accessible::Property::ValueText(&text),
        ]);
    }

    fn seek_to(&self, ms: u64) {
        if let Some(f) = self.imp().on_seek.borrow().as_ref() {
            f(ms);
        }
    }

    fn setup_controllers(&self) {
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to = bar)]
            self,
            move |g, x, _| {
                if bar.duration_ms() == 0 {
                    return;
                }
                let imp = bar.imp();
                imp.line.input.set(input_of(g));
                imp.line.dragging.set(true);
                imp.drag_start.set(x);
                imp.drag_x.set(x);
                bar.add_css_class("dragging");
                bar.grab_focus();
                retarget(&bar);
                bar.update_accessible();
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[weak(rename_to = bar)]
            self,
            move |_, dx, _| {
                let imp = bar.imp();
                if imp.line.dragging.get() {
                    imp.drag_x.set(imp.drag_start.get() + dx);
                    bar.queue_draw();
                }
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak(rename_to = bar)]
            self,
            move |_, dx, _| {
                let imp = bar.imp();
                if !imp.line.dragging.get() {
                    return;
                }
                imp.drag_x.set(imp.drag_start.get() + dx);
                let ms = bar.shown_ms();
                imp.line.dragging.set(false);
                bar.remove_css_class("dragging");
                // Show the new position at once; the core's anchor follows.
                imp.position_us.set(i64::try_from(ms).unwrap_or(0) * 1_000);
                imp.second.set(-1);
                retarget(&bar);
                bar.queue_draw();
                bar.seek_to(ms);
            }
        ));
        drag.connect_cancel(glib::clone!(
            #[weak(rename_to = bar)]
            self,
            move |_, _| {
                bar.imp().line.dragging.set(false);
                bar.remove_css_class("dragging");
                retarget(&bar);
            }
        ));
        self.add_controller(drag);
        track_hover_and_focus(self);

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = bar)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                let dur = bar.duration_ms();
                if dur == 0 || !modifiers.is_empty() {
                    return glib::Propagation::Proceed;
                }
                let Some(k) = slider_key(key, false) else {
                    return glib::Propagation::Proceed;
                };
                let pos = format::us_to_ms(bar.imp().position_us.get());
                bar.seek_to(seek::seek_key(k, pos, dur));
                glib::Propagation::Stop
            }
        ));
        self.add_controller(keys);
    }

    /// A time label's layout, rebuilt only when its text changes.
    fn label(&self, index: usize, text: String) -> pango::Layout {
        let mut labels = self.imp().labels.borrow_mut();
        let slot = &mut labels[index];
        if slot.0 != text || slot.1.is_none() {
            slot.1 = Some(self.layout_for(&text));
            slot.0 = text;
        }
        slot.1.clone().unwrap_or_else(|| self.layout_for(""))
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        let geometry = self.geometry();
        let (thickness, thumb) = imp.line.shown();
        let has_length = self.duration_ms() > 0;
        let paint = Paint::of(self);
        paint_line(
            snapshot,
            &geometry.line,
            self.shown_fraction(),
            thickness,
            if has_length { thumb } else { 0.0 },
            &paint,
        );
        if self.times() == Times::None {
            return;
        }
        // The times: elapsed at the start, the time left at the end, in
        // ink-70; while dragging they follow the thumb.
        let ms = self.shown_ms();
        let dur = self.duration_ms();
        let elapsed = self.label(0, format::clock(ms));
        let remaining = self.label(1, format::remaining(ms, dur));
        let ink = style::style().rgba("ink.ink-70");
        let (ex, ey) = geometry.elapsed;
        snapshot.save();
        snapshot.translate(&graphene::Point::new(ex as f32, ey as f32));
        snapshot.append_layout(&elapsed, &ink);
        snapshot.restore();
        let (rx, ry) = geometry.remaining;
        let rw = f64::from(remaining.pixel_size().0);
        snapshot.save();
        snapshot.translate(&graphene::Point::new((rx - rw) as f32, ry as f32));
        snapshot.append_layout(&remaining, &ink);
        snapshot.restore();
    }
}

// ---------------------------------------------------------------------------
// The volume slider

mod vol {
    use super::*;

    #[derive(Default)]
    pub struct VolumeBar {
        pub line: LineState,
        pub natural: Cell<i32>,
        pub value: Cell<f64>,
        pub drag_x: Cell<f64>,
        pub drag_start: Cell<f64>,
        pub on_change: RefCell<Option<VolumeFn>>,
    }

    impl std::fmt::Debug for VolumeBar {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("VolumeBar").finish_non_exhaustive()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VolumeBar {
        const NAME: &'static str = "JfVolumeBar";
        type Type = super::VolumeBar;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("volumebar");
            klass.set_accessible_role(gtk::AccessibleRole::Slider);
        }
    }

    impl ObjectImpl for VolumeBar {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_focusable(true);
            obj.set_hexpand(true);
            obj.set_cursor_from_name(Some("pointer"));
            obj.setup_controllers();
            obj.update_accessible();
        }
    }

    impl WidgetImpl for VolumeBar {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => {
                    let natural = match self.natural.get() {
                        0 => 200,
                        n => n.max(64),
                    };
                    (64, natural, -1, -1)
                }
                _ => {
                    let h = seek::seek_height(Times::None, 0.0).ceil() as i32;
                    (h, h, -1, -1)
                }
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let line = obj.line();
            let (thickness, thumb) = self.line.shown();
            let paint = Paint::of(&*obj);
            let thumb = if obj.is_sensitive() { thumb } else { 0.0 };
            paint_line(snapshot, &line, self.value.get(), thickness, thumb, &paint);
        }
    }

    impl HasLine for super::VolumeBar {
        fn line_state(&self) -> &LineState {
            &self.imp().line
        }
    }
}

glib::wrapper! {
    /// The volume: the same line between two speaker glyphs.
    pub struct VolumeBar(ObjectSubclass<vol::VolumeBar>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for VolumeBar {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl VolumeBar {
    pub fn new() -> VolumeBar {
        Self::default()
    }

    /// Called with the new volume (0 to 1) while the person changes it.
    pub fn connect_changed(&self, f: impl Fn(f64) + 'static) {
        *self.imp().on_change.borrow_mut() = Some(Box::new(f));
    }

    pub fn value(&self) -> f64 {
        self.imp().value.get()
    }

    /// The width the line asks for when it has room (200 px by default).
    pub fn set_natural_width(&self, width: i32) {
        if self.imp().natural.replace(width) != width {
            self.queue_resize();
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.imp().line.dragging.get()
    }

    /// Shows a volume the core reported (ignored while dragging).
    pub fn set_value(&self, value: f64) {
        let v = value.clamp(0.0, 1.0);
        if self.is_dragging() || (self.imp().value.get() - v).abs() < 1e-4 {
            return;
        }
        self.imp().value.set(v);
        self.update_accessible();
        self.queue_draw();
    }

    fn line(&self) -> Line {
        seek::seek_layout(
            f64::from(self.width()),
            f64::from(self.height()),
            Times::None,
            0.0,
            0.0,
        )
        .line
    }

    fn update_accessible(&self) {
        let percent = seek::percent(self.imp().value.get());
        let text = trf("{percent} %", &[("percent", &percent.to_string())]);
        self.update_property(&[
            gtk::accessible::Property::ValueMin(0.0),
            gtk::accessible::Property::ValueMax(100.0),
            gtk::accessible::Property::ValueNow(f64::from(percent)),
            gtk::accessible::Property::ValueText(&text),
        ]);
    }

    /// Sets the volume from the person's input and reports it.
    fn change(&self, value: f64) {
        let v = value.clamp(0.0, 1.0);
        self.imp().value.set(v);
        self.update_accessible();
        self.queue_draw();
        if let Some(f) = self.imp().on_change.borrow().as_ref() {
            f(v);
        }
    }

    fn setup_controllers(&self) {
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to = bar)]
            self,
            move |g, x, _| {
                if !bar.is_sensitive() {
                    return;
                }
                let imp = bar.imp();
                imp.line.input.set(input_of(g));
                imp.line.dragging.set(true);
                imp.drag_start.set(x);
                imp.drag_x.set(x);
                bar.grab_focus();
                retarget(&bar);
                bar.change(bar.line().fraction_at(x));
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[weak(rename_to = bar)]
            self,
            move |_, dx, _| {
                let imp = bar.imp();
                if imp.line.dragging.get() {
                    imp.drag_x.set(imp.drag_start.get() + dx);
                    bar.change(bar.line().fraction_at(imp.drag_x.get()));
                }
            }
        ));
        let end = glib::clone!(
            #[weak(rename_to = bar)]
            self,
            move || {
                let imp = bar.imp();
                if imp.line.dragging.replace(false) {
                    retarget(&bar);
                    bar.change(imp.value.get());
                }
            }
        );
        let end2 = end.clone();
        drag.connect_drag_end(move |_, _, _| end());
        drag.connect_cancel(move |_, _| end2());
        self.add_controller(drag);
        track_hover_and_focus(self);

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = bar)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                if !bar.is_sensitive() || !modifiers.is_empty() {
                    return glib::Propagation::Proceed;
                }
                let Some(k) = slider_key(key, true) else {
                    return glib::Propagation::Proceed;
                };
                bar.change(seek::volume_key(k, bar.value()));
                glib::Propagation::Stop
            }
        ));
        self.add_controller(keys);
        self.connect_sensitive_notify(|b| b.queue_draw());
    }
}

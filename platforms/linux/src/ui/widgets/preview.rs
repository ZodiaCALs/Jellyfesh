//! Appearance's pictures and pickers (design-controls.md, "Preferences →
//! Appearance"): a round colour dot for the accent swatches, the custom hue
//! slider on a hue-gradient track, and small previews of the theme (System,
//! Light, Dark), of the Now Playing background (Artwork, Misty) and of glass
//! at a transparency. Colours come from the Misty theme at the hue shown.

use std::cell::{Cell, RefCell};

use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, prelude::*};

use crate::logic::appearance::{slider_hue, wrap_hue};
use crate::logic::hue::color_at;
use crate::logic::seek::SliderKey;
use crate::logic::theme::Colors;
use crate::ui::i18n::trf;
use crate::ui::state::try_app;
use crate::ui::style;

fn rgba([r, g, b, a]: [u8; 4]) -> gdk::RGBA {
    gdk::RGBA::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        f32::from(a) / 255.0,
    )
}

fn rounded(rect: graphene::Rect, radius: f32) -> gsk::RoundedRect {
    let r = graphene::Size::new(radius, radius);
    gsk::RoundedRect::new(rect, r, r, r, r)
}

fn fill(snapshot: &gtk::Snapshot, rect: graphene::Rect, radius: f32, color: &gdk::RGBA) {
    snapshot.push_rounded_clip(&rounded(rect, radius));
    snapshot.append_color(color, &rect);
    snapshot.pop();
}

/// Whether the app shows its dark appearance now.
fn dark_now() -> bool {
    adw::StyleManager::default().is_dark()
}

// ---------------------------------------------------------------------------
// A colour dot

mod dot {
    use super::*;

    #[derive(Debug, Default)]
    pub struct ColorDot {
        pub hue: Cell<f32>,
        pub size: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ColorDot {
        const NAME: &'static str = "JfColorDot";
        type Type = super::ColorDot;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("colordot");
            klass.set_accessible_role(gtk::AccessibleRole::Presentation);
        }
    }

    impl ObjectImpl for ColorDot {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_can_target(false);
        }
    }

    impl WidgetImpl for ColorDot {
        fn measure(&self, _o: gtk::Orientation, _f: i32) -> (i32, i32, i32, i32) {
            let s = self.size.get();
            (s, s, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let w = self.obj();
            let (width, height) = (w.width() as f32, w.height() as f32);
            let side = width.min(height);
            let rect = graphene::Rect::new((width - side) / 2.0, (height - side) / 2.0, side, side);
            let color = color_at("accent.accent", self.hue.get(), dark_now()).unwrap_or([0, 0, 0, 255]);
            fill(snapshot, rect, side / 2.0, &rgba(color));
        }
    }
}

glib::wrapper! {
    /// A round dot in the accent colour of a hue.
    pub struct ColorDot(ObjectSubclass<dot::ColorDot>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ColorDot {
    pub fn new(hue: f32, size: i32) -> ColorDot {
        let d: ColorDot = glib::Object::new();
        d.imp().hue.set(hue);
        d.imp().size.set(size);
        d
    }

    pub fn set_hue(&self, hue: f32) {
        if self.imp().hue.replace(hue) != hue {
            self.queue_draw();
        }
    }
}

// ---------------------------------------------------------------------------
// The custom hue slider

type HueFn = Box<dyn Fn(f32)>;

mod hue {
    use super::*;

    #[derive(Default)]
    pub struct HueSlider {
        pub hue: Cell<f32>,
        pub dragging: Cell<bool>,
        pub on_change: RefCell<Option<HueFn>>,
    }

    impl std::fmt::Debug for HueSlider {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("HueSlider").finish_non_exhaustive()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HueSlider {
        const NAME: &'static str = "JfHueSlider";
        type Type = super::HueSlider;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("hueslider");
            klass.set_accessible_role(gtk::AccessibleRole::Slider);
        }
    }

    impl ObjectImpl for HueSlider {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_focusable(true);
            obj.set_hexpand(true);
            obj.set_cursor_from_name(Some("pointer"));
            obj.setup();
        }
    }

    impl WidgetImpl for HueSlider {
        fn measure(&self, o: gtk::Orientation, _f: i32) -> (i32, i32, i32, i32) {
            match o {
                gtk::Orientation::Horizontal => (120, 240, -1, -1),
                _ => (32, 32, -1, -1),
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    /// A slider over the hue circle, 0 to 360°, on a track of accent colours.
    pub struct HueSlider(ObjectSubclass<hue::HueSlider>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for HueSlider {
    fn default() -> Self {
        glib::Object::new()
    }
}

/// The track's thickness and the thumb's diameter.
const HUE_TRACK: f32 = 12.0;
const HUE_THUMB: f32 = 22.0;

impl HueSlider {
    pub fn new() -> HueSlider {
        Self::default()
    }

    /// Called with the hue while the person moves the slider.
    pub fn connect_changed(&self, f: impl Fn(f32) + 'static) {
        *self.imp().on_change.borrow_mut() = Some(Box::new(f));
    }

    pub fn hue(&self) -> f32 {
        self.imp().hue.get()
    }

    pub fn is_dragging(&self) -> bool {
        self.imp().dragging.get()
    }

    /// Shows a hue from the settings (ignored while dragging).
    pub fn set_hue(&self, hue: f32) {
        if self.is_dragging() {
            return;
        }
        let h = wrap_hue(f64::from(hue));
        if self.imp().hue.replace(h) != h {
            self.update_accessible();
            self.queue_draw();
        }
    }

    fn span(&self) -> (f32, f32) {
        let inset = HUE_THUMB / 2.0 + 1.0;
        (inset, (self.width() as f32 - inset).max(inset))
    }

    fn hue_at(&self, x: f64) -> f32 {
        let (x0, x1) = self.span();
        let f = ((x as f32 - x0) / (x1 - x0).max(1.0)).clamp(0.0, 1.0);
        slider_hue(f64::from(f) * 360.0)
    }

    fn change(&self, hue: f32) {
        let h = wrap_hue(f64::from(hue));
        self.imp().hue.set(h);
        self.update_accessible();
        self.queue_draw();
        if let Some(f) = self.imp().on_change.borrow().as_ref() {
            f(h);
        }
    }

    fn update_accessible(&self) {
        let hue = self.imp().hue.get().round();
        let text = trf("{degrees}°", &[("degrees", &format!("{hue}"))]);
        self.update_property(&[
            gtk::accessible::Property::ValueMin(0.0),
            gtk::accessible::Property::ValueMax(359.0),
            gtk::accessible::Property::ValueNow(f64::from(hue)),
            gtk::accessible::Property::ValueText(&text),
        ]);
    }

    fn setup(&self) {
        let drag = gtk::GestureDrag::new();
        let start = std::rc::Rc::new(Cell::new(0.0));
        let s = start.clone();
        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to = slider)]
            self,
            move |_, x, _| {
                s.set(x);
                slider.imp().dragging.set(true);
                slider.grab_focus();
                slider.change(slider.hue_at(x));
            }
        ));
        let s = start.clone();
        drag.connect_drag_update(glib::clone!(
            #[weak(rename_to = slider)]
            self,
            move |_, dx, _| slider.change(slider.hue_at(s.get() + dx))
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak(rename_to = slider)]
            self,
            move |_, _, _| {
                slider.imp().dragging.set(false);
                slider.change(slider.hue());
            }
        ));
        self.add_controller(drag);
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = slider)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                let k = match key {
                    gdk::Key::Left | gdk::Key::Down => SliderKey::Back,
                    gdk::Key::Right | gdk::Key::Up => SliderKey::Forward,
                    gdk::Key::Page_Down => SliderKey::PageBack,
                    gdk::Key::Page_Up => SliderKey::PageForward,
                    gdk::Key::Home => SliderKey::Home,
                    gdk::Key::End => SliderKey::End,
                    _ => return glib::Propagation::Proceed,
                };
                let h = f64::from(slider.hue());
                let next = match k {
                    SliderKey::Back => h - 1.0,
                    SliderKey::Forward => h + 1.0,
                    SliderKey::PageBack => h - 15.0,
                    SliderKey::PageForward => h + 15.0,
                    SliderKey::Home => 0.0,
                    SliderKey::End => 359.0,
                };
                slider.change(slider_hue(next));
                glib::Propagation::Stop
            }
        ));
        self.add_controller(keys);
        self.update_accessible();
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let (x0, x1) = self.span();
        let cy = self.height() as f32 / 2.0;
        let dark = dark_now();
        // The accent around the hue circle, every 30°.
        let stops: Vec<gsk::ColorStop> = (0..=12)
            .map(|i| {
                let h = i as f32 * 30.0;
                let c = color_at("accent.accent", h % 360.0, dark).unwrap_or([0, 0, 0, 255]);
                gsk::ColorStop::new(i as f32 / 12.0, rgba(c))
            })
            .collect();
        let track = graphene::Rect::new(
            x0 - HUE_TRACK / 2.0,
            cy - HUE_TRACK / 2.0,
            x1 - x0 + HUE_TRACK,
            HUE_TRACK,
        );
        snapshot.push_rounded_clip(&rounded(track, HUE_TRACK / 2.0));
        snapshot.append_linear_gradient(
            &track,
            &graphene::Point::new(x0, cy),
            &graphene::Point::new(x1, cy),
            &stops,
        );
        snapshot.pop();
        let hue = self.imp().hue.get();
        let x = x0 + (x1 - x0) * hue / 360.0;
        let rect = graphene::Rect::new(x - HUE_THUMB / 2.0, cy - HUE_THUMB / 2.0, HUE_THUMB, HUE_THUMB);
        let disc = rounded(rect, HUE_THUMB / 2.0);
        let s = style::style();
        snapshot.append_outset_shadow(&disc, &s.rgba("elevation.shadow-1.1"), 0.0, 1.0, 0.0, 3.0);
        let c = color_at("accent.accent", hue, dark).unwrap_or([0, 0, 0, 255]);
        fill(snapshot, rect, HUE_THUMB / 2.0, &rgba(c));
        let ring = s.rgba("glass.cloud");
        snapshot.append_border(&disc, &[3.0; 4], &[ring; 4]);
        if self.has_focus() && self.has_visible_focus() {
            let focus = s.rgba("accent.accent-text");
            let outer = rounded(
                graphene::Rect::new(rect.x() - 3.0, rect.y() - 3.0, HUE_THUMB + 6.0, HUE_THUMB + 6.0),
                HUE_THUMB / 2.0 + 3.0,
            );
            snapshot.append_border(&outer, &[2.0; 4], &[focus; 4]);
        }
    }
}

// ---------------------------------------------------------------------------
// Previews

/// What a preview pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Picture {
    /// The app in its light (false) or dark (true) appearance, or half and
    /// half for "System" (`None`).
    #[default]
    SystemTheme,
    Theme(bool),
    /// Now Playing over the artwork's backdrop.
    Artwork,
    /// Now Playing over the Misty field.
    Misty,
    /// A glass panel over colour, at the preview's transparency.
    Glass,
}

mod pv {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Preview {
        pub picture: Cell<Picture>,
        pub transparency: Cell<f32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Preview {
        const NAME: &'static str = "JfPreview";
        type Type = super::Preview;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("preview");
            klass.set_accessible_role(gtk::AccessibleRole::Presentation);
        }
    }

    impl ObjectImpl for Preview {
        fn constructed(&self) {
            self.parent_constructed();
            self.transparency.set(0.5);
            self.obj().set_can_target(false);
        }
    }

    impl WidgetImpl for Preview {
        fn measure(&self, o: gtk::Orientation, _f: i32) -> (i32, i32, i32, i32) {
            match o {
                gtk::Orientation::Horizontal => (96, 112, -1, -1),
                _ => (64, 70, -1, -1),
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    /// A small picture of an Appearance choice.
    pub struct Preview(ObjectSubclass<pv::Preview>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// The theme colours (with the glass at `transparency`) for an appearance.
fn colors(dark: bool, transparency: Option<f32>) -> Colors {
    let Some(app) = try_app() else {
        return Colors::new();
    };
    match transparency {
        Some(t) => style::theme_for(&app, dark).with_transparency(t).colors,
        None => style::colors_for(&app, dark),
    }
}

fn pick(colors: &Colors, name: &str) -> gdk::RGBA {
    rgba(colors.get(name).copied().unwrap_or([128, 128, 128, 255]))
}

impl Preview {
    pub fn new(picture: Picture) -> Preview {
        let p: Preview = glib::Object::new();
        p.imp().picture.set(picture);
        p
    }

    /// The transparency a glass preview shows (follows its slider at once).
    pub fn set_transparency(&self, t: f32) {
        if self.imp().transparency.replace(t) != t {
            self.queue_draw();
        }
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let (w, h) = (self.width() as f32, self.height() as f32);
        let frame = graphene::Rect::new(0.0, 0.0, w, h);
        snapshot.push_rounded_clip(&rounded(frame, 10.0));
        match self.imp().picture.get() {
            Picture::SystemTheme => {
                let half = graphene::Rect::new(0.0, 0.0, w / 2.0, h);
                snapshot.push_clip(&half);
                self.app_window(snapshot, &colors(false, None), w, h);
                snapshot.pop();
                let other = graphene::Rect::new(w / 2.0, 0.0, w / 2.0, h);
                snapshot.push_clip(&other);
                self.app_window(snapshot, &colors(true, None), w, h);
                snapshot.pop();
            }
            Picture::Theme(dark) => self.app_window(snapshot, &colors(dark, None), w, h),
            Picture::Artwork | Picture::Misty => self.now_playing(snapshot, w, h),
            Picture::Glass => self.glass(snapshot, w, h),
        }
        snapshot.pop();
        // A hairline around the picture.
        let edge = pick(&colors(dark_now(), None), "line.line");
        snapshot.append_border(&rounded(frame, 10.0), &[1.0; 4], &[edge; 4]);
    }

    /// A tiny library: sidebar, a grid of covers and the floating player.
    fn app_window(&self, snapshot: &gtk::Snapshot, c: &Colors, w: f32, h: f32) {
        snapshot.append_color(&pick(c, "field.sky"), &graphene::Rect::new(0.0, 0.0, w, h));
        snapshot.append_color(
            &pick(c, "glass.cloud-soft"),
            &graphene::Rect::new(0.0, 0.0, w * 0.26, h),
        );
        let ink = pick(c, "ink.ink-10");
        for i in 0..3 {
            let y = 10.0 + i as f32 * 9.0;
            fill(
                snapshot,
                graphene::Rect::new(6.0, y, w * 0.26 - 12.0, 4.0),
                2.0,
                &ink,
            );
        }
        let accent = pick(c, "accent.accent-soft");
        fill(
            snapshot,
            graphene::Rect::new(4.0, 8.0, w * 0.26 - 8.0, 8.0),
            3.0,
            &accent,
        );
        let cover = w * 0.16;
        for i in 0..3 {
            let x = w * 0.32 + i as f32 * (cover + 5.0);
            fill(
                snapshot,
                graphene::Rect::new(x, 10.0, cover, cover),
                3.0,
                &pick(c, "ink.ink-10"),
            );
        }
        let bar = graphene::Rect::new(w * 0.32, h - 20.0, w * 0.62, 13.0);
        fill(snapshot, bar, 6.5, &pick(c, "glass.cloud"));
        fill(
            snapshot,
            graphene::Rect::new(bar.x() + bar.width() - 11.0, bar.y() + 3.0, 7.0, 7.0),
            3.5,
            &pick(c, "accent.accent"),
        );
    }

    /// Now Playing in the chosen background style.
    fn now_playing(&self, snapshot: &gtk::Snapshot, w: f32, h: f32) {
        let c = colors(dark_now(), None);
        let full = graphene::Rect::new(0.0, 0.0, w, h);
        let art = graphene::Rect::new(10.0, (h - 40.0) / 2.0, 40.0, 40.0);
        if self.imp().picture.get() == Picture::Artwork {
            // The artwork's colours, deep and blurred.
            let stops = [
                gsk::ColorStop::new(0.0, pick(&c, "accent.accent")),
                gsk::ColorStop::new(1.0, pick(&c, "accent.accent-press")),
            ];
            snapshot.append_linear_gradient(
                &full,
                &graphene::Point::new(0.0, 0.0),
                &graphene::Point::new(w, h),
                &stops,
            );
            let veil = pick(&c, "elevation.scrim");
            snapshot.append_color(&veil, &full);
        } else {
            snapshot.append_color(&pick(&c, "field.sky"), &full);
            for (name, x, y, r) in [
                ("field.aurora-1", 0.1, 0.1, 0.55),
                ("field.aurora-2", 0.92, 0.9, 0.5),
                ("field.aurora-3", 0.05, 0.95, 0.45),
            ] {
                let color = pick(&c, name);
                let (cx, cy, rad) = (w * x, h * y, w * r);
                let stops = [
                    gsk::ColorStop::new(0.0, color),
                    gsk::ColorStop::new(1.0, gdk::RGBA::new(color.red(), color.green(), color.blue(), 0.0)),
                ];
                snapshot.append_radial_gradient(
                    &full,
                    &graphene::Point::new(cx, cy),
                    rad,
                    rad * 0.8,
                    0.0,
                    1.0,
                    &stops,
                );
            }
            let frame = graphene::Rect::new(art.x() - 4.0, art.y() - 4.0, 48.0, 48.0);
            fill(snapshot, frame, 9.0, &pick(&c, "glass.tint-clear"));
        }
        let cover = [
            gsk::ColorStop::new(0.0, pick(&c, "accent.accent-hover")),
            gsk::ColorStop::new(1.0, pick(&c, "accent.accent")),
        ];
        snapshot.push_rounded_clip(&rounded(art, 6.0));
        snapshot.append_linear_gradient(
            &art,
            &graphene::Point::new(art.x(), art.y()),
            &graphene::Point::new(art.x() + 40.0, art.y() + 40.0),
            &cover,
        );
        snapshot.pop();
        let ink = if self.imp().picture.get() == Picture::Artwork {
            gdk::RGBA::new(1.0, 1.0, 1.0, 0.85)
        } else {
            pick(&c, "ink.ink-70")
        };
        for (i, width) in [0.36, 0.28, 0.32].iter().enumerate() {
            let y = h * 0.3 + i as f32 * 10.0;
            fill(snapshot, graphene::Rect::new(60.0, y, w * width, 5.0), 2.5, &ink);
        }
    }

    /// Glass over colour: how much shows through at the transparency.
    fn glass(&self, snapshot: &gtk::Snapshot, w: f32, h: f32) {
        let dark = dark_now();
        let t = self.imp().transparency.get();
        let c = colors(dark, Some(t));
        let full = graphene::Rect::new(0.0, 0.0, w, h);
        snapshot.append_color(&pick(&c, "field.sky"), &full);
        // Stripes of accent and aurora under the glass.
        for (i, name) in [
            "accent.accent",
            "field.aurora-1",
            "accent.accent-hover",
            "field.aurora-2",
        ]
        .iter()
        .enumerate()
        {
            let x = i as f32 * w / 4.0;
            snapshot.append_color(&pick(&c, name), &graphene::Rect::new(x, 0.0, w / 4.0, h));
        }
        let panel = graphene::Rect::new(10.0, h * 0.28, w - 20.0, h * 0.44);
        fill(snapshot, panel, 8.0, &pick(&c, "glass.tint-veil"));
        let edge = pick(&c, "glass.glass-edge");
        snapshot.append_border(&rounded(panel, 8.0), &[1.0; 4], &[edge; 4]);
        fill(
            snapshot,
            graphene::Rect::new(
                panel.x() + 10.0,
                panel.y() + panel.height() / 2.0 - 3.0,
                w * 0.4,
                6.0,
            ),
            3.0,
            &pick(&c, "ink.ink-70"),
        );
    }
}

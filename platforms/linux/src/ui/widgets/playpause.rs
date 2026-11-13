//! The play/pause glyph that morphs between its shapes (design-controls.md,
//! "Transport": `dur-2` Spring). Each shape is two quadrilaterals: the play
//! triangle cut down its middle, and the two pause bars; the morph moves
//! every corner from one to the other. They are filled and stroked with
//! round joins in the widget's CSS colour, so their corners are as soft as
//! the other Misty glyphs'. With reduced motion the glyph changes at once.

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, graphene, gsk};

use crate::logic::motion;
use crate::ui::style;

type Quad = [(f32, f32); 4];

/// The play triangle in a 24-unit box, as two quads (the right one has a
/// doubled tip), inset by half the rounding stroke.
const PLAY: [Quad; 2] = [
    [(8.8, 6.4), (13.7, 9.2), (13.7, 14.8), (8.8, 17.6)],
    [(13.7, 9.2), (18.6, 12.0), (18.6, 12.0), (13.7, 14.8)],
];
/// The pause bars, inset the same way.
const PAUSE: [Quad; 2] = [
    [(7.3, 5.8), (9.3, 5.8), (9.3, 18.2), (7.3, 18.2)],
    [(14.7, 5.8), (16.7, 5.8), (16.7, 18.2), (14.7, 18.2)],
];
/// The rounding stroke, in glyph units.
const ROUND: f32 = 1.6;

/// The shape at `t`: 0 is play, 1 is pause; a spring may overshoot a bit.
fn shape(t: f32) -> [Quad; 2] {
    let t = t.clamp(-0.1, 1.15);
    let mut out = PLAY;
    for (q, quad) in out.iter_mut().enumerate() {
        for (i, (x, y)) in quad.iter_mut().enumerate() {
            let (px, py) = PLAY[q][i];
            let (sx, sy) = PAUSE[q][i];
            *x = px + (sx - px) * t;
            *y = py + (sy - py) * t;
        }
    }
    out
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct PlayPause {
        pub size: Cell<i32>,
        pub playing: Cell<bool>,
        /// The morph shown: 0 play, 1 pause.
        pub shown: Cell<f64>,
        pub from: Cell<f64>,
        pub animation: RefCell<Option<adw::TimedAnimation>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlayPause {
        const NAME: &'static str = "JfPlayPause";
        type Type = super::PlayPause;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("glyph");
            klass.set_accessible_role(gtk::AccessibleRole::Presentation);
        }
    }

    impl ObjectImpl for PlayPause {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_can_target(false);
        }
    }

    impl WidgetImpl for PlayPause {
        fn measure(&self, _o: gtk::Orientation, _f: i32) -> (i32, i32, i32, i32) {
            let s = self.size.get();
            (s, s, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let size = self.size.get() as f32;
            let scale = size / 24.0;
            let builder = gsk::PathBuilder::new();
            for quad in shape(self.shown.get() as f32) {
                builder.move_to(quad[0].0, quad[0].1);
                for &(x, y) in &quad[1..] {
                    builder.line_to(x, y);
                }
                builder.close();
            }
            let path = builder.to_path();
            let color = widget.color();
            snapshot.save();
            snapshot.translate(&graphene::Point::new((w - size) / 2.0, (h - size) / 2.0));
            snapshot.scale(scale, scale);
            snapshot.append_fill(&path, gsk::FillRule::Winding, &color);
            let stroke = gsk::Stroke::new(ROUND);
            stroke.set_line_join(gsk::LineJoin::Round);
            snapshot.append_stroke(&path, &stroke, &color);
            snapshot.restore();
        }
    }
}

glib::wrapper! {
    /// A play glyph that morphs into pause.
    pub struct PlayPause(ObjectSubclass<imp::PlayPause>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PlayPause {
    pub fn new(size: i32) -> PlayPause {
        let g: PlayPause = glib::Object::new();
        g.imp().size.set(size);
        g
    }

    pub fn is_playing(&self) -> bool {
        self.imp().playing.get()
    }

    /// Shows pause while playing and play while paused, morphing when the
    /// widget is on screen and motion is allowed.
    pub fn set_playing(&self, playing: bool) {
        let imp = self.imp();
        let target = if playing { 1.0 } else { 0.0 };
        let settled = (imp.shown.get() - target).abs() < f64::EPSILON;
        if imp.playing.get() == playing && (settled || imp.animation.borrow().is_some()) {
            return;
        }
        imp.playing.set(playing);
        if let Some(a) = imp.animation.borrow_mut().take() {
            a.pause();
        }
        let plan = motion::morph_plan(style::allowed());
        if !plan.animates() || !self.is_mapped() {
            imp.shown.set(target);
            self.queue_draw();
            return;
        }
        imp.from.set(imp.shown.get());
        let weak = self.downgrade();
        let callback = adw::CallbackAnimationTarget::new(move |t| {
            let Some(g) = weak.upgrade() else { return };
            let imp = g.imp();
            let to = if imp.playing.get() { 1.0 } else { 0.0 };
            let from = imp.from.get();
            imp.shown.set(from + (to - from) * plan.ease(t));
            g.queue_draw();
        });
        let anim = adw::TimedAnimation::new(self, 0.0, 1.0, plan.duration_ms, callback);
        anim.set_easing(adw::Easing::Linear);
        let weak = self.downgrade();
        anim.connect_done(move |_| {
            if let Some(g) = weak.upgrade() {
                g.imp().animation.borrow_mut().take();
            }
        });
        anim.play();
        *imp.animation.borrow_mut() = Some(anim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_morph_ends_on_each_shape() {
        assert_eq!(shape(0.0), PLAY);
        let pause = shape(1.0);
        for q in 0..2 {
            for i in 0..4 {
                assert!((pause[q][i].0 - PAUSE[q][i].0).abs() < 1e-5);
                assert!((pause[q][i].1 - PAUSE[q][i].1).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn shapes_stay_in_the_glyph_box() {
        for t in [-1.0, 0.0, 0.5, 1.0, 1.1, 3.0] {
            for quad in shape(t) {
                for (x, y) in quad {
                    assert!((ROUND / 2.0..=24.0 - ROUND / 2.0).contains(&x), "{t}: {x}");
                    assert!((ROUND / 2.0..=24.0 - ROUND / 2.0).contains(&y), "{t}: {y}");
                }
            }
        }
    }
}

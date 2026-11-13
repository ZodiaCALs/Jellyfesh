//! The mini-player's progress (design-controls.md, "Mini-player and the
//! desktop player bar"): a straight 2 px line along the capsule's bottom
//! edge, inset by its corner radius, instead of v0.0.3's ring. It covers
//! the capsule (laid over it, never taking clicks) and redraws only when
//! its fill moves by a device pixel.

use std::cell::Cell;

use gtk::subclass::prelude::*;
use gtk::{glib, graphene, gsk, prelude::*};

use crate::logic::{layout, schedule, seek};
use crate::ui::style;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct ProgressLine {
        pub fraction: Cell<f64>,
        pub duration_us: Cell<Option<i64>>,
        pub step_index: Cell<i64>,
        pub radius: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProgressLine {
        const NAME: &'static str = "JfProgressLine";
        type Type = super::ProgressLine;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("progressline");
            klass.set_accessible_role(gtk::AccessibleRole::Presentation);
        }
    }

    impl ObjectImpl for ProgressLine {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_can_target(false);
            self.step_index.set(-1);
        }
    }

    impl WidgetImpl for ProgressLine {
        fn measure(&self, _o: gtk::Orientation, _f: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    /// A progress line along the bottom edge of the widget it covers.
    pub struct ProgressLine(ObjectSubclass<imp::ProgressLine>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ProgressLine {
    /// A line for a surface with `corner_radius` px corners.
    pub fn new(corner_radius: f64) -> ProgressLine {
        let line: ProgressLine = glib::Object::new();
        line.imp().radius.set(corner_radius);
        line
    }

    fn line(&self) -> seek::Line {
        layout::mini_line(
            f64::from(self.width()),
            f64::from(self.height()),
            self.imp().radius.get(),
        )
    }

    /// The time one device pixel of the line stands for.
    fn step_us(&self, duration_us: Option<i64>) -> Option<i64> {
        schedule::bar_step_device_us(duration_us, self.line().length(), f64::from(self.scale_factor()))
    }

    /// Shows a position; redraws only when the fill moved by a device pixel.
    pub fn set_position(&self, position_us: i64, duration_us: Option<i64>) {
        let imp = self.imp();
        let step = self.step_us(duration_us).unwrap_or(i64::MAX);
        let index = schedule::bar_step_index(position_us, step);
        if index != imp.step_index.get() || imp.duration_us.get() != duration_us {
            imp.step_index.set(index);
            imp.duration_us.set(duration_us);
            imp.fraction.set(seek::fraction(position_us, duration_us));
            self.queue_draw();
        }
    }

    /// When the line next moves by a device pixel while playing.
    pub fn next_change(&self, position_us: i64) -> Option<i64> {
        let step = self.step_us(self.imp().duration_us.get());
        schedule::next_bar_change(position_us, step, false)
    }

    pub fn fraction(&self) -> f64 {
        self.imp().fraction.get()
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let line = self.line();
        if line.length() <= 0.0 {
            return;
        }
        let thickness = f64::from(jf_tokens::player::MINI_PROGRESS) as f32;
        // Quiet on the capsule: the played part in its CSS colour (ink-70)
        // over ink-10.
        let (played, rest) = (self.color(), style::style().rgba("ink.ink-10"));
        let (x0, x1, y) = (line.x0 as f32, line.x1 as f32, line.y as f32);
        let r = graphene::Size::new(thickness / 2.0, thickness / 2.0);
        let track = graphene::Rect::new(x0, y - thickness / 2.0, x1 - x0, thickness);
        snapshot.push_rounded_clip(&gsk::RoundedRect::new(track, r, r, r, r));
        snapshot.append_color(&rest, &track);
        let xp = line.x_at(self.imp().fraction.get()) as f32;
        if xp > x0 {
            let fill = graphene::Rect::new(x0, y - thickness / 2.0, xp - x0, thickness);
            snapshot.push_rounded_clip(&gsk::RoundedRect::new(fill, r, r, r, r));
            snapshot.append_color(&played, &fill);
            snapshot.pop();
        }
        snapshot.pop();
    }
}

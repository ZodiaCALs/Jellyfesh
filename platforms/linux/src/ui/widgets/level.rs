//! The level glyph on the playing row (design-controls.md, "Album page"):
//! three bars in the accent that move only while playing. Paused, hidden,
//! or with reduced motion they stand still; while they move they follow
//! the frame clock (only while the row is on screen).

use std::cell::{Cell, RefCell};

use gtk::subclass::prelude::*;
use gtk::{glib, graphene, gsk, prelude::*};

use crate::logic::library::{LEVEL_STILL, level_bars};
use crate::logic::motion;
use crate::ui::style;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Level {
        pub size: Cell<i32>,
        pub playing: Cell<bool>,
        pub bars: Cell<[f64; 3]>,
        pub tick: RefCell<Option<gtk::TickCallbackId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Level {
        const NAME: &'static str = "JfLevel";
        type Type = super::Level;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("level");
            klass.set_accessible_role(gtk::AccessibleRole::Presentation);
        }
    }

    impl ObjectImpl for Level {
        fn constructed(&self) {
            self.parent_constructed();
            self.bars.set(LEVEL_STILL);
            let obj = self.obj();
            obj.set_can_target(false);
            obj.connect_map(|l| l.update());
            obj.connect_unmap(|l| l.stop());
        }
    }

    impl WidgetImpl for Level {
        fn measure(&self, _o: gtk::Orientation, _f: i32) -> (i32, i32, i32, i32) {
            let s = self.size.get();
            (s, s, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let size = self.size.get() as f32;
            // Three bars in a square, a bar's width apart.
            let bar = size / 5.0;
            let (left, bottom) = ((w - size) / 2.0, (h + size) / 2.0);
            let color = widget.color();
            for (i, level) in self.bars.get().iter().enumerate() {
                let height = (size * *level as f32).max(bar);
                let rect = graphene::Rect::new(left + bar * 2.0 * i as f32, bottom - height, bar, height);
                let r = graphene::Size::new(bar / 2.0, bar / 2.0);
                snapshot.push_rounded_clip(&gsk::RoundedRect::new(rect, r, r, r, r));
                snapshot.append_color(&color, &rect);
                snapshot.pop();
            }
        }
    }
}

glib::wrapper! {
    /// Three moving bars for the row that is playing.
    pub struct Level(ObjectSubclass<imp::Level>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Level {
    pub fn new(size: i32) -> Level {
        let l: Level = glib::Object::new();
        l.imp().size.set(size);
        l
    }

    /// Moves while `playing` (and motion is allowed), stands still otherwise.
    pub fn set_playing(&self, playing: bool) {
        if self.imp().playing.replace(playing) != playing {
            self.update();
        }
    }

    /// Starts or stops the bars for the state, the screen and the settings.
    pub fn update(&self) {
        let moving = motion::level_moves(self.imp().playing.get(), style::allowed()) && self.is_mapped();
        if !moving {
            self.stop();
            return;
        }
        if self.imp().tick.borrow().is_some() {
            return;
        }
        let id = self.add_tick_callback(|l, clock| {
            let t = clock.frame_time() as f64 / 1_000_000.0;
            l.imp().bars.set(level_bars(t));
            l.queue_draw();
            glib::ControlFlow::Continue
        });
        *self.imp().tick.borrow_mut() = Some(id);
    }

    fn stop(&self) {
        if let Some(id) = self.imp().tick.borrow_mut().take() {
            id.remove();
        }
        if self.imp().bars.replace(LEVEL_STILL) != LEVEL_STILL {
            self.queue_draw();
        }
    }
}

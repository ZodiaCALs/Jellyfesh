//! A bin whose child fades out over bands at its top and bottom edges
//! (Luminous: the pages fade out just above the floating player, and the
//! lyrics fade at their ends). The middle draws as is; only the bands go
//! through a mask (GSK mask nodes, GTK 4.10 and later), and the child's
//! render node is reused for both parts, so a frame costs one offscreen of
//! the band's size.

use std::cell::Cell;

use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, prelude::*};

use crate::logic::layout::fade_bands;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct FadeBin {
        pub child: std::cell::RefCell<Option<gtk::Widget>>,
        pub top: Cell<f64>,
        pub bottom: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FadeBin {
        const NAME: &'static str = "JfFadeBin";
        type Type = super::FadeBin;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("fadebin");
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for FadeBin {
        fn dispose(&self) {
            if let Some(child) = self.child.borrow_mut().take() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for FadeBin {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let Some(child) = self.child.borrow().clone() else {
                return;
            };
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let (top, bottom) = fade_bands(f64::from(h), self.top.get(), self.bottom.get());
            let (top, bottom) = (top as f32, bottom as f32);
            if w <= 0.0 || h <= 0.0 || (top <= 0.0 && bottom <= 0.0) {
                widget.snapshot_child(&child, snapshot);
                return;
            }
            // The opaque middle.
            snapshot.push_clip(&graphene::Rect::new(0.0, top, w, h - top - bottom));
            widget.snapshot_child(&child, snapshot);
            snapshot.pop();
            // Each band: the child, masked by a gradient to transparent at
            // the edge.
            let solid = gdk::RGBA::new(0.0, 0.0, 0.0, 1.0);
            let clear = gdk::RGBA::new(0.0, 0.0, 0.0, 0.0);
            for (y, height, from, to) in [(0.0, top, clear, solid), (h - bottom, bottom, solid, clear)] {
                if height <= 0.0 {
                    continue;
                }
                let band = graphene::Rect::new(0.0, y, w, height);
                snapshot.push_clip(&band);
                snapshot.push_mask(gsk::MaskMode::Alpha);
                snapshot.append_linear_gradient(
                    &band,
                    &graphene::Point::new(0.0, y),
                    &graphene::Point::new(0.0, y + height),
                    &[gsk::ColorStop::new(0.0, from), gsk::ColorStop::new(1.0, to)],
                );
                snapshot.pop();
                widget.snapshot_child(&child, snapshot);
                snapshot.pop();
                snapshot.pop();
            }
        }
    }
}

glib::wrapper! {
    /// A bin that fades its child out at its top and bottom edges.
    pub struct FadeBin(ObjectSubclass<imp::FadeBin>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl FadeBin {
    pub fn new(child: &impl IsA<gtk::Widget>) -> FadeBin {
        let bin: FadeBin = glib::Object::new();
        child.set_parent(&bin);
        *bin.imp().child.borrow_mut() = Some(child.clone().upcast());
        bin
    }

    /// The fade bands at the top and bottom, in px (0: none).
    pub fn set_fades(&self, top: f64, bottom: f64) {
        let imp = self.imp();
        if imp.top.replace(top) != top || imp.bottom.replace(bottom) != bottom {
            self.queue_draw();
        }
    }
}

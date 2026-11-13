//! A bin that moves and scales its child without laying it out again: the
//! child is allocated once at its size, and each frame of an animation only
//! changes the transform it is allocated with (GTK keeps its layout). Pages
//! rise as they appear, Now Playing slides, and the artwork breathes
//! through this.

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, graphene, gsk};

use crate::logic::motion::Plan;

mod imp {
    use super::*;

    #[derive(Debug)]
    pub struct MotionBin {
        pub child: RefCell<Option<gtk::Widget>>,
        pub offset_y: Cell<f64>,
        pub scale: Cell<f64>,
        pub animation: RefCell<Option<adw::TimedAnimation>>,
    }

    impl Default for MotionBin {
        fn default() -> Self {
            MotionBin {
                child: RefCell::new(None),
                offset_y: Cell::new(0.0),
                scale: Cell::new(1.0),
                animation: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MotionBin {
        const NAME: &'static str = "JfMotionBin";
        type Type = super::MotionBin;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("motionbin");
        }
    }

    impl ObjectImpl for MotionBin {
        fn dispose(&self) {
            if let Some(a) = self.animation.borrow_mut().take() {
                a.pause();
            }
            if let Some(child) = self.child.borrow_mut().take() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for MotionBin {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            self.child
                .borrow()
                .as_ref()
                .map_or(gtk::SizeRequestMode::ConstantSize, |c| c.request_mode())
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match self.child.borrow().as_ref() {
                Some(c) if c.should_layout() => c.measure(orientation, for_size),
                _ => (0, 0, -1, -1),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(child) = self.child.borrow().clone() else {
                return;
            };
            if !child.should_layout() {
                return;
            }
            let (dy, s) = (self.offset_y.get() as f32, self.scale.get() as f32);
            let transform = if dy == 0.0 && (s - 1.0).abs() < 1e-6 {
                None
            } else {
                // Scale about the centre, then move.
                let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
                Some(
                    gsk::Transform::new()
                        .translate(&graphene::Point::new(cx, cy + dy))
                        .scale(s, s)
                        .translate(&graphene::Point::new(-cx, -cy)),
                )
            };
            child.allocate(width, height, baseline, transform);
        }
    }
}

glib::wrapper! {
    /// Moves and scales one child by transform only.
    pub struct MotionBin(ObjectSubclass<imp::MotionBin>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl MotionBin {
    pub fn new(child: &impl IsA<gtk::Widget>) -> MotionBin {
        let bin: MotionBin = glib::Object::new();
        child.set_parent(&bin);
        *bin.imp().child.borrow_mut() = Some(child.clone().upcast());
        bin
    }

    pub fn child(&self) -> Option<gtk::Widget> {
        self.imp().child.borrow().clone()
    }

    /// Moves the child down by `offset_y` px and scales it by `scale`.
    pub fn set_transform(&self, offset_y: f64, scale: f64) {
        let imp = self.imp();
        if imp.offset_y.get() != offset_y || imp.scale.get() != scale {
            imp.offset_y.set(offset_y);
            imp.scale.set(scale);
            self.queue_allocate();
        }
    }

    pub fn scale(&self) -> f64 {
        self.imp().scale.get()
    }

    /// Stops any running animation where it is.
    pub fn stop(&self) {
        if let Some(a) = self.imp().animation.borrow_mut().take() {
            a.pause();
        }
    }

    /// Runs `step(eased progress)` over `plan`, then `done`. Without a
    /// duration (or unmapped) it jumps to the end.
    pub fn animate(
        &self,
        plan: Plan,
        step: impl Fn(&MotionBin, f64) + 'static,
        done: impl FnOnce(&MotionBin) + 'static,
    ) {
        self.stop();
        if !plan.animates() || !self.is_mapped() {
            step(self, 1.0);
            done(self);
            return;
        }
        let weak = self.downgrade();
        let callback = adw::CallbackAnimationTarget::new(move |t| {
            if let Some(bin) = weak.upgrade() {
                step(&bin, plan.ease(t));
            }
        });
        let anim = adw::TimedAnimation::new(self, 0.0, 1.0, plan.duration_ms, callback);
        anim.set_easing(adw::Easing::Linear);
        let done = RefCell::new(Some(done));
        let weak = self.downgrade();
        anim.connect_done(move |_| {
            if let Some(bin) = weak.upgrade() {
                bin.imp().animation.borrow_mut().take();
                if let Some(f) = done.borrow_mut().take() {
                    f(&bin);
                }
            }
        });
        *self.imp().animation.borrow_mut() = Some(anim.clone());
        anim.play();
    }

    /// Fades in and rises from `plan.distance` px below (a page appearing),
    /// then runs `done`. Interrupting a leave, it starts from where that
    /// left off.
    pub fn enter(&self, plan: Plan, done: impl FnOnce(&MotionBin) + 'static) {
        let interrupted = self.imp().animation.borrow().is_some();
        let start = if interrupted { self.opacity() } else { 0.0 };
        let distance = plan.distance * (1.0 - start);
        let scale_from = plan.scale_from + (1.0 - plan.scale_from) * start;
        self.set_opacity(start);
        self.set_transform(distance, scale_from);
        self.animate(
            plan,
            move |bin, e| {
                bin.set_opacity((start + (1.0 - start) * e).clamp(0.0, 1.0));
                bin.set_transform(distance * (1.0 - e), scale_from + (1.0 - scale_from) * e);
            },
            move |bin| {
                bin.set_opacity(1.0);
                bin.set_transform(0.0, 1.0);
                done(bin);
            },
        );
    }

    /// Fades out and sinks by `plan.distance` px, then runs `done`.
    pub fn leave(&self, plan: Plan, done: impl FnOnce(&MotionBin) + 'static) {
        let distance = plan.distance;
        let start = self.opacity();
        self.animate(
            plan,
            move |bin, e| {
                bin.set_opacity((start * (1.0 - e)).clamp(0.0, 1.0));
                bin.set_transform(distance * e, 1.0);
            },
            move |bin| {
                bin.set_opacity(0.0);
                bin.set_transform(0.0, 1.0);
                done(bin);
            },
        );
    }
}

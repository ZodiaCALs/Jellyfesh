//! A large page title over a list or grid that scrolls with it
//! (design-controls.md, "Large titles"), without giving up the list's
//! recycling. GTK 4.14's grid view has no header, and a list in a plain box
//! would build every row; so this widget is itself scrollable: it puts the
//! title above its scrollable child and maps the scrolled window's
//! adjustment onto the child's own, which starts once the title has
//! scrolled away. Every scroll step only moves the two by transform.

use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use gtk::subclass::prelude::*;
use gtk::{glib, graphene, gsk, prelude::*};

mod imp {
    use super::*;

    #[derive(Debug)]
    pub struct TitledScroll {
        pub title: RefCell<Option<gtk::Widget>>,
        pub child: RefCell<Option<gtk::Widget>>,
        /// The scrolled window's vertical adjustment, and the child's own.
        pub outer: RefCell<Option<gtk::Adjustment>>,
        pub outer_handler: RefCell<Option<glib::SignalHandlerId>>,
        pub inner: gtk::Adjustment,
        pub hadjustment: RefCell<Option<gtk::Adjustment>>,
        pub hpolicy: Cell<gtk::ScrollablePolicy>,
        pub vpolicy: Cell<gtk::ScrollablePolicy>,
        pub title_height: Cell<f64>,
        /// Set while this widget changes an adjustment itself.
        pub guard: Cell<bool>,
    }

    impl Default for TitledScroll {
        fn default() -> Self {
            TitledScroll {
                title: RefCell::new(None),
                child: RefCell::new(None),
                outer: RefCell::new(None),
                outer_handler: RefCell::new(None),
                inner: gtk::Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                hadjustment: RefCell::new(None),
                hpolicy: Cell::new(gtk::ScrollablePolicy::Minimum),
                vpolicy: Cell::new(gtk::ScrollablePolicy::Minimum),
                title_height: Cell::new(0.0),
                guard: Cell::new(false),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TitledScroll {
        const NAME: &'static str = "JfTitledScroll";
        type Type = super::TitledScroll;
        type ParentType = gtk::Widget;
        type Interfaces = (gtk::Scrollable,);

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("titledscroll");
        }
    }

    impl ObjectImpl for TitledScroll {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPS: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPS.get_or_init(|| {
                vec![
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("hadjustment"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("vadjustment"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("hscroll-policy"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("vscroll-policy"),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "hadjustment" => {
                    let adj = value.get::<Option<gtk::Adjustment>>().ok().flatten();
                    if let Some(child) = self
                        .child
                        .borrow()
                        .as_ref()
                        .and_then(|c| c.dynamic_cast_ref::<gtk::Scrollable>())
                    {
                        child.set_hadjustment(adj.as_ref());
                    }
                    *self.hadjustment.borrow_mut() = adj;
                }
                "vadjustment" => {
                    let adj = value.get::<Option<gtk::Adjustment>>().ok().flatten();
                    self.obj().set_outer(adj);
                }
                "hscroll-policy" => {
                    if let Ok(p) = value.get() {
                        self.hpolicy.set(p);
                    }
                }
                "vscroll-policy" => {
                    if let Ok(p) = value.get() {
                        self.vpolicy.set(p);
                    }
                }
                _ => {}
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "hadjustment" => self.hadjustment.borrow().to_value(),
                "vadjustment" => self.outer.borrow().to_value(),
                "hscroll-policy" => self.hpolicy.get().to_value(),
                _ => self.vpolicy.get().to_value(),
            }
        }

        fn dispose(&self) {
            if let (Some(adj), Some(id)) = (
                self.outer.borrow_mut().take(),
                self.outer_handler.borrow_mut().take(),
            ) {
                adj.disconnect(id);
            }
            if let Some(t) = self.title.borrow_mut().take() {
                t.unparent();
            }
            if let Some(c) = self.child.borrow_mut().take() {
                c.unparent();
            }
        }
    }

    impl WidgetImpl for TitledScroll {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let (mut min, mut nat) = (0, 0);
            for w in [self.title.borrow().clone(), self.child.borrow().clone()]
                .into_iter()
                .flatten()
            {
                let (m, n, _, _) = w.measure(orientation, for_size);
                if orientation == gtk::Orientation::Horizontal {
                    min = min.max(m);
                    nat = nat.max(n);
                } else {
                    min += m;
                    nat += n;
                }
            }
            (min, nat, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let (Some(title), Some(child)) = (self.title.borrow().clone(), self.child.borrow().clone())
            else {
                return;
            };
            let title_h = title.measure(gtk::Orientation::Vertical, width).1.max(0);
            self.title_height.set(f64::from(title_h));
            let outer = self.outer.borrow().clone();
            let v = outer.as_ref().map_or(0.0, |a| a.value()).max(0.0);
            // The child scrolls only once the title is gone.
            self.guard.set(true);
            self.inner.set_value((v - f64::from(title_h)).max(0.0));
            self.guard.set(false);
            title.allocate(
                width,
                title_h,
                -1,
                Some(gsk::Transform::new().translate(&graphene::Point::new(0.0, -v as f32))),
            );
            let child_y = (f64::from(title_h) - v).max(0.0) as f32;
            child.allocate(
                width,
                height,
                -1,
                Some(gsk::Transform::new().translate(&graphene::Point::new(0.0, child_y))),
            );
            if let Some(outer) = outer {
                let upper = f64::from(title_h) + self.inner.upper();
                let page = f64::from(height);
                let value = v.clamp(0.0, (upper - page).max(0.0));
                self.guard.set(true);
                outer.configure(
                    value,
                    0.0,
                    upper.max(page),
                    self.inner.step_increment().max(1.0),
                    page * 0.9,
                    page,
                );
                self.guard.set(false);
            }
        }
    }

    impl ScrollableImpl for TitledScroll {}
}

glib::wrapper! {
    /// A title that scrolls away above a recycling list or grid.
    pub struct TitledScroll(ObjectSubclass<imp::TitledScroll>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
}

impl TitledScroll {
    /// `child` must be scrollable (a list view, a grid view or a clamp
    /// around one).
    pub fn new(
        title: &impl IsA<gtk::Widget>,
        child: &(impl IsA<gtk::Scrollable> + IsA<gtk::Widget>),
    ) -> TitledScroll {
        let w: TitledScroll = glib::Object::new();
        w.set_overflow(gtk::Overflow::Hidden);
        let imp = w.imp();
        title.set_parent(&w);
        child.set_parent(&w);
        child.set_vadjustment(Some(&imp.inner));
        *imp.title.borrow_mut() = Some(title.clone().upcast());
        *imp.child.borrow_mut() = Some(child.clone().upcast::<gtk::Widget>());
        // The child scrolled itself (to show a focused row): follow it.
        let weak = w.downgrade();
        imp.inner.connect_value_changed(move |inner| {
            let Some(w) = weak.upgrade() else { return };
            let imp = w.imp();
            if imp.guard.get() {
                return;
            }
            if let Some(outer) = imp.outer.borrow().as_ref() {
                outer.set_value(inner.value() + imp.title_height.get());
            }
        });
        w
    }

    fn set_outer(&self, adj: Option<gtk::Adjustment>) {
        let imp = self.imp();
        if let (Some(old), Some(id)) = (
            imp.outer.borrow_mut().take(),
            imp.outer_handler.borrow_mut().take(),
        ) {
            old.disconnect(id);
        }
        if let Some(a) = &adj {
            let weak = self.downgrade();
            let id = a.connect_value_changed(move |_| {
                if let Some(w) = weak.upgrade()
                    && !w.imp().guard.get()
                {
                    w.queue_allocate();
                }
            });
            *imp.outer_handler.borrow_mut() = Some(id);
        }
        *imp.outer.borrow_mut() = adj;
        self.queue_allocate();
    }

    /// The title's height as last laid out.
    pub fn title_height(&self) -> f64 {
        self.imp().title_height.get()
    }
}

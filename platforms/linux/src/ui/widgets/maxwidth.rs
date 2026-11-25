//! A column at most so wide, lined up with the start of the space it is
//! given (Jellyfesh Web's `max-width` blocks: Settings, the Search field,
//! the Sources page, at `layout.reading`). `AdwClamp` centres its child;
//! this keeps it at the start, as the web does.

use std::cell::Cell;

use gtk::subclass::prelude::*;
use gtk::{glib, prelude::*};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct MaxWidth {
        pub child: std::cell::RefCell<Option<gtk::Widget>>,
        pub maximum: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MaxWidth {
        const NAME: &'static str = "JfMaxWidth";
        type Type = super::MaxWidth;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("maxwidth");
        }
    }

    impl ObjectImpl for MaxWidth {
        fn dispose(&self) {
            if let Some(child) = self.child.borrow_mut().take() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for MaxWidth {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.child.borrow().clone() else {
                return (0, 0, -1, -1);
            };
            let maximum = self.maximum.get();
            match orientation {
                gtk::Orientation::Horizontal => {
                    let (min, nat, _, _) = child.measure(orientation, -1);
                    (min, nat.min(maximum).max(min), -1, -1)
                }
                _ => {
                    let width = if for_size < 0 { -1 } else { for_size.min(maximum) };
                    let width = if width >= 0 {
                        let (min, _, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
                        width.max(min)
                    } else {
                        width
                    };
                    child.measure(orientation, width)
                }
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(child) = self.child.borrow().clone() else {
                return;
            };
            let (min, _, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
            let w = width.min(self.maximum.get()).max(min);
            let x = if self.obj().direction() == gtk::TextDirection::Rtl {
                width - w
            } else {
                0
            };
            child.size_allocate(&gtk::Allocation::new(x, 0, w, height), baseline);
        }
    }
}

glib::wrapper! {
    pub struct MaxWidth(ObjectSubclass<imp::MaxWidth>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl MaxWidth {
    /// `child` at most `maximum` px wide, at the start.
    pub fn new(child: &impl IsA<gtk::Widget>, maximum: i32) -> MaxWidth {
        let w: MaxWidth = glib::Object::new();
        w.imp().maximum.set(maximum);
        child.as_ref().set_parent(&w);
        *w.imp().child.borrow_mut() = Some(child.clone().upcast());
        w
    }
}

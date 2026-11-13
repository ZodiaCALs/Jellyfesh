//! A QR code for sign-in flows, drawn natively (dark modules on white with
//! a quiet zone, whatever the theme, so phones can read it).

use std::cell::RefCell;

use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, prelude::*};

const QUIET: usize = 4;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Qr {
        pub modules: RefCell<Vec<bool>>,
        pub width: RefCell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Qr {
        const NAME: &'static str = "JfQr";
        type Type = super::Qr;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("qrcode");
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for Qr {}

    impl WidgetImpl for Qr {
        fn measure(&self, _o: gtk::Orientation, _f: i32) -> (i32, i32, i32, i32) {
            (200, 240, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let n = *self.width.borrow();
            let side = obj.width().min(obj.height()) as f32;
            if n == 0 || side <= 0.0 {
                return;
            }
            let total = (n + 2 * QUIET) as f32;
            let cell = (side / total).floor().max(1.0);
            let drawn = cell * total;
            let x0 = (obj.width() as f32 - drawn) / 2.0;
            let y0 = (obj.height() as f32 - drawn) / 2.0;
            snapshot.append_color(
                &gdk::RGBA::new(1.0, 1.0, 1.0, 1.0),
                &graphene::Rect::new(x0, y0, drawn, drawn),
            );
            let dark = gdk::RGBA::new(0.0, 0.0, 0.0, 1.0);
            let modules = self.modules.borrow();
            for (i, on) in modules.iter().enumerate() {
                if *on {
                    let (x, y) = ((i % n + QUIET) as f32, (i / n + QUIET) as f32);
                    snapshot.append_color(
                        &dark,
                        &graphene::Rect::new(x0 + x * cell, y0 + y * cell, cell, cell),
                    );
                }
            }
        }
    }
}

glib::wrapper! {
    /// A QR code.
    pub struct Qr(ObjectSubclass<imp::Qr>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Qr {
    pub fn new(data: &str) -> Qr {
        let q: Qr = glib::Object::new();
        q.set_data(data);
        q
    }

    pub fn set_data(&self, data: &str) {
        let (modules, width) = match qrcode::QrCode::new(data.as_bytes()) {
            Ok(code) => (
                code.to_colors()
                    .into_iter()
                    .map(|c| c == qrcode::Color::Dark)
                    .collect(),
                code.width(),
            ),
            Err(_) => (Vec::new(), 0),
        };
        *self.imp().modules.borrow_mut() = modules;
        *self.imp().width.borrow_mut() = width;
        self.queue_draw();
    }
}

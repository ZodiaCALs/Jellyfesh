//! Square artwork with rounded corners, loaded off the main thread.
//!
//! Without artwork (or until it is decoded) the widget shows its CSS
//! background with a quiet note glyph, so a grid of covers never jumps.

use std::cell::{Cell, RefCell};

use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, prelude::*};

use crate::ui::state::try_app;
use crate::ui::thumbs::{ArtKey, size_for};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Art {
        pub texture: RefCell<Option<gdk::Texture>>,
        pub key: RefCell<Option<ArtKey>>,
        pub radius: Cell<f32>,
        pub size: Cell<i32>,
        pub min: Cell<i32>,
        pub round: Cell<bool>,
        /// Grows past its size with the space it gets (grid cards) or not.
        pub fixed: Cell<bool>,
        pub generation: Cell<u64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Art {
        const NAME: &'static str = "JfArt";
        type Type = super::Art;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("art");
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for Art {}

    impl WidgetImpl for Art {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let size = self.size.get();
            match orientation {
                gtk::Orientation::Horizontal => (self.min.get().min(size), size, -1, -1),
                _ => {
                    let side = match (for_size > 0, self.fixed.get()) {
                        (true, true) => for_size.min(size),
                        (true, false) => for_size,
                        (false, _) => size,
                    };
                    (side, side, -1, -1)
                }
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            if w <= 0.0 || h <= 0.0 {
                return;
            }
            let side = w.min(h);
            let bounds = graphene::Rect::new((w - side) / 2.0, (h - side) / 2.0, side, side);
            let radius = if self.round.get() {
                side / 2.0
            } else {
                // The look's corners (ADR-0020); 1 in Luminous.
                let scale = crate::ui::state::try_app()
                    .map_or(1.0, |a| crate::logic::look::corner_scale(&a.look.borrow()));
                (f64::from(self.radius.get()) * scale) as f32
            };
            let corner = graphene::Size::new(radius, radius);
            let rounded = gsk::RoundedRect::new(bounds, corner, corner, corner, corner);
            if let Some(texture) = self.texture.borrow().as_ref() {
                snapshot.push_rounded_clip(&rounded);
                // Cover: fill the square, crop the longer side.
                let (tw, th) = (texture.width() as f32, texture.height() as f32);
                let scale = (side / tw).max(side / th);
                let (dw, dh) = (tw * scale, th * scale);
                let rect = graphene::Rect::new(
                    bounds.x() + (side - dw) / 2.0,
                    bounds.y() + (side - dh) / 2.0,
                    dw,
                    dh,
                );
                snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Trilinear, &rect);
                snapshot.pop();
            } else {
                // A placeholder note in the CSS colour, at a third of the side.
                let glyph = side / 3.0;
                if glyph >= 8.0 {
                    let Ok(path) = gsk::Path::parse(
                        "M9 17.5V6l10-2v11.5 M9 17.5a2.5 2.5 0 1 1-5 0a2.5 2.5 0 1 1 5 0z \
                         M19 15.5a2.5 2.5 0 1 1-5 0a2.5 2.5 0 1 1 5 0z",
                    ) else {
                        return;
                    };
                    snapshot.save();
                    snapshot.translate(&graphene::Point::new(
                        bounds.x() + (side - glyph) / 2.0,
                        bounds.y() + (side - glyph) / 2.0,
                    ));
                    let s = glyph / 24.0;
                    snapshot.scale(s, s);
                    let stroke = gsk::Stroke::new(1.5);
                    stroke.set_line_cap(gsk::LineCap::Round);
                    stroke.set_line_join(gsk::LineJoin::Round);
                    snapshot.append_stroke(&path, &stroke, &widget.color());
                    snapshot.restore();
                }
            }
        }
    }
}

glib::wrapper! {
    /// Artwork in a rounded square.
    pub struct Art(ObjectSubclass<imp::Art>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Art {
    /// Artwork of natural `size` px with corner `radius` (artwork tokens,
    /// times the look's corner scale).
    pub fn new(size: i32, radius: f32) -> Art {
        let a: Art = glib::Object::new();
        a.imp().size.set(size);
        a.imp().min.set(size.min(48));
        a.imp().radius.set(radius);
        a.add_css_class("art");
        ARTS.with(|arts| arts.borrow_mut().push(a.downgrade()));
        a
    }

    /// A circle instead of a rounded square (mini-player, artists).
    pub fn set_round(&self, round: bool) {
        self.imp().round.set(round);
        self.queue_draw();
    }

    /// Keeps the artwork at its size even when its parent offers more.
    pub fn set_fixed(&self, fixed: bool) {
        self.imp().fixed.set(fixed);
        self.queue_resize();
    }

    /// The narrowest the artwork gets; grid views size their columns by it.
    pub fn set_min_size(&self, min: i32) {
        self.imp().min.set(min);
        self.queue_resize();
    }

    pub fn set_size(&self, size: i32) {
        if self.imp().size.get() != size {
            self.imp().size.set(size);
            self.queue_resize();
        }
    }

    /// Shows artwork by key, loading it at the size the widget needs.
    pub fn set_art(&self, key: Option<ArtKey>) {
        let imp = self.imp();
        if *imp.key.borrow() == key && (key.is_none() || imp.texture.borrow().is_some()) {
            return;
        }
        let generation = imp.generation.get() + 1;
        imp.generation.set(generation);
        *imp.key.borrow_mut() = key.clone();
        let Some(key) = key else {
            self.set_texture(None);
            return;
        };
        let Some(app) = try_app() else { return };
        let size = size_for(imp.size.get() * self.scale_factor().max(1));
        if let Some(t) = app.thumbs.cached(&key, size) {
            self.set_texture(Some(t));
            return;
        }
        // Keep showing the old picture until the new one is ready only when
        // it is the same artwork; otherwise show the placeholder.
        self.set_texture(None);
        let weak = self.downgrade();
        app.thumbs.load(key, size, move |texture| {
            if let Some(art) = weak.upgrade()
                && art.imp().generation.get() == generation
            {
                art.set_texture(texture.cloned());
            }
        });
    }

    pub fn set_texture(&self, texture: Option<gdk::Texture>) {
        *self.imp().texture.borrow_mut() = texture;
        self.queue_draw();
    }

    pub fn texture(&self) -> Option<gdk::Texture> {
        self.imp().texture.borrow().clone()
    }
}

thread_local! {
    /// Every artwork, so a new look redraws their corners.
    static ARTS: std::cell::RefCell<Vec<glib::WeakRef<Art>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Redraws every artwork (the look's corner scale changed).
pub fn redraw_all() {
    ARTS.with(|arts| {
        arts.borrow_mut().retain(|w| match w.upgrade() {
            Some(a) => {
                a.queue_draw();
                true
            }
            None => false,
        })
    });
}

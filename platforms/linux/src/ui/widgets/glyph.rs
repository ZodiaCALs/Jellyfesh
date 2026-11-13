//! Misty glyphs: 24-unit outline icons with 1.75-unit round strokes (ICO-01),
//! filled only for active states (ICO-03), drawn as GSK paths in the
//! widget's CSS colour so they follow the theme without icon files.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gtk::subclass::prelude::*;
use gtk::{glib, graphene, gsk, prelude::*};

/// Stroked outline or filled shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Stroke,
    Fill,
}

/// Path data per glyph name, in a 24 × 24 box (SVG path syntax). The
/// transport, toggle and top bar glyphs are the design canvas's own.
fn data(name: &str) -> Option<(Mode, &'static str)> {
    use Mode::{Fill, Stroke};
    Some(match name {
        "back" => (Stroke, "M15 5l-7 7 7 7"),
        "forward" => (Stroke, "M9 5l7 7-7 7"),
        "lyrics" => (Stroke, "M5 5h14v10H10l-5 4z M9 9h6"),
        "queue" => (Stroke, "M4 6h16M4 12h16M4 18h10"),
        "speaker" => (
            Stroke,
            "M11 5L6 9H3v6h3l5 4z M16 9a4 4 0 0 1 0 6M19 6a8 8 0 0 1 0 12",
        ),
        "volume-low" => (Stroke, "M11 5L6 9H3v6h3l5 4z"),
        "volume-high" => (
            Stroke,
            "M11 5L6 9H3v6h3l5 4z M15.5 9.5a3.5 3.5 0 0 1 0 5M18.5 6.5a8 8 0 0 1 0 11",
        ),
        "volume-muted" => (Stroke, "M11 5L6 9H3v6h3l5 4z M16 9.5l5 5M21 9.5l-5 5"),
        "shuffle" => (
            Stroke,
            "M3 7h3c1.7 0 3.2.9 4 2.3l3.9 5.4c.8 1.4 2.3 2.3 4 2.3H21 M18 14l3 3-3 3 \
             M3 17h3c1 0 1.9-.3 2.7-.8M15.3 7.8c.8-.5 1.7-.8 2.7-.8H21 M18 4l3 3-3 3",
        ),
        "repeat" => (
            Stroke,
            "M4 12V9a3 3 0 0 1 3-3h12 M16 3l3 3-3 3 M20 12v3a3 3 0 0 1-3 3H5 M8 21l-3-3 3-3",
        ),
        "repeat-one" => (
            Stroke,
            "M4 12V9a3 3 0 0 1 3-3h12 M16 3l3 3-3 3 M20 12v3a3 3 0 0 1-3 3H5 M8 21l-3-3 3-3 \
             M11.2 10.6l1.2-.8v4.4",
        ),
        "previous" => (Stroke, "M6 5v14 M19 6l-10 6 10 6z"),
        "next" => (Stroke, "M18 5v14 M5 6l10 6-10 6z"),
        "play" => (
            Fill,
            "M8 5.6v12.8a1 1 0 0 0 1.5.86l10-6.4a1 1 0 0 0 0-1.72l-10-6.4A1 1 0 0 0 8 5.6z",
        ),
        "pause" => (
            Fill,
            "M7.7 5h1.2a1.2 1.2 0 0 1 1.2 1.2v11.6a1.2 1.2 0 0 1-1.2 1.2H7.7a1.2 1.2 0 0 1-1.2-1.2V6.2A1.2 1.2 0 0 1 7.7 5z \
             M15.1 5h1.2a1.2 1.2 0 0 1 1.2 1.2v11.6a1.2 1.2 0 0 1-1.2 1.2h-1.2a1.2 1.2 0 0 1-1.2-1.2V6.2a1.2 1.2 0 0 1 1.2-1.2z",
        ),
        "home" => (Stroke, "M4 11l8-7 8 7 M6 9.5V20h12V9.5 M10 20v-5h4v5"),
        "album" => (
            Stroke,
            "M12 4a8 8 0 1 1 0 16a8 8 0 1 1 0-16z M12 9.5a2.5 2.5 0 1 1 0 5a2.5 2.5 0 1 1 0-5z",
        ),
        "artist" => (
            Stroke,
            "M12 4a4 4 0 1 1 0 8a4 4 0 1 1 0-8z M4.5 20a7.5 7.5 0 0 1 15 0",
        ),
        "song" => (
            Stroke,
            "M9 17.5V6l10-2v11.5 M9 17.5a2.5 2.5 0 1 1-5 0a2.5 2.5 0 1 1 5 0z \
             M19 15.5a2.5 2.5 0 1 1-5 0a2.5 2.5 0 1 1 5 0z",
        ),
        "playlist" => (
            Stroke,
            "M4 6h12M4 11h12M4 16h6 M17 18.5a2 2 0 1 1-4 0a2 2 0 1 1 4 0z M17 18.5V10l4-1",
        ),
        "heart" => (
            Stroke,
            "M12 19.5s-7-4.3-7-9.6A3.9 3.9 0 0 1 12 7.6a3.9 3.9 0 0 1 7 2.3c0 5.3-7 9.6-7 9.6z",
        ),
        "heart-filled" => (
            Fill,
            "M12 19.5s-7-4.3-7-9.6A3.9 3.9 0 0 1 12 7.6a3.9 3.9 0 0 1 7 2.3c0 5.3-7 9.6-7 9.6z",
        ),
        "search" => (Stroke, "M11 4a7 7 0 1 1 0 14a7 7 0 1 1 0-14z M20 20l-4-4"),
        "sources" => (
            Stroke,
            "M7 18h10a4 4 0 0 0 .5-7.97A6 6 0 0 0 6.1 9.6 4.2 4.2 0 0 0 7 18z",
        ),
        "more" => (
            Fill,
            "M5 12a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0z M10.5 12a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0z \
             M16 12a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0z",
        ),
        "drag" => (
            Fill,
            "M8 7a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0-2.6 0z M13.4 7a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0-2.6 0z \
             M8 12a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0-2.6 0z M13.4 12a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0-2.6 0z \
             M8 17a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0-2.6 0z M13.4 17a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0-2.6 0z",
        ),
        "plus" => (Stroke, "M12 5v14M5 12h14"),
        "close" => (Stroke, "M6 6l12 12M18 6L6 18"),
        "check" => (Stroke, "M5 12.5l4.5 4.5L19 7.5"),
        "folder" => (
            Stroke,
            "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
        ),
        "trash" => (Stroke, "M5 7h14 M10 7V5h4v2 M7 7l1 12h8l1-12"),
        "info" => (Stroke, "M12 4a8 8 0 1 1 0 16a8 8 0 1 1 0-16z M12 11v5 M12 8v.01"),
        "play-next" => (Stroke, "M4 6h10M4 11h10M4 16h6 M17 13v6M14 16h6"),
        "play-last" => (Stroke, "M4 6h16M4 11h16M4 16h6 M17 15v6M14 18h6"),
        "clear" => (Stroke, "M4 6h16M4 12h10M4 18h7 M15.5 14.5l5 5M20.5 14.5l-5 5"),
        "link" => (
            Stroke,
            "M14 5h5v5M19 5l-8 8 M10 6H6a1 1 0 0 0-1 1v11a1 1 0 0 0 1 1h11a1 1 0 0 0 1-1v-4",
        ),
        "refresh" => (Stroke, "M19 12a7 7 0 1 1-2.05-4.95 M19 4v4h-4"),
        _ => return None,
    })
}

thread_local! {
    static PATHS: RefCell<HashMap<String, Option<(Mode, gsk::Path)>>> = RefCell::new(HashMap::new());
}

fn path(name: &str) -> Option<(Mode, gsk::Path)> {
    PATHS.with(|p| {
        p.borrow_mut()
            .entry(name.to_owned())
            .or_insert_with(|| {
                let (mode, d) = data(name)?;
                match gsk::Path::parse(d) {
                    Ok(path) => Some((mode, path)),
                    Err(e) => {
                        tracing::warn!("glyph {name}: {e}");
                        None
                    }
                }
            })
            .clone()
    })
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Glyph {
        pub name: RefCell<String>,
        pub size: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Glyph {
        const NAME: &'static str = "JfGlyph";
        type Type = super::Glyph;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("glyph");
            klass.set_accessible_role(gtk::AccessibleRole::Presentation);
        }
    }

    impl ObjectImpl for Glyph {
        fn constructed(&self) {
            self.parent_constructed();
            self.size.set(20);
            self.obj().set_can_target(false);
        }
    }

    impl WidgetImpl for Glyph {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let s = self.size.get();
            (s, s, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let Some((mode, path)) = path(&self.name.borrow()) else {
                return;
            };
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let size = self.size.get() as f32;
            let scale = size / 24.0;
            snapshot.save();
            snapshot.translate(&graphene::Point::new((w - size) / 2.0, (h - size) / 2.0));
            snapshot.scale(scale, scale);
            let color = widget.color();
            match mode {
                Mode::Stroke => {
                    let stroke = gsk::Stroke::new(1.75);
                    stroke.set_line_cap(gsk::LineCap::Round);
                    stroke.set_line_join(gsk::LineJoin::Round);
                    snapshot.append_stroke(&path, &stroke, &color);
                }
                Mode::Fill => snapshot.append_fill(&path, gsk::FillRule::Winding, &color),
            }
            snapshot.restore();
        }
    }
}

glib::wrapper! {
    /// A Misty glyph drawn in the widget's CSS colour.
    pub struct Glyph(ObjectSubclass<imp::Glyph>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Glyph {
    pub fn new(name: &str, size: i32) -> Glyph {
        let g: Glyph = glib::Object::new();
        g.imp().size.set(size);
        g.set_name_glyph(name);
        g
    }

    /// Changes the glyph; redraws only when it differs.
    pub fn set_name_glyph(&self, name: &str) {
        if *self.imp().name.borrow() != name {
            *self.imp().name.borrow_mut() = name.to_owned();
            self.queue_draw();
        }
    }

    pub fn glyph_name(&self) -> String {
        self.imp().name.borrow().clone()
    }

    pub fn set_pixel_size(&self, size: i32) {
        if self.imp().size.get() != size {
            self.imp().size.set(size);
            self.queue_resize();
        }
    }
}

/// A flat button showing a glyph, with a tooltip and accessible name.
pub fn button(name: &str, size: i32, label: &str) -> gtk::Button {
    let b = gtk::Button::builder()
        .child(&Glyph::new(name, size))
        .tooltip_text(label)
        .build();
    b.add_css_class("flat");
    b.update_property(&[gtk::accessible::Property::Label(label)]);
    b
}

/// The glyph inside a button made by [`button`].
pub fn of(button: &impl IsA<gtk::Widget>) -> Option<Glyph> {
    let child = button.as_ref().first_child()?;
    child.downcast::<Glyph>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_has_data() {
        for name in [
            "back",
            "lyrics",
            "queue",
            "speaker",
            "shuffle",
            "repeat",
            "repeat-one",
            "previous",
            "next",
            "play",
            "pause",
            "home",
            "album",
            "artist",
            "song",
            "playlist",
            "heart",
            "heart-filled",
            "search",
            "sources",
            "more",
            "drag",
            "plus",
            "close",
            "check",
            "folder",
            "trash",
            "info",
            "play-next",
            "play-last",
            "clear",
            "link",
            "refresh",
            "volume-low",
            "volume-high",
            "volume-muted",
            "forward",
        ] {
            assert!(data(name).is_some(), "{name}");
        }
        assert!(data("nope").is_none());
    }
}

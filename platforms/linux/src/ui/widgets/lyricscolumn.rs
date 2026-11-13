//! The column of lyric lines. It stacks its lines like a vertical box, and
//! draws each one with its own transform: an offset (the cascade when the
//! active line changes) and a scale (inactive lines at
//! `lyrics.scale-inactive`, from their start edge or their centre). A frame
//! of the cascade only re-allocates the lines with new transforms; their
//! layout is measured once per width.

use std::cell::{Cell, RefCell};

use gtk::subclass::prelude::*;
use gtk::{glib, graphene, gsk, prelude::*};
use jf_core::LyricsAlign;

use crate::logic::lyrics::scale_origin;

mod imp {
    use super::*;

    #[derive(Debug)]
    pub struct LyricsColumn {
        pub children: RefCell<Vec<gtk::Widget>>,
        /// Per line: offset (px, down) and scale.
        pub motion: RefCell<Vec<(f64, f64)>>,
        /// Per line, from the last allocation: top and height, unmoved.
        pub rects: RefCell<Vec<(f64, f64)>>,
        pub spacing: Cell<i32>,
        pub align: Cell<LyricsAlign>,
    }

    impl Default for LyricsColumn {
        fn default() -> Self {
            LyricsColumn {
                children: RefCell::new(Vec::new()),
                motion: RefCell::new(Vec::new()),
                rects: RefCell::new(Vec::new()),
                spacing: Cell::new(20),
                align: Cell::new(LyricsAlign::Start),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LyricsColumn {
        const NAME: &'static str = "JfLyricsColumn";
        type Type = super::LyricsColumn;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("lyricscolumn");
            klass.set_accessible_role(gtk::AccessibleRole::List);
        }
    }

    impl ObjectImpl for LyricsColumn {
        fn dispose(&self) {
            for child in self.children.borrow_mut().drain(..) {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for LyricsColumn {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let children = self.children.borrow();
            let shown = children.iter().filter(|c| c.should_layout());
            match orientation {
                gtk::Orientation::Horizontal => {
                    let (mut min, mut nat) = (0, 0);
                    for c in shown {
                        let (m, n, _, _) = c.measure(orientation, -1);
                        min = min.max(m);
                        nat = nat.max(n);
                    }
                    (min, nat, -1, -1)
                }
                _ => {
                    let (mut min, mut nat, mut count) = (0, 0, 0);
                    for c in shown {
                        let (m, n, _, _) = c.measure(orientation, for_size);
                        min += m;
                        nat += n;
                        count += 1;
                    }
                    let gaps = self.spacing.get() * (count - 1).max(0);
                    (min + gaps, nat + gaps, -1, -1)
                }
            }
        }

        fn size_allocate(&self, width: i32, _height: i32, _baseline: i32) {
            let children = self.children.borrow();
            let motion = self.motion.borrow();
            let mut rects = self.rects.borrow_mut();
            rects.clear();
            let rtl = self.obj().direction() == gtk::TextDirection::Rtl;
            let ox = scale_origin(self.align.get(), rtl, f64::from(width)) as f32;
            let mut y = 0;
            for (i, child) in children.iter().enumerate() {
                if !child.should_layout() {
                    rects.push((f64::from(y), 0.0));
                    continue;
                }
                let (_, h, _, _) = child.measure(gtk::Orientation::Vertical, width);
                rects.push((f64::from(y), f64::from(h)));
                let (dy, s) = motion.get(i).copied().unwrap_or((0.0, 1.0));
                let (dy, s) = (dy as f32, s as f32);
                let oy = h as f32 / 2.0;
                let transform = gsk::Transform::new()
                    .translate(&graphene::Point::new(ox, y as f32 + dy + oy))
                    .scale(s, s)
                    .translate(&graphene::Point::new(-ox, -oy));
                child.allocate(width, h, -1, Some(transform));
                y += h + self.spacing.get();
            }
        }
    }
}

glib::wrapper! {
    /// A column of lyric lines with per-line offsets and scales.
    pub struct LyricsColumn(ObjectSubclass<imp::LyricsColumn>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for LyricsColumn {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl LyricsColumn {
    pub fn new(spacing: i32) -> LyricsColumn {
        let c = Self::default();
        c.imp().spacing.set(spacing);
        c
    }

    pub fn append(&self, child: &impl IsA<gtk::Widget>) {
        child.set_parent(self);
        self.imp().children.borrow_mut().push(child.clone().upcast());
        self.imp().motion.borrow_mut().push((0.0, 1.0));
        self.queue_resize();
    }

    pub fn clear(&self) {
        for child in self.imp().children.borrow_mut().drain(..) {
            child.unparent();
        }
        self.imp().motion.borrow_mut().clear();
        self.imp().rects.borrow_mut().clear();
        self.queue_resize();
    }

    pub fn len(&self) -> usize {
        self.imp().children.borrow().len()
    }

    /// Where lines scale from: their start edge, or their centre.
    pub fn set_align(&self, align: LyricsAlign) {
        if self.imp().align.get() != align {
            self.imp().align.set(align);
            self.queue_allocate();
        }
    }

    /// Line `index`'s top and height in the column, as laid out (without
    /// its offset or scale).
    pub fn line_rect(&self, index: usize) -> Option<(f64, f64)> {
        self.imp()
            .rects
            .borrow()
            .get(index)
            .copied()
            .filter(|(_, h)| *h > 0.0)
    }

    /// Whether the column has been laid out for its lines.
    pub fn is_laid_out(&self) -> bool {
        self.imp().rects.borrow().len() == self.len() && self.len() > 0
    }

    /// The offset and scale line `index` is drawn with now.
    pub fn motion(&self, index: usize) -> (f64, f64) {
        self.imp()
            .motion
            .borrow()
            .get(index)
            .copied()
            .unwrap_or((0.0, 1.0))
    }

    /// Sets every line's offset and scale, then re-allocates once.
    pub fn set_motion(&self, f: impl Fn(usize) -> (f64, f64)) {
        let mut changed = false;
        {
            let mut motion = self.imp().motion.borrow_mut();
            for (i, m) in motion.iter_mut().enumerate() {
                let next = f(i);
                if *m != next {
                    *m = next;
                    changed = true;
                }
            }
        }
        if changed {
            self.queue_allocate();
        }
    }
}

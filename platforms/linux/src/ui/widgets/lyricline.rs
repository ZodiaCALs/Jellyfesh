//! One lyric line. Inactive and line-timed lines are plain text in their CSS
//! colour; the active word-timed line is drawn twice, the second time in the
//! sung colour clipped to the swept region, so the sweep moves smoothly
//! through each word instead of jumping a letter at a time.

use std::cell::{Cell, RefCell};

use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, pango, prelude::*};

use crate::logic::sweep::{LineText, Span, sung_spans};

/// Line heights from the lyrics tokens: `lyrics.line` (1.18 in Luminous),
/// and `lyrics.line-cjk-line-height` for Han, Kana and Hangul (TYP-08).
const LINE_HEIGHT: f32 = jf_tokens::lyrics::LINE.line_height;
const LINE_HEIGHT_CJK: f32 = jf_tokens::lyrics::LINE_CJK_LINE_HEIGHT;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct LyricLine {
        pub text: RefCell<Option<LineText>>,
        pub layout: RefCell<Option<pango::Layout>>,
        pub width: Cell<i32>,
        pub font: RefCell<Option<pango::FontDescription>>,
        pub spans: RefCell<Vec<Vec<Span>>>,
        pub rows: RefCell<Vec<(f32, f32)>>,
        pub sweep: Cell<Option<(usize, f32)>>,
        pub sung: RefCell<Option<gdk::RGBA>>,
        pub rtl: Cell<bool>,
        pub centred: Cell<bool>,
        /// One row, cut with an ellipsis (the lyrics bar, Desktop Lyrics).
        pub single: Cell<bool>,
    }

    impl std::fmt::Debug for LyricLine {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("LyricLine").finish_non_exhaustive()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LyricLine {
        const NAME: &'static str = "JfLyricLine";
        type Type = super::LyricLine;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("lyricline");
            klass.set_accessible_role(gtk::AccessibleRole::Label);
        }
    }

    impl ObjectImpl for LyricLine {}

    impl WidgetImpl for LyricLine {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let obj = self.obj();
            match orientation {
                gtk::Orientation::Horizontal => {
                    let layout = obj.make_layout(-1);
                    let (w, _) = layout.pixel_size();
                    let min = if self.single.get() { 0 } else { w.min(120) };
                    (min, w, -1, -1)
                }
                _ => {
                    let layout = obj.make_layout(for_size);
                    let (_, h) = layout.pixel_size();
                    (h, h, -1, -1)
                }
            }
        }

        fn size_allocate(&self, width: i32, _height: i32, _baseline: i32) {
            if width != self.width.get() {
                self.width.set(width);
                self.layout.borrow_mut().take();
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    /// A lyric line with a word sweep.
    pub struct LyricLine(ObjectSubclass<imp::LyricLine>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

fn is_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c as u32,
            0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF66..=0xFF9F)
    })
}

impl LyricLine {
    pub fn new(text: LineText) -> LyricLine {
        let l: LyricLine = glib::Object::new();
        l.update_property(&[gtk::accessible::Property::Label(&text.text)]);
        l.imp()
            .rtl
            .set(pango::find_base_dir(&text.text) == pango::Direction::Rtl);
        *l.imp().text.borrow_mut() = Some(text);
        l
    }

    /// Keeps the line on one row, cut with an ellipsis when it does not fit
    /// (the lyrics bar and Desktop Lyrics).
    pub fn set_single_line(&self, single: bool) {
        let imp = self.imp();
        if imp.single.replace(single) != single {
            imp.layout.borrow_mut().take();
            self.queue_resize();
        }
    }

    /// Centres the line (the Lyrics alignment setting) instead of starting
    /// it at the start edge.
    pub fn set_centred(&self, centred: bool) {
        let imp = self.imp();
        if imp.centred.replace(centred) != centred {
            imp.layout.borrow_mut().take();
            self.queue_resize();
        }
    }

    /// The sweep: the current word and its progress, or `None` (unsung).
    pub fn set_sweep(&self, sweep: Option<(usize, f32)>, sung: &gdk::RGBA) {
        let imp = self.imp();
        if imp.sweep.get() != sweep || imp.sung.borrow().as_ref() != Some(sung) {
            imp.sweep.set(sweep);
            *imp.sung.borrow_mut() = Some(*sung);
            self.queue_draw();
        }
    }

    fn make_layout(&self, width: i32) -> pango::Layout {
        let text = self.imp().text.borrow();
        let text = text.as_ref().map_or("", |t| t.text.as_str());
        let layout = self.create_pango_layout(Some(text));
        layout.set_wrap(pango::WrapMode::WordChar);
        if self.imp().single.get() {
            layout.set_ellipsize(pango::EllipsizeMode::End);
        }
        if width > 0 {
            layout.set_width(width * pango::SCALE);
        }
        if self.imp().centred.get() {
            layout.set_alignment(pango::Alignment::Center);
        }
        layout.set_line_spacing(if is_cjk(text) {
            LINE_HEIGHT_CJK
        } else {
            LINE_HEIGHT
        });
        layout
    }

    /// The layout for the allocated width, rebuilt when the width or font
    /// changes, with each word's position measured once.
    fn layout(&self) -> pango::Layout {
        let imp = self.imp();
        let font = self.pango_context().font_description();
        if *imp.font.borrow() != font {
            *imp.font.borrow_mut() = font;
            imp.layout.borrow_mut().take();
        }
        if let Some(l) = imp.layout.borrow().as_ref() {
            return l.clone();
        }
        let layout = self.make_layout(self.width());
        self.measure_words(&layout);
        *imp.layout.borrow_mut() = Some(layout.clone());
        layout
    }

    fn measure_words(&self, layout: &pango::Layout) {
        let imp = self.imp();
        let mut rows: Vec<(f32, f32)> = Vec::new();
        let mut iter = layout.iter();
        loop {
            let (_, logical) = iter.line_extents();
            rows.push((
                logical.y() as f32 / pango::SCALE as f32,
                logical.height() as f32 / pango::SCALE as f32,
            ));
            if !iter.next_line() {
                break;
            }
        }
        let text = imp.text.borrow();
        let mut spans = Vec::new();
        if let Some(t) = text.as_ref() {
            for range in &t.words {
                let mut word: Vec<Span> = Vec::new();
                for (offset, _) in t.text[range.clone()].char_indices() {
                    let index = i32::try_from(range.start + offset).unwrap_or(i32::MAX);
                    let r = layout.index_to_pos(index);
                    let (x, w) = (r.x() as f32, r.width() as f32);
                    let (x0, x1) = if w < 0.0 { (x + w, x) } else { (x, x + w) };
                    let (x0, x1) = (x0 / pango::SCALE as f32, x1 / pango::SCALE as f32);
                    let y = r.y() as f32 / pango::SCALE as f32;
                    let line = rows.iter().rposition(|(ry, _)| *ry <= y + 0.5).unwrap_or(0);
                    match word.iter_mut().find(|s| s.line == line) {
                        Some(s) => {
                            s.x0 = s.x0.min(x0);
                            s.x1 = s.x1.max(x1);
                        }
                        None => word.push(Span { line, x0, x1 }),
                    }
                }
                spans.push(word);
            }
        }
        *imp.spans.borrow_mut() = spans;
        *imp.rows.borrow_mut() = rows;
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let layout = self.layout();
        let base = self.color();
        snapshot.append_layout(&layout, &base);
        let imp = self.imp();
        let Some((word, progress)) = imp.sweep.get() else {
            return;
        };
        let Some(sung) = *imp.sung.borrow() else { return };
        let lit = sung_spans(&imp.spans.borrow(), Some(word), progress, imp.rtl.get());
        let rows = imp.rows.borrow();
        for span in lit {
            let Some((y, h)) = rows.get(span.line) else {
                continue;
            };
            snapshot.push_clip(&graphene::Rect::new(span.x0, *y, span.x1 - span.x0, *h));
            snapshot.append_layout(&layout, &sung);
            snapshot.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_cjk;

    #[test]
    fn detects_cjk_for_line_height() {
        assert!(is_cjk("清晨把港湾折进光里"));
        assert!(is_cjk("ひかり"));
        assert!(!is_cjk("Morning folds the harbour"));
    }
}

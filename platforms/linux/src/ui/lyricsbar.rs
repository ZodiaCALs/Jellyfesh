//! The lyrics bar (ADR-0022 §6, docs/ui.md "Lyrics, word by word"): with
//! Show Lyrics Bar on and timed lyrics, the line being sung fills word by
//! word in the highlight colour, with its translation when Show
//! Translations is on. The player bar shows it under the title, instead of
//! the artist; Desktop Lyrics shows it with the next line.
//!
//! What shows when comes from the core's timing helper
//! (`jf_core::lyrics_bar`): the view wakes when a line or a word changes
//! (`LyricTimeline::next_change`) and draws every frame only while a word
//! is being sung.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gtk::glib;
use gtk::prelude::*;
use jf_core::{LyricsBar, LyricsSnapshot};

use crate::logic::lyrics::aux_lines;
use crate::logic::lyricsbar;
use crate::logic::schedule::cursor_ms;
use crate::logic::sweep::line_text;
use crate::ui::clock::frame_time_to_core_ns;
use crate::ui::state::{Change, app, try_app};
use crate::ui::style;
use crate::ui::widgets::lyricline::LyricLine;

/// What the view shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The line and its translation (the player bar).
    Line,
    /// The line, then the next one (Desktop Lyrics).
    WithNext,
}

/// Told whether a line shows.
type ShowingFn = Box<dyn Fn(bool)>;

pub struct LyricsBarView {
    /// Always shown (so the clock keeps updating it); holds `content`.
    pub root: gtk::Box,
    /// The lines, shown while there is a line to show.
    content: gtk::Box,
    slot: gtk::Box,
    second: gtk::Label,
    line: RefCell<Option<LyricLine>>,
    /// The lyrics and the line on screen.
    shown: RefCell<Option<(Arc<LyricsSnapshot>, usize)>>,
    tick: RefCell<Option<gtk::TickCallbackId>>,
    mode: Mode,
    /// Called with true while a line shows.
    on_showing: RefCell<Option<ShowingFn>>,
}

impl std::fmt::Debug for LyricsBarView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LyricsBarView").finish_non_exhaustive()
    }
}

impl LyricsBarView {
    pub fn new(mode: Mode) -> Rc<LyricsBarView> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("lyrics-bar");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 1);
        content.set_visible(false);
        let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        slot.add_css_class("lyrics-bar-line");
        let second = gtk::Label::builder()
            .xalign(if mode == Mode::WithNext { 0.5 } else { 0.0 })
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .single_line_mode(true)
            .build();
        second.add_css_class(match mode {
            Mode::Line => "lyrics-bar-translation",
            Mode::WithNext => "lyrics-bar-next",
        });
        content.append(&slot);
        content.append(&second);
        root.append(&content);
        let view = Rc::new(LyricsBarView {
            root,
            content,
            slot,
            second,
            line: RefCell::new(None),
            shown: RefCell::new(None),
            tick: RefCell::new(None),
            mode,
            on_showing: RefCell::new(None),
        });
        let a = app();
        let (draw, next) = (Rc::downgrade(&view), Rc::downgrade(&view));
        a.scheduler.add(
            &view.root,
            move |_, pos, running| {
                if let Some(v) = draw.upgrade() {
                    v.update(pos, running);
                }
            },
            move |_, pos| next.upgrade().and_then(|v| v.next_change(pos)),
        );
        // A new song, new lyrics or a changed setting redraws at once.
        let weak = Rc::downgrade(&view);
        a.listen_for(&view.root, move |change| {
            if matches!(
                change,
                Change::Lyrics | Change::Settings | Change::Remote | Change::NowPlaying | Change::Look
            ) && let Some(v) = weak.upgrade()
            {
                let pos = app().position_us();
                let running = app().display_clock().is_running();
                v.update(pos, running);
            }
        });
        view
    }

    /// Tells `f` whether a line shows (the player bar hides the artist).
    pub fn connect_showing(&self, f: impl Fn(bool) + 'static) {
        *self.on_showing.borrow_mut() = Some(Box::new(f));
    }

    /// The lyrics and their timeline, while the bar is to show.
    fn lyrics(&self) -> Option<Arc<LyricsSnapshot>> {
        let a = try_app()?;
        let setting = match self.mode {
            Mode::Line => a.settings.borrow().show_lyrics_bar,
            Mode::WithNext => true,
        };
        let lyrics = a.lyrics.borrow().clone()?;
        let synced = lyrics.timeline.is_some() && lyrics.doc.as_ref().is_some_and(|d| d.is_synced());
        lyricsbar::shown(setting, a.is_remote(), synced).then_some(lyrics)
    }

    fn bar_at(lyrics: &LyricsSnapshot, pos_us: i64) -> Option<LyricsBar> {
        let timeline = lyrics.timeline.as_ref()?;
        Some(jf_core::lyrics_bar(timeline, cursor_ms(pos_us)))
    }

    /// When the bar next changes while playing.
    fn next_change(&self, pos_us: i64) -> Option<i64> {
        let lyrics = self.lyrics()?;
        let next = lyrics.timeline.as_ref()?.next_change(cursor_ms(pos_us))?;
        Some(i64::from(next) * 1_000)
    }

    /// Draws the bar at `pos_us`.
    pub fn update(self: &Rc<Self>, pos_us: i64, running: bool) {
        let Some(lyrics) = self.lyrics() else {
            self.hide();
            return;
        };
        let Some(bar) = Self::bar_at(&lyrics, pos_us) else {
            self.hide();
            return;
        };
        let Some(index) = bar.line else {
            self.hide();
            return;
        };
        let Some(doc) = lyrics.doc.as_ref() else {
            self.hide();
            return;
        };
        let Some(line) = doc.lines.get(index) else {
            self.hide();
            return;
        };
        let same = self
            .shown
            .borrow()
            .as_ref()
            .is_some_and(|(l, i)| Arc::ptr_eq(l, &lyrics) && *i == index);
        if !same {
            let widget = LyricLine::new(line_text(line));
            widget.set_single_line(true);
            widget.set_centred(self.mode == Mode::WithNext);
            widget.add_css_class("lyrics-bar-text");
            if line.words.is_empty() {
                // A line without word timing shows whole, in the sung colour.
                widget.add_css_class("sung");
            }
            if let Some(old) = self.line.borrow_mut().take() {
                self.slot.remove(&old);
            }
            self.slot.append(&widget);
            *self.line.borrow_mut() = Some(widget);
            let second = match self.mode {
                Mode::Line => {
                    let show = try_app().is_some_and(|a| a.settings.borrow().show_translation);
                    aux_lines(doc, line, show, false)
                        .into_iter()
                        .map(|(_, text)| text)
                        .next()
                }
                Mode::WithNext => bar.next.and_then(|n| doc.lines.get(n)).map(|l| l.text.clone()),
            };
            self.second.set_label(second.as_deref().unwrap_or_default());
            self.second
                .set_visible(second.is_some_and(|s| !s.trim().is_empty()));
            *self.shown.borrow_mut() = Some((lyrics.clone(), index));
        }
        self.show();
        let sweeping = self.apply_sweep(&bar);
        if sweeping && running && style::animations_enabled() {
            self.start_tick();
        } else {
            self.stop_tick();
        }
    }

    /// Draws the words' progress; true while a word is being sung.
    fn apply_sweep(&self, bar: &LyricsBar) -> bool {
        let line = self.line.borrow();
        let Some(widget) = line.as_ref() else {
            return false;
        };
        if !style::animations_enabled() {
            // With reduced motion a word lights whole as it starts.
            let lit: Vec<f32> = bar
                .words
                .iter()
                .map(|w| if *w > 0.0 { 1.0 } else { 0.0 })
                .collect();
            widget.set_sweep(lyricsbar::sweep(&lit), &style::lyrics_sung());
            return false;
        }
        widget.set_sweep(lyricsbar::sweep(&bar.words), &style::lyrics_sung());
        lyricsbar::sweeping(bar)
    }

    fn show(&self) {
        if !self.content.is_visible() {
            self.content.set_visible(true);
            if let Some(f) = self.on_showing.borrow().as_ref() {
                f(true);
            }
        }
    }

    fn hide(&self) {
        self.stop_tick();
        if self.content.is_visible() {
            self.content.set_visible(false);
            if let Some(f) = self.on_showing.borrow().as_ref() {
                f(false);
            }
        }
        self.shown.borrow_mut().take();
    }

    /// Every frame while a word is sung: the bar follows the clock.
    fn start_tick(self: &Rc<Self>) {
        if self.tick.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        let id = self.root.add_tick_callback(move |_, clock| {
            let Some(view) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let Some(a) = try_app() else {
                return glib::ControlFlow::Break;
            };
            let now = frame_time_to_core_ns(clock.frame_time());
            let pos = a.display_clock().position_us(now);
            let shown = view.shown.borrow().as_ref().map(|(l, i)| (l.clone(), *i));
            let bar = shown.as_ref().and_then(|(l, _)| Self::bar_at(l, pos));
            let same_line = matches!((&shown, &bar), (Some((_, i)), Some(b)) if b.line == Some(*i));
            if !same_line || !bar.as_ref().is_some_and(|b| view.apply_sweep(b)) {
                // The word ended or the line moved on: the scheduler takes
                // over until the next change.
                view.tick.borrow_mut().take();
                a.scheduler.refresh();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        *self.tick.borrow_mut() = Some(id);
    }

    fn stop_tick(&self) {
        if let Some(id) = self.tick.borrow_mut().take() {
            id.remove();
        }
    }
}

//! The lyrics panel (design-controls.md, "Now Playing"): every line of the
//! current song, the active one at full size in `ink-90` and held at
//! `lyrics.anchor-position`, the others at `lyrics.scale-inactive` (sung ones
//! fainter); a word sweep for word-timed lyrics; translation and
//! romanization lines per settings; the Lyrics size and alignment settings.
//! When the active line changes the lines move up in a cascade (`dur-3`
//! Fluid, each following line `lyrics.stagger` later). Tapping a line, or
//! pressing Enter on it, seeks to it.
//!
//! Drawing follows the core's rules: the cursor comes from the display
//! clock, the panel wakes at `LyricTimeline::next_change`, and a frame clock
//! runs only while a word is being swept or the lines move. With reduced
//! motion the active line is highlighted whole and the lines jump.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::glib;
use jf_core::{Command, Cursor, LyricsAlign, LyricsSize, LyricsSnapshot};

use crate::logic::lyrics::{
    LineState, aux_lines, aux_sizes, cascade_ms, cascade_offset, last_started, line_scale, line_size,
    line_state, scroll_target,
};
use crate::logic::motion::{self, FLUID};
use crate::logic::schedule::{self, cursor_ms};
use crate::logic::sweep::{self, line_text};
use crate::ui::clock::frame_time_to_core_ns;
use crate::ui::i18n::{tr, trf};
use crate::ui::state::{app, try_app};
use crate::ui::style;
use crate::ui::widgets::lyricline::LyricLine;
use crate::ui::widgets::lyricscolumn::LyricsColumn;

/// Manual scrolling pauses following for this long.
const FOLLOW_PAUSE: Duration = Duration::from_secs(4);
/// Lines further down the screen than this all start with the last one.
const MAX_STAGGERED: usize = 12;

struct Line {
    container: gtk::Box,
    text: LyricLine,
    words: bool,
}

/// The settings the lines were built for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Look {
    translation: bool,
    romanization: bool,
    align: LyricsAlign,
}

/// A running cascade: per line, where it started and where it goes.
#[derive(Default)]
struct Cascade {
    from: Vec<(f64, f64)>,
    to_scale: Vec<f64>,
    delay_rank: Vec<usize>,
    duration_ms: f64,
    stagger_ms: f64,
}

pub struct Lyrics {
    pub root: gtk::Overlay,
    stack: gtk::Stack,
    scroll: gtk::ScrolledWindow,
    column: LyricsColumn,
    lines: RefCell<Vec<Line>>,
    lyrics: RefCell<Option<Arc<LyricsSnapshot>>>,
    synced: Cell<bool>,
    active: Cell<Option<usize>>,
    last: Cell<Option<usize>>,
    tick: RefCell<Option<gtk::TickCallbackId>>,
    paused_until: Cell<Option<Instant>>,
    animation: RefCell<Option<adw::TimedAnimation>>,
    cascade: RefCell<Cascade>,
    size_class: RefCell<String>,
    aux_class: RefCell<String>,
    look: Cell<Option<Look>>,
    width: Cell<i32>,
    /// Set while the lines wait for their first layout.
    waiting_layout: Cell<bool>,
    /// The panel's size, as last reported.
    size: Cell<(i32, i32)>,
    /// The Lyrics size the type is set for.
    shown_size: Cell<Option<LyricsSize>>,
    /// This panel, for callbacks that outlive a borrow of it.
    weak_self: RefCell<std::rc::Weak<Lyrics>>,
    empty: adw::StatusPage,
    remote: adw::StatusPage,
    /// The service whose remote session plays, if one does.
    remote_service: RefCell<Option<String>>,
}

impl std::fmt::Debug for Lyrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lyrics").finish_non_exhaustive()
    }
}

fn settings_look() -> (Look, LyricsSize) {
    let s = app().settings.borrow().clone();
    (
        Look {
            translation: s.show_translation,
            romanization: s.show_romanization,
            align: s.lyrics_align,
        },
        s.lyrics_size,
    )
}

impl Lyrics {
    pub fn new() -> Rc<Lyrics> {
        let column = LyricsColumn::new(jf_tokens::lyrics::LINE_GAP.round() as i32);
        column.add_css_class("lyrics-column");
        column.update_property(&[gtk::accessible::Property::Label(&tr("Lyrics"))]);
        let scroll = gtk::ScrolledWindow::builder()
            .child(&column)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        scroll.add_css_class("lyrics-scroll");
        let stack = gtk::Stack::new();
        stack.add_named(&scroll, Some("lines"));
        let empty = adw::StatusPage::builder()
            .title(tr("No Lyrics"))
            .description(tr(
                "Jellyfesh shows lyrics from a .lrc, .ttml or .vtt file next to the song, or from its tags.",
            ))
            .build();
        empty.add_css_class("compact");
        empty.add_css_class("lyrics-empty");
        stack.add_named(&empty, Some("empty"));
        // While a remote session plays, the service has the lyrics.
        let remote = adw::StatusPage::new();
        remote.add_css_class("compact");
        remote.add_css_class("lyrics-empty");
        stack.add_named(&remote, Some("remote"));
        // An empty drawing area over the panel reports its size, for the
        // anchor padding and the fluid type size.
        let sizer = gtk::DrawingArea::new();
        sizer.set_can_target(false);
        let root = gtk::Overlay::new();
        root.set_child(Some(&stack));
        root.add_overlay(&sizer);
        let lyrics = Rc::new(Lyrics {
            root,
            stack,
            scroll,
            column,
            lines: RefCell::new(Vec::new()),
            lyrics: RefCell::new(None),
            synced: Cell::new(false),
            active: Cell::new(None),
            last: Cell::new(None),
            tick: RefCell::new(None),
            paused_until: Cell::new(None),
            animation: RefCell::new(None),
            cascade: RefCell::new(Cascade::default()),
            size_class: RefCell::new(String::new()),
            aux_class: RefCell::new(String::new()),
            look: Cell::new(None),
            width: Cell::new(0),
            waiting_layout: Cell::new(false),
            size: Cell::new((0, 0)),
            shown_size: Cell::new(None),
            weak_self: RefCell::new(std::rc::Weak::new()),
            empty,
            remote,
            remote_service: RefCell::new(None),
        });
        *lyrics.weak_self.borrow_mut() = Rc::downgrade(&lyrics);
        // Scrolling by hand (wheel, touchpad, scrollbar) pauses following for
        // a few seconds; layout changes do not.
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&lyrics);
        wheel.connect_scroll(move |_, _, _| {
            if let Some(l) = weak.upgrade() {
                l.manual();
            }
            glib::Propagation::Proceed
        });
        lyrics.scroll.add_controller(wheel);
        let drag = gtk::GestureDrag::new();
        drag.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&lyrics);
        drag.connect_drag_update(move |_, _, dy| {
            if dy.abs() > 4.0
                && let Some(l) = weak.upgrade()
            {
                l.manual();
            }
        });
        lyrics.scroll.vscrollbar().add_controller(drag);
        let weak = Rc::downgrade(&lyrics);
        // The size arrives during layout; the padding and type size follow
        // right after it, so a frame never lays itself out again and again.
        sizer.connect_resize(move |_, w, h| {
            let Some(l) = weak.upgrade() else { return };
            if l.size.replace((w, h)) == (w, h) {
                return;
            }
            let weak = Rc::downgrade(&l);
            glib::idle_add_local_once(move || {
                if let Some(l) = weak.upgrade() {
                    let (w, h) = l.size.get();
                    l.resized(w, h);
                }
            });
        });
        lyrics
    }

    fn manual(&self) {
        self.paused_until.set(Some(Instant::now() + FOLLOW_PAUSE));
        self.settle();
    }

    /// Ends a cascade at once: every line where it belongs.
    fn settle(&self) {
        if let Some(a) = self.animation.borrow_mut().take() {
            a.pause();
        }
        let scales: Vec<f64> = self.cascade.borrow().to_scale.clone();
        let n = self.column.len();
        if scales.len() == n {
            self.column.set_motion(|i| (0.0, scales[i]));
        } else {
            let states = self.states();
            self.column
                .set_motion(|i| (0.0, states.get(i).map_or(1.0, |s| *s)));
        }
    }

    /// Re-lays the panel for its size: padding so the first and last lines
    /// can reach the anchor, and the fluid line size.
    pub fn resized(&self, width: i32, height: i32) {
        let top = (f64::from(height) * f64::from(jf_tokens::lyrics::ANCHOR_POSITION)) as i32;
        if self.column.margin_top() != top {
            self.column.set_margin_top(top);
            self.column.set_margin_bottom((height - top).max(0));
        }
        self.width.set(width);
        self.apply_size();
        self.place_after_layout();
    }

    /// Puts the active line back at the anchor once the lines have been
    /// laid out again (a new size, type size or alignment), without a
    /// cascade.
    fn place_after_layout(&self) {
        if self.lines.borrow().is_empty() || self.waiting_layout.replace(true) {
            return;
        }
        // A frame lays the column out; the next one places it.
        let frames = Cell::new(0);
        let lyrics = self.weak_self.borrow().clone();
        self.column.add_tick_callback(move |column, _| {
            let Some(l) = lyrics.upgrade() else {
                return glib::ControlFlow::Break;
            };
            frames.set(frames.get() + 1);
            if frames.get() < 2 || !column.is_laid_out() {
                return glib::ControlFlow::Continue;
            }
            l.waiting_layout.set(false);
            l.follow_with(l.active.get().or(l.last.get()), false);
            glib::ControlFlow::Break
        });
    }

    /// The type size classes for the column's width and the Lyrics size.
    fn apply_size(&self) {
        let size = try_app().map_or(LyricsSize::Medium, |a| a.settings.borrow().lyrics_size);
        let px = line_size(f64::from(self.width.get()), size) as i32;
        let (translation, _) = aux_sizes(size);
        for (cell, class) in [
            (&self.size_class, format!("lyrics-{px}")),
            (&self.aux_class, format!("lyrics-aux-{}", translation as i32)),
        ] {
            if *cell.borrow() != class {
                let old = cell.replace(class.clone());
                if !old.is_empty() {
                    self.column.remove_css_class(&old);
                }
                self.column.add_css_class(&class);
            }
        }
    }

    /// Applies changed Lyrics settings: sizes at once; translation,
    /// romanization and alignment rebuild the lines.
    pub fn apply_settings(self: &Rc<Self>) {
        let (look, size) = settings_look();
        let sized = self.shown_size.replace(Some(size)).is_some_and(|s| s != size);
        self.apply_size();
        if self.look.get().is_some_and(|l| l != look) {
            self.rebuild();
        } else if sized {
            self.place_after_layout();
        }
    }

    /// While a remote session plays (`service`), the pane says that the
    /// service has the lyrics; afterwards the engine's lyrics show again.
    pub fn set_remote(self: &Rc<Self>, service: Option<&str>) {
        let service = service.map(str::to_owned);
        if *self.remote_service.borrow() == service {
            return;
        }
        *self.remote_service.borrow_mut() = service.clone();
        match service {
            Some(name) => {
                self.stop_tick();
                self.remote.set_description(Some(&trf(
                    "Lyrics aren't shown while {service} plays.",
                    &[("service", &name)],
                )));
                self.stack.set_visible_child_name("remote");
            }
            None => self.rebuild(),
        }
    }

    /// Shows the lyrics of the current song.
    pub fn set_lyrics(self: &Rc<Self>, lyrics: Option<Arc<LyricsSnapshot>>) {
        let same = match (&*self.lyrics.borrow(), &lyrics) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if same && !self.lines.borrow().is_empty() {
            return;
        }
        *self.lyrics.borrow_mut() = lyrics;
        self.rebuild();
    }

    /// Rebuilds the lines (new song or changed Lyrics settings).
    pub fn rebuild(self: &Rc<Self>) {
        self.stop_tick();
        if self.remote_service.borrow().is_some() {
            return;
        }
        if let Some(a) = self.animation.borrow_mut().take() {
            a.pause();
        }
        *self.cascade.borrow_mut() = Cascade::default();
        self.column.clear();
        self.lines.borrow_mut().clear();
        self.active.set(None);
        self.last.set(None);
        let (look, _) = settings_look();
        self.look.set(Some(look));
        self.column.set_align(look.align);
        if look.align == LyricsAlign::Center {
            self.column.add_css_class("centred");
        } else {
            self.column.remove_css_class("centred");
        }
        let lyrics = self.lyrics.borrow().clone();
        let Some(doc) = lyrics.as_ref().and_then(|l| l.doc.as_ref()) else {
            let loading = lyrics.is_none() && try_app().is_some_and(|a| a.now.borrow().is_some());
            let hint = tr(
                "Jellyfesh shows lyrics from a .lrc, .ttml or .vtt file next to the song, or from its tags.",
            );
            self.empty
                .set_title(&if loading { String::new() } else { tr("No Lyrics") });
            self.empty
                .set_description(if loading { None } else { Some(&hint) });
            self.stack.set_visible_child_name("empty");
            return;
        };
        if doc
            .lines
            .iter()
            .all(|l| l.text.trim().is_empty() && l.words.is_empty())
        {
            self.stack.set_visible_child_name("empty");
            return;
        }
        let synced = doc.is_synced();
        self.synced.set(synced);
        let centred = look.align == LyricsAlign::Center;
        let mut lines = Vec::with_capacity(doc.lines.len());
        for line in &doc.lines {
            let container = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(4)
                .accessible_role(gtk::AccessibleRole::ListItem)
                .build();
            container.add_css_class("lyric");
            container.add_css_class(if synced { "future" } else { "plain" });
            let text = LyricLine::new(line_text(line));
            text.add_css_class("lyric-text");
            text.set_centred(centred);
            container.append(&text);
            for (kind, aux) in aux_lines(doc, line, look.translation, look.romanization) {
                let label = gtk::Label::new(Some(&aux));
                label.set_xalign(if centred { 0.5 } else { 0.0 });
                label.set_justify(if centred {
                    gtk::Justification::Center
                } else {
                    gtk::Justification::Left
                });
                label.set_wrap(true);
                label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                label.add_css_class(match kind {
                    jf_core::AuxKind::Translation => "lyric-translation",
                    jf_core::AuxKind::Romanization => "lyric-romanization",
                });
                container.append(&label);
            }
            let empty_line = line.text.trim().is_empty() && line.words.is_empty();
            container.set_visible(!empty_line);
            if synced && let Some(start) = line.start_ms {
                // Tap a line, or press Enter on it, to sing from there.
                let seek = move || {
                    app().send(Command::Seek {
                        position_ms: u64::from(start),
                    });
                };
                let click = gtk::GestureClick::new();
                let tap = seek;
                click.connect_released(move |_, _, _, _| tap());
                container.add_controller(click);
                let keys = gtk::EventControllerKey::new();
                keys.connect_key_pressed(move |_, key, _, _| match key {
                    gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                        seek();
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                });
                container.add_controller(keys);
                container.set_focusable(true);
                container.set_cursor_from_name(Some("pointer"));
            }
            self.column.append(&container);
            lines.push(Line {
                container,
                text,
                words: !line.words.is_empty(),
            });
        }
        *self.lines.borrow_mut() = lines;
        self.stack.set_visible_child_name("lines");
        self.scroll.vadjustment().set_value(0.0);
        let states = self.states();
        self.column
            .set_motion(|i| (0.0, states.get(i).copied().unwrap_or(1.0)));
        if let Some(a) = try_app() {
            let pos = a.position_us();
            self.update(pos, a.clock.borrow().is_running());
        }
    }

    /// Each line's scale for the current active line: plain lyrics at full
    /// size, synced ones smaller unless active.
    fn states(&self) -> Vec<f64> {
        let n = self.lines.borrow().len();
        if !self.synced.get() {
            return vec![1.0; n];
        }
        (0..n)
            .map(|i| line_scale(line_state(i, self.active.get(), self.last.get())))
            .collect()
    }

    fn cursor(&self, pos_us: i64) -> Option<Cursor> {
        let lyrics = self.lyrics.borrow();
        let timeline = lyrics.as_ref()?.timeline.as_ref()?;
        Some(timeline.cursor(cursor_ms(pos_us)))
    }

    /// When the lyrics next change while playing.
    pub fn next_change(&self, pos_us: i64) -> Option<i64> {
        if !self.synced.get() {
            return None;
        }
        let lyrics = self.lyrics.borrow();
        let timeline = lyrics.as_ref()?.timeline.as_ref()?;
        match schedule::lyrics_plan(timeline, cursor_ms(pos_us), false) {
            schedule::Plan::At(t) => Some(t),
            _ => None,
        }
    }

    /// Draws the lyrics at `pos_us`.
    pub fn update(self: &Rc<Self>, pos_us: i64, running: bool) {
        if !self.synced.get() {
            return;
        }
        let Some(cursor) = self.cursor(pos_us) else { return };
        let started = {
            let lyrics = self.lyrics.borrow();
            lyrics
                .as_ref()
                .and_then(|l| l.timeline.as_ref())
                .and_then(|t| last_started(&t.line_starts, cursor_ms(pos_us)))
        };
        if cursor.line != self.active.get() || started != self.last.get() {
            self.active.set(cursor.line);
            self.last.set(started);
            self.restyle();
            self.follow(cursor.line.or(started));
        }
        let sweeping = self.apply_sweep(&cursor);
        if sweeping && running {
            self.start_tick();
        } else {
            self.stop_tick();
        }
    }

    fn restyle(&self) {
        let lines = self.lines.borrow();
        for (i, line) in lines.iter().enumerate() {
            let state = line_state(i, self.active.get(), self.last.get());
            let c = &line.container;
            for (s, class) in [
                (LineState::Past, "past"),
                (LineState::Active, "active"),
                (LineState::Future, "future"),
            ] {
                if state == s {
                    c.add_css_class(class);
                } else {
                    c.remove_css_class(class);
                }
            }
            if line.words && state == LineState::Active && style::animations_enabled() {
                c.add_css_class("swept");
            } else {
                c.remove_css_class("swept");
            }
            c.update_state(&[gtk::accessible::State::Selected(Some(state == LineState::Active))]);
            if state != LineState::Active {
                line.text.set_sweep(None, &style::style().rgba("ink.ink-90"));
            }
        }
    }

    /// Updates the active line's sweep; true while a word is being swept.
    fn apply_sweep(&self, cursor: &Cursor) -> bool {
        let Some(index) = cursor.line else { return false };
        let lines = self.lines.borrow();
        let Some(line) = lines.get(index) else {
            return false;
        };
        if !line.words || !style::animations_enabled() {
            return false;
        }
        let sung = style::lyrics_sung();
        line.text
            .set_sweep(cursor.word.map(|w| (w, cursor.word_progress)), &sung);
        let lyrics = self.lyrics.borrow();
        let doc_line = lyrics
            .as_ref()
            .and_then(|l| l.doc.as_ref())
            .and_then(|d| d.lines.get(index));
        doc_line.is_some_and(|l| sweep::is_sweeping(l, cursor))
    }

    fn start_tick(self: &Rc<Self>) {
        if self.tick.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        let id = self.scroll.add_tick_callback(move |_, clock| {
            let Some(lyrics) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let Some(a) = try_app() else {
                return glib::ControlFlow::Break;
            };
            let now = frame_time_to_core_ns(clock.frame_time());
            let pos = a.clock.borrow_mut().position_us(now);
            let Some(cursor) = lyrics.cursor(pos) else {
                lyrics.tick.borrow_mut().take();
                return glib::ControlFlow::Break;
            };
            if cursor.line != lyrics.active.get() || !lyrics.apply_sweep(&cursor) {
                // The word finished or the line moved on: the scheduler takes
                // over until the next change.
                lyrics.tick.borrow_mut().take();
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

    /// Brings `index` to the anchor (unless the person scrolled recently),
    /// the lines settling in a cascade, and scales the lines for the new
    /// active one.
    fn follow(self: &Rc<Self>, index: Option<usize>) {
        self.follow_with(index, true);
    }

    /// [`Lyrics::follow`], in a cascade when `cascade` (and motion is
    /// allowed), or at once (the first placement of a song's lines).
    fn follow_with(self: &Rc<Self>, index: Option<usize>, cascade: bool) {
        let following = !self.paused_until.get().is_some_and(|t| Instant::now() < t);
        if !self.column.is_laid_out() {
            // Not laid out yet: place the lines once the column has its size
            // (on a coming frame, which only runs while the panel shows).
            if self.waiting_layout.replace(true) {
                return;
            }
            let weak = Rc::downgrade(self);
            self.column.add_tick_callback(move |column, _| {
                let Some(l) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if !column.is_laid_out() {
                    return glib::ControlFlow::Continue;
                }
                l.waiting_layout.set(false);
                l.follow_with(l.active.get().or(l.last.get()), false);
                glib::ControlFlow::Break
            });
            return;
        }
        let adj = self.scroll.vadjustment();
        let from = adj.value();
        let target = match (following, index.and_then(|i| self.column.line_rect(i))) {
            (true, Some((top, height))) => {
                let viewport = adj.page_size();
                // Rects are in the column's own coordinates; its top margin
                // (the room above the first line) is part of the content.
                scroll_target(
                    top + f64::from(self.column.margin_top()),
                    height,
                    viewport,
                    f64::from(jf_tokens::lyrics::ANCHOR_POSITION),
                    adj.upper() - viewport,
                )
            }
            _ => from,
        };
        let scales = self.states();
        let plan = motion::lyrics_cascade(style::allowed()).filter(|_| cascade);
        let Some((duration_ms, stagger_ms)) = plan.filter(|_| self.scroll.is_mapped()) else {
            // Reduced motion, or not on screen: the lines jump.
            if let Some(a) = self.animation.borrow_mut().take() {
                a.pause();
            }
            adj.set_value(target);
            self.column
                .set_motion(|i| (0.0, scales.get(i).copied().unwrap_or(1.0)));
            return;
        };
        // The view jumps to the target, and each line starts from where it
        // is drawn now, moving the rest of the way in the cascade.
        let distance = target - from;
        adj.set_value(target);
        let n = self.column.len();
        let viewport_top = f64::from(self.column.margin_top()) - from;
        let mut rank = vec![0; n];
        let mut first_on_screen: Option<usize> = None;
        for (i, r) in rank.iter_mut().enumerate() {
            let Some((top, height)) = self.column.line_rect(i) else {
                continue;
            };
            // Where the line is on screen before the move.
            let y = top + viewport_top;
            if y + height >= 0.0 {
                let first = *first_on_screen.get_or_insert(i);
                *r = (i - first).min(MAX_STAGGERED);
            }
        }
        let start: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let (dy, s) = self.column.motion(i);
                (dy + distance, s)
            })
            .collect();
        let lines_moving = rank.iter().copied().max().unwrap_or(0) + 1;
        let total = cascade_ms(lines_moving, duration_ms, stagger_ms);
        *self.cascade.borrow_mut() = Cascade {
            from: start.clone(),
            to_scale: scales,
            delay_rank: rank,
            duration_ms,
            stagger_ms,
        };
        self.column.set_motion(|i| start[i]);
        if let Some(a) = self.animation.borrow_mut().take() {
            a.pause();
        }
        let weak = Rc::downgrade(self);
        let callback = adw::CallbackAnimationTarget::new(move |t| {
            let Some(l) = weak.upgrade() else { return };
            let c = l.cascade.borrow();
            let elapsed = t * total;
            l.column.set_motion(|i| {
                let (dy, s0) = c.from.get(i).copied().unwrap_or((0.0, 1.0));
                let s1 = c.to_scale.get(i).copied().unwrap_or(1.0);
                let k = c.delay_rank.get(i).copied().unwrap_or(0);
                let offset = cascade_offset(k, elapsed, c.duration_ms, c.stagger_ms, dy, &FLUID);
                let p = motion::delayed_progress(elapsed, k as f64 * c.stagger_ms, c.duration_ms);
                (offset, s0 + (s1 - s0) * FLUID.ease(p))
            });
        });
        let ms = total.ceil() as u32;
        let anim = adw::TimedAnimation::new(&self.scroll, 0.0, 1.0, ms.max(1), callback);
        anim.set_easing(adw::Easing::Linear);
        let weak = Rc::downgrade(self);
        anim.connect_done(move |_| {
            if let Some(l) = weak.upgrade() {
                l.animation.borrow_mut().take();
            }
        });
        anim.play();
        *self.animation.borrow_mut() = Some(anim);
    }
}

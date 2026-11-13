//! What Now Playing and the player bar share: the transport buttons'
//! states, the shuffle and repeat toggles, the mute button, and volume
//! changes sent at most every 60 ms while a slider moves. Each control
//! sends a command; its state always comes back from the core.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Command, RepeatMode};

use crate::ui::i18n::tr;
use crate::ui::state::app;
use crate::ui::widgets::glyph::{self, Glyph};
use crate::ui::widgets::playpause::PlayPause;
use crate::ui::widgets::seekbar::VolumeBar;

/// Volume changes are sent at most this often while dragging.
const VOLUME_THROTTLE: Duration = Duration::from_millis(60);

/// A play button with the morphing glyph.
pub fn play_button(size: i32) -> (gtk::Button, PlayPause) {
    let glyph = PlayPause::new(size);
    let button = gtk::Button::builder().child(&glyph).build();
    button.add_css_class("flat");
    button.update_property(&[gtk::accessible::Property::Label(&tr("Play"))]);
    button.set_tooltip_text(Some(&tr("Play")));
    button.connect_clicked(|_| {
        app().send(Command::TogglePlayPause);
    });
    (button, glyph)
}

/// Shows play or pause on a play button for the core's state.
pub fn update_play(button: &gtk::Button, glyph: &PlayPause) {
    let playing = app().is_playing();
    glyph.set_playing(playing);
    let label = if playing { tr("Pause") } else { tr("Play") };
    button.set_tooltip_text(Some(&label));
    button.update_property(&[gtk::accessible::Property::Label(&label)]);
}

pub fn shuffle_button(size: i32) -> gtk::Button {
    let b = glyph::button("shuffle", size, &tr("Shuffle"));
    b.connect_clicked(|_| {
        let a = app();
        let on = a.queue.borrow().shuffle;
        a.send(Command::SetShuffle { on: !on });
    });
    b
}

pub fn repeat_button(size: i32) -> gtk::Button {
    let b = glyph::button("repeat", size, &tr("Repeat"));
    b.connect_clicked(|_| {
        let a = app();
        let mode = match a.queue.borrow().repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
        a.send(Command::SetRepeat { mode });
    });
    b
}

pub fn skip_button(next: bool, size: i32) -> gtk::Button {
    let (name, label) = if next {
        ("next", tr("Next"))
    } else {
        ("previous", tr("Previous"))
    };
    let b = glyph::button(name, size, &label);
    b.connect_clicked(move |_| {
        app().send(if next { Command::Next } else { Command::Previous });
    });
    b
}

/// Marks a toggle button on or off, with its label and pressed state.
pub fn set_on(button: &gtk::Button, on: bool, label: &str) {
    if on {
        button.add_css_class("on");
    } else {
        button.remove_css_class("on");
    }
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button.update_state(&[gtk::accessible::State::Pressed(if on {
        gtk::AccessibleTristate::True
    } else {
        gtk::AccessibleTristate::False
    })]);
}

/// Shows the queue's shuffle and repeat states.
pub fn update_toggles(shuffle: &gtk::Button, repeat: &gtk::Button) {
    let a = app();
    let q = a.queue.borrow();
    set_on(
        shuffle,
        q.shuffle,
        &if q.shuffle {
            tr("Shuffle, on")
        } else {
            tr("Shuffle, off")
        },
    );
    let (glyph_name, label) = match q.repeat {
        RepeatMode::Off => ("repeat", tr("Repeat, off")),
        RepeatMode::All => ("repeat", tr("Repeat all")),
        RepeatMode::One => ("repeat-one", tr("Repeat one")),
    };
    if let Some(g) = glyph::of(repeat) {
        g.set_name_glyph(glyph_name);
    }
    set_on(repeat, q.repeat != RepeatMode::Off, &label);
}

/// The mute button and volume slider between two speaker glyphs.
pub struct Volume {
    pub row: gtk::Box,
    pub mute: gtk::Button,
    pub bar: VolumeBar,
    pending: Rc<Cell<Option<f64>>>,
    timer: Rc<RefCell<Option<glib::SourceId>>>,
}

impl std::fmt::Debug for Volume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Volume").finish_non_exhaustive()
    }
}

impl Volume {
    /// `loud_glyph` adds the louder speaker after the line.
    pub fn new(glyph_size: i32, loud_glyph: bool) -> Volume {
        let mute = glyph::button("volume-low", glyph_size, &tr("Mute"));
        mute.add_css_class("np-volume-glyph");
        mute.connect_clicked(|_| {
            let a = app();
            let muted = a.muted.get();
            a.send(Command::SetMuted { muted: !muted });
        });
        let bar = VolumeBar::new();
        bar.add_css_class("volume");
        bar.update_property(&[gtk::accessible::Property::Label(&tr("Volume"))]);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("volume-row");
        row.append(&mute);
        row.append(&bar);
        if loud_glyph {
            let loud = Glyph::new("volume-high", glyph_size);
            loud.add_css_class("np-volume-glyph");
            row.append(&loud);
        }
        let pending: Rc<Cell<Option<f64>>> = Rc::new(Cell::new(None));
        let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let (p, t) = (pending.clone(), timer.clone());
        bar.connect_changed(move |v| {
            p.set(Some(v));
            if t.borrow().is_some() {
                return;
            }
            send(&p);
            let (p2, t2) = (p.clone(), t.clone());
            let id = glib::timeout_add_local(VOLUME_THROTTLE, move || {
                if p2.get().is_some() {
                    send(&p2);
                    glib::ControlFlow::Continue
                } else {
                    t2.borrow_mut().take();
                    glib::ControlFlow::Break
                }
            });
            *t.borrow_mut() = Some(id);
        });
        Volume {
            row,
            mute,
            bar,
            pending,
            timer,
        }
    }

    /// Shows the core's volume and mute state.
    pub fn update(&self) {
        let a = app();
        let volume = f64::from(a.playback.borrow().volume);
        if self.timer.borrow().is_none() && self.pending.get().is_none() {
            self.bar.set_value(volume);
        }
        let muted = a.muted.get();
        if let Some(g) = glyph::of(&self.mute) {
            g.set_name_glyph(if muted { "volume-muted" } else { "volume-low" });
        }
        let label = if muted { tr("Unmute") } else { tr("Mute") };
        self.mute.set_tooltip_text(Some(&label));
        self.mute
            .update_property(&[gtk::accessible::Property::Label(&label)]);
        self.bar.set_sensitive(!muted);
    }
}

fn send(pending: &Cell<Option<f64>>) {
    if let Some(v) = pending.take() {
        app().send(Command::SetVolume { volume: v as f32 });
    }
}

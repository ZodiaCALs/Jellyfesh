//! The quality badge and its menu (ADR-0026 §3; docs/ui.md, "The player bar
//! and the mini player"). The badge shows the playing quality's label (the
//! chosen level until the engine has opened the song). Its menu has the
//! format line ("FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s"), then the
//! levels the core offers for what plays with the chosen one checked, or
//! why there are none: "Files play as they are." or, for a source without
//! levels and during remote playback, "Quality is set in {service}.".
//!
//! A level chosen here is Streaming Quality, the same setting as Settings ›
//! Playback › Streaming; the core applies it at once when the song can
//! seek, and from the next song otherwise.

use std::rc::Rc;

use adw::prelude::*;
use jf_core::StreamingQuality;

use crate::logic::format;
use crate::logic::quality::{self, Badge, Choices};
use crate::ui::i18n::{tr, trf};
use crate::ui::state::{Change, app};
use crate::ui::style;
use crate::ui::widgets::glyph::Glyph;

/// A level's name, as the menu and Settings › Playback › Streaming show it.
pub fn level_name(level: StreamingQuality) -> String {
    match level {
        StreamingQuality::Auto => tr("Auto"),
        StreamingQuality::Lossless => tr("Lossless"),
        StreamingQuality::High => tr("High"),
        StreamingQuality::Normal => tr("Normal"),
        StreamingQuality::Low => tr("Low"),
    }
}

/// "FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s".
pub fn line_text(line: &quality::Line) -> String {
    let depth = line.bit_depth.map(|b| trf("{n}-bit", &[("n", &b.to_string())]));
    let rate = line.sample_rate_hz.map(format::sample_rate);
    let bitrate = line
        .bitrate_kbps
        .map(|k| trf("{n} kbit/s", &[("n", &format::count(u64::from(k)))]));
    format::dot_join([
        line.codec.as_deref(),
        depth.as_deref(),
        rate.as_deref(),
        bitrate.as_deref(),
    ])
}

/// The badge, with its menu.
pub struct QualityButton {
    pub button: gtk::MenuButton,
    label: gtk::Label,
    line: gtk::Label,
    levels: gtk::Box,
    note: gtk::Label,
    popover: gtk::Popover,
}

impl std::fmt::Debug for QualityButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QualityButton").finish_non_exhaustive()
    }
}

impl QualityButton {
    pub fn new() -> Rc<QualityButton> {
        let label = gtk::Label::new(None);
        label.add_css_class("quality-badge-text");
        let button = gtk::MenuButton::builder()
            .child(&label)
            .valign(gtk::Align::Center)
            .tooltip_text(tr("Quality"))
            .build();
        button.add_css_class("quality-badge");
        let popover = gtk::Popover::new();
        popover.add_css_class("menu");
        popover.add_css_class("quality-popover");
        style::animate_popup(&popover);
        let title = gtk::Label::builder()
            .label(tr("Quality"))
            .xalign(0.0)
            .accessible_role(gtk::AccessibleRole::Heading)
            .build();
        title.add_css_class("quality-popover-title");
        let line = gtk::Label::builder().xalign(0.0).wrap(true).build();
        line.add_css_class("quality-line");
        let levels = gtk::Box::new(gtk::Orientation::Vertical, 0);
        levels.add_css_class("quality-levels");
        let note = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(32)
            .build();
        note.add_css_class("quality-note");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content.append(&title);
        content.append(&line);
        content.append(&levels);
        content.append(&note);
        popover.set_child(Some(&content));
        button.set_popover(Some(&popover));
        let this = Rc::new(QualityButton {
            button,
            label,
            line,
            levels,
            note,
            popover,
        });
        this.refresh();
        let weak = Rc::downgrade(&this);
        app().listen_for(&this.button, move |change| {
            if matches!(change, Change::NowPlaying | Change::Settings | Change::Remote)
                && let Some(b) = weak.upgrade()
            {
                b.refresh();
            }
        });
        this
    }

    /// Shows the playing quality on the badge and refills the menu.
    pub fn refresh(&self) {
        let a = app();
        let chosen = a.settings.borrow().streaming_quality;
        let remote = a.is_remote();
        let np = a.now.borrow().clone();
        let (text, line, choices) = match np.as_deref() {
            Some(np) if !remote => {
                let text = match quality::badge(np, chosen) {
                    Badge::Label(label) => crate::ui::details::quality(label),
                    Badge::Level(level) => level_name(level),
                };
                let line = np
                    .quality
                    .as_ref()
                    .map(|f| line_text(&quality::line(f)))
                    .unwrap_or_default();
                (text, line, quality::choices(np, chosen))
            }
            _ => {
                let note = a.remote.borrow().quality_note.clone();
                (
                    level_name(chosen),
                    String::new(),
                    quality::note_choices(note.as_ref()),
                )
            }
        };
        self.label.set_text(&text);
        self.button
            .update_property(&[gtk::accessible::Property::Label(&trf(
                "Quality: {quality}",
                &[("quality", &text)],
            ))]);
        self.line.set_text(&line);
        self.line.set_visible(!line.is_empty());
        while let Some(c) = self.levels.first_child() {
            self.levels.remove(&c);
        }
        let note = match choices {
            Choices::Levels(levels) => {
                for (level, checked) in levels {
                    self.levels.append(&self.row(level, checked));
                }
                None
            }
            Choices::FilesPlayAsTheyAre => Some(tr("Files play as they are.")),
            Choices::SetIn(service) => Some(trf("Quality is set in {service}.", &[("service", &service)])),
            Choices::Nothing => None,
        };
        self.levels.set_visible(self.levels.first_child().is_some());
        self.note.set_text(note.as_deref().unwrap_or(""));
        self.note.set_visible(note.is_some());
    }

    fn row(&self, level: StreamingQuality, checked: bool) -> gtk::Button {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let check = Glyph::new("check", 16);
        check.set_opacity(if checked { 1.0 } else { 0.0 });
        row.append(&check);
        let name = level_name(level);
        let l = gtk::Label::new(Some(&name));
        l.set_xalign(0.0);
        row.append(&l);
        let button = gtk::Button::builder()
            .child(&row)
            .accessible_role(gtk::AccessibleRole::MenuItemRadio)
            .build();
        button.add_css_class("flat");
        button.update_property(&[gtk::accessible::Property::Label(&name)]);
        button.update_state(&[gtk::accessible::State::Checked(if checked {
            gtk::AccessibleTristate::True
        } else {
            gtk::AccessibleTristate::False
        })]);
        let pop = self.popover.downgrade();
        button.connect_clicked(move |_| {
            if let Some(p) = pop.upgrade() {
                p.popdown();
            }
            if app().settings.borrow().streaming_quality != level {
                app().update_settings(|s| s.streaming_quality = level);
            }
        });
        button
    }
}

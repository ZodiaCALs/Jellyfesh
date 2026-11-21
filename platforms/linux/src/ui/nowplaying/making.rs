//! Making lyrics in the Lyrics pane (docs/ui.md "AI Lyrics, Customize and
//! Pro"): Create Synced Lyrics with no lyrics, Time These Lyrics below plain
//! ones, the song's job ("Recognising… 42%" with a bar and Cancel), what is
//! needed first (a model to download, an API key to add), a failed job with
//! Try Again, and "Made by AI Lyrics · {engine}" under made lyrics. The
//! offer is `logic::ailyrics`'s.

use std::cell::RefCell;

use adw::prelude::*;
use jf_core::{Command, LyricsJobStage, SongRef};

use crate::logic::ailyrics::Offer;
use crate::logic::format;
use crate::ui::i18n::{tr, trf};
use crate::ui::state::app;

pub struct Making {
    pub root: gtk::Box,
    text: gtk::Label,
    bar: gtk::ProgressBar,
    primary: gtk::Button,
    cancel: gtk::Button,
    made: gtk::Label,
    offer: std::rc::Rc<RefCell<Offer>>,
}

impl std::fmt::Debug for Making {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Making").finish_non_exhaustive()
    }
}

fn percent(fraction: f32) -> String {
    format!("{:.0}", (fraction * 100.0).clamp(0.0, 100.0))
}

/// A job's stage, worded.
fn stage_text(stage: &LyricsJobStage) -> String {
    match stage {
        LyricsJobStage::Waiting => tr("Waiting…"),
        LyricsJobStage::ReadingAudio => tr("Reading Audio…"),
        LyricsJobStage::Sending { origin } => trf("Sending to {origin}…", &[("origin", origin)]),
        LyricsJobStage::Recognising { fraction } => {
            trf("Recognising… {percent}%", &[("percent", &percent(*fraction))])
        }
        LyricsJobStage::TimingWords => tr("Timing Words…"),
        LyricsJobStage::Done | LyricsJobStage::Cancelled | LyricsJobStage::Failed { .. } => String::new(),
    }
}

impl Making {
    pub fn new() -> Making {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.add_css_class("lyrics-making");
        root.set_halign(gtk::Align::Center);
        let text = gtk::Label::builder()
            .wrap(true)
            .justify(gtk::Justification::Center)
            .max_width_chars(44)
            .build();
        text.add_css_class("lyrics-making-text");
        let bar = gtk::ProgressBar::new();
        bar.set_width_request(220);
        bar.set_halign(gtk::Align::Center);
        let primary = gtk::Button::new();
        primary.add_css_class("pill");
        primary.add_css_class("glass-pill");
        let cancel = gtk::Button::with_label(&tr("Cancel"));
        cancel.add_css_class("pill");
        cancel.add_css_class("flat");
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::Center);
        buttons.append(&primary);
        buttons.append(&cancel);
        let made = gtk::Label::new(None);
        made.add_css_class("lyrics-made-by");
        made.add_css_class("dim-label");
        root.append(&text);
        root.append(&bar);
        root.append(&buttons);
        root.append(&made);
        let offer = std::rc::Rc::new(RefCell::new(Offer::None));
        let shown = offer.clone();
        primary.connect_clicked(move |_| match &*shown.borrow() {
            Offer::Create | Offer::Failed { .. } => crate::ui::ailyrics::make(SongRef::Current),
            Offer::DownloadModel { id, .. } => {
                app().send(Command::DownloadSpeechModel { id: id.clone() });
            }
            Offer::SignIn => crate::ui::ailyrics::open_settings(),
            _ => {}
        });
        let shown = offer.clone();
        cancel.connect_clicked(move |_| match &*shown.borrow() {
            Offer::Running { job, .. } => {
                app().send(Command::CancelLyrics { job: *job });
            }
            Offer::DownloadingModel { id, .. } => {
                app().send(Command::CancelSpeechModelDownload { id: id.clone() });
            }
            _ => {}
        });
        let m = Making {
            root,
            text,
            bar,
            primary,
            cancel,
            made,
            offer,
        };
        m.show(&Offer::None, false, None);
        m
    }

    /// Shows `offer` (Time These Lyrics when `times` lyrics already exist)
    /// and, under made lyrics, who made them. Hidden with nothing to show.
    pub fn show(&self, offer: &Offer, times: bool, made_with: Option<&str>) {
        *self.offer.borrow_mut() = offer.clone();
        let (text, primary, cancel, bar): (String, Option<String>, bool, Option<f64>) = match offer {
            Offer::None => (String::new(), None, false, None),
            Offer::Create => (
                String::new(),
                Some(if times {
                    tr("Time These Lyrics")
                } else {
                    tr("Create Synced Lyrics")
                }),
                false,
                None,
            ),
            Offer::Running { stage, progress, .. } => {
                (stage_text(stage), None, true, progress.map(f64::from))
            }
            Offer::Failed { message } => (
                trf("Couldn’t make lyrics: {message}", &[("message", message)]),
                Some(tr("Try Again")),
                false,
                None,
            ),
            Offer::DownloadModel { name, bytes, .. } => (
                trf(
                    "Download the {model} model ({size}) to make lyrics on this device.",
                    &[("model", name), ("size", &format::size(*bytes))],
                ),
                Some(tr("Download")),
                false,
                None,
            ),
            Offer::DownloadingModel { name, fraction, .. } => (
                trf(
                    "Downloading {model}… {percent}%",
                    &[("model", name), ("percent", &percent(*fraction))],
                ),
                None,
                true,
                Some(f64::from(*fraction)),
            ),
            Offer::SignIn => (
                tr("Add your API key to make lyrics online."),
                Some(tr("Open AI Lyrics Settings")),
                false,
                None,
            ),
            Offer::Unavailable { reason } => (reason.clone(), None, false, None),
        };
        self.text.set_label(&text);
        self.text.set_visible(!text.is_empty());
        match &primary {
            Some(label) => {
                self.primary.set_label(label);
                self.primary.set_visible(true);
            }
            None => self.primary.set_visible(false),
        }
        self.cancel.set_visible(cancel);
        match bar {
            Some(f) => {
                self.bar.set_fraction(f);
                self.bar.set_visible(true);
            }
            None => self.bar.set_visible(false),
        }
        let made = made_with
            .filter(|_| matches!(offer, Offer::None | Offer::Create))
            .map(|engine| trf("Made by AI Lyrics · {engine}", &[("engine", engine)]));
        self.made.set_label(made.as_deref().unwrap_or_default());
        self.made.set_visible(made.is_some());
        self.root
            .set_visible(!text.is_empty() || primary.is_some() || cancel || made.is_some());
    }
}

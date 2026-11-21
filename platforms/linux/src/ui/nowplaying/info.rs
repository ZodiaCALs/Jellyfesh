//! The Info panel (docs/ui.md, "Now Playing" and "Details"): About This
//! Song (Title, Artist, Album, Version, Year, Genre, Composer, Lyricist,
//! Label, Length, then Lyrics), Audio (Quality, Format as actually decoded,
//! Bitrate, Size, ReplayGain, File or Source), then Get Info and Open in
//! {Service} for a song with a link (ADR-0018). The core's details for the
//! song (`Command::Details`, asked once per queue entry while the panel
//! shows) fill in what the playing entry does not carry; rows without a
//! value are left out. While a remote session runs it shows the service's
//! track.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use jf_core::{Command, CommandId, DetailsPage, LyricsSnapshot, LyricsSource, NowPlaying, TrackRef};

use crate::logic::details::{self, Known};
use crate::logic::format;
use crate::ui::details::{InfoArgs, fill_rows, item_of_track, rows_list, worded};
use crate::ui::i18n::{tr, trf};
use crate::ui::state::app;

/// The core's details asked for a queue entry, and the answer.
#[derive(Default)]
struct Asked {
    uid: u64,
    request: Option<CommandId>,
    page: Option<Arc<DetailsPage>>,
}

/// The song on screen and its lyrics.
type Shown = (Arc<NowPlaying>, Option<Arc<LyricsSnapshot>>);

struct State {
    about: gtk::ListBox,
    audio_heading: gtk::Label,
    audio: gtk::ListBox,
    get_info: gtk::Button,
    /// Open in {Service}, beside Get Info.
    open_link: gtk::Button,
    link: RefCell<Option<String>>,
    /// What Get Info opens.
    args: RefCell<Option<InfoArgs>>,
    asked: RefCell<Asked>,
    /// The song and lyrics on screen, to show again when details arrive.
    shown: RefCell<Option<Shown>>,
}

pub struct Info {
    pub root: gtk::ScrolledWindow,
    state: Rc<State>,
}

impl std::fmt::Debug for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Info").finish_non_exhaustive()
    }
}

fn heading(text: &str) -> gtk::Label {
    let h = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    h.add_css_class("panel-title");
    h
}

impl Info {
    pub fn new() -> Info {
        let about = rows_list(&[]);
        about.add_css_class("np-info");
        let audio = rows_list(&[]);
        audio.add_css_class("np-info");
        let audio_heading = heading(&tr("Audio"));
        audio_heading.set_visible(false);
        let get_info = gtk::Button::with_label(&tr("Get Info"));
        get_info.add_css_class("pill");
        get_info.add_css_class("action-pill");
        get_info.add_css_class("glass-pill");
        get_info.set_visible(false);
        let open_link = gtk::Button::builder().visible(false).build();
        let link_face = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        link_face.append(&crate::ui::widgets::glyph::Glyph::new("link", 18));
        link_face.append(&gtk::Label::new(None));
        open_link.set_child(Some(&link_face));
        open_link.add_css_class("pill");
        open_link.add_css_class("action-pill");
        open_link.add_css_class("glass-pill");
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::Start);
        buttons.add_css_class("np-info-buttons");
        buttons.append(&get_info);
        buttons.append(&open_link);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.append(&heading(&tr("About This Song")));
        body.append(&about);
        body.append(&audio_heading);
        body.append(&audio);
        body.append(&buttons);
        let root = gtk::ScrolledWindow::builder()
            .child(&body)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        root.add_css_class("info-panel");
        let state = Rc::new(State {
            about,
            audio_heading,
            audio,
            get_info,
            open_link,
            link: RefCell::new(None),
            args: RefCell::new(None),
            asked: RefCell::new(Asked::default()),
            shown: RefCell::new(None),
        });
        let weak = Rc::downgrade(&state);
        state.get_info.connect_clicked(move |_| {
            let args = weak.upgrade().and_then(|s| s.args.borrow().clone());
            if let Some(args) = args {
                crate::ui::details::present(args);
            }
        });
        let weak = Rc::downgrade(&state);
        state.open_link.connect_clicked(move |_| {
            let link = weak.upgrade().and_then(|s| s.link.borrow().clone());
            if let Some(l) = link {
                crate::ui::actions::open_link(&l);
            }
        });
        // The details are asked for while the panel shows.
        let weak = Rc::downgrade(&state);
        root.connect_map(move |_| {
            if let Some(s) = weak.upgrade() {
                s.ask();
            }
        });
        Info { root, state }
    }

    /// The remote session's track: what the service reports about it.
    pub fn update_remote(&self, view: &crate::logic::remote::RemoteView) {
        let s = &self.state;
        s.shown.borrow_mut().take();
        s.args.borrow_mut().take();
        let known = Known {
            title: view.title.clone(),
            artist: view.artist.clone(),
            album: view.album.clone(),
            duration_ms: view.duration_ms,
            ..Known::default()
        };
        let (about, _) = details::song_rows(None, &known, None);
        fill_rows(&s.about, &worded(&about));
        fill_rows(&s.audio, &[]);
        s.audio_heading.set_visible(false);
        s.get_info.set_visible(false);
        s.show_link(view.link.as_deref(), &view.service);
    }

    /// The core's details: shown when they are for the song on screen.
    pub fn details_arrived(&self, page: &Arc<DetailsPage>) {
        let s = &self.state;
        {
            let mut asked = s.asked.borrow_mut();
            if asked.request != Some(page.request) {
                return;
            }
            asked.page = Some(page.clone());
        }
        s.render();
    }

    /// The song playing (or none) and its lyrics.
    pub fn update(&self, now: Option<&Arc<NowPlaying>>, lyrics: Option<&Arc<LyricsSnapshot>>) {
        let s = &self.state;
        let Some(np) = now else {
            s.shown.borrow_mut().take();
            s.args.borrow_mut().take();
            fill_rows(&s.about, &[]);
            fill_rows(&s.audio, &[]);
            s.audio_heading.set_visible(false);
            s.get_info.set_visible(false);
            s.show_link(None, "");
            return;
        };
        match &np.track {
            TrackRef::Source(src) => s.show_link(np.link.as_deref(), &app().source_name(&src.plugin)),
            _ => s.show_link(None, ""),
        }
        *s.shown.borrow_mut() = Some((np.clone(), lyrics.cloned()));
        if self.root.is_mapped() {
            s.ask();
        }
        s.render();
    }
}

impl State {
    /// Asks the core for the details of the song on screen, once per queue
    /// entry.
    fn ask(&self) {
        let Some(np) = self.shown.borrow().as_ref().map(|(np, _)| np.clone()) else {
            return;
        };
        let mut asked = self.asked.borrow_mut();
        if asked.uid == np.uid && asked.request.is_some() {
            return;
        }
        let request = app().send_quiet(Command::Details {
            item: item_of_track(&np.track),
        });
        *asked = Asked {
            uid: np.uid,
            request: Some(request),
            page: None,
        };
    }

    fn render(&self) {
        let Some((np, lyrics)) = self.shown.borrow().clone() else {
            return;
        };
        let source = match &np.track {
            TrackRef::Source(s) => Some(app().source_name(&s.plugin)),
            _ => None,
        };
        let known = Known {
            title: Some(np.title.clone()),
            artist: np.artist.clone(),
            album: np.album.clone(),
            year: None,
            duration_ms: np.duration_ms,
            path: np.path.clone(),
            source: source.clone(),
        };
        let page = {
            let asked = self.asked.borrow();
            asked.page.clone().filter(|_| asked.uid == np.uid)
        };
        // What is decoded wins over the file's own format.
        let (about, audio) = details::song_rows(page.as_deref(), &known, np.quality.as_ref());
        let mut about = worded(&about);
        let lyrics_text = lyrics_line(&np, lyrics.as_deref());
        if !lyrics_text.is_empty() {
            about.push((tr("Lyrics"), lyrics_text));
        }
        fill_rows(&self.about, &about);
        let audio = worded(&audio);
        self.audio_heading.set_visible(!audio.is_empty());
        fill_rows(&self.audio, &audio);
        *self.args.borrow_mut() = Some(InfoArgs {
            item: item_of_track(&np.track),
            title: known.title,
            artist: known.artist,
            album: known.album,
            duration_ms: known.duration_ms,
            path: known.path,
            source,
        });
        self.get_info.set_visible(true);
    }

    /// Shows Open in {Service} for `link` (hidden without one).
    fn show_link(&self, link: Option<&str>, service: &str) {
        let link = link.filter(|l| crate::logic::links::openable(l));
        *self.link.borrow_mut() = link.map(str::to_owned);
        let label = crate::ui::actions::open_in_label(service);
        if let Some(l) = self
            .open_link
            .child()
            .and_then(|c| c.last_child())
            .and_downcast::<gtk::Label>()
        {
            l.set_label(&label);
        }
        self.open_link
            .update_property(&[gtk::accessible::Property::Label(&label)]);
        self.open_link.set_visible(link.is_some());
    }
}

/// The Lyrics row: how the lyrics are timed and where they come from.
fn lyrics_line(np: &NowPlaying, lyrics: Option<&LyricsSnapshot>) -> String {
    let Some(l) = lyrics.filter(|l| l.uid == np.uid) else {
        return String::new();
    };
    let Some(doc) = &l.doc else {
        return tr("None found");
    };
    let timing = describe_timing(doc);
    let from = match &l.source {
        Some(LyricsSource::Sidecar(path)) => path.file_name().map(|n| n.to_string_lossy().into_owned()),
        Some(LyricsSource::Embedded) => Some(tr("in the file's tags")),
        Some(LyricsSource::Made { engine, .. }) => {
            Some(trf("Made by AI Lyrics · {engine}", &[("engine", engine)]))
        }
        Some(LyricsSource::Plugin(_)) | None => None,
    };
    format::dot_join([Some(timing.as_str()), from.as_deref()])
}

fn describe_timing(doc: &jf_core::LyricDoc) -> String {
    if !doc.is_synced() {
        tr("Unsynced")
    } else if doc.lines.iter().any(|l| !l.words.is_empty()) {
        tr("Word by word")
    } else {
        tr("Line by line")
    }
}

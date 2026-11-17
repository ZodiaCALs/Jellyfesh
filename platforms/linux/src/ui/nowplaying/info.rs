//! The Info panel, "About This Song" (docs/ui.md, "Now Playing"): Title,
//! Artist, Album, Year, Length, Format, Lyrics and File, then Open in
//! {Service} for a song with a link (ADR-0018). While a remote session
//! runs it shows the service's track.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use jf_core::{LyricsSnapshot, LyricsSource, NowPlaying, TrackRef};

use crate::logic::format;
use crate::ui::i18n::{tr, trn};
use crate::ui::state::{app, spawn};

pub struct Info {
    pub root: gtk::ScrolledWindow,
    rows: Vec<(String, adw::ActionRow)>,
    /// Open in {Service}, under the facts.
    open_link: gtk::Button,
    link: Rc<std::cell::RefCell<Option<String>>>,
    /// The queue entry whose year the Year row shows (or is looking up).
    year_of: Rc<Cell<Option<u64>>>,
}

impl std::fmt::Debug for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Info").finish_non_exhaustive()
    }
}

const KEYS: [&str; 8] = [
    "title", "artist", "album", "year", "duration", "format", "lyrics", "file",
];

impl Info {
    pub fn new() -> Info {
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.add_css_class("np-info");
        list.set_selection_mode(gtk::SelectionMode::None);
        let mut rows = Vec::new();
        for key in KEYS {
            let title = match key {
                "title" => tr("Title"),
                "artist" => tr("Artist"),
                "album" => tr("Album"),
                "year" => tr("Year"),
                "duration" => tr("Length"),
                "format" => tr("Format"),
                "lyrics" => tr("Lyrics"),
                _ => tr("File"),
            };
            let row = adw::ActionRow::builder()
                .title(title)
                .subtitle_selectable(true)
                .build();
            row.add_css_class("property");
            list.append(&row);
            rows.push((key.to_owned(), row));
        }
        let heading = gtk::Label::builder()
            .label(tr("About This Song"))
            .xalign(0.0)
            .accessible_role(gtk::AccessibleRole::Heading)
            .build();
        heading.add_css_class("panel-title");
        let open_link = gtk::Button::builder()
            .halign(gtk::Align::Start)
            .visible(false)
            .build();
        let link_face = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        link_face.append(&crate::ui::widgets::glyph::Glyph::new("link", 18));
        link_face.append(&gtk::Label::new(None));
        open_link.set_child(Some(&link_face));
        open_link.add_css_class("pill");
        open_link.add_css_class("action-pill");
        open_link.add_css_class("glass-pill");
        open_link.add_css_class("np-open-link");
        let link: Rc<std::cell::RefCell<Option<String>>> = Rc::new(std::cell::RefCell::new(None));
        let target = link.clone();
        open_link.connect_clicked(move |_| {
            if let Some(l) = target.borrow().clone() {
                crate::ui::actions::open_link(&l);
            }
        });
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.append(&heading);
        body.append(&list);
        body.append(&open_link);
        let root = gtk::ScrolledWindow::builder()
            .child(&body)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        root.add_css_class("info-panel");
        Info {
            root,
            rows,
            open_link,
            link,
            year_of: Rc::new(Cell::new(None)),
        }
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

    /// The remote session's track: what the service reports about it.
    pub fn update_remote(&self, view: &crate::logic::remote::RemoteView) {
        self.year_of.set(None);
        self.set("title", view.title.as_deref().unwrap_or_default());
        self.set("artist", view.artist.as_deref().unwrap_or_default());
        self.set("album", view.album.as_deref().unwrap_or_default());
        self.set("year", "");
        self.set(
            "duration",
            &view.duration_ms.map(format::clock).unwrap_or_default(),
        );
        for key in ["format", "lyrics", "file"] {
            self.set(key, "");
        }
        self.show_link(view.link.as_deref(), &view.service);
    }

    fn set(&self, key: &str, value: &str) {
        if let Some((_, row)) = self.rows.iter().find(|(k, _)| k == key) {
            row.set_subtitle(&gtk::glib::markup_escape_text(value));
            row.set_visible(!value.is_empty());
        }
    }

    pub fn update(&self, now: Option<&NowPlaying>, lyrics: Option<&LyricsSnapshot>) {
        let Some(np) = now else {
            for key in KEYS {
                self.set(key, "");
            }
            self.year_of.set(None);
            self.show_link(None, "");
            return;
        };
        match &np.track {
            TrackRef::Source(s) => self.show_link(np.link.as_deref(), &app().source_name(&s.plugin)),
            _ => self.show_link(None, ""),
        }
        self.set("title", &np.title);
        self.show_year(np);
        self.set("artist", np.artist.as_deref().unwrap_or_default());
        self.set("album", np.album.as_deref().unwrap_or_default());
        self.set("duration", &np.duration_ms.map(format::clock).unwrap_or_default());
        let fmt = np.format.as_ref().map(|f| {
            let mut parts = vec![
                format::codec(&f.codec),
                format::sample_rate(f.sample_rate),
                trn("{n} channel", "{n} channels", u64::from(f.channels)),
            ];
            if f.gapless {
                parts.push(tr("gapless"));
            }
            parts.join(" · ")
        });
        self.set("format", &fmt.unwrap_or_default());
        let lyrics_text = match lyrics.filter(|l| l.uid == np.uid) {
            Some(l) => match (&l.doc, &l.source) {
                (Some(doc), Some(LyricsSource::Sidecar(path))) => format!(
                    "{} · {}",
                    describe_timing(doc),
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
                (Some(doc), _) => format!("{} · {}", describe_timing(doc), tr("in the file's tags")),
                (None, _) => tr("None found"),
            },
            None => String::new(),
        };
        self.set("lyrics", &lyrics_text);
        self.set(
            "file",
            &np.path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        );
    }
}

impl Info {
    /// The year of a library song, from the core (the playing entry does not
    /// carry it); looked up once per entry.
    fn show_year(&self, np: &NowPlaying) {
        if self.year_of.get() == Some(np.uid) {
            return;
        }
        self.year_of.set(Some(np.uid));
        self.set("year", "");
        let TrackRef::Library(id) = np.track else {
            return;
        };
        let Some(row) = self
            .rows
            .iter()
            .find(|(k, _)| k == "year")
            .map(|(_, r)| r.downgrade())
        else {
            return;
        };
        let (year_of, uid) = (self.year_of.clone(), np.uid);
        spawn(async move {
            let year = app()
                .query(move |core| core.track(id).ok().flatten().and_then(|t| t.year))
                .await
                .flatten();
            if let (Some(row), Some(year)) = (row.upgrade(), year)
                && year_of.get() == Some(uid)
            {
                row.set_subtitle(&year.to_string());
                row.set_visible(true);
            }
        });
    }
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

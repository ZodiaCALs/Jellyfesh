//! The Info panel: what is playing and how (format, lyrics source, file).

use adw::prelude::*;
use jf_core::{LyricsSnapshot, LyricsSource, NowPlaying};

use crate::logic::format;
use crate::ui::i18n::{tr, trn};

pub struct Info {
    pub root: gtk::ScrolledWindow,
    rows: Vec<(String, adw::ActionRow)>,
}

impl std::fmt::Debug for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Info").finish_non_exhaustive()
    }
}

const KEYS: [&str; 7] = ["title", "artist", "album", "duration", "format", "lyrics", "file"];

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
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.append(&heading);
        body.append(&list);
        let root = gtk::ScrolledWindow::builder()
            .child(&body)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        root.add_css_class("info-panel");
        Info { root, rows }
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
            return;
        };
        self.set("title", &np.title);
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

fn describe_timing(doc: &jf_core::LyricDoc) -> String {
    if !doc.is_synced() {
        tr("Unsynced")
    } else if doc.lines.iter().any(|l| !l.words.is_empty()) {
        tr("Word by word")
    } else {
        tr("Line by line")
    }
}

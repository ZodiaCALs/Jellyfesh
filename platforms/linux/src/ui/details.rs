//! Get Info (ADR-0022 §3, docs/ui.md "Details"): for a song, About This
//! Song (Title, Artist, Album, Version, Year, Genre, Composer, Lyricist,
//! Label, Length) and Audio (Quality, Format, Bitrate, Size, ReplayGain,
//! File or Source); for an album or an artist, its picture, notes or
//! biography and facts. The core answers `Command::Details` with
//! `Event::Details`; until it does (or when it cannot), what the view
//! already knows shows. `logic::details` picks the rows; this words them.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use jf_core::{
    Command, DetailsPage, FactKey, ItemKind, ItemRef, QualityLabel, ReplayGainValues, SourceItem, TrackRef,
};
use serde::{Deserialize, Serialize};

use crate::logic::details::{self, FormatParts, Key, Known, Value};
use crate::logic::format;
use crate::ui::convert;
use crate::ui::i18n::{tr, trf, trn};
use crate::ui::models::{ItemData, Kind};
use crate::ui::state::{Change, app, spawn};
use crate::ui::widgets::art::Art;

/// What Get Info is for: the item, and what the view knows of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoArgs {
    pub item: ItemRef,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub path: Option<PathBuf>,
    /// The source's name, for a source's item.
    pub source: Option<String>,
}

/// The item a song in the queue or a list stands for.
pub fn item_of_track(track: &TrackRef) -> ItemRef {
    match track {
        TrackRef::Library(id) => ItemRef::Track { id: *id },
        TrackRef::File(path) => ItemRef::File { path: path.clone() },
        TrackRef::Source(s) => ItemRef::Source {
            plugin: s.plugin.clone(),
            item_kind: ItemKind::Track,
            id: s.id.clone(),
        },
    }
}

/// The item a source's row stands for: a song, an album or an artist (the
/// library's own, in unified search, by its id).
fn item_of_source(s: &SourceItem) -> Option<ItemRef> {
    if s.source == crate::logic::search::LIBRARY {
        let id = s.id.parse().ok()?;
        return match s.kind {
            ItemKind::Track => Some(ItemRef::Track { id }),
            ItemKind::Album => Some(ItemRef::Album { id }),
            ItemKind::Artist => Some(ItemRef::Artist { id }),
            ItemKind::Playlist | ItemKind::Folder => None,
        };
    }
    matches!(s.kind, ItemKind::Track | ItemKind::Album | ItemKind::Artist).then(|| ItemRef::Source {
        plugin: s.source.clone(),
        item_kind: s.kind,
        id: s.id.clone(),
    })
}

impl InfoArgs {
    /// Get Info for a row, a card or a page: songs, albums and artists
    /// (not playlists or folders).
    pub fn of(data: &ItemData) -> Option<InfoArgs> {
        let item = match (data.kind, &data.source, &data.track) {
            (Kind::Album, _, _) => ItemRef::Album { id: data.id },
            (Kind::Artist, _, _) => ItemRef::Artist { id: data.id },
            (Kind::Playlist, _, _) => return None,
            (Kind::Source, Some(s), _) => item_of_source(s)?,
            (_, _, Some(track)) => item_of_track(track),
            _ => return None,
        };
        let source = data.source.as_deref();
        let remote = source.filter(|s| s.source != crate::logic::search::LIBRARY);
        Some(InfoArgs {
            item,
            title: Some(data.title.clone()).filter(|t| !t.trim().is_empty()),
            artist: data
                .artist
                .clone()
                .or_else(|| source.and_then(|s| s.subtitle.clone())),
            album: source.and_then(|s| s.album.clone()),
            duration_ms: source.and_then(|s| s.duration_ms),
            path: match &data.track {
                Some(TrackRef::File(path)) => Some(path.clone()),
                _ => None,
            },
            source: remote.map(|s| app().source_name(&s.source)),
        })
    }

    fn known(&self) -> Known {
        Known {
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            year: None,
            duration_ms: self.duration_ms,
            path: self.path.clone(),
            source: self.source.clone(),
        }
    }

    /// Whether the item is a song (About This Song and Audio) rather than
    /// an album or an artist.
    fn is_song(&self) -> bool {
        matches!(
            self.item,
            ItemRef::Track { .. }
                | ItemRef::File { .. }
                | ItemRef::Source {
                    item_kind: ItemKind::Track,
                    ..
                }
        )
    }

    fn is_artist(&self) -> bool {
        matches!(
            self.item,
            ItemRef::Artist { .. }
                | ItemRef::Source {
                    item_kind: ItemKind::Artist,
                    ..
                }
        )
    }
}

/// A row's title.
pub fn word(key: Key) -> String {
    match key {
        Key::Title => tr("Title"),
        Key::Artist => tr("Artist"),
        Key::Album => tr("Album"),
        Key::Version => tr("Version"),
        Key::Year => tr("Year"),
        Key::Genre => tr("Genre"),
        Key::Composer => tr("Composer"),
        Key::Lyricist => tr("Lyricist"),
        Key::Label => tr("Label"),
        Key::Length => tr("Length"),
        Key::Quality => tr("Quality"),
        Key::Format => tr("Format"),
        Key::Bitrate => tr("Bitrate"),
        Key::Size => tr("Size"),
        Key::ReplayGain => tr("ReplayGain"),
        Key::File => tr("File"),
        Key::Source => tr("Source"),
    }
}

/// An album's or an artist's fact, named.
pub fn fact_word(key: &FactKey) -> String {
    match key {
        FactKey::Version => tr("Version"),
        FactKey::Genre => tr("Genre"),
        FactKey::Language => tr("Language"),
        FactKey::Released => tr("Released"),
        FactKey::Label => tr("Label"),
        FactKey::Composer => tr("Composer"),
        FactKey::Lyricist => tr("Lyricist"),
        FactKey::Arranger => tr("Arranger"),
        FactKey::Producer => tr("Producer"),
        FactKey::Popularity => tr("Popularity"),
        FactKey::Quality => tr("Quality"),
        FactKey::Isrc => tr("ISRC"),
        FactKey::Disc => tr("Disc"),
        FactKey::Track => tr("Track"),
        FactKey::Other { label } => label.clone(),
    }
}

/// The core's quality label, worded.
pub fn quality(label: QualityLabel) -> String {
    match label {
        QualityLabel::HiResLossless => tr("Hi-Res Lossless"),
        QualityLabel::Lossless => tr("Lossless"),
        QualityLabel::High => tr("High"),
        QualityLabel::Standard => tr("Standard"),
    }
}

/// "FLAC · 24-bit · 96 kHz · Stereo".
pub fn format_line(parts: &FormatParts) -> String {
    let depth = parts.bit_depth.map(|b| trf("{n}-bit", &[("n", &b.to_string())]));
    let rate = parts.sample_rate_hz.map(format::sample_rate);
    let channels = parts.channels.map(|c| match c {
        1 => tr("Mono"),
        2 => tr("Stereo"),
        n => trn("{n} channel", "{n} channels", u64::from(n)),
    });
    format::dot_join([
        parts.codec.as_deref(),
        depth.as_deref(),
        rate.as_deref(),
        channels.as_deref(),
    ])
}

/// "Track −6.20 dB · Album −5.80 dB".
fn replaygain(values: &ReplayGainValues) -> String {
    let gain = details::decibels;
    let track = values
        .track_gain_db
        .map(|g| trf("Track {gain}", &[("gain", &gain(g))]));
    let album = values
        .album_gain_db
        .map(|g| trf("Album {gain}", &[("gain", &gain(g))]));
    format::dot_join([track.as_deref(), album.as_deref()])
}

/// A row's value, worded.
pub fn value_text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        Value::Length(ms) => format::clock(*ms),
        Value::Quality(label) => quality(*label),
        Value::Format(parts) => format_line(parts),
        Value::Bitrate(kbps) => trf("{n} kbps", &[("n", &format::count(u64::from(*kbps)))]),
        Value::Size(bytes) => format::size(*bytes),
        Value::ReplayGain(values) => replaygain(values),
        Value::Path(path) => path.display().to_string(),
    }
}

/// A boxed list of property rows (hidden when empty).
pub fn rows_list(rows: &[(String, String)]) -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    fill_rows(&list, rows);
    list
}

/// Replaces the rows of `list`.
pub fn fill_rows(list: &gtk::ListBox, rows: &[(String, String)]) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    for (title, value) in rows {
        let row = adw::ActionRow::builder()
            .title(title.as_str())
            .subtitle(gtk::glib::markup_escape_text(value).as_str())
            .subtitle_selectable(true)
            .build();
        row.add_css_class("property");
        list.append(&row);
    }
    list.set_visible(!rows.is_empty());
}

/// The rows, worded.
pub fn worded(rows: &[(Key, Value)]) -> Vec<(String, String)> {
    rows.iter().map(|(k, v)| (word(*k), value_text(v))).collect()
}

/// A section heading.
pub fn heading(text: &str) -> gtk::Label {
    let l = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    l.add_css_class("heading");
    l
}

/// Notes or a biography: four lines, and More shows the rest.
pub fn description(text: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let l = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .lines(4)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .selectable(true)
        .build();
    l.add_css_class("details-description");
    let more = gtk::Button::with_label(&tr("More"));
    more.add_css_class("flat");
    more.set_halign(gtk::Align::Start);
    let weak = l.downgrade();
    more.connect_clicked(move |b| {
        if let Some(l) = weak.upgrade() {
            l.set_lines(-1);
            l.set_ellipsize(gtk::pango::EllipsizeMode::None);
        }
        b.set_visible(false);
    });
    b.append(&l);
    b.append(&more);
    b
}

/// A dim line of text: an attribution, or why details are missing.
fn note(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::builder().label(text).xalign(0.0).wrap(true).build();
    l.add_css_class(class);
    l
}

/// Fills `body` with what is known and, once it arrives, the core's page:
/// the picture and the heading, then About This Song and Audio for a song,
/// or the notes or biography and the facts for an album or an artist.
fn fill(body: &gtk::Box, args: &InfoArgs, page: Option<&DetailsPage>, failed: Option<&str>) {
    while let Some(c) = body.first_child() {
        body.remove(&c);
    }
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.add_css_class("details-header");
    if let Some(image) = page.and_then(|p| p.image.as_ref()) {
        let art = Art::new(64, jf_tokens::artwork::RADIUS_LIST);
        art.set_round(args.is_artist());
        art.set_valign(gtk::Align::Center);
        art.set_art(convert::art_of(Some(image)));
        header.append(&art);
    }
    let names = gtk::Box::new(gtk::Orientation::Vertical, 2);
    names.set_valign(gtk::Align::Center);
    let kind = if args.is_song() {
        tr("About This Song")
    } else if args.is_artist() {
        tr("About This Artist")
    } else {
        tr("About This Album")
    };
    names.append(&heading(&kind));
    let title = page
        .map(|p| p.title.clone())
        .filter(|t| !t.trim().is_empty())
        .or_else(|| args.title.clone());
    if let Some(title) = title {
        let t = gtk::Label::builder()
            .label(&title)
            .xalign(0.0)
            .wrap(true)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        t.add_css_class("dim-label");
        names.append(&t);
    }
    header.append(&names);
    body.append(&header);
    if args.is_song() {
        let (about, audio) = details::song_rows(page, &args.known(), None);
        body.append(&rows_list(&worded(&about)));
        if !audio.is_empty() {
            body.append(&heading(&tr("Audio")));
            body.append(&rows_list(&worded(&audio)));
        }
    } else {
        if let Some(text) = page
            .and_then(|p| p.description.as_deref())
            .filter(|d| !d.trim().is_empty())
        {
            body.append(&description(text));
        }
        let facts: Vec<(String, String)> = page
            .map(details::item_facts)
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| (fact_word(&key), value))
            .collect();
        if !facts.is_empty() {
            body.append(&rows_list(&facts));
        }
    }
    if let Some(attribution) = page.and_then(|p| p.attribution.as_deref()) {
        body.append(&note(
            &trf("Content from {service}", &[("service", attribution)]),
            "attribution",
        ));
    }
    if let Some(error) = page.and_then(|p| p.error.as_deref()).or(failed) {
        body.append(&note(error, "dim-label"));
    } else if page.is_none() {
        let spinner = gtk::Spinner::builder()
            .spinning(true)
            .halign(gtk::Align::Start)
            .build();
        spinner.update_property(&[gtk::accessible::Property::Label(&tr("Loading"))]);
        body.append(&spinner);
    }
}

/// Shows Get Info for an item.
pub fn present(args: InfoArgs) {
    let a = app();
    let dialog = adw::Dialog::builder()
        .title(tr("Get Info"))
        .content_width(460)
        .content_height(560)
        .build();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.add_css_class("details");
    let scroll = gtk::ScrolledWindow::builder()
        .child(&body)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&scroll));
    dialog.set_child(Some(&view));
    fill(&body, &args, None, None);
    let request = a.send_quiet(Command::Details {
        item: args.item.clone(),
    });
    let library_song = match args.item {
        ItemRef::Track { id } if args.album.is_none() => Some(id),
        _ => None,
    };
    let shown = Rc::new(Shown {
        args: RefCell::new(args),
        page: RefCell::new(None),
        failed: RefCell::new(None),
    });
    let redraw = {
        let (weak, shown) = (body.downgrade(), shown.clone());
        move || {
            if let Some(body) = weak.upgrade() {
                let page = shown.page.borrow().clone();
                fill(
                    &body,
                    &shown.args.borrow(),
                    page.as_deref(),
                    shown.failed.borrow().as_deref(),
                );
            }
        }
    };
    let redraw = Rc::new(redraw);
    let (again, kept) = (redraw.clone(), Rc::downgrade(&shown));
    a.listen_for(&dialog, move |change| match change {
        Change::Details(page) if page.request == request => {
            *shown.page.borrow_mut() = Some(Arc::clone(page));
            again();
        }
        Change::Failed(id, error) if *id == request => {
            *shown.failed.borrow_mut() = Some((*error).to_owned());
            again();
        }
        _ => {}
    });
    // A library song's album, which its details do not carry.
    if let Some(id) = library_song {
        let redraw = Rc::downgrade(&redraw);
        spawn(async move {
            let album = app()
                .query(move |core| core.track(id).ok().flatten().and_then(|t| t.album))
                .await
                .flatten();
            if let (Some(album), Some(shown), Some(redraw)) = (album, kept.upgrade(), redraw.upgrade()) {
                shown.args.borrow_mut().album = Some(album);
                redraw();
            }
        });
    }
    let parent = a.window.borrow().upgrade();
    dialog.present(parent.as_ref());
}

/// What the dialog shows: the item and what is known, the core's page, and
/// why there is none.
struct Shown {
    args: RefCell<InfoArgs>,
    page: RefCell<Option<Arc<DetailsPage>>>,
    failed: RefCell<Option<String>>,
}

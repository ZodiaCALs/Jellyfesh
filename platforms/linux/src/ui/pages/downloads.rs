//! Downloads (docs/ui.md, "Downloads"; ADR-0025): Play and Shuffle with
//! "{n} songs · {size} used"; In Progress, each download with "Waiting", its
//! percentage, or why it failed and Try Again, and Cancel on each; why
//! downloads wait while they do; then the downloaded songs by album, with
//! Remove Download… on each song and on each album (asks first). Empty:
//! "No Downloads Yet".

use std::rc::Rc;

use adw::prelude::*;
use jf_core::{Collection, Command, DownloadError, DownloadItem, DownloadsPaused};

use crate::logic::downloads::{self, Album, Progress};
use crate::logic::format;
use crate::ui::actions;
use crate::ui::i18n::{tr, trf, trn};
use crate::ui::pages::{
    follow_title, frame, heading, keep_alive, large_title, play_and_shuffle, scroller, small_title, status,
};
use crate::ui::state::{Change, app, spawn};
use crate::ui::widgets::glyph::{self, Glyph};
use crate::ui::widgets::maxwidth::MaxWidth;

/// Why a download failed, in the app's words.
pub fn error_text(error: &DownloadError) -> String {
    match error {
        DownloadError::NotAllowed { service } => trf(
            "Your {service} server doesn't allow downloads for this account.",
            &[("service", service)],
        ),
        DownloadError::NotOffered { service } => {
            trf("{service} doesn't offer downloads.", &[("service", service)])
        }
        DownloadError::Network { message }
        | DownloadError::Storage { message }
        | DownloadError::Other { message } => message.clone(),
    }
}

struct Page {
    summary: gtk::Label,
    actions: gtk::Box,
    waiting: gtk::Label,
    in_progress: gtk::Box,
    albums: gtk::Box,
    stack: gtk::Stack,
}

pub fn page() -> adw::NavigationPage {
    let title = large_title(&tr("Downloads"));
    let summary = gtk::Label::builder().xalign(0.0).build();
    summary.add_css_class("page-subtitle");
    let (actions_row, play, shuffle) = play_and_shuffle();
    play.connect_clicked(|_| play_downloads(0, false));
    shuffle.connect_clicked(|_| play_downloads(0, true));
    let waiting = gtk::Label::builder().xalign(0.0).wrap(true).build();
    waiting.add_css_class("downloads-waiting");
    let in_progress = gtk::Box::new(gtk::Orientation::Vertical, 8);
    in_progress.add_css_class("downloads-in-progress");
    let albums = gtk::Box::new(gtk::Orientation::Vertical, 20);
    albums.add_css_class("downloads-albums");

    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.add_css_class("page-column");
    column.add_css_class("downloads-page");
    column.append(&title);
    column.append(&summary);
    column.append(&actions_row);
    column.append(&waiting);
    column.append(&in_progress);
    column.append(&albums);
    let holder = MaxWidth::new(&column, 1000);
    let scroll = scroller(&holder);
    let empty = status(
        "folder-download-symbolic",
        &tr("No Downloads Yet"),
        &tr("Download albums and playlists from your servers to play them offline."),
    );
    let stack = gtk::Stack::new();
    stack.add_named(&scroll, Some("list"));
    stack.add_named(&empty, Some("empty"));
    let (page, header) = frame(&tr("Downloads"), "downloads", &stack);
    let small = small_title(&header, &tr("Downloads"));
    follow_title(&scroll, &title, &small);

    let this = Rc::new(Page {
        summary,
        actions: actions_row,
        waiting,
        in_progress,
        albums,
        stack,
    });
    render(&this);
    let weak = Rc::downgrade(&this);
    app().listen_for(&page, move |change| {
        if let (Change::Downloads, Some(p)) = (change, weak.upgrade()) {
            render(&p);
        }
    });
    keep_alive(&page, this);
    page
}

/// Plays the downloaded songs from the `start`th, shuffled or not.
fn play_downloads(start: usize, shuffle: bool) {
    let tracks = downloads::finished(&app().downloads.borrow())
        .into_iter()
        .map(downloads::track)
        .collect::<Vec<_>>();
    if !tracks.is_empty() {
        actions::play(Collection::Tracks { tracks }, start, shuffle);
    }
}

fn render(p: &Page) {
    let d = app().downloads.borrow().clone();
    let finished = downloads::finished(&d);
    let in_progress = downloads::in_progress(&d);
    p.stack
        .set_visible_child_name(if d.items.is_empty() { "empty" } else { "list" });
    // "12 songs · 1.2 GB used", once something has downloaded.
    let summary = (!finished.is_empty()).then(|| {
        format!(
            "{} · {}",
            trn("{n} song", "{n} songs", finished.len() as u64),
            trf("{size} used", &[("size", &format::size(d.bytes_used))]),
        )
    });
    p.summary.set_text(summary.as_deref().unwrap_or(""));
    p.summary.set_visible(summary.is_some());
    p.actions.set_visible(!finished.is_empty());
    let waiting = downloads::waiting_because(&d).map(|why| match why {
        DownloadsPaused::Offline => tr("Downloads resume when you're back online."),
        DownloadsPaused::Metered => tr("Downloads resume when you're off a metered connection."),
    });
    p.waiting.set_text(waiting.as_deref().unwrap_or(""));
    p.waiting.set_visible(waiting.is_some());

    clear(&p.in_progress);
    if !in_progress.is_empty() {
        p.in_progress.append(&heading(&tr("In Progress")));
        let list = boxed_list();
        for item in in_progress {
            list.append(&progress_row(item));
        }
        p.in_progress.append(&list);
    }
    p.in_progress.set_visible(p.in_progress.first_child().is_some());

    clear(&p.albums);
    let mut index = 0;
    for album in downloads::albums(&d) {
        p.albums.append(&album_block(&album, index));
        index += album.songs.len();
    }
    p.albums.set_visible(p.albums.first_child().is_some());
}

fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

fn boxed_list() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list
}

/// A download in progress: the song, where it is, Try Again after a
/// failure, and Cancel.
fn progress_row(item: &DownloadItem) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(glib_escape(&item.track.title))
        .build();
    let id = item.id;
    let (line, fraction) = match downloads::progress(item) {
        Progress::Waiting => (tr("Waiting"), None),
        Progress::Downloading(Some(percent)) => (
            trf("{percent} %", &[("percent", &percent.to_string())]),
            Some(f64::from(percent) / 100.0),
        ),
        Progress::Downloading(None) => (tr("Downloading…"), None),
        Progress::Failed(error) => {
            let again = gtk::Button::builder()
                .label(tr("Try Again"))
                .valign(gtk::Align::Center)
                .build();
            again.add_css_class("pill");
            again.connect_clicked(move |_| {
                app().send(Command::RetryDownload { id });
            });
            row.add_suffix(&again);
            row.add_css_class("failed");
            (error_text(&error), None)
        }
    };
    row.set_subtitle(&glib_escape(&line));
    if let Some(f) = fraction {
        let bar = gtk::ProgressBar::builder()
            .fraction(f)
            .valign(gtk::Align::Center)
            .width_request(96)
            .build();
        bar.update_property(&[gtk::accessible::Property::Label(&line)]);
        row.add_suffix(&bar);
    }
    let cancel = gtk::Button::builder()
        .label(tr("Cancel"))
        .valign(gtk::Align::Center)
        .build();
    cancel.add_css_class("flat");
    cancel.connect_clicked(move |_| {
        app().send(Command::CancelDownload { id });
    });
    row.add_suffix(&cancel);
    row
}

/// An album's downloaded songs, with Remove Download… for all of them.
/// `first` is the place of its first song among all the downloaded songs.
fn album_block(album: &Album<'_>, first: usize) -> gtk::Box {
    let name = album
        .title
        .clone()
        .unwrap_or_else(crate::ui::convert::unknown_album);
    let title = heading(&name);
    title.set_hexpand(true);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
    texts.append(&title);
    if let Some(artist) = &album.artist {
        let l = gtk::Label::builder().label(artist).xalign(0.0).build();
        l.add_css_class("page-subtitle");
        texts.append(&l);
    }
    let ids: Vec<u64> = album.songs.iter().map(|s| s.id).collect();
    let remove = remove_button(&name, ids);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    head.add_css_class("downloads-album-head");
    head.append(&texts);
    head.append(&remove);

    let list = boxed_list();
    for (i, song) in album.songs.iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(glib_escape(&song.track.title))
            .activatable(true)
            .build();
        if let Some(artist) = &song.track.artist {
            row.set_subtitle(&glib_escape(artist));
        }
        let mark = Glyph::new("download", 16);
        mark.add_css_class("downloaded-mark");
        mark.set_tooltip_text(Some(&tr("Downloaded")));
        row.add_prefix(&mark);
        if let Some(ms) = song.track.duration_ms {
            let l = gtk::Label::new(Some(&format::clock(ms)));
            l.add_css_class("dim-label");
            row.add_suffix(&l);
        }
        row.add_suffix(&remove_button(&song.track.title, vec![song.id]));
        let start = first + i;
        row.connect_activated(move |_| play_downloads(start, false));
        list.append(&row);
    }
    let block = gtk::Box::new(gtk::Orientation::Vertical, 8);
    block.append(&head);
    block.append(&list);
    block
}

/// Remove Download…: asks first, then removes the downloads `ids` (a song,
/// or an album's songs).
fn remove_button(name: &str, ids: Vec<u64>) -> gtk::Button {
    let b = glyph::button("trash", 16, &tr("Remove Download…"));
    b.add_css_class("flat");
    b.set_valign(gtk::Align::Center);
    let name = name.to_owned();
    b.connect_clicked(move |_| {
        let (name, ids) = (name.clone(), ids.clone());
        spawn(async move {
            let a = app();
            let dialog = adw::AlertDialog::new(
                Some(&trf("Remove the Download of “{name}”?", &[("name", &name)])),
                Some(&tr(
                    "The file is deleted from this device. You can download it again.",
                )),
            );
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("remove", &tr("Remove Download"));
            dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
            dialog.set_close_response("cancel");
            let parent = a.window.borrow().upgrade();
            if dialog.choose_future(parent.as_ref()).await == "remove" {
                for id in ids {
                    a.send(Command::RemoveDownload { id });
                }
            }
        });
    });
    b
}

/// Rows take markup: song titles are text.
fn glib_escape(text: &str) -> String {
    gtk::glib::markup_escape_text(text).to_string()
}

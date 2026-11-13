//! Home: what was added and played recently.

use std::collections::HashSet;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Collection, Track, TrackRef};

use crate::ui::convert;
use crate::ui::i18n::{tr, trn};
use crate::ui::models::{Item, ItemData, Kind};
use crate::ui::pages::{follow_title, frame, heading, keep_alive, large_title, scroller, small_title};
use crate::ui::rows::{Card, ListRow, RowStyle};
use crate::ui::state::{Change, app, spawn};
use crate::ui::thumbs::ArtKey;
use crate::ui::window;

const RECENT_TRACKS: usize = 200;
const RECENT_ALBUMS: usize = 12;
const RECENT_PLAYED: usize = 10;

struct Home {
    stats: gtk::Label,
    welcome: gtk::Widget,
    added_heading: gtk::Label,
    added: gtk::Box,
    added_scroll: gtk::ScrolledWindow,
    played_heading: gtk::Label,
    played: gtk::ListBox,
    played_refs: std::cell::RefCell<Vec<TrackRef>>,
}

pub fn page() -> adw::NavigationPage {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.add_css_class("home");
    let title = large_title(&tr("Home"));
    let stats = gtk::Label::new(None);
    stats.set_xalign(0.0);
    stats.add_css_class("page-subtitle");
    body.append(&title);
    body.append(&stats);

    let welcome = welcome();
    body.append(&welcome);

    let added_heading = heading(&tr("Recently Added"));
    let added = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    added.add_css_class("shelf");
    let added_scroll = gtk::ScrolledWindow::builder()
        .child(&added)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .build();
    added_scroll.add_css_class("shelf-scroll");
    body.append(&added_heading);
    body.append(&added_scroll);

    let played_heading = heading(&tr("Recently Played"));
    let played = gtk::ListBox::new();
    played.add_css_class("track-box");
    played.set_selection_mode(gtk::SelectionMode::None);
    played.update_property(&[gtk::accessible::Property::Label(&tr("Recently Played"))]);
    body.append(&played_heading);
    body.append(&played);

    let clamp = adw::Clamp::builder().child(&body).maximum_size(1100).build();
    let scroll = scroller(&clamp);
    let (page, header) = frame(&tr("Home"), "home", &scroll);
    let small = small_title(&header, &tr("Home"));
    follow_title(&scroll, &title, &small);

    let home = std::rc::Rc::new(Home {
        stats,
        welcome: welcome.upcast(),
        added_heading,
        added,
        added_scroll,
        played_heading,
        played,
        played_refs: std::cell::RefCell::new(Vec::new()),
    });
    refresh(&home);
    let weak = std::rc::Rc::downgrade(&home);
    app().listen_for(&page, move |change| {
        if let Some(home) = weak.upgrade()
            && matches!(change, Change::Library | Change::NowPlaying)
        {
            refresh(&home);
        }
    });
    home.played.connect_row_activated(glib::clone!(
        #[weak]
        home,
        move |_, row| {
            let refs = home.played_refs.borrow().clone();
            if let Ok(index) = usize::try_from(row.index()) {
                crate::ui::actions::play(Collection::Tracks { tracks: refs }, index, false);
            }
        }
    ));
    // The page keeps the parts alive.
    keep_alive(&page, home);
    page
}

fn welcome() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.add_css_class("welcome");
    let t = gtk::Label::new(Some(&tr("Welcome to Jellyfesh")));
    t.add_css_class("welcome-title");
    t.set_xalign(0.0);
    let d = gtk::Label::new(Some(&tr(
        "Add the folders that hold your music. Jellyfesh reads their tags, artwork and lyrics and keeps watching them.",
    )));
    d.set_wrap(true);
    d.set_xalign(0.0);
    d.add_css_class("welcome-text");
    let add = gtk::Button::with_label(&tr("Add Music Folder…"));
    add.add_css_class("pill");
    add.add_css_class("suggested-action");
    add.set_halign(gtk::Align::Start);
    add.connect_clicked(|_| window::choose_folder());
    b.append(&t);
    b.append(&d);
    b.append(&add);
    b.set_visible(false);
    b
}

/// Albums of the most recently added tracks, newest first, each once.
fn recent_albums(tracks: &[Track]) -> Vec<ItemData> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for t in tracks {
        let Some(album_id) = t.album_id else { continue };
        if !seen.insert(album_id) {
            continue;
        }
        out.push(ItemData {
            kind: Kind::Album,
            id: album_id,
            title: t.album.clone().unwrap_or_else(convert::unknown_album),
            subtitle: t
                .album_artist
                .clone()
                .or_else(|| t.artist.clone())
                .unwrap_or_else(convert::unknown_artist),
            art: t.artwork_hash.clone().map(ArtKey::Hash),
            album_id: Some(album_id),
            playable: true,
            ..ItemData::default()
        });
        if out.len() == RECENT_ALBUMS {
            break;
        }
    }
    out
}

fn refresh(home: &std::rc::Rc<Home>) {
    let weak = std::rc::Rc::downgrade(home);
    spawn(async move {
        let a = app();
        let result = a
            .query(|core| {
                let stats = core.stats().ok();
                let added = core.recently_added(RECENT_TRACKS).unwrap_or_default();
                let played: Vec<ItemData> = match core.recently_played(RECENT_PLAYED) {
                    Ok(rows) => rows.iter().map(convert::track_row).collect(),
                    // Until play statistics land: the queue's play history.
                    Err(_) => {
                        let mut seen = HashSet::new();
                        core.history()
                            .into_iter()
                            .rev()
                            .filter(|h| seen.insert(h.track.clone()))
                            .filter_map(|h| match h.track {
                                TrackRef::Library(id) => {
                                    core.track(id).ok().flatten().map(|t| convert::track(&t))
                                }
                                _ => None,
                            })
                            .take(RECENT_PLAYED)
                            .collect()
                    }
                };
                (stats, recent_albums(&added), played)
            })
            .await;
        let (Some(home), Some((stats, albums, played))) = (weak.upgrade(), result) else {
            return;
        };
        let tracks = stats.map_or(0, |s| s.tracks);
        home.stats.set_label(&match stats {
            Some(s) if s.tracks > 0 => format!(
                "{} · {} · {}",
                trn("{n} song", "{n} songs", s.tracks),
                trn("{n} album", "{n} albums", s.albums),
                trn("{n} artist", "{n} artists", s.artists)
            ),
            _ => String::new(),
        });
        home.welcome.set_visible(tracks == 0);

        while let Some(child) = home.added.first_child() {
            home.added.remove(&child);
        }
        for data in &albums {
            let card = Card::new(160);
            card.set_size_request(160, -1);
            card.bind(&Item::new(Some(data.clone())));
            card.set_open(|d| window::show_album(d.id, Some(d.clone())));
            home.added.append(&card);
        }
        home.added_heading.set_visible(!albums.is_empty());
        home.added_scroll.set_visible(!albums.is_empty());

        while let Some(child) = home.played.first_child() {
            home.played.remove(&child);
        }
        *home.played_refs.borrow_mut() = played.iter().filter_map(|d| d.track.clone()).collect();
        for data in &played {
            let row = ListRow::new(RowStyle::TRACK);
            row.bind(&Item::new(Some(data.clone())));
            let lbr = gtk::ListBoxRow::builder().child(&row).activatable(true).build();
            home.played.append(&lbr);
        }
        home.played_heading.set_visible(!played.is_empty());
        home.played.set_visible(!played.is_empty());
    });
}

//! Home (docs/ui.md, "Home"): the library's size, then Recently Added
//! (albums), Recently Played and Most Played (songs), in the look's order
//! with Favourites when it lists them (ADR-0020); a welcome instead while
//! the library is empty and no source is signed in.

use std::collections::HashSet;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Collection, Track, TrackRef};

use crate::ui::convert;
use crate::ui::i18n::{tr, trc, trn};
use crate::ui::models::{Item, ItemData, Kind};
use crate::ui::pages::{follow_title, frame, heading, keep_alive, large_title, scroller, small_title};
use crate::ui::rows::{Card, ListRow, RowStyle};
use crate::ui::state::{Change, app, spawn};
use crate::ui::thumbs::ArtKey;
use crate::ui::widgets::maxwidth::MaxWidth;
use crate::ui::window::{self, Section};

const RECENT_TRACKS: usize = 200;
const RECENT_ALBUMS: usize = 12;
const RECENT_PLAYED: usize = 10;
const MOST_PLAYED: usize = 10;
const FAVOURITES: usize = 10;

/// A list of songs under a heading (Recently Played, Most Played); a row
/// plays the list from that song.
struct Shelf {
    /// The heading and the list, together, to put in the look's order.
    section: gtk::Box,
    heading: gtk::Label,
    list: gtk::ListBox,
    refs: std::cell::RefCell<Vec<TrackRef>>,
}

impl Shelf {
    fn new(title: &str) -> std::rc::Rc<Shelf> {
        let heading = heading(title);
        let list = gtk::ListBox::new();
        list.add_css_class("track-box");
        list.set_selection_mode(gtk::SelectionMode::None);
        list.update_property(&[gtk::accessible::Property::Label(title)]);
        let section = gtk::Box::new(gtk::Orientation::Vertical, 12);
        section.append(&heading);
        section.append(&list);
        let shelf = std::rc::Rc::new(Shelf {
            section,
            heading,
            list,
            refs: std::cell::RefCell::new(Vec::new()),
        });
        shelf.list.connect_row_activated(glib::clone!(
            #[weak]
            shelf,
            move |_, row| {
                let refs = shelf.refs.borrow().clone();
                if let Ok(index) = usize::try_from(row.index()) {
                    crate::ui::actions::play(Collection::Tracks { tracks: refs }, index, false);
                }
            }
        ));
        shelf
    }

    fn fill(&self, rows: &[ItemData]) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        *self.refs.borrow_mut() = rows.iter().filter_map(|d| d.track.clone()).collect();
        for data in rows {
            let row = ListRow::new(RowStyle::TRACK);
            row.bind(&Item::new(Some(data.clone())));
            let lbr = gtk::ListBoxRow::builder().child(&row).activatable(true).build();
            self.list.append(&lbr);
        }
        self.heading.set_visible(!rows.is_empty());
        self.list.set_visible(!rows.is_empty());
    }
}

struct Home {
    stats: gtk::Label,
    welcome: gtk::Widget,
    /// The sections, in the look's order.
    sections: gtk::Box,
    added_section: gtk::Box,
    added_heading: gtk::Label,
    added: gtk::Box,
    added_scroll: gtk::ScrolledWindow,
    played: std::rc::Rc<Shelf>,
    most: std::rc::Rc<Shelf>,
    favourites: std::rc::Rc<Shelf>,
    /// The order the sections were last put in.
    order: std::cell::RefCell<Vec<jf_core::HomeSection>>,
}

pub fn page() -> adw::NavigationPage {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.add_css_class("home");
    let title = large_title(&tr("Home"));
    let stats = gtk::Label::new(None);
    stats.set_xalign(0.0);
    stats.add_css_class("page-subtitle");
    body.append(&title);

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
    let added_section = gtk::Box::new(gtk::Orientation::Vertical, 12);
    added_section.append(&added_heading);
    added_section.append(&added_scroll);

    let played = Shelf::new(&tr("Recently Played"));
    let most = Shelf::new(&tr("Most Played"));
    let favourites = Shelf::new(&trc("sidebar", "Favourites"));
    let sections = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.append(&sections);

    // At most 1,100 px wide, lined up with the page's start (the web's).
    let clamp = MaxWidth::new(&body, 1100);
    let scroll = scroller(&clamp);
    let (page, header) = frame(&tr("Home"), "home", &scroll);
    let small = small_title(&header, &tr("Home"));
    follow_title(&scroll, &title, &small);

    let home = std::rc::Rc::new(Home {
        stats,
        welcome: welcome.upcast(),
        sections,
        added_section,
        added_heading,
        added,
        added_scroll,
        played,
        most,
        favourites,
        order: std::cell::RefCell::new(Vec::new()),
    });
    arrange(&home);
    refresh(&home);
    let weak = std::rc::Rc::downgrade(&home);
    app().listen_for(&page, move |change| {
        let Some(home) = weak.upgrade() else { return };
        match change {
            Change::Library | Change::NowPlaying | Change::Sources => refresh(&home),
            Change::Look => {
                arrange(&home);
                refresh(&home);
            }
            _ => {}
        }
    });
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
        "Add your music folders, or connect a server in Sources.",
    )));
    d.set_wrap(true);
    d.set_xalign(0.0);
    d.add_css_class("welcome-text");
    let add = gtk::Button::with_label(&tr("Add Music Folder…"));
    add.add_css_class("pill");
    add.add_css_class("suggested-action");
    add.connect_clicked(|_| window::choose_folder());
    let source = gtk::Button::with_label(&tr("Add a Source"));
    source.add_css_class("pill");
    source.connect_clicked(|_| window::show_section(Section::Sources));
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::Start);
    buttons.append(&add);
    buttons.append(&source);
    b.append(&t);
    b.append(&d);
    b.append(&buttons);
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

/// The sections in the look's order (Luminous's while it is not
/// customized): the library's size, Recently Added, Recently Played, Most
/// Played and Favourites, each where the look lists it.
fn arrange(home: &Home) {
    use jf_core::HomeSection;
    let order = crate::logic::look::home(&app().look.borrow());
    if *home.order.borrow() == order {
        return;
    }
    while let Some(child) = home.sections.first_child() {
        home.sections.remove(&child);
    }
    for section in &order {
        let widget: gtk::Widget = match section {
            HomeSection::Summary => home.stats.clone().upcast(),
            HomeSection::RecentlyAdded => home.added_section.clone().upcast(),
            HomeSection::RecentlyPlayed => home.played.section.clone().upcast(),
            HomeSection::MostPlayed => home.most.section.clone().upcast(),
            HomeSection::Favourites => home.favourites.section.clone().upcast(),
        };
        home.sections.append(&widget);
    }
    *home.order.borrow_mut() = order;
}

/// Whether the look shows `section` on Home.
fn shows(home: &Home, section: jf_core::HomeSection) -> bool {
    home.order.borrow().contains(&section)
}

fn refresh(home: &std::rc::Rc<Home>) {
    let favourites_shown = shows(home, jf_core::HomeSection::Favourites);
    let weak = std::rc::Rc::downgrade(home);
    spawn(async move {
        let a = app();
        let result = a
            .query(move |core| {
                let stats = core.stats().ok();
                let added = core.recently_added(RECENT_TRACKS).unwrap_or_default();
                let most: Vec<ItemData> = core
                    .most_played(MOST_PLAYED)
                    .map(|rows| rows.iter().map(convert::track_row).collect())
                    .unwrap_or_default();
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
                let favourites: Vec<ItemData> = if favourites_shown {
                    core.favorites(0, FAVOURITES)
                        .map(|rows| rows.iter().map(convert::track_row).collect())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                (stats, recent_albums(&added), played, most, favourites)
            })
            .await;
        let (Some(home), Some((stats, albums, played, most, favourites))) = (weak.upgrade(), result) else {
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
        home.stats.set_visible(!home.stats.label().is_empty());
        // The welcome, while there is nothing to play: no library and no
        // source signed in.
        let signed_in = app().sources.borrow().plugins.iter().any(|p| p.account.is_some());
        home.welcome.set_visible(tracks == 0 && !signed_in);

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

        home.played.fill(&played);
        home.most.fill(&most);
        home.favourites.fill(&favourites);
    });
}

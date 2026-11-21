//! A source's Discover page (docs/ui.md "Discover", ADR-0022): choosing a
//! source on the Sources page opens it. The source's name and status, then
//! its shelves from the core (`Command::Discover`, `Event::Discover`), each
//! with its title and See All when it has more (which browses it): row
//! shelves scroll sideways, grid shelves wrap, chart shelves are numbered
//! lists. Browse (the source's own folders) is a button in the header.
//! While loading, placeholders the shape of the shelves; on failure, the
//! error and Try Again.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use jf_core::{Command, CommandId, DiscoverPage, Shelf, ShelfStyle};

use crate::logic::discover::{self, Known};
use crate::ui::convert;
use crate::ui::i18n::tr;
use crate::ui::models::{Item, ItemData};
use crate::ui::pages::detail::{fill_track_box, track_box};
use crate::ui::pages::sources::{account_text, attribution_text, badge_text};
use crate::ui::pages::{
    follow_title, frame, heading, keep_alive, large_title, scroller, small_title, status,
};
use crate::ui::rows::{Card, RowStyle};
use crate::ui::state::{Change, app};
use crate::ui::{actions, window};

/// The cover size on shelves.
const CARD: i32 = 160;

struct Discover {
    plugin: String,
    stack: gtk::Stack,
    shelves: gtk::Box,
    error: adw::StatusPage,
    request: Cell<Option<CommandId>>,
}

pub fn page(plugin: &str) -> adw::NavigationPage {
    let a = app();
    let name = a.source_name(plugin);
    let summary = a
        .sources
        .borrow()
        .plugins
        .iter()
        .find(|p| p.id == plugin)
        .cloned();
    let title = large_title(&name);
    let line = gtk::Label::builder().xalign(0.0).wrap(true).build();
    line.add_css_class("page-subtitle");
    if let Some(p) = &summary {
        line.set_label(&crate::logic::format::dot_join([
            Some(badge_text(p).as_str()),
            Some(account_text(p).as_str()),
        ]));
    }
    let top = gtk::Box::new(gtk::Orientation::Vertical, 6);
    top.append(&title);
    top.append(&line);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 20);
    body.add_css_class("discover");
    body.append(&top);
    if let Some(text) = attribution_text(plugin) {
        let note = gtk::Label::builder().label(text).xalign(0.0).wrap(true).build();
        note.add_css_class("attribution");
        body.append(&note);
    }

    let stack = gtk::Stack::new();
    stack.set_vhomogeneous(false);
    stack.add_named(&placeholders(), Some("loading"));
    let shelves = gtk::Box::new(gtk::Orientation::Vertical, 12);
    stack.add_named(&shelves, Some("shelves"));
    let error = status("dialog-warning-symbolic", &tr("This Source Did Not Answer"), "");
    error.set_vexpand(false);
    let again = gtk::Button::with_label(&tr("Try Again"));
    again.add_css_class("pill");
    again.set_halign(gtk::Align::Center);
    error.set_child(Some(&again));
    stack.add_named(&error, Some("error"));
    body.append(&stack);

    let clamp = adw::Clamp::builder().child(&body).maximum_size(1100).build();
    let scroll = scroller(&clamp);
    let (page, header) = frame(&name, &format!("discover-{plugin}"), &scroll);
    let small = small_title(&header, &name);
    follow_title(&scroll, &title, &small);
    // Browse: the source's own folders.
    let browse = gtk::Button::with_label(&tr("Browse"));
    browse.add_css_class("flat");
    let (id, shown_name) = (plugin.to_owned(), name.clone());
    browse.connect_clicked(move |_| window::show_source(&id, None, &shown_name));
    header.pack_end(&browse);

    let d = Rc::new(Discover {
        plugin: plugin.to_owned(),
        stack,
        shelves,
        error,
        request: Cell::new(None),
    });
    let weak = Rc::downgrade(&d);
    again.connect_clicked(move |_| {
        if let Some(d) = weak.upgrade() {
            d.load();
        }
    });
    let weak = Rc::downgrade(&d);
    a.listen_for(&page, move |change| {
        let Some(d) = weak.upgrade() else { return };
        match change {
            Change::Discover(answer) if Some(answer.request) == d.request.get() => d.show(answer),
            Change::Failed(id, message) if Some(*id) == d.request.get() => d.fail(message),
            _ => {}
        }
    });
    d.load();
    keep_alive(&page, d);
    page
}

/// Grey shapes of a heading and a row of covers, twice, while loading.
fn placeholders() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.update_property(&[gtk::accessible::Property::Label(&tr("Loading"))]);
    for _ in 0..2 {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        line.add_css_class("placeholder");
        line.add_css_class("placeholder-heading");
        line.set_size_request(180, 22);
        line.set_halign(gtk::Align::Start);
        b.append(&line);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        for _ in 0..5 {
            let cover = gtk::Box::new(gtk::Orientation::Vertical, 0);
            cover.add_css_class("placeholder");
            cover.set_size_request(CARD, CARD);
            row.append(&cover);
        }
        b.append(&row);
    }
    b
}

impl Discover {
    fn load(&self) {
        self.stack.set_visible_child_name("loading");
        let request = app().send_quiet(Command::Discover {
            source: self.plugin.clone(),
        });
        self.request.set(Some(request));
    }

    fn fail(&self, message: &str) {
        self.request.set(None);
        self.error.set_description(Some(message));
        self.stack.set_visible_child_name("error");
    }

    fn show(&self, answer: &Arc<DiscoverPage>) {
        self.request.set(None);
        if let Some(e) = &answer.error {
            self.fail(e);
            return;
        }
        while let Some(c) = self.shelves.first_child() {
            self.shelves.remove(&c);
        }
        let shown = discover::shown(&answer.shelves);
        if shown.is_empty() {
            let empty = status("folder-symbolic", &tr("Nothing Here"), "");
            empty.set_vexpand(false);
            self.shelves.append(&empty);
        }
        for shelf in shown {
            self.shelves.append(&shelf_view(&self.plugin, shelf));
        }
        self.stack.set_visible_child_name("shelves");
    }
}

/// A shelf's title: the core's own shelves worded by id.
fn shelf_title(shelf: &Shelf) -> String {
    match discover::known(shelf) {
        Some(Known::ForYou) => tr("For You"),
        Some(Known::RecentlyAdded) => tr("Recently Added"),
        Some(Known::MostPlayed) => tr("Most Played"),
        Some(Known::Rediscover) => tr("Rediscover"),
        None => shelf.title.clone(),
    }
}

/// A shelf: its heading with See All, then its items.
fn shelf_view(plugin: &str, shelf: &Shelf) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
    b.add_css_class("discover-shelf");
    let title = shelf_title(shelf);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let h = heading(&title);
    h.set_hexpand(true);
    top.append(&h);
    if let Some(folder) = discover::see_all(plugin, shelf) {
        let all = gtk::Button::with_label(&tr("See All"));
        all.add_css_class("flat");
        all.set_valign(gtk::Align::Center);
        let (id, title) = (plugin.to_owned(), title.clone());
        all.connect_clicked(move |_| window::show_source(&id, Some(folder.clone()), &title));
        top.append(&all);
    }
    b.append(&top);
    let items: Rc<Vec<ItemData>> = Rc::new(shelf.items.iter().map(convert::source_item).collect());
    match shelf.style {
        ShelfStyle::Row => {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
            row.add_css_class("shelf");
            for (i, data) in items.iter().enumerate() {
                row.append(&card(&items, i, data));
            }
            let scroll = gtk::ScrolledWindow::builder()
                .child(&row)
                .vscrollbar_policy(gtk::PolicyType::Never)
                .hscrollbar_policy(gtk::PolicyType::Automatic)
                .build();
            scroll.add_css_class("shelf-scroll");
            b.append(&scroll);
        }
        ShelfStyle::Grid => {
            let grid = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .homogeneous(true)
                .column_spacing(16)
                .row_spacing(16)
                .min_children_per_line(2)
                .max_children_per_line(8)
                .build();
            grid.add_css_class("shelf-grid");
            for (i, data) in items.iter().enumerate() {
                grid.append(&card(&items, i, data));
            }
            b.append(&grid);
        }
        ShelfStyle::Chart => {
            let numbered: Vec<ItemData> = items
                .iter()
                .enumerate()
                .map(|(i, d)| ItemData {
                    number: u32::try_from(i + 1).ok(),
                    ..d.clone()
                })
                .collect();
            let played = items.clone();
            let list = track_box(RowStyle::ALBUM_TRACK, move |i| {
                actions::activate_in_list(&played, i)
            });
            fill_track_box(&list, RowStyle::ALBUM_TRACK, &numbered);
            list.update_property(&[gtk::accessible::Property::Label(&title)]);
            b.append(&list);
        }
    }
    b
}

/// A cover: songs play the shelf from them; albums, artists and playlists
/// open their page.
fn card(items: &Rc<Vec<ItemData>>, index: usize, data: &ItemData) -> Card {
    let c = Card::new(CARD);
    c.set_size_request(CARD, -1);
    c.bind(&Item::new(Some(data.clone())));
    let items = items.clone();
    c.set_open(move |_| actions::activate_in_list(&items, index));
    c
}

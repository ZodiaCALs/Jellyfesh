//! Favourites (docs/ui.md, "Favourites"): every song marked with a heart,
//! with Play and Shuffle, then the sources' likes, one section per
//! signed-in source (ADR-0024; `pages::sourcelib`). The sources' sections
//! are read-only: the heart marks a song in Jellyfesh only. "No Favourites
//! Yet" shows only when the sources have nothing either.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use jf_core::Collection;

use crate::logic::sourcelib;
use crate::ui::actions;
use crate::ui::convert;
use crate::ui::i18n::tr;
use crate::ui::models::ItemData;
use crate::ui::pages::detail::{fill_track_box, track_box};
use crate::ui::pages::library::LIST_WIDTH;
use crate::ui::pages::{
    follow_title, frame, keep_alive, large_title, play_and_shuffle, scroller, small_title, status,
};
use crate::ui::rows::RowStyle;
use crate::ui::state::{Change, app, spawn};
use crate::ui::widgets::maxwidth::MaxWidth;

/// Favourites come in pages of this size until a short page ends the list
/// (the core gives no count).
const PAGE: usize = 200;

struct Favourites {
    rows: RefCell<Vec<ItemData>>,
    /// Why the library's favourites could not load.
    failed: RefCell<Option<String>>,
    actions: gtk::Box,
    list: gtk::ListBox,
    sources: gtk::Box,
    stack: gtk::Stack,
    error: adw::StatusPage,
}

pub fn page() -> adw::NavigationPage {
    let title = large_title(&tr("Favourites"));
    // Play and Shuffle under the title, as on Songs and in every app.
    let (actions_row, play, shuffle) = play_and_shuffle();
    play.connect_clicked(|_| actions::play(Collection::Favorites, 0, false));
    shuffle.connect_clicked(|_| actions::play(Collection::Favorites, 0, true));
    let list = track_box(RowStyle::TRACK, |i| {
        actions::play(Collection::Favorites, i, false)
    });
    list.add_css_class("track-list");
    list.update_property(&[gtk::accessible::Property::Label(&tr("Favourites"))]);
    let sources = gtk::Box::new(gtk::Orientation::Vertical, 12);
    sources.add_css_class("source-sections");
    sources.set_visible(false);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.add_css_class("titled-list");
    column.append(&title);
    column.append(&actions_row);
    column.append(&list);
    column.append(&sources);
    let holder = MaxWidth::new(&column, LIST_WIDTH);
    let scroll = scroller(&holder);
    let stack = gtk::Stack::new();
    stack.add_named(&scroll, Some("list"));
    stack.add_named(
        &status(
            "starred-symbolic",
            &tr("No Favourites Yet"),
            &tr("Songs you mark with the heart appear here."),
        ),
        Some("empty"),
    );
    let error = status(
        "dialog-information-symbolic",
        &tr("Could Not Load Favourites"),
        "",
    );
    stack.add_named(&error, Some("error"));
    let (page, header) = frame(&tr("Favourites"), "favourites", &stack);
    let small = small_title(&header, &tr("Favourites"));
    follow_title(&scroll, &title, &small);

    let this = Rc::new(Favourites {
        rows: RefCell::new(Vec::new()),
        failed: RefCell::new(None),
        actions: actions_row,
        list,
        sources,
        stack,
        error,
    });
    refresh(&this);
    show_sources(&this);
    let weak = Rc::downgrade(&this);
    page.connect_showing(move |_| {
        if let Some(f) = weak.upgrade() {
            refresh(&f);
        }
        crate::ui::pages::sourcelib::page_shown("favourites");
    });
    let weak = Rc::downgrade(&this);
    app().listen_for(&page, move |change| {
        let Some(f) = weak.upgrade() else { return };
        match change {
            Change::Library => refresh(&f),
            Change::SourceLibrary => show_sources(&f),
            _ => {}
        }
    });
    keep_alive(&page, this);
    page
}

/// Loads the library's favourites, then shows them.
fn refresh(f: &Rc<Favourites>) {
    let weak = Rc::downgrade(f);
    spawn(async move {
        let result = app()
            .query(|core| {
                let mut all = Vec::new();
                loop {
                    let page = core.favorites(all.len(), PAGE).map_err(|e| e.to_string())?;
                    let short = page.len() < PAGE;
                    all.extend(page.iter().map(convert::track_row));
                    if short {
                        return Ok::<_, String>(all);
                    }
                }
            })
            .await;
        let Some(f) = weak.upgrade() else { return };
        match result {
            Some(Ok(rows)) => {
                fill_track_box(&f.list, RowStyle::TRACK, &rows);
                *f.rows.borrow_mut() = rows;
                f.failed.replace(None);
            }
            Some(Err(e)) => {
                f.failed.replace(Some(e));
            }
            None => return,
        }
        show(&f);
    });
}

/// The sources' likes, after the library's.
fn show_sources(f: &Favourites) {
    let library = app().source_library.borrow().clone();
    crate::ui::pages::sourcelib::fill(&f.sources, &sourcelib::likes(&library));
    show(f);
}

fn show(f: &Favourites) {
    let has_rows = !f.rows.borrow().is_empty();
    f.actions.set_visible(has_rows);
    f.list.set_visible(has_rows);
    let has_sources = f.sources.is_visible();
    if let Some(e) = f.failed.borrow().as_deref()
        && !has_sources
    {
        f.error.set_description(Some(e));
        f.stack.set_visible_child_name("error");
        return;
    }
    f.stack
        .set_visible_child_name(if has_rows || has_sources { "list" } else { "empty" });
}

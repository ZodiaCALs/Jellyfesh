//! The sources' likes and playlists on Favourites and Playlists (ADR-0024
//! §2–3; docs/ui.md, "Favourites" and "Playlists"): after the library's
//! own, a section per signed-in source with its name and credit ("Content
//! from Spotify"), its lists, why a list could not load, and how old a
//! cached list is ("Updated 5 minutes ago"). Songs play from their rows,
//! albums are cards, artists and playlists open their pages. The pages ask
//! for fresh lists when they show, and again when returned to after ten
//! minutes.

use std::cell::RefCell;
use std::collections::HashMap;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Command, ItemKind, SearchGroup, SourceLibraryPart};

use crate::logic::sourcelib::{self, Age, Section};
use crate::ui::i18n::{trf, trn};
use crate::ui::pages::heading;
use crate::ui::state::app;

thread_local! {
    /// When each page last asked for fresh lists (ms since the epoch).
    static ASKED: RefCell<HashMap<&'static str, i64>> = RefCell::new(HashMap::new());
}

fn now_ms() -> i64 {
    glib::real_time() / 1000
}

/// A page (`favourites` or `playlists`) shows: fresh lists, unless it asked
/// less than ten minutes ago.
pub fn page_shown(page: &'static str) {
    let now = now_ms();
    let ask = ASKED.with(|asked| {
        let mut asked = asked.borrow_mut();
        let ask = sourcelib::should_refresh(asked.get(page).copied(), now);
        if ask {
            asked.insert(page, now);
        }
        ask
    });
    if ask {
        app().send_quiet(Command::RefreshSourceLibrary { plugin: None });
    }
}

fn kind_of(part: SourceLibraryPart) -> ItemKind {
    match part {
        SourceLibraryPart::LikedSongs => ItemKind::Track,
        SourceLibraryPart::LikedAlbums => ItemKind::Album,
        SourceLibraryPart::LikedArtists => ItemKind::Artist,
        SourceLibraryPart::Playlists => ItemKind::Playlist,
    }
}

fn updated(age: Age) -> String {
    match age {
        Age::Minutes(n) => trn("Updated {n} minute ago", "Updated {n} minutes ago", n),
        Age::Hours(n) => trn("Updated {n} hour ago", "Updated {n} hours ago", n),
        Age::Days(n) => trn("Updated {n} day ago", "Updated {n} days ago", n),
    }
}

fn line(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::builder().label(text).xalign(0.0).wrap(true).build();
    l.add_css_class(class);
    l
}

/// Fills `into` with the sources' sections; hidden when there are none.
pub fn fill(into: &gtk::Box, sections: &[Section<'_>]) {
    while let Some(c) = into.first_child() {
        into.remove(&c);
    }
    let now = now_ms();
    for (source, lists) in sections {
        let h = heading(&source.name);
        h.add_css_class("source-heading");
        into.append(&h);
        if let Some(service) = &source.attribution {
            into.append(&line(
                &trf("Content from {service}", &[("service", service)]),
                "dim-label",
            ));
        }
        for list in lists {
            let group = SearchGroup {
                kind: kind_of(list.part),
                items: list.entries.iter().map(|e| e.item.clone()).collect(),
                next: None,
                total: list.total,
            };
            crate::ui::pages::search::render_group(into, &group, None);
            if let Some(error) = &list.error {
                into.append(&line(error, "error-text"));
            }
            if let Some(age) = sourcelib::age(list, now) {
                into.append(&line(&updated(age), "dim-label"));
            }
        }
    }
    into.set_visible(into.first_child().is_some());
}

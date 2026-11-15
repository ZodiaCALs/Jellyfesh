//! Unified search (docs/ui.md, "Search"): the library's results first
//! (Songs, Albums, Artists), then From Your Sources, one group per source,
//! each appearing as it arrives.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Collection, Command, ItemKind, SearchScope};

use crate::logic::search::{LIBRARY, SearchState, Section};
use crate::ui::convert;
use crate::ui::i18n::tr;
use crate::ui::models::{Item, ItemData, Kind};
use crate::ui::pages::detail::{fill_track_box, track_box};
use crate::ui::pages::{frame, heading, keep_alive, scroller, status};
use crate::ui::rows::{Card, RowStyle};
use crate::ui::state::{Change, app};
use crate::ui::{actions, window};

thread_local! {
    static ENTRY: RefCell<Option<gtk::glib::WeakRef<gtk::SearchEntry>>> = const { RefCell::new(None) };
}

/// Searches for `words` (the field shows them).
pub fn search_for(words: &str) {
    ENTRY.with(|e| {
        if let Some(entry) = e.borrow().as_ref().and_then(|w| w.upgrade()) {
            entry.set_text(words);
            entry.grab_focus();
            entry.set_position(-1);
        }
    });
}

/// Puts the cursor in the search field.
pub fn focus() {
    ENTRY.with(|e| {
        if let Some(entry) = e.borrow().as_ref().and_then(|w| w.upgrade()) {
            entry.grab_focus();
        }
    });
}

/// Artwork hash and favourite state per library item, and the search they
/// belong to.
type Extra = (jf_core::CommandId, Vec<(Option<String>, bool)>);

struct Search {
    state: RefCell<SearchState>,
    extra: RefCell<Extra>,
    results: gtk::Box,
    stack: gtk::Stack,
    spinner: gtk::Spinner,
}

pub fn page() -> adw::NavigationPage {
    let entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Songs, albums and artists"))
        .hexpand(true)
        .search_delay(180)
        .build();
    entry.update_property(&[gtk::accessible::Property::Label(&tr("Search"))]);
    ENTRY.with(|e| *e.borrow_mut() = Some(entry.downgrade()));
    let entry_clamp = adw::Clamp::builder().child(&entry).maximum_size(520).build();

    let results = gtk::Box::new(gtk::Orientation::Vertical, 18);
    results.add_css_class("search-results");
    let clamp = adw::Clamp::builder().child(&results).maximum_size(1000).build();
    let stack = gtk::Stack::new();
    stack.add_named(
        &status(
            "system-search-symbolic",
            &tr("Search Your Music"),
            &tr("Find songs, albums and artists in your library and in every source you have enabled."),
        ),
        Some("hint"),
    );
    stack.add_named(&scroller(&clamp), Some("results"));
    stack.add_named(
        &status(
            "system-search-symbolic",
            &tr("No Results"),
            &tr("Try other words."),
        ),
        Some("none"),
    );
    let (page, header) = frame(&tr("Search"), "search", &stack);
    header.set_title_widget(Some(&entry_clamp));
    let spinner = gtk::Spinner::new();
    header.pack_end(&spinner);

    let search = Rc::new(Search {
        state: RefCell::new(SearchState::new()),
        extra: RefCell::new((jf_core::CommandId(0), Vec::new())),
        results,
        stack,
        spinner,
    });
    entry.connect_search_changed(glib::clone!(
        #[weak]
        search,
        move |entry| {
            let text = entry.text().trim().to_owned();
            if text.is_empty() {
                search.state.borrow_mut().clear();
                render(&search);
                return;
            }
            let id = app().send(Command::Search {
                query: text.clone(),
                scope: SearchScope::All,
            });
            search.state.borrow_mut().start(&text, id);
            render(&search);
        }
    ));
    entry.connect_stop_search(|e| e.set_text(""));
    let weak = Rc::downgrade(&search);
    app().listen_for(&page, move |change| {
        if let (Change::Search(update), Some(search)) = (change, weak.upgrade())
            && search.state.borrow_mut().apply(update)
        {
            render(&search);
            if update.source == LIBRARY {
                enrich(&search, update.request);
            }
        }
    });
    keep_alive(&page, search);
    page
}

/// The core's search results carry no artwork for library items: look it
/// up (and the favourite state) on the query thread, then redraw.
fn enrich(search: &Rc<Search>, request: jf_core::CommandId) {
    let items: Vec<jf_core::SourceItem> = search
        .state
        .borrow()
        .sections()
        .iter()
        .filter(|s| s.source == LIBRARY)
        .flat_map(|s| s.items.clone())
        .collect();
    let weak = Rc::downgrade(search);
    glib::spawn_future_local(async move {
        let extra = app()
            .query(move |core| {
                items
                    .iter()
                    .map(|item| {
                        let id: i64 = item.id.parse().unwrap_or(0);
                        match item.kind {
                            ItemKind::Track => {
                                let hash = core.track(id).ok().flatten().and_then(|t| t.artwork_hash);
                                let favorite =
                                    core.is_favorite(&jf_core::TrackRef::Library(id)).unwrap_or(false);
                                (hash, favorite)
                            }
                            ItemKind::Album => {
                                let hash = core
                                    .album_tracks(id)
                                    .ok()
                                    .and_then(|t| t.into_iter().find_map(|t| t.artwork_hash));
                                (hash, false)
                            }
                            _ => (None, false),
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .await;
        let (Some(search), Some(extra)) = (weak.upgrade(), extra) else {
            return;
        };
        search.extra.replace((request, extra));
        render(&search);
    });
}

fn source_name(id: &str) -> String {
    app()
        .sources
        .borrow()
        .plugins
        .iter()
        .find(|p| p.id == id)
        .map_or_else(|| id.to_owned(), |p| p.name.clone())
}

fn kind_title(kind: ItemKind) -> String {
    match kind {
        ItemKind::Track => tr("Songs"),
        ItemKind::Album => tr("Albums"),
        ItemKind::Artist => tr("Artists"),
        ItemKind::Playlist => tr("Playlists"),
        ItemKind::Folder => tr("Folders"),
    }
}

fn render(search: &Search) {
    let state = search.state.borrow();
    search.spinner.set_spinning(state.is_waiting());
    search.spinner.set_visible(state.is_waiting());
    while let Some(c) = search.results.first_child() {
        search.results.remove(&c);
    }
    if state.query().is_empty() {
        search.stack.set_visible_child_name("hint");
        return;
    }
    if state.is_empty_result() {
        search.stack.set_visible_child_name("none");
        return;
    }
    if !state.is_done() && state.sections().is_empty() {
        // Still waiting for the first source: keep the previous page calm.
        search.stack.set_visible_child_name("results");
        return;
    }
    search.stack.set_visible_child_name("results");
    let mut from_sources = false;
    for section in state.sections() {
        if section.items.is_empty() && section.error.is_none() {
            continue;
        }
        if section.source != LIBRARY {
            if !from_sources {
                let h = heading(&tr("From Your Sources"));
                h.add_css_class("from-sources");
                search.results.append(&h);
                from_sources = true;
            }
            let h = heading(&source_name(&section.source));
            h.add_css_class("source-heading");
            search.results.append(&h);
        }
        if let Some(e) = &section.error {
            let l = gtk::Label::new(Some(e));
            l.set_xalign(0.0);
            l.set_wrap(true);
            l.add_css_class("error-text");
            search.results.append(&l);
        }
        let extra = search.extra.borrow();
        let library_extra =
            (section.source == LIBRARY && state.request() == Some(extra.0)).then_some(&extra.1);
        render_section(&search.results, section, library_extra);
    }
}

fn render_section(into: &gtk::Box, section: &Section, extra: Option<&Vec<(Option<String>, bool)>>) {
    // Artwork and favourites found for library items, in section order.
    let looked_up = |index: usize, mut data: ItemData| {
        if let Some((hash, favorite)) = extra.and_then(|e| e.get(index)) {
            if data.art.is_none() {
                data.art = hash.clone().map(crate::ui::thumbs::ArtKey::Hash);
            }
            data.favorite = *favorite;
        }
        data
    };
    for kind in section.kinds() {
        let sub = gtk::Label::new(Some(&kind_title(kind)));
        sub.set_xalign(0.0);
        sub.add_css_class("kind-heading");
        into.append(&sub);
        let items: Vec<ItemData> = section
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.kind == kind)
            .map(|(n, i)| looked_up(n, convert::source_item(i)))
            .collect();
        match kind {
            ItemKind::Track => {
                let refs: Vec<_> = items.iter().filter_map(|d| d.track.clone()).collect();
                let list = track_box(RowStyle::TRACK, move |i| {
                    actions::play(Collection::Tracks { tracks: refs.clone() }, i, false);
                });
                fill_track_box(&list, RowStyle::TRACK, &items);
                into.append(&list);
            }
            ItemKind::Album => {
                let flow = gtk::FlowBox::builder()
                    .selection_mode(gtk::SelectionMode::None)
                    .homogeneous(true)
                    .min_children_per_line(2)
                    .max_children_per_line(6)
                    .column_spacing(12)
                    .row_spacing(12)
                    .build();
                flow.add_css_class("album-flow");
                flow.connect_child_activated(|_, child| {
                    if let Some(d) = child.child().and_downcast::<Card>().and_then(|c| c.data()) {
                        open(&d);
                    }
                });
                for data in items {
                    let card = Card::new(140);
                    card.bind(&Item::new(Some(data)));
                    flow.append(&card);
                }
                into.append(&flow);
            }
            _ => {
                let style = if kind == ItemKind::Artist {
                    RowStyle::ARTIST
                } else {
                    RowStyle::PLAYLIST
                };
                let opened = items.clone();
                let list = track_box(style, move |i| {
                    if let Some(d) = opened.get(i) {
                        open(d);
                    }
                });
                fill_track_box(&list, style, &items);
                into.append(&list);
            }
        }
    }
}

/// Opens an album, artist, playlist or source container.
fn open(data: &ItemData) {
    match data.kind {
        Kind::Album => window::show_album(data.id, Some(data.clone())),
        Kind::Artist => window::show_artist(data.id, Some(data.clone())),
        Kind::Playlist => window::show_playlist(data.id, Some(data.clone())),
        _ => {
            if let Some(s) = &data.source {
                window::show_source(&s.source, Some(s.id.clone()), &s.title);
            }
        }
    }
}

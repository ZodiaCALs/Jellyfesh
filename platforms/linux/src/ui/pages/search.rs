//! Search by source (docs/ui.md, "Search"; ADR-0024 §4): one field, then
//! the tabs All, Library and one per source that found something.
//! * All: the library's results (Songs, Albums, Artists), then From Your
//!   Sources, one group per source whose terms allow merging, then a link
//!   to the tab of each source whose terms forbid it ("Results from
//!   Spotify").
//! * A source's tab: its Songs, Albums, Artists and Playlists, each with
//!   Show More while the source has more.
//!
//! Each answer appears as it arrives; a new search starts on All.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Command, CommandId, ItemKind, SearchGroup, SearchScope, SourceItem};

use crate::logic::search::{LIBRARY, SearchState, Section, Tab};
use crate::ui::actions;
use crate::ui::convert;
use crate::ui::i18n::{tr, trf};
use crate::ui::models::{Item, ItemData};
use crate::ui::pages::detail::{fill_track_box, track_box};
use crate::ui::pages::{frame, heading, keep_alive, pill_tabs, scroller, status};
use crate::ui::rows::{Card, RowStyle};
use crate::ui::state::{Change, app};

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

/// Artwork hash and favourite state of the library's items, by kind and
/// id, and the search they belong to.
type Extra = (CommandId, HashMap<(ItemKind, String), (Option<String>, bool)>);

struct Search {
    state: RefCell<SearchState>,
    extra: RefCell<Extra>,
    /// The tabs row and the tabs it shows.
    tabs_holder: gtk::Box,
    tabs_shown: RefCell<Vec<Tab>>,
    tab_buttons: RefCell<Vec<gtk::ToggleButton>>,
    results: gtk::Box,
    scroll: gtk::ScrolledWindow,
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
    let clamp = crate::ui::widgets::maxwidth::MaxWidth::new(&results, 1000);
    let scroll = scroller(&clamp);
    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.add_named(
        &status(
            "system-search-symbolic",
            &tr("Search Your Music"),
            &tr("Find songs, albums and artists in your library and in your sources."),
        ),
        Some("hint"),
    );
    stack.add_named(&scroll, Some("results"));
    stack.add_named(
        &status(
            "system-search-symbolic",
            &tr("No Results"),
            &tr("Try other words."),
        ),
        Some("none"),
    );
    let tabs_holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    tabs_holder.add_css_class("search-tabs");
    tabs_holder.set_visible(false);
    // The tabs sit in the reading column, at the page's start (the web's).
    let tabs_column =
        crate::ui::widgets::maxwidth::MaxWidth::new(&tabs_holder, jf_tokens::layout::READING as i32);
    tabs_column.add_css_class("search-tabs-column");
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&tabs_column);
    body.append(&stack);
    let (page, header) = frame(&tr("Search"), "search", &body);
    header.set_title_widget(Some(&entry_clamp));
    let spinner = gtk::Spinner::new();
    header.pack_end(&spinner);

    let search = Rc::new(Search {
        state: RefCell::new(SearchState::new()),
        extra: RefCell::new((CommandId(0), HashMap::new())),
        tabs_holder,
        tabs_shown: RefCell::new(Vec::new()),
        tab_buttons: RefCell::new(Vec::new()),
        results,
        scroll,
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
        let Some(search) = weak.upgrade() else { return };
        match change {
            Change::Search(update) => {
                if search.state.borrow_mut().apply(update) {
                    render(&search);
                    if update.source == LIBRARY {
                        enrich(&search, update.request);
                    }
                }
            }
            // Show More's answer, or why it failed.
            Change::SearchPage(more) if search.state.borrow_mut().apply_page(more) => render(&search),
            Change::Failed(id, error) if search.state.borrow_mut().more_failed(*id, error) => render(&search),
            _ => {}
        }
    });
    keep_alive(&page, search);
    page
}

/// The core's search results carry no artwork for library items: look it
/// up (and the favourite state) on the query thread, then redraw.
fn enrich(search: &Rc<Search>, request: CommandId) {
    let items: Vec<SourceItem> = search
        .state
        .borrow()
        .section(LIBRARY)
        .map(|s| {
            let mut all = s.items.clone();
            all.extend(s.groups.iter().flat_map(|g| g.items.iter().cloned()));
            all
        })
        .unwrap_or_default();
    let weak = Rc::downgrade(search);
    glib::spawn_future_local(async move {
        let extra = app()
            .query(move |core| {
                let mut found = HashMap::new();
                for item in &items {
                    let key = (item.kind, item.id.clone());
                    if found.contains_key(&key) {
                        continue;
                    }
                    let id: i64 = item.id.parse().unwrap_or(0);
                    let looked_up = match item.kind {
                        ItemKind::Track => {
                            let hash = core.track(id).ok().flatten().and_then(|t| t.artwork_hash);
                            let favorite = core.is_favorite(&jf_core::TrackRef::Library(id)).unwrap_or(false);
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
                    };
                    found.insert(key, looked_up);
                }
                found
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

/// The service a source's views credit ("Content from Spotify"), if any.
fn source_attribution(id: &str) -> Option<String> {
    app()
        .sources
        .borrow()
        .plugins
        .iter()
        .find(|p| p.id == id)
        .and_then(|p| p.attribution.clone())
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

fn tab_title(tab: &Tab) -> String {
    match tab {
        Tab::All => tr("All"),
        Tab::Library => tr("Library"),
        Tab::Source(id) => source_name(id),
    }
}

/// The tabs row: rebuilt when the tabs change, with the shown one chosen.
fn render_tabs(search: &Rc<Search>, state: &SearchState) {
    let tabs = state.tabs();
    let shown = state.tab();
    if *search.tabs_shown.borrow() != tabs {
        while let Some(c) = search.tabs_holder.first_child() {
            search.tabs_holder.remove(&c);
        }
        let items: Vec<(usize, String)> = tabs.iter().enumerate().map(|(i, t)| (i, tab_title(t))).collect();
        let (row, buttons) = pill_tabs(&items, &tr("Search"));
        search.tabs_holder.append(&row);
        let weak = Rc::downgrade(search);
        let mut toggles = Vec::new();
        for (i, b) in buttons {
            let tab = tabs[i].clone();
            let weak = weak.clone();
            b.connect_toggled(move |b| {
                let Some(search) = weak.upgrade() else { return };
                if !b.is_active() || search.state.borrow().tab() == tab {
                    return;
                }
                search.state.borrow_mut().set_tab(tab.clone());
                search.scroll.vadjustment().set_value(0.0);
                render(&search);
            });
            toggles.push(b);
        }
        *search.tab_buttons.borrow_mut() = toggles;
        *search.tabs_shown.borrow_mut() = tabs.clone();
    }
    search.tabs_holder.set_visible(!tabs.is_empty());
    if let Some(i) = tabs.iter().position(|t| *t == shown)
        && let Some(b) = search.tab_buttons.borrow().get(i)
        && !b.is_active()
    {
        b.set_active(true);
    }
}

fn render(search: &Rc<Search>) {
    let state = search.state.borrow();
    search.spinner.set_spinning(state.is_waiting());
    search.spinner.set_visible(state.is_waiting());
    render_tabs(search, &state);
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
    search.stack.set_visible_child_name("results");
    let extra = search.extra.borrow();
    let library_extra = (state.request() == Some(extra.0)).then_some(&extra.1);
    match state.tab() {
        Tab::All => render_all(search, &state, library_extra),
        Tab::Library => {
            let groups = state
                .section(LIBRARY)
                .map(Section::tab_groups)
                .unwrap_or_default();
            if groups.is_empty() && state.is_done() {
                search.results.append(&quiet_line(&tr("No Results")));
            }
            for group in &groups {
                render_group(&search.results, group, library_extra);
            }
        }
        Tab::Source(source) => render_source_tab(search, &state, &source),
    }
}

/// All: the library, From Your Sources, then a link to each source whose
/// terms keep its results apart.
fn render_all(search: &Rc<Search>, state: &SearchState, library_extra: Option<&ExtraMap>) {
    if let Some(library) = state.section(LIBRARY) {
        error_line(&search.results, library.error.as_deref());
        for kind in library.kinds() {
            render_group(&search.results, &rows_of(library, kind), library_extra);
        }
    }
    let merged = state.merged_sources();
    if !merged.is_empty() {
        let h = heading(&tr("From Your Sources"));
        h.add_css_class("from-sources");
        search.results.append(&h);
    }
    for section in merged {
        let h = heading(&source_name(&section.source));
        h.add_css_class("source-heading");
        search.results.append(&h);
        error_line(&search.results, section.error.as_deref());
        for kind in section.kinds() {
            render_group(&search.results, &rows_of(section, kind), None);
        }
    }
    let separate = state.separate_sources();
    if separate.is_empty() {
        return;
    }
    let links = gtk::ListBox::new();
    links.add_css_class("boxed-list");
    links.add_css_class("separate-sources");
    links.set_selection_mode(gtk::SelectionMode::None);
    for section in separate {
        let row = adw::ActionRow::builder()
            .title(trf(
                "Results from {service}",
                &[("service", &source_name(&section.source))],
            ))
            .activatable(true)
            .build();
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let tab = Tab::Source(section.source.clone());
        let weak = Rc::downgrade(search);
        row.connect_activated(move |_| {
            if let Some(search) = weak.upgrade() {
                search.state.borrow_mut().set_tab(tab.clone());
                search.scroll.vadjustment().set_value(0.0);
                render(&search);
            }
        });
        links.append(&row);
    }
    search.results.append(&links);
}

/// A source's tab: its name and credit, then each kind it found with Show
/// More while it has more.
fn render_source_tab(search: &Rc<Search>, state: &SearchState, source: &str) {
    let Some(section) = state.section(source) else {
        return;
    };
    let h = heading(&source_name(source));
    h.add_css_class("source-heading");
    search.results.append(&h);
    if let Some(service) = source_attribution(source) {
        search.results.append(&quiet_line(&trf(
            "Content from {service}",
            &[("service", &service)],
        )));
    }
    error_line(&search.results, section.error.as_deref());
    for group in section.tab_groups() {
        render_group(&search.results, &group, None);
        let kind = group.kind;
        if state.is_loading_more(source, kind) {
            let spinner = gtk::Spinner::builder()
                .spinning(true)
                .halign(gtk::Align::Start)
                .build();
            spinner.add_css_class("show-more-spinner");
            search.results.append(&spinner);
        } else if let Some(cursor) = state.more_cursor(source, kind) {
            let more = gtk::Button::with_label(&tr("Show More"));
            more.add_css_class("pill");
            more.add_css_class("show-more");
            more.set_halign(gtk::Align::Start);
            let (source, weak) = (source.to_owned(), Rc::downgrade(search));
            more.connect_clicked(move |_| {
                let Some(search) = weak.upgrade() else { return };
                let id = app().send_quiet(Command::SearchMore {
                    plugin: source.clone(),
                    kind,
                    cursor: cursor.clone(),
                });
                search.state.borrow_mut().more_sent(&source, kind, id);
                render(&search);
            });
            search.results.append(&more);
        }
        error_line(&search.results, state.more_error(source, kind));
    }
}

type ExtraMap = HashMap<(ItemKind, String), (Option<String>, bool)>;

/// A section's rows of one kind, as a group (no Show More).
fn rows_of(section: &Section, kind: ItemKind) -> SearchGroup {
    SearchGroup {
        kind,
        items: section.items.iter().filter(|i| i.kind == kind).cloned().collect(),
        next: None,
        total: None,
    }
}

fn quiet_line(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    l.set_wrap(true);
    l.add_css_class("dim-label");
    l
}

fn error_line(into: &gtk::Box, error: Option<&str>) {
    if let Some(e) = error {
        let l = gtk::Label::new(Some(e));
        l.set_xalign(0.0);
        l.set_wrap(true);
        l.add_css_class("error-text");
        into.append(&l);
    }
}

/// One kind of results: songs as rows, albums as cards, artists and
/// playlists as rows that open their pages.
pub(crate) fn render_group(into: &gtk::Box, group: &SearchGroup, extra: Option<&ExtraMap>) {
    if group.items.is_empty() {
        return;
    }
    // Artwork and favourites found for library items.
    let looked_up = |item: &SourceItem| {
        let mut data = convert::source_item(item);
        if let Some((hash, favorite)) = extra.and_then(|e| e.get(&(item.kind, item.id.clone()))) {
            if data.art.is_none() {
                data.art = hash.clone().map(crate::ui::thumbs::ArtKey::Hash);
            }
            data.favorite = *favorite;
        }
        data
    };
    let sub = gtk::Label::new(Some(&kind_title(group.kind)));
    sub.set_xalign(0.0);
    sub.add_css_class("kind-heading");
    into.append(&sub);
    let items: Vec<ItemData> = group.items.iter().map(looked_up).collect();
    match group.kind {
        ItemKind::Track => {
            // Plays from the row (with the group's playable songs), or opens
            // a song that cannot play here in its service.
            let rows = items.clone();
            let list = track_box(RowStyle::TRACK, move |i| actions::activate_in_list(&rows, i));
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
                    actions::open_page(&d);
                }
            });
            for data in items {
                let card = Card::new(140);
                card.bind(&Item::new(Some(data)));
                flow.append(&card);
            }
            into.append(&flow);
        }
        kind => {
            let style = if kind == ItemKind::Artist {
                RowStyle::ARTIST
            } else {
                RowStyle::PLAYLIST
            };
            let opened = items.clone();
            let list = track_box(style, move |i| {
                if let Some(d) = opened.get(i) {
                    actions::open_page(d);
                }
            });
            fill_track_box(&list, style, &items);
            into.append(&list);
        }
    }
}

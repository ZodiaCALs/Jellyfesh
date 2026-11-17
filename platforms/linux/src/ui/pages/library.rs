//! Albums (a grid of covers), Artists and Songs: long lists, virtualised and
//! loaded page by page from the query thread.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Command, LibraryStats, TrackQuery, TrackRef};

use crate::logic::{layout, library};
use crate::ui::convert;
use crate::ui::i18n::tr;
use crate::ui::models::{Item, ItemData, PagedModel};
use crate::ui::pages::{
    frame, play_and_shuffle, show_small_title, status, titled_scroller, titled_scroller_with,
};
use crate::ui::rows::{RowStyle, card_factory, row_factory};
use crate::ui::state::{Change, app, spawn};
use crate::ui::window;

/// A paged model whose pages come from `fetch(offset, limit)` on the query
/// thread.
pub fn paged(
    fetch: impl Fn(&jf_core::Core, usize, usize) -> Result<Vec<ItemData>, String> + Send + Sync + Clone + 'static,
) -> PagedModel {
    PagedModel::new(move |model, offset, limit, generation| {
        let weak = model.downgrade();
        let fetch = fetch.clone();
        spawn(async move {
            let rows = app()
                .query(move |core| fetch(core, offset as usize, limit as usize))
                .await;
            let Some(model) = weak.upgrade() else { return };
            match rows {
                Some(Ok(rows)) => model.deliver(generation, offset, rows),
                Some(Err(e)) => {
                    tracing::warn!("a page failed to load: {e}");
                    model.failed(generation, offset);
                }
                None => model.failed(generation, offset),
            }
        });
    })
}

/// Sets the model's length from the library counts, now and whenever the
/// library changes; shows `empty` while there is nothing.
pub fn keep_counted(
    model: &PagedModel,
    stack: &gtk::Stack,
    count: impl Fn(&LibraryStats) -> u64 + Send + Sync + Copy + 'static,
) {
    let refresh = {
        let model = model.downgrade();
        let stack = stack.downgrade();
        move || {
            let model = model.clone();
            let stack = stack.clone();
            spawn(async move {
                let stats = app().query(|core| core.stats()).await;
                let (Some(model), Some(stack)) = (model.upgrade(), stack.upgrade()) else {
                    return;
                };
                let n = match stats {
                    Some(Ok(s)) => u32::try_from(count(&s)).unwrap_or(u32::MAX),
                    _ => 0,
                };
                model.reload(n);
                stack.set_visible_child_name(if n == 0 { "empty" } else { "list" });
            });
        }
    };
    refresh();
    app().listen_for(stack, move |change| {
        if matches!(change, Change::Library) {
            refresh();
        }
    });
}

fn empty_library(title: &str, description: &str) -> adw::StatusPage {
    let s = status("folder-music-symbolic", title, description);
    let add = gtk::Button::with_label(&tr("Add Music Folder…"));
    add.add_css_class("pill");
    add.add_css_class("suggested-action");
    add.set_halign(gtk::Align::Center);
    add.connect_clicked(|_| window::choose_folder());
    s.set_child(Some(&add));
    s
}

fn item_at(view_model: Option<gtk::gio::ListModel>, position: u32) -> Option<ItemData> {
    view_model?.item(position).and_downcast::<Item>()?.data()
}

/// The order a sort menu chose: its key (`logic::library`) and Descending.
type SortState = Rc<Cell<(&'static str, bool)>>;

/// The menu item for a sort key of `logic::library`.
fn sort_label(key: &str) -> String {
    match key {
        "title" => tr("Title"),
        "artist" => tr("Artist"),
        "album" => tr("Album"),
        "year" => tr("Year"),
        _ => tr("Recently Added"),
    }
}

/// A sort menu (docs/ui.md, "Albums" and "Songs"): the orders, then
/// Descending. Choosing an order sets the direction it starts with
/// (`library::starts_descending`); Descending flips it. `changed` runs after
/// either.
fn sort_menu(
    prefix: &str,
    tooltip: &str,
    choices: &[(String, &'static str)],
    state: SortState,
    changed: impl Fn() + 'static,
) -> gtk::MenuButton {
    let menu = gtk::gio::Menu::new();
    let orders = gtk::gio::Menu::new();
    for (label, key) in choices {
        let item = gtk::gio::MenuItem::new(Some(label), None);
        item.set_action_and_target_value(Some(&format!("{prefix}.sort")), Some(&key.to_variant()));
        orders.append_item(&item);
    }
    // Descending in the same section, so its check lines up with the orders'.
    orders.append(Some(&tr("Descending")), Some(&format!("{prefix}.descending")));
    menu.append_section(None, &orders);

    let (key, descending) = state.get();
    let sort = gtk::gio::SimpleAction::new_stateful("sort", Some(glib::VariantTy::STRING), &key.to_variant());
    let flip = gtk::gio::SimpleAction::new_stateful("descending", None, &descending.to_variant());
    let changed = Rc::new(changed);
    let keys: Vec<&'static str> = choices.iter().map(|(_, k)| *k).collect();
    let (s, f, c) = (state.clone(), flip.downgrade(), changed.clone());
    sort.connect_change_state(move |action, value| {
        let Some(wanted) = value.and_then(|v| v.get::<String>()) else {
            return;
        };
        let Some(key) = keys.iter().copied().find(|k| *k == wanted) else {
            return;
        };
        let descending = library::starts_descending(key);
        s.set((key, descending));
        action.set_state(&key.to_variant());
        if let Some(f) = f.upgrade() {
            f.set_state(&descending.to_variant());
        }
        c();
    });
    flip.connect_change_state(move |action, value| {
        let Some(on) = value.and_then(|v| v.get::<bool>()) else {
            return;
        };
        let (key, _) = state.get();
        state.set((key, on));
        action.set_state(&on.to_variant());
        changed();
    });
    let group = gtk::gio::SimpleActionGroup::new();
    group.add_action(&sort);
    group.add_action(&flip);
    let button = gtk::MenuButton::builder()
        .icon_name("view-sort-descending-symbolic")
        .menu_model(&menu)
        .tooltip_text(tooltip)
        .build();
    button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    button.insert_action_group(prefix, Some(&group));
    if let Some(p) = button.popover() {
        crate::ui::style::animate_popup(&p);
    }
    button
}

/// Reloads a paged model in place (a new order).
fn reorder(model: &glib::WeakRef<PagedModel>) {
    if let Some(model) = model.upgrade() {
        let len = model.len();
        model.reset(len);
    }
}

pub fn albums() -> adw::NavigationPage {
    let sort: SortState = Rc::new(Cell::new(("title", false)));
    let current = sort.clone();
    let model = PagedModel::new(move |model, offset, limit, generation| {
        let weak = model.downgrade();
        let (key, descending) = current.get();
        let by = library::album_sort(key);
        spawn(async move {
            let rows = app()
                .query(move |core| {
                    core.albums_sorted(by, descending, offset as usize, limit as usize)
                        .map(|v| v.iter().map(convert::album).collect::<Vec<_>>())
                })
                .await;
            let Some(model) = weak.upgrade() else { return };
            match rows {
                Some(Ok(rows)) => model.deliver(generation, offset, rows),
                _ => model.failed(generation, offset),
            }
        });
    });
    let grid = gtk::GridView::builder()
        .model(&gtk::NoSelection::new(Some(model.clone())))
        .factory(&card_factory(layout::GRID_COVER as i32))
        .min_columns(2)
        .max_columns(12)
        .single_click_activate(true)
        .build();
    grid.add_css_class("album-grid");
    follow_grid_width(&grid);
    grid.update_property(&[gtk::accessible::Property::Label(&tr("Albums"))]);
    grid.connect_activate(|grid, position| {
        if let Some(data) = item_at(grid.model().and_upcast(), position) {
            window::show_album(data.id, Some(data));
        }
    });
    let (scroll, small) = titled_scroller(&tr("Albums"), &grid, None);
    let stack = gtk::Stack::new();
    stack.add_named(&scroll, Some("list"));
    stack.add_named(
        &empty_library(
            &tr("No Albums Yet"),
            &tr("Add a music folder to see your albums here."),
        ),
        Some("empty"),
    );
    keep_counted(&model, &stack, |s| s.albums);
    let (page, header) = frame(&tr("Albums"), "albums", &stack);
    show_small_title(&header, &small);
    let choices: Vec<(String, &'static str)> = library::ALBUM_SORTS
        .iter()
        .map(|(key, _)| (sort_label(key), *key))
        .collect();
    let weak = model.downgrade();
    header.pack_end(&sort_menu(
        "albums",
        &tr("Sort Albums"),
        &choices,
        sort,
        move || reorder(&weak),
    ));
    page
}

pub fn artists() -> adw::NavigationPage {
    let model = paged(|core, offset, limit| {
        core.artists(offset, limit)
            .map(|v| v.iter().map(convert::artist).collect())
            .map_err(|e| e.to_string())
    });
    let list = gtk::ListView::builder()
        .model(&gtk::NoSelection::new(Some(model.clone())))
        .factory(&row_factory(RowStyle::ARTIST, None))
        .single_click_activate(true)
        .build();
    list.add_css_class("track-list");
    list.update_property(&[gtk::accessible::Property::Label(&tr("Artists"))]);
    list.connect_activate(|list, position| {
        if let Some(data) = item_at(list.model().and_upcast(), position) {
            window::show_artist(data.id, Some(data));
        }
    });
    let (scroll, small) = titled_scroller(&tr("Artists"), &clamped(&list), Some(LIST_WIDTH));
    let stack = gtk::Stack::new();
    stack.add_named(&scroll, Some("list"));
    stack.add_named(
        &empty_library(
            &tr("No Artists Yet"),
            &tr("Add a music folder to see your artists here."),
        ),
        Some("empty"),
    );
    keep_counted(&model, &stack, |s| s.artists);
    let (page, header) = frame(&tr("Artists"), "artists", &stack);
    show_small_title(&header, &small);
    page
}

/// The songs in the order the Songs page shows: `limit` from `offset`.
fn song_query(sort: (&'static str, bool), offset: usize, limit: usize) -> TrackQuery {
    let (key, descending) = sort;
    TrackQuery {
        offset,
        limit,
        sort: library::song_sort(key),
        descending,
        ..TrackQuery::default()
    }
}

/// Songs (docs/ui.md, "Songs"): Play and Shuffle, then the list, in the
/// order of its sort menu.
pub fn songs() -> adw::NavigationPage {
    let sort: SortState = Rc::new(Cell::new(("title", false)));
    let current = sort.clone();
    let model = PagedModel::new(move |model, offset, limit, generation| {
        let weak = model.downgrade();
        let query = song_query(current.get(), offset as usize, limit as usize);
        spawn(async move {
            let rows = app()
                .query(move |core| {
                    core.tracks(&query).map(|v| {
                        v.iter()
                            .map(|t| convert::track_with_favorite(core, t))
                            .collect::<Vec<_>>()
                    })
                })
                .await;
            let Some(model) = weak.upgrade() else { return };
            match rows {
                Some(Ok(rows)) => model.deliver(generation, offset, rows),
                Some(Err(e)) => {
                    tracing::warn!("a page failed to load: {e}");
                    model.failed(generation, offset);
                }
                None => model.failed(generation, offset),
            }
        });
    });
    let list = gtk::ListView::builder()
        .model(&gtk::NoSelection::new(Some(model.clone())))
        .factory(&row_factory(RowStyle::TRACK, None))
        .single_click_activate(true)
        .build();
    list.add_css_class("track-list");
    list.update_property(&[gtk::accessible::Property::Label(&tr("Songs"))]);
    let s = sort.clone();
    list.connect_activate(move |_, position| play_songs_from(s.get(), position, false));
    let (actions, play, shuffle) = play_and_shuffle();
    let s = sort.clone();
    play.connect_clicked(move |_| play_songs_from(s.get(), 0, false));
    let s = sort.clone();
    shuffle.connect_clicked(move |_| play_songs_from(s.get(), 0, true));
    let (scroll, small) = titled_scroller_with(
        &tr("Songs"),
        Some(actions.upcast_ref()),
        &clamped(&list),
        Some(LIST_WIDTH),
    );
    let stack = gtk::Stack::new();
    stack.add_named(&scroll, Some("list"));
    stack.add_named(
        &empty_library(
            &tr("No Songs Yet"),
            &tr("Add a music folder to see your songs here."),
        ),
        Some("empty"),
    );
    keep_counted(&model, &stack, |s| s.tracks);
    let (page, header) = frame(&tr("Songs"), "songs", &stack);
    show_small_title(&header, &small);
    let choices: Vec<(String, &'static str)> = library::SONG_SORTS
        .iter()
        .map(|(key, _)| (sort_label(key), *key))
        .collect();
    let weak = model.downgrade();
    header.pack_end(&sort_menu(
        "songs",
        &tr("Sort Songs"),
        &choices,
        sort,
        move || reorder(&weak),
    ));
    page
}

/// Songs played from the list in its order: from `position`, that song and
/// the next 499 (a queue of the whole library would be long and slow); or,
/// shuffled, up to 5,000 of them.
fn play_songs_from(sort: (&'static str, bool), position: u32, shuffle: bool) {
    let query = if shuffle {
        song_query(sort, 0, 5_000)
    } else {
        song_query(sort, position as usize, 500)
    };
    spawn(async move {
        let refs = app().query(move |core| core.tracks(&query)).await;
        if let Some(Ok(tracks)) = refs
            && !tracks.is_empty()
        {
            let tracks = tracks.into_iter().map(|t| TrackRef::Library(t.id)).collect();
            app().send(if shuffle {
                Command::PlayCollection {
                    collection: jf_core::Collection::Tracks { tracks },
                    start: 0,
                    shuffle: true,
                }
            } else {
                Command::PlayTracks { tracks, start: 0 }
            });
        }
    });
}

/// The widest long lists get (their titles line up with them).
pub const LIST_WIDTH: i32 = 1000;

/// Keeps long rows at a readable width on wide windows.
pub fn clamped(child: &impl IsA<gtk::Widget>) -> adw::ClampScrollable {
    let clamp = adw::ClampScrollable::builder()
        .child(child)
        .maximum_size(LIST_WIDTH)
        .tightening_threshold(700)
        .build();
    clamp.add_css_class("list-clamp");
    clamp
}

/// Keeps the grid's covers near their Luminous size: the column count
/// follows the grid's width (its horizontal adjustment's page), after the
/// layout that changed it.
fn follow_grid_width(grid: &gtk::GridView) {
    let fit = |grid: &gtk::GridView| {
        let Some(adj) = grid.hadjustment() else { return };
        let columns = layout::grid_columns(adj.page_size(), layout::GRID_COVER);
        if grid.max_columns() != columns {
            let weak = grid.downgrade();
            glib::idle_add_local_once(move || {
                if let Some(grid) = weak.upgrade() {
                    grid.set_max_columns(columns);
                }
            });
        }
    };
    let watch = move |grid: &gtk::GridView| {
        if let Some(adj) = grid.hadjustment() {
            let weak = grid.downgrade();
            adj.connect_page_size_notify(move |_| {
                if let Some(grid) = weak.upgrade() {
                    fit(&grid);
                }
            });
        }
    };
    watch(grid);
    grid.connect_hadjustment_notify(move |grid| watch(grid));
}

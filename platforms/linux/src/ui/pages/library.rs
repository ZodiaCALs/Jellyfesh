//! Albums (a grid of covers), Artists and Songs: long lists, virtualised and
//! loaded page by page from the query thread.

use adw::prelude::*;
use gtk::glib;
use jf_core::{AlbumSort, Command, LibraryStats, TrackQuery, TrackRef, TrackSort};

use crate::ui::convert;
use crate::ui::i18n::tr;
use crate::ui::models::{Item, ItemData, PagedModel};
use crate::ui::pages::{frame, show_small_title, status, titled_scroller};
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

pub fn albums() -> adw::NavigationPage {
    let sort = std::rc::Rc::new(std::cell::Cell::new((AlbumSort::Title, false)));
    let current = sort.clone();
    let model = PagedModel::new(move |model, offset, limit, generation| {
        let weak = model.downgrade();
        let (by, descending) = current.get();
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
        .factory(&card_factory(176))
        .min_columns(2)
        .max_columns(12)
        .single_click_activate(true)
        .build();
    grid.add_css_class("album-grid");
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
            &tr("Add a folder with music to fill your library."),
        ),
        Some("empty"),
    );
    keep_counted(&model, &stack, |s| s.albums);
    let (page, header) = frame(&tr("Albums"), "albums", &stack);
    show_small_title(&header, &small);
    header.pack_end(&album_sort_menu(&model, sort));
    page
}

/// The album order: title, artist, year or recently added.
fn album_sort_menu(
    model: &PagedModel,
    sort: std::rc::Rc<std::cell::Cell<(AlbumSort, bool)>>,
) -> gtk::MenuButton {
    let menu = gtk::gio::Menu::new();
    for (label, key) in [
        (tr("Title"), "title"),
        (tr("Artist"), "artist"),
        (tr("Year"), "year"),
        (tr("Recently Added"), "added"),
    ] {
        let item = gtk::gio::MenuItem::new(Some(&label), None);
        item.set_action_and_target_value(Some("albums.sort"), Some(&key.to_variant()));
        menu.append_item(&item);
    }
    let action =
        gtk::gio::SimpleAction::new_stateful("sort", Some(glib::VariantTy::STRING), &"title".to_variant());
    let weak = model.downgrade();
    action.connect_change_state(move |action, value| {
        let Some(key) = value.and_then(|v| v.get::<String>()) else {
            return;
        };
        let order = match key.as_str() {
            "artist" => (AlbumSort::Artist, false),
            "year" => (AlbumSort::Year, true),
            "added" => (AlbumSort::Added, true),
            _ => (AlbumSort::Title, false),
        };
        sort.set(order);
        action.set_state(&key.to_variant());
        if let Some(model) = weak.upgrade() {
            let len = model.len();
            model.reset(len);
        }
    });
    let group = gtk::gio::SimpleActionGroup::new();
    group.add_action(&action);
    let button = gtk::MenuButton::builder()
        .icon_name("view-sort-descending-symbolic")
        .menu_model(&menu)
        .tooltip_text(tr("Sort Albums"))
        .build();
    button.update_property(&[gtk::accessible::Property::Label(&tr("Sort Albums"))]);
    button.insert_action_group("albums", Some(&group));
    if let Some(p) = button.popover() {
        crate::ui::style::animate_popup(&p);
    }
    button
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
            &tr("Add a folder with music to fill your library."),
        ),
        Some("empty"),
    );
    keep_counted(&model, &stack, |s| s.artists);
    let (page, header) = frame(&tr("Artists"), "artists", &stack);
    show_small_title(&header, &small);
    page
}

/// Songs sorted by title.
const SONG_SORT: TrackSort = TrackSort::Title;

pub fn songs() -> adw::NavigationPage {
    let model = paged(|core, offset, limit| {
        core.tracks(&TrackQuery {
            offset,
            limit,
            sort: SONG_SORT,
            ..TrackQuery::default()
        })
        .map(|v| v.iter().map(|t| convert::track_with_favorite(core, t)).collect())
        .map_err(|e| e.to_string())
    });
    let list = gtk::ListView::builder()
        .model(&gtk::NoSelection::new(Some(model.clone())))
        .factory(&row_factory(RowStyle::TRACK, None))
        .single_click_activate(true)
        .build();
    list.add_css_class("track-list");
    list.update_property(&[gtk::accessible::Property::Label(&tr("Songs"))]);
    list.connect_activate(|_, position| play_songs_from(position));
    let (scroll, small) = titled_scroller(&tr("Songs"), &clamped(&list), Some(LIST_WIDTH));
    let stack = gtk::Stack::new();
    stack.add_named(&scroll, Some("list"));
    stack.add_named(
        &empty_library(
            &tr("No Songs Yet"),
            &tr("Add a folder with music to fill your library."),
        ),
        Some("empty"),
    );
    keep_counted(&model, &stack, |s| s.tracks);
    let (page, header) = frame(&tr("Songs"), "songs", &stack);
    show_small_title(&header, &small);
    let shuffle = crate::ui::widgets::glyph::button("shuffle", 18, &tr("Shuffle All"));
    shuffle.connect_clicked(|_| {
        spawn(async {
            let refs = app()
                .query(|core| {
                    core.tracks(&TrackQuery {
                        limit: 5_000,
                        sort: SONG_SORT,
                        ..TrackQuery::default()
                    })
                })
                .await;
            if let Some(Ok(tracks)) = refs
                && !tracks.is_empty()
            {
                let tracks = tracks.into_iter().map(|t| TrackRef::Library(t.id)).collect();
                app().send(Command::PlayCollection {
                    collection: jf_core::Collection::Tracks { tracks },
                    start: 0,
                    shuffle: true,
                });
            }
        });
    });
    header.pack_end(&shuffle);
    page
}

/// Plays the song list from `position`: that song and the next 499 in the
/// same order (a queue of the whole library would be long and slow).
fn play_songs_from(position: u32) {
    spawn(async move {
        let refs = app()
            .query(move |core| {
                core.tracks(&TrackQuery {
                    offset: position as usize,
                    limit: 500,
                    sort: SONG_SORT,
                    ..TrackQuery::default()
                })
            })
            .await;
        if let Some(Ok(tracks)) = refs
            && !tracks.is_empty()
        {
            let tracks = tracks.into_iter().map(|t| TrackRef::Library(t.id)).collect();
            app().send(Command::PlayTracks { tracks, start: 0 });
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

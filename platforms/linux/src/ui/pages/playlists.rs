//! Playlists: the list, and a playlist's page with drag-and-drop ordering.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Collection, Command};

use crate::logic::queue::move_target;
use crate::ui::convert;
use crate::ui::i18n::tr;
use crate::ui::models::{Item, ItemData, Kind};
use crate::ui::pages::detail::Header;
use crate::ui::pages::library::{clamped, paged};
use crate::ui::pages::{follow_title, frame, keep_alive, large_title, pill, scroller, small_title, status};
use crate::ui::rows::{ListRow, RowStyle};
use crate::ui::state::{Change, app, spawn};
use crate::ui::thumbs::ArtKey;
use crate::ui::widgets::maxwidth::MaxWidth;
use crate::ui::{actions, dnd, window};

pub fn page() -> adw::NavigationPage {
    let list = gtk::ListBox::new();
    list.add_css_class("track-box");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.update_property(&[gtk::accessible::Property::Label(&tr("Playlists"))]);
    list.connect_row_activated(|_, row| {
        if let Some(data) = row.child().and_downcast::<ListRow>().and_then(|r| r.data()) {
            window::show_playlist(data.id, Some(data));
        }
    });
    let title = large_title(&tr("Playlists"));
    // New Playlist… and Import Playlist… under the title, as in every app.
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.add_css_class("page-actions");
    let add = pill(&tr("New Playlist…"), "plus", false);
    add.add_css_class("glass-pill");
    add.connect_clicked(|_| new_playlist());
    let import = pill(&tr("Import Playlist…"), "folder", false);
    import.add_css_class("glass-pill");
    import.connect_clicked(|_| import_playlist());
    buttons.append(&add);
    buttons.append(&import);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.add_css_class("titled-list");
    column.append(&title);
    column.append(&buttons);
    column.append(&list);
    // The sources' playlists, one section per signed-in source (ADR-0024).
    let sources = gtk::Box::new(gtk::Orientation::Vertical, 12);
    sources.add_css_class("source-sections");
    sources.set_visible(false);
    column.append(&sources);
    let clamp = MaxWidth::new(&column, 1000);
    let scroll = scroller(&clamp);
    let stack = gtk::Stack::new();
    stack.add_named(&scroll, Some("list"));
    let empty = status(
        "view-list-symbolic",
        &tr("No Playlists Yet"),
        &tr("Make a playlist to keep songs together."),
    );
    let new_button = gtk::Button::with_label(&tr("New Playlist…"));
    new_button.add_css_class("pill");
    new_button.add_css_class("suggested-action");
    new_button.set_halign(gtk::Align::Center);
    new_button.connect_clicked(|_| new_playlist());
    empty.set_child(Some(&new_button));
    stack.add_named(&empty, Some("empty"));
    let error = status("dialog-information-symbolic", &tr("Could Not Load Playlists"), "");
    stack.add_named(&error, Some("error"));

    let (page, header) = frame(&tr("Playlists"), "playlists", &stack);
    let small = small_title(&header, &tr("Playlists"));
    follow_title(&scroll, &title, &small);

    let refresh = {
        let list = list.downgrade();
        let stack = stack.downgrade();
        let error = error.downgrade();
        let sources = sources.downgrade();
        move || {
            let (list, stack, error, sources) = (list.clone(), stack.clone(), error.clone(), sources.clone());
            spawn(async move {
                let result = app()
                    .query(|core| core.playlists().map_err(|e| e.to_string()))
                    .await;
                let (Some(list), Some(stack), Some(error), Some(sources)) = (
                    list.upgrade(),
                    stack.upgrade(),
                    error.upgrade(),
                    sources.upgrade(),
                ) else {
                    return;
                };
                match result {
                    Some(Ok(playlists)) => {
                        while let Some(c) = list.first_child() {
                            list.remove(&c);
                        }
                        for p in &playlists {
                            let row = ListRow::new(RowStyle::PLAYLIST);
                            row.bind(&Item::new(Some(convert::playlist(p))));
                            list.append(&gtk::ListBoxRow::builder().child(&row).build());
                        }
                        // "No Playlists Yet" only when the sources have none either.
                        list.set_visible(!playlists.is_empty());
                        let empty = playlists.is_empty() && !sources.is_visible();
                        stack.set_visible_child_name(if empty { "empty" } else { "list" });
                    }
                    Some(Err(e)) => {
                        error.set_description(Some(&e));
                        stack.set_visible_child_name("error");
                    }
                    None => {}
                }
            });
        }
    };
    // The sources' playlists, after Jellyfesh's.
    let show_sources = {
        let (sources, list, stack) = (sources.downgrade(), list.downgrade(), stack.downgrade());
        move || {
            let (Some(sources), Some(list), Some(stack)) =
                (sources.upgrade(), list.upgrade(), stack.upgrade())
            else {
                return;
            };
            let library = app().source_library.borrow().clone();
            crate::ui::pages::sourcelib::fill(&sources, &crate::logic::sourcelib::playlists(&library));
            if sources.is_visible() && stack.visible_child_name().as_deref() == Some("empty") {
                list.set_visible(false);
                stack.set_visible_child_name("list");
            }
        }
    };
    show_sources();
    refresh();
    let r = refresh.clone();
    page.connect_showing(move |_| {
        r();
        crate::ui::pages::sourcelib::page_shown("playlists");
    });
    app().listen_for(&page, move |change| match change {
        Change::Library => refresh(),
        Change::SourceLibrary => {
            show_sources();
            refresh();
        }
        _ => {}
    });
    page
}

fn new_playlist() {
    spawn(async {
        let a = app();
        let dialog = adw::AlertDialog::new(Some(&tr("New Playlist")), None);
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("create", &tr("Create"));
        dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("create"));
        dialog.set_close_response("cancel");
        let entry = gtk::Entry::builder()
            .placeholder_text(tr("Name"))
            .activates_default(true)
            .build();
        dialog.set_extra_child(Some(&entry));
        let parent = a.window.borrow().upgrade();
        if dialog.choose_future(parent.as_ref()).await == "create" {
            let name = entry.text().trim().to_owned();
            let name = if name.is_empty() { tr("New Playlist") } else { name };
            let request = a.send(Command::CreatePlaylist {
                name,
                tracks: Vec::new(),
            });
            open_when_created(request);
        }
    });
}

/// Opens the playlist the command `request` makes, when the core reports it.
pub fn open_when_created(request: jf_core::CommandId) {
    let a = app();
    let id = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let listener = id.clone();
    id.set(a.listen(move |change| {
        let done = match change {
            Change::PlaylistCreated(r, playlist) if *r == request => {
                window::show_playlist(*playlist, None);
                true
            }
            Change::Failed(r, _) => *r == request,
            _ => false,
        };
        if done {
            let listener = listener.get();
            glib::idle_add_local_once(move || app().unlisten(listener));
        }
    }));
}

fn import_playlist() {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(&tr("Playlists (M3U8)")));
    filter.add_suffix("m3u");
    filter.add_suffix("m3u8");
    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let dialog = gtk::FileDialog::builder()
        .title(tr("Import Playlist"))
        .filters(&filters)
        .modal(true)
        .build();
    let parent = app().window.borrow().upgrade();
    dialog.open(parent.as_ref(), gtk::gio::Cancellable::NONE, |result| {
        if let Ok(file) = result
            && let Some(path) = file.path()
        {
            let request = app().send(Command::ImportPlaylist { path });
            open_when_created(request);
        }
    });
}

fn export_playlist(id: i64, name: &str) {
    let dialog = gtk::FileDialog::builder()
        .title(tr("Export Playlist"))
        .initial_name(format!("{name}.m3u8"))
        .modal(true)
        .build();
    let parent = app().window.borrow().upgrade();
    dialog.save(parent.as_ref(), gtk::gio::Cancellable::NONE, move |result| {
        let Ok(file) = result else { return };
        let Some(path) = file.path() else { return };
        spawn(async move {
            let shown = path.display().to_string();
            let result = app()
                .query(move |core| core.export_playlist(id, &path).map_err(|e| e.to_string()))
                .await;
            match result {
                Some(Ok(())) => app().toast(&crate::ui::i18n::trf("Saved to {path}", &[("path", &shown)])),
                Some(Err(e)) => app().toast(&e),
                None => {}
            }
        });
    });
}

/// Puts `label` (for `action`) into `menu` right before the item for
/// `before`, in that item's section; at the end of the menu without one.
fn insert_before(menu: &gtk::gio::Menu, before: &str, label: &str, action: &str) {
    for i in 0..menu.n_items() {
        let Some(section) = menu
            .item_link(i, gtk::gio::MENU_LINK_SECTION.as_str())
            .and_downcast::<gtk::gio::Menu>()
        else {
            continue;
        };
        for j in 0..section.n_items() {
            let found = section
                .item_attribute_value(j, gtk::gio::MENU_ATTRIBUTE_ACTION.as_str(), None)
                .and_then(|v| v.get::<String>())
                .is_some_and(|a| a == before);
            if found {
                section.insert(j, Some(label), Some(action));
                return;
            }
        }
    }
    let section = gtk::gio::Menu::new();
    section.append(Some(label), Some(action));
    menu.append_section(None, &section);
}

pub fn playlist_page(id: i64, known: Option<ItemData>) -> adw::NavigationPage {
    let header = Header::new(&tr("Playlist"), false);
    header.subtitle.set_visible(false);
    let collection = Collection::Playlist { id };
    header.play.connect_clicked(glib::clone!(
        #[strong]
        collection,
        move |_| actions::play(collection.clone(), 0, false)
    ));
    header.shuffle.connect_clicked(glib::clone!(
        #[strong]
        collection,
        move |_| actions::play(collection.clone(), 0, true)
    ));
    let name = known.as_ref().map_or_else(|| tr("Playlist"), |k| k.title.clone());
    let menu = actions::more_menu_for(&ItemData {
        kind: Kind::Playlist,
        id,
        title: name.clone(),
        ..ItemData::default()
    });
    // Rename…, Export Playlist…, Delete Playlist…, as in every app.
    insert_before(
        &menu,
        "win.delete-playlist",
        &tr("Export Playlist…"),
        "playlist.export",
    );
    header.more.set_menu_model(Some(&menu));
    let group = gtk::gio::SimpleActionGroup::new();
    let export_action = gtk::gio::SimpleAction::new("export", None);
    let export_name = name.clone();
    export_action.connect_activate(move |_, _| export_playlist(id, &export_name));
    group.add_action(&export_action);
    header.root.insert_action_group("playlist", Some(&group));
    header.title.set_label(&name);
    if let Some(k) = &known {
        header
            .meta
            .set_label(format!("{} {}", k.subtitle, k.detail).trim());
        header.art.set_art(k.art.clone());
    }

    let model = paged(move |core, offset, limit| {
        core.playlist_entries(id, offset, limit)
            .map(|v| v.iter().map(convert::playlist_entry).collect())
            .map_err(|e| e.to_string())
    });
    let on_remove: crate::ui::rows::Handler = Rc::new(move |data: &ItemData| {
        app().send(Command::RemoveFromPlaylist {
            id,
            entries: vec![data.id],
        });
    });
    let weak_model = model.downgrade();
    let on_move: dnd::OnMove = Rc::new(move |from, target, side| {
        let Some(model) = weak_model.upgrade() else { return };
        let Some(entry) = model.data_at(from) else { return };
        if let Some(to) = move_target(from as usize, target as usize, side, model.len() as usize) {
            app().send(Command::MovePlaylistEntry {
                id,
                entry: entry.id,
                to,
            });
        }
    });
    let factory = crate::ui::rows::row_factory(RowStyle::PLAYLIST_ENTRY, Some(on_remove));
    factory.connect_setup(move |_, obj| {
        if let Some(li) = obj.downcast_ref::<gtk::ListItem>()
            && let Some(row) = li.child()
        {
            dnd::attach(&row, li, on_move.clone());
        }
    });
    let list = gtk::ListView::builder()
        .model(&gtk::NoSelection::new(Some(model.clone())))
        .factory(&factory)
        .single_click_activate(true)
        .build();
    list.add_css_class("track-list");
    list.connect_activate(move |_, position| {
        actions::play(Collection::Playlist { id }, position as usize, false);
    });
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let header_clamp = MaxWidth::new(&header.root, 1000);
    header_clamp.add_css_class("detail");
    body.append(&header_clamp);
    let list_scroll = scroller(&clamped(&list));
    body.append(&list_scroll);
    // A playlist without songs says how to add some.
    let empty = status(
        "view-list-symbolic",
        &tr("This Playlist Is Empty"),
        &tr("Add songs from their menu with Add to Playlist."),
    );
    empty.set_visible(false);
    body.append(&empty);
    let (page, _) = frame(&name, &format!("playlist-{id}"), &body);

    let header = Rc::new(header);
    let refresh = {
        let header = Rc::downgrade(&header);
        let model = model.downgrade();
        let page = page.downgrade();
        let (list_scroll, empty) = (list_scroll.downgrade(), empty.downgrade());
        move || {
            let (header, model, page) = (header.clone(), model.clone(), page.clone());
            let (list_scroll, empty) = (list_scroll.clone(), empty.clone());
            spawn(async move {
                let result = app()
                    .query(move |core| core.playlist(id).map_err(|e| e.to_string()))
                    .await;
                let (Some(header), Some(model), Some(page)) =
                    (header.upgrade(), model.upgrade(), page.upgrade())
                else {
                    return;
                };
                match result {
                    Some(Ok(found)) => {
                        let Some(p) = found.as_ref() else {
                            // Deleted elsewhere: back to the list.
                            window::pop_to_section();
                            return;
                        };
                        header.title.set_label(&p.name);
                        page.set_title(&p.name);
                        let data = convert::playlist(p);
                        header.meta.set_label(&crate::logic::format::dot_join([
                            Some(data.subtitle.as_str()),
                            Some(data.detail.as_str()),
                        ]));
                        header.art.set_art(p.artwork_hash.clone().map(ArtKey::Hash));
                        model.reload(p.entry_count);
                        if let (Some(list), Some(empty)) = (list_scroll.upgrade(), empty.upgrade()) {
                            list.set_visible(p.entry_count > 0);
                            empty.set_visible(p.entry_count == 0);
                        }
                    }
                    Some(Err(e)) => header.meta.set_label(&e),
                    None => {}
                }
            });
        }
    };
    refresh();
    app().listen_for(&page, move |change| {
        if matches!(change, Change::Library) {
            refresh();
        }
    });
    keep_alive(&page, header);
    page
}

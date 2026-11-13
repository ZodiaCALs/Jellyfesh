//! What rows, cards and menus do: play, queue, add to a playlist,
//! favourite, and go to an album or artist. Each is a command to the core;
//! nothing here changes state on its own.

use adw::prelude::*;
use gtk::{gio, glib};
use jf_core::{Collection, Command, TrackQuery, TrackRef, TrackSort};
use serde::{Deserialize, Serialize};

use crate::ui::i18n::tr;
use crate::ui::models::{ItemData, Kind};
use crate::ui::state::{app, spawn};
use crate::ui::window;

#[derive(Debug, Serialize, Deserialize)]
struct PlayArgs {
    collection: Collection,
    start: usize,
    shuffle: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct FavoriteArgs {
    track: TrackRef,
    favorite: bool,
}

pub fn play(collection: Collection, start: usize, shuffle: bool) {
    app().send(Command::PlayCollection {
        collection,
        start,
        shuffle,
    });
}

pub fn enqueue(collection: Collection, play_next: bool) {
    let a = app();
    a.send(Command::EnqueueCollection {
        collection,
        play_next,
    });
    a.toast(&if play_next {
        tr("Plays next")
    } else {
        tr("Added to the queue")
    });
}

/// The collection an item stands for, if it can be played.
pub fn collection_of(data: &ItemData) -> Option<Collection> {
    match data.kind {
        Kind::Album => Some(Collection::Album { id: data.id }),
        Kind::Artist => Some(Collection::Artist { id: data.id }),
        Kind::Playlist => Some(Collection::Playlist { id: data.id }),
        Kind::Source => match (&data.track, &data.source) {
            (Some(t), _) => Some(Collection::Tracks {
                tracks: vec![t.clone()],
            }),
            (None, Some(s)) => Some(Collection::Source {
                plugin: s.source.clone(),
                id: s.id.clone(),
            }),
            _ => None,
        },
        Kind::Track | Kind::QueueEntry | Kind::PlaylistEntry => {
            data.track.clone().map(|t| Collection::Tracks { tracks: vec![t] })
        }
    }
}

pub fn toggle_favorite(_from: &impl IsA<gtk::Widget>, data: &ItemData) {
    let Some(track) = data.track.clone() else { return };
    app().send(Command::SetFavorite {
        track,
        favorite: !data.favorite,
    });
}

fn json_target(value: &impl Serialize) -> glib::Variant {
    serde_json::to_string(value).unwrap_or_default().to_variant()
}

fn entry(menu: &gio::Menu, label: &str, action: &str, target: glib::Variant) {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(Some(action), Some(&target));
    menu.append_item(&item);
}

/// The menu for an item.
pub fn menu_for(data: &ItemData) -> gio::Menu {
    let menu = gio::Menu::new();
    let playing = gio::Menu::new();
    if let Some(c) = collection_of(data) {
        if matches!(data.kind, Kind::Album | Kind::Artist | Kind::Playlist) {
            let args = PlayArgs {
                collection: c.clone(),
                start: 0,
                shuffle: false,
            };
            entry(&playing, &tr("Play"), "win.play", json_target(&args));
            let args = PlayArgs {
                shuffle: true,
                ..args
            };
            entry(&playing, &tr("Shuffle"), "win.play", json_target(&args));
        }
        if data.kind != Kind::QueueEntry {
            entry(&playing, &tr("Play Next"), "win.play-next", json_target(&c));
            entry(&playing, &tr("Play Last"), "win.play-last", json_target(&c));
        }
        if data.kind != Kind::Source || data.track.is_some() {
            entry(
                &playing,
                &tr("Add to Playlist…"),
                "win.add-to-playlist",
                json_target(&c),
            );
        }
    }
    menu.append_section(None, &playing);
    let more = gio::Menu::new();
    if let Some(track) = &data.track
        && !matches!(track, TrackRef::File(_))
    {
        let args = FavoriteArgs {
            track: track.clone(),
            favorite: !data.favorite,
        };
        let label = if data.favorite {
            tr("Remove from Favourites")
        } else {
            tr("Add to Favourites")
        };
        entry(&more, &label, "win.favorite", json_target(&args));
    }
    if let Some(album) = data.album_id
        && data.kind != Kind::Album
    {
        entry(&more, &tr("Go to Album"), "win.show-album", album.to_variant());
    }
    if let Some(artist) = data.artist_id
        && data.kind != Kind::Artist
    {
        entry(&more, &tr("Go to Artist"), "win.show-artist", artist.to_variant());
    }
    if data.kind == Kind::Playlist {
        entry(&more, &tr("Rename…"), "win.rename-playlist", data.id.to_variant());
        entry(&more, &tr("Delete"), "win.delete-playlist", data.id.to_variant());
    }
    if more.n_items() > 0 {
        menu.append_section(None, &more);
    }
    menu
}

/// Shows the item's menu under a button.
pub fn popup_menu(anchor: &gtk::Button, data: &ItemData) {
    let pop = gtk::PopoverMenu::from_model(Some(&menu_for(data)));
    crate::ui::style::animate_popup(&pop);
    pop.set_parent(anchor);
    pop.set_has_arrow(true);
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
}

/// Shows the item's menu at a point in `widget`.
pub fn popup_menu_at(widget: &impl IsA<gtk::Widget>, data: &ItemData, x: f64, y: f64) {
    let pop = gtk::PopoverMenu::from_model(Some(&menu_for(data)));
    crate::ui::style::animate_popup(&pop);
    pop.set_parent(widget);
    pop.set_has_arrow(false);
    pop.set_halign(gtk::Align::Start);
    pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
}

fn string_action(name: &str, f: impl Fn(&str) + 'static) -> gio::SimpleAction {
    let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, param| {
        if let Some(s) = param.and_then(|p| p.get::<String>()) {
            f(&s);
        }
    });
    action
}

fn id_action(name: &str, f: impl Fn(i64) + 'static) -> gio::SimpleAction {
    let action = gio::SimpleAction::new(name, Some(glib::VariantTy::INT64));
    action.connect_activate(move |_, param| {
        if let Some(id) = param.and_then(|p| p.get::<i64>()) {
            f(id);
        }
    });
    action
}

/// Installs the item actions on the main window.
pub fn install(window: &adw::ApplicationWindow) {
    window.add_action(&string_action("play", |json| {
        if let Ok(args) = serde_json::from_str::<PlayArgs>(json) {
            play(args.collection, args.start, args.shuffle);
        }
    }));
    window.add_action(&string_action("play-next", |json| {
        if let Ok(c) = serde_json::from_str::<Collection>(json) {
            enqueue(c, true);
        }
    }));
    window.add_action(&string_action("play-last", |json| {
        if let Ok(c) = serde_json::from_str::<Collection>(json) {
            enqueue(c, false);
        }
    }));
    window.add_action(&string_action("add-to-playlist", |json| {
        if let Ok(c) = serde_json::from_str::<Collection>(json) {
            add_to_playlist(c);
        }
    }));
    window.add_action(&string_action("favorite", |json| {
        if let Ok(args) = serde_json::from_str::<FavoriteArgs>(json) {
            app().send(Command::SetFavorite {
                track: args.track,
                favorite: args.favorite,
            });
        }
    }));
    window.add_action(&id_action("show-album", |id| window::show_album(id, None)));
    window.add_action(&id_action("show-artist", |id| window::show_artist(id, None)));
    window.add_action(&id_action("show-playlist", |id| window::show_playlist(id, None)));
    window.add_action(&id_action("rename-playlist", rename_playlist));
    window.add_action(&id_action("delete-playlist", delete_playlist));
}

/// The track references of a collection (resolved on the query thread).
async fn tracks_of(collection: Collection) -> Result<Vec<TrackRef>, String> {
    let result = app()
        .query(move |core| -> Result<Vec<TrackRef>, String> {
            let refs =
                |tracks: Vec<jf_core::Track>| tracks.into_iter().map(|t| TrackRef::Library(t.id)).collect();
            match collection {
                Collection::Tracks { tracks } => Ok(tracks),
                Collection::Album { id } => core.album_tracks(id).map(refs).map_err(|e| e.to_string()),
                Collection::Artist { id } => core
                    .tracks(&TrackQuery {
                        artist_id: Some(id),
                        sort: TrackSort::Artist,
                        limit: usize::MAX,
                        ..TrackQuery::default()
                    })
                    .map(refs)
                    .map_err(|e| e.to_string()),
                Collection::Playlist { id } => core
                    .playlist_entries(id, 0, usize::MAX)
                    .map(|e| e.into_iter().map(|e| e.row.track).collect())
                    .map_err(|e| e.to_string()),
                Collection::Favorites => core
                    .favorites(0, usize::MAX)
                    .map(|r| r.into_iter().map(|r| r.track).collect())
                    .map_err(|e| e.to_string()),
                Collection::Source { .. } => Err(tr("Add the songs from the source's own page.")),
            }
        })
        .await;
    result.unwrap_or_else(|| Err(String::new()))
}

/// Asks which playlist to add a collection to (or names a new one).
pub fn add_to_playlist(collection: Collection) {
    spawn(async move {
        let a = app();
        let playlists = a.query(|core| core.playlists().map_err(|e| e.to_string())).await;
        let playlists = match playlists {
            Some(Ok(p)) => p,
            Some(Err(e)) => {
                a.toast(&e);
                Vec::new()
            }
            None => return,
        };
        let dialog = adw::AlertDialog::new(Some(&tr("Add to Playlist")), None);
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("add", &tr("Add"));
        dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("add"));
        dialog.set_close_response("cancel");
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::Single);
        let new_row = adw::EntryRow::builder().title(tr("New Playlist")).build();
        list.append(&new_row);
        for p in &playlists {
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&p.name).as_str())
                .subtitle(crate::ui::i18n::trn(
                    "{n} song",
                    "{n} songs",
                    u64::from(p.entry_count),
                ))
                .build();
            row.set_widget_name(&p.id.to_string());
            list.append(&row);
        }
        if let Some(first) = list.row_at_index(if playlists.is_empty() { 0 } else { 1 }) {
            list.select_row(Some(&first));
        }
        let scroller = gtk::ScrolledWindow::builder()
            .child(&list)
            .propagate_natural_height(true)
            .max_content_height(320)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        dialog.set_extra_child(Some(&scroller));
        let parent = a.window.borrow().upgrade();
        let response = dialog.choose_future(parent.as_ref()).await;
        if response != "add" {
            return;
        }
        let tracks = match tracks_of(collection).await {
            Ok(t) => t,
            Err(e) => {
                if !e.is_empty() {
                    a.toast(&e);
                }
                return;
            }
        };
        let selected = list.selected_row();
        let new_name = new_row.text().trim().to_owned();
        match selected.and_then(|r| r.widget_name().parse::<i64>().ok()) {
            Some(id) if new_name.is_empty() => {
                a.send(Command::AddToPlaylist { id, tracks });
            }
            _ => {
                let name = if new_name.is_empty() {
                    tr("New Playlist")
                } else {
                    new_name
                };
                let request = a.send(Command::CreatePlaylist { name, tracks });
                crate::ui::pages::playlists::open_when_created(request);
            }
        }
    });
}

fn rename_playlist(id: i64) {
    spawn(async move {
        let a = app();
        let dialog = adw::AlertDialog::new(Some(&tr("Rename Playlist")), None);
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("rename", &tr("Rename"));
        dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("rename"));
        dialog.set_close_response("cancel");
        let entry = gtk::Entry::builder().activates_default(true).build();
        dialog.set_extra_child(Some(&entry));
        let parent = a.window.borrow().upgrade();
        if dialog.choose_future(parent.as_ref()).await == "rename" {
            let name = entry.text().trim().to_owned();
            if !name.is_empty() {
                a.send(Command::RenamePlaylist { id, name });
            }
        }
    });
}

fn delete_playlist(id: i64) {
    spawn(async move {
        let a = app();
        let dialog = adw::AlertDialog::new(
            Some(&tr("Delete Playlist?")),
            Some(&tr("The songs stay in your library.")),
        );
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("delete", &tr("Delete"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_close_response("cancel");
        let parent = a.window.borrow().upgrade();
        if dialog.choose_future(parent.as_ref()).await == "delete" {
            a.send(Command::DeletePlaylist { id });
            window::pop_to_section();
        }
    });
}

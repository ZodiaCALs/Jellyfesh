//! What rows, cards and menus do: play, queue, add to a playlist,
//! favourite, go to an album or artist, show a file in Files, and open an
//! item in its service (Open in {Service}, ADR-0018). Each is a command to
//! the core (or the file manager, or the system's handler for links);
//! nothing here changes state on its own.

use std::path::Path;

use adw::prelude::*;
use gtk::{gio, glib};
use jf_core::{Collection, Command, TrackQuery, TrackRef, TrackSort};
use serde::{Deserialize, Serialize};

use crate::logic::links::{self, MainAction};
use crate::ui::details::InfoArgs;
use crate::ui::i18n::{tr, trf};
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
            // Albums, artists and playlists of a source; a song without a
            // track cannot play here.
            (None, Some(s)) if s.kind != jf_core::ItemKind::Track && s.playable => Some(Collection::Source {
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

/// The menu for an item (right click, long press, or a row's More): for a
/// song, Play Next, Play Last, Add to Playlist…, the favourite, Go to Album,
/// Go to Artist and Show in Files (docs/ui.md, "Song menu"); albums, artists
/// and playlists also offer Play and Shuffle.
pub fn menu_for(data: &ItemData) -> gio::Menu {
    build_menu(data, false)
}

/// The More menu of an album, artist or playlist page, whose Play and
/// Shuffle are buttons beside it: Play Next, Play Last, Add to Playlist…
/// (and Rename… and Delete Playlist… on a playlist).
pub fn more_menu_for(data: &ItemData) -> gio::Menu {
    build_menu(data, true)
}

/// The item's link and the name of the service it opens in, for Open in
/// {Service}.
pub fn link_of(data: &ItemData) -> Option<(String, String)> {
    let s = data.source.as_ref()?;
    let link = s.link.clone().filter(|l| links::openable(l))?;
    Some((link, app().source_name(&s.source)))
}

/// "Open in {Service}".
pub fn open_in_label(service: &str) -> String {
    trf("Open in {service}", &[("service", service)])
}

/// Whether Play Next, Play Last and Add to Playlist are offered: not for a
/// remote source's items (they never join the queue, ADR-0018) nor for
/// items that cannot play here.
fn offers_queueing(data: &ItemData) -> bool {
    match &data.source {
        Some(s) if s.source != crate::logic::search::LIBRARY => {
            links::offers_queueing(s, app().is_remote_source(&s.source))
        }
        _ => data.playable,
    }
}

/// The menu for `data`; `own_page` for the More menu on the item's own page.
fn build_menu(data: &ItemData, own_page: bool) -> gio::Menu {
    let menu = gio::Menu::new();
    let playing = gio::Menu::new();
    let queueing = offers_queueing(data);
    if let Some(c) = collection_of(data) {
        if !own_page && matches!(data.kind, Kind::Album | Kind::Artist | Kind::Playlist) {
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
        if data.kind != Kind::QueueEntry && queueing {
            entry(&playing, &tr("Play Next"), "win.play-next", json_target(&c));
            entry(&playing, &tr("Play Last"), "win.play-last", json_target(&c));
        }
        if (data.kind != Kind::Source || data.track.is_some()) && queueing {
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
        && !own_page
    {
        entry(&more, &tr("Go to Album"), "win.show-album", album.to_variant());
    }
    if data.kind != Kind::Artist && !own_page {
        if let Some(artist) = data.artist_id {
            entry(&more, &tr("Go to Artist"), "win.show-artist", artist.to_variant());
        } else if let Some(name) = data.artist.as_deref().filter(|n| !n.trim().is_empty()) {
            entry(
                &more,
                &tr("Go to Artist"),
                "win.show-artist-named",
                name.to_variant(),
            );
        }
    }
    // Open in {Service}, after Go to Artist (docs/ui.md).
    if let Some((link, service)) = link_of(data) {
        entry(
            &more,
            &open_in_label(&service),
            "win.open-link",
            link.to_variant(),
        );
    }
    if data.kind == Kind::Playlist {
        entry(&more, &tr("Rename…"), "win.rename-playlist", data.id.to_variant());
        entry(
            &more,
            &tr("Delete Playlist…"),
            "win.delete-playlist",
            data.id.to_variant(),
        );
    }
    if more.n_items() > 0 {
        menu.append_section(None, &more);
    }
    // Songs that are files on this computer.
    if let Some(track @ (TrackRef::Library(_) | TrackRef::File(_))) = &data.track
        && matches!(data.kind, Kind::Track | Kind::QueueEntry | Kind::PlaylistEntry)
    {
        let files = gio::Menu::new();
        entry(
            &files,
            &tr("Show in Files"),
            "win.show-in-files",
            json_target(track),
        );
        menu.append_section(None, &files);
    }
    // Get Info, last (docs/ui.md, "Details").
    if let Some(args) = InfoArgs::of(data) {
        let info = gio::Menu::new();
        entry(&info, &tr("Get Info"), "win.get-info", json_target(&args));
        menu.append_section(None, &info);
    }
    menu
}

/// Desktop Lyrics at the end of Now Playing's More (docs/ui.md, "Lyrics,
/// word by word"): the setting as a check item and, while the window
/// shows, its lock.
fn desktop_lyrics_section(menu: &gio::Menu) {
    let section = gio::Menu::new();
    section.append(Some(&tr("Desktop Lyrics")), Some("app.desktop-lyrics"));
    if crate::ui::desktoplyrics::shown() {
        section.append(Some(&tr("Lock Desktop Lyrics")), Some("app.lock-desktop-lyrics"));
    }
    menu.append_section(None, &section);
}

/// Now Playing's More while a remote session runs: Open in {Service} and
/// Stop Controlling (docs/ui.md, "Links and remote playback").
pub fn remote_menu(view: &crate::logic::remote::RemoteView) -> gio::Menu {
    let menu = gio::Menu::new();
    if let Some(link) = view.link.as_ref().filter(|l| links::openable(l)) {
        let section = gio::Menu::new();
        entry(
            &section,
            &open_in_label(&view.service),
            "win.open-link",
            link.to_variant(),
        );
        menu.append_section(None, &section);
    }
    let section = gio::Menu::new();
    section.append(Some(&tr("Stop Controlling")), Some("win.stop-controlling"));
    menu.append_section(None, &section);
    desktop_lyrics_section(&menu);
    menu
}

/// Now Playing's More for the engine's track: what the song menu offers
/// for the song playing (Add to Playlist…, Go to Album, Go to Artist, Open
/// in {Service}, Show in Files); its heart is beside the title.
pub fn now_playing_menu(np: &jf_core::NowPlaying, album_id: Option<i64>) -> gio::Menu {
    let menu = gio::Menu::new();
    let playing = gio::Menu::new();
    let collection = Collection::Tracks {
        tracks: vec![np.track.clone()],
    };
    let remote_source = matches!(&np.track, TrackRef::Source(s) if app().is_remote_source(&s.plugin));
    if !remote_source {
        entry(
            &playing,
            &tr("Add to Playlist…"),
            "win.add-to-playlist",
            json_target(&collection),
        );
    }
    if playing.n_items() > 0 {
        menu.append_section(None, &playing);
    }
    let more = gio::Menu::new();
    if let Some(album) = album_id {
        entry(&more, &tr("Go to Album"), "win.show-album", album.to_variant());
    }
    if let (TrackRef::Library(_), Some(name)) =
        (&np.track, np.artist.as_deref().filter(|n| !n.trim().is_empty()))
    {
        entry(
            &more,
            &tr("Go to Artist"),
            "win.show-artist-named",
            name.to_variant(),
        );
    }
    if let (TrackRef::Source(s), Some(link)) = (&np.track, np.link.as_ref().filter(|l| links::openable(l))) {
        let service = app().source_name(&s.plugin);
        entry(
            &more,
            &open_in_label(&service),
            "win.open-link",
            link.to_variant(),
        );
    }
    if more.n_items() > 0 {
        menu.append_section(None, &more);
    }
    if matches!(np.track, TrackRef::Library(_) | TrackRef::File(_)) {
        let files = gio::Menu::new();
        entry(
            &files,
            &tr("Show in Files"),
            "win.show-in-files",
            json_target(&np.track),
        );
        menu.append_section(None, &files);
    }
    desktop_lyrics_section(&menu);
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
    window.add_action(&string_action("show-in-files", |json| {
        if let Ok(track) = serde_json::from_str::<TrackRef>(json) {
            show_track_in_files(track);
        }
    }));
    window.add_action(&id_action("show-album", |id| window::show_album(id, None)));
    window.add_action(&id_action("show-artist", |id| window::show_artist(id, None)));
    window.add_action(&string_action("show-artist-named", window::show_artist_named));
    window.add_action(&id_action("show-playlist", |id| window::show_playlist(id, None)));
    window.add_action(&id_action("rename-playlist", rename_playlist));
    window.add_action(&id_action("delete-playlist", delete_playlist));
    window.add_action(&string_action("open-link", open_link));
    window.add_action(&string_action("get-info", |json| {
        if let Ok(args) = serde_json::from_str::<InfoArgs>(json) {
            crate::ui::details::present(args);
        }
    }));
    // Stop Controlling: the service carries on playing on its own.
    let stop = gio::SimpleAction::new("stop-controlling", None);
    stop.connect_activate(|_, _| {
        app().send(Command::Remote {
            action: jf_core::RemoteAction::End,
        });
    });
    window.add_action(&stop);
}

/// Opens an item's `https` link with the system's handler (the browser, or
/// the service's own app when it claims its links).
pub fn open_link(link: &str) {
    if !links::openable(link) {
        tracing::warn!("not opening {link:?}: only https links open");
        return;
    }
    let launcher = gtk::UriLauncher::new(link);
    let parent = app().window.borrow().upgrade();
    launcher.launch(parent.as_ref(), gio::Cancellable::NONE, |result| {
        if let Err(e) = result
            && !e.matches(gtk::DialogError::Dismissed)
        {
            app().toast(e.message());
        }
    });
}

/// Does what activating a search or browse row does (docs/ui.md, "Links
/// and remote playback"): plays it with the playable rows of its list
/// (`rows`), opens its page, or opens it in its service when it cannot
/// play here.
pub fn activate_in_list(rows: &[ItemData], index: usize) {
    let Some(data) = rows.get(index) else { return };
    let Some(item) = data.source.as_deref() else {
        // Library rows of other lists.
        if let Some(c) = collection_of(data) {
            play(c, 0, false);
        }
        return;
    };
    match links::main_action(item) {
        MainAction::Play => {
            let playable: Vec<bool> = rows.iter().map(|r| r.track.is_some() && r.playable).collect();
            let tracks: Vec<TrackRef> = rows
                .iter()
                .filter(|r| r.track.is_some() && r.playable)
                .filter_map(|r| r.track.clone())
                .collect();
            let start = links::start_index(&playable, index);
            play(Collection::Tracks { tracks }, start, false);
        }
        MainAction::Browse => open_page(data),
        MainAction::OpenLink => {
            if let Some((link, _)) = link_of(data) {
                open_link(&link);
            }
        }
        MainAction::Nothing => {}
    }
}

/// Opens the page of an album, artist, playlist or source container; a
/// source item that cannot play here opens in its service instead.
pub fn open_page(data: &ItemData) {
    if let Some(item) = data.source.as_deref()
        && links::main_action(item) == MainAction::OpenLink
    {
        if let Some((link, _)) = link_of(data) {
            open_link(&link);
        }
        return;
    }
    match data.kind {
        Kind::Album => window::show_album(data.id, Some(data.clone())),
        Kind::Artist => window::show_artist(data.id, Some(data.clone())),
        Kind::Playlist => window::show_playlist(data.id, Some(data.clone())),
        _ => {
            if let Some(s) = &data.source {
                window::show_source(&s.source, Some((**s).clone()), &s.title);
            }
        }
    }
}

/// Opens Files (the file manager) on the folder that holds `path`, with it
/// selected.
pub fn show_in_files(path: &Path) {
    let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(path)));
    let parent = app().window.borrow().upgrade();
    launcher.open_containing_folder(parent.as_ref(), gio::Cancellable::NONE, |result| {
        if let Err(e) = result
            && !e.matches(gtk::DialogError::Dismissed)
        {
            app().toast(e.message());
        }
    });
}

/// Show in Files for a library track (its path comes from the core) or a
/// file.
fn show_track_in_files(track: TrackRef) {
    spawn(async move {
        let path = match track {
            TrackRef::File(path) => Some(path),
            TrackRef::Library(id) => app()
                .query(move |core| core.track(id).ok().flatten().map(|t| t.path))
                .await
                .flatten(),
            _ => None,
        };
        if let Some(path) = path {
            show_in_files(&path);
        }
    });
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
                Collection::Songs { sort, descending } => core
                    .tracks(&TrackQuery {
                        sort,
                        descending,
                        limit: usize::MAX,
                        ..TrackQuery::default()
                    })
                    .map(refs)
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
        let name = a
            .query(move |core| core.playlist(id).map_err(|e| e.to_string()))
            .await
            .and_then(Result::ok)
            .flatten()
            .map(|p| p.name)
            .unwrap_or_default();
        let dialog = adw::AlertDialog::new(
            Some(&trf("Delete “{name}”?", &[("name", &name)])),
            Some(&tr("The songs stay in your library.")),
        );
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("delete", &tr("Delete Playlist"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_close_response("cancel");
        let parent = a.window.borrow().upgrade();
        if dialog.choose_future(parent.as_ref()).await == "delete" {
            a.send(Command::DeletePlaylist { id });
            window::pop_to_section();
        }
    });
}

//! Sources: adding a source, the installed ones, and browsing a source's
//! catalogue.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Collection, Command, CommandId, ItemKind, PluginBadge, PluginSummary, SourceItem};

use crate::logic::browse;
use crate::ui::convert;
use crate::ui::i18n::{tr, trf, trn};
use crate::ui::models::ItemData;
use crate::ui::pages::detail::{Header, fill_track_box, songs_and_minutes, track_box};
use crate::ui::pages::{
    follow_title, frame, keep_alive, large_title, play_and_shuffle, scroller, small_title, status,
};
use crate::ui::rows::RowStyle;
use crate::ui::sources_ui::preset_row;
use crate::ui::state::{Change, app};
use crate::ui::widgets::glyph;
use crate::ui::widgets::maxwidth::MaxWidth;
use crate::ui::{actions, window};

/// The badge line for a source (ADR-0004, ADR-0012).
pub fn badge_text(p: &PluginSummary) -> String {
    let mut parts = vec![match p.badge {
        PluginBadge::Official => tr("Official"),
        PluginBadge::Community => tr("Community · unofficial"),
    }];
    if p.vendor_playback {
        parts.push(tr("Vendor playback"));
    }
    parts.join(" · ")
}

/// Who is signed in, or that nobody is.
pub fn account_text(p: &PluginSummary) -> String {
    match &p.account {
        Some(a) => trf("Signed in as {account}", &[("account", a)]),
        None if p.auth_methods.is_empty() => String::new(),
        None => tr("Not signed in"),
    }
}

/// Sources (docs/ui.md, "Sources"): what sources are, Add a Source (the
/// core's presets, with the same connect step as Settings), Your Sources
/// (each installed source with its status; choosing one browses it), and
/// Sources Settings….
pub fn page() -> adw::NavigationPage {
    let title = large_title(&tr("Sources"));
    let intro = gtk::Label::builder()
        .label(tr(
            "Sources bring music from servers and services into Jellyfesh.",
        ))
        .xalign(0.0)
        .wrap(true)
        .build();
    intro.add_css_class("page-subtitle");
    let add = adw::PreferencesGroup::builder()
        .title(tr("Add a Source"))
        .visible(false)
        .build();
    let presets = gtk::ListBox::new();
    presets.add_css_class("boxed-list");
    presets.set_selection_mode(gtk::SelectionMode::None);
    presets.update_property(&[gtk::accessible::Property::Label(&tr("Add a Source"))]);
    add.add(&presets);
    let yours = adw::PreferencesGroup::builder().title(tr("Your Sources")).build();
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.update_property(&[gtk::accessible::Property::Label(&tr("Your Sources"))]);
    yours.add(&list);
    let settings = gtk::Button::with_label(&tr("Sources Settings…"));
    settings.add_css_class("pill");
    settings.set_halign(gtk::Align::Start);
    settings.connect_clicked(|_| window::show_settings(Some("sources")));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 24);
    column.add_css_class("sources-page");
    let top = gtk::Box::new(gtk::Orientation::Vertical, 6);
    top.append(&title);
    top.append(&intro);
    column.append(&top);
    column.append(&add);
    column.append(&yours);
    column.append(&settings);
    let clamp = MaxWidth::new(&column, 800);
    clamp.add_css_class("detail");
    let scroll = scroller(&clamp);
    let (page, header) = frame(&tr("Sources"), "sources", &scroll);
    let small = small_title(&header, &tr("Sources"));
    follow_title(&scroll, &title, &small);

    let rebuild = {
        let (presets, list, add) = (presets.downgrade(), list.downgrade(), add.downgrade());
        move || {
            let (Some(presets), Some(list), Some(add)) = (presets.upgrade(), list.upgrade(), add.upgrade())
            else {
                return;
            };
            let sources = app().sources.borrow().clone();
            while let Some(c) = presets.first_child() {
                presets.remove(&c);
            }
            for preset in &sources.presets {
                presets.append(&preset_row(preset, &sources.plugins));
            }
            add.set_visible(!sources.presets.is_empty());
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            let installed: Vec<&PluginSummary> = sources
                .plugins
                .iter()
                .filter(|p| p.kind == jf_core::PluginKind::Source)
                .collect();
            if installed.is_empty() {
                let row = adw::ActionRow::builder()
                    .title(tr("No Sources Installed"))
                    .subtitle(tr("Install a source plugin from a .jfpkg file."))
                    .build();
                list.append(&row);
            }
            for p in installed {
                list.append(&source_row(p));
            }
        }
    };
    rebuild();
    app().listen_for(&page, move |change| {
        if matches!(change, Change::Sources) {
            rebuild();
        }
    });
    page
}

/// An installed source with its status; activating it browses the source.
fn source_row(p: &PluginSummary) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(gtk::glib::markup_escape_text(&p.name).as_str())
        .subtitle(gtk::glib::markup_escape_text(&crate::logic::format::dot_join([
            Some(badge_text(p).as_str()),
            Some(account_text(p).as_str()),
        ])))
        .activatable(p.enabled)
        .build();
    match &p.icon {
        Some(path) => {
            let icon = gtk::Image::from_file(path);
            icon.set_pixel_size(32);
            row.add_prefix(&icon);
        }
        None => row.add_prefix(&glyph::Glyph::new("sources", 22)),
    }
    if p.enabled {
        row.add_suffix(&glyph::Glyph::new("forward", 18));
        let (id, name) = (p.id.clone(), p.name.clone());
        let _ = name;
        row.connect_activated(move |_| window::show_discover(&id));
    } else {
        let off = gtk::Label::new(Some(&tr("Off")));
        off.add_css_class("dim-label");
        row.add_suffix(&off);
    }
    row
}

/// "Content from {Service}" for a source whose terms ask its views to name
/// it (`attribution`, docs/ui.md); no logos until they are approved.
pub fn attribution_text(plugin: &str) -> Option<String> {
    let sources = app().sources.borrow().clone();
    let p = sources.plugins.iter().find(|p| p.id == plugin)?;
    p.attribution
        .as_ref()
        .map(|_| trf("Content from {service}", &[("service", &p.name)]))
}

/// One page of a source's catalogue as the core lists it.
struct Browse {
    plugin: String,
    node: Option<String>,
    /// Numbered rows (an album's songs).
    numbered: bool,
    requests: RefCell<Vec<CommandId>>,
    next_page: Cell<Option<u32>>,
    /// What the core listed, and the rows made from it.
    listed: RefCell<Vec<SourceItem>>,
    items: Rc<RefCell<Vec<ItemData>>>,
    list: gtk::ListBox,
    more: gtk::Button,
    footer: gtk::Label,
    /// "{n} songs" under the title, once every page is loaded.
    meta: Option<gtk::Label>,
    /// A folder's Play and Shuffle, shown once it lists a song that plays.
    play_row: Option<gtk::Box>,
    stack: gtk::Stack,
    error: adw::StatusPage,
    /// A source's album or artist: its details (the picture, the notes or
    /// biography), asked once.
    details: Cell<Option<CommandId>>,
    art: Option<crate::ui::widgets::art::Art>,
    notes: gtk::Box,
}

/// A source's catalogue: its root (`item` `None`, titled `title`), or the
/// page of an album, artist, playlist or folder it listed (docs/ui.md,
/// "Album, artist and playlist pages"; the web's Browse page is the model):
/// the artwork, the kind, the title, the artist in accent text, the song
/// count, Play, Shuffle and More, then the songs (numbered on an album),
/// Load More and "12 songs, 48 minutes".
pub fn browse_page(plugin: &str, item: Option<SourceItem>, title: &str) -> adw::NavigationPage {
    let facts = item.as_ref().map(page_facts);
    let numbered = facts.is_some_and(|f| f.numbered);
    let style = if numbered {
        RowStyle::ALBUM_TRACK
    } else {
        RowStyle::TRACK
    };
    let browse_items: Rc<RefCell<Vec<ItemData>>> = Rc::new(RefCell::new(Vec::new()));
    let opened = browse_items.clone();
    // Songs play with the playable songs listed; containers open; items
    // that cannot play here open in their service.
    let list = track_box(style, move |i| {
        let items = opened.borrow().clone();
        actions::activate_in_list(&items, i);
    });
    let more = gtk::Button::with_label(&tr("Load More"));
    more.add_css_class("pill");
    more.set_halign(gtk::Align::Center);
    more.set_visible(false);
    let footer = gtk::Label::builder().xalign(0.0).visible(false).build();
    footer.add_css_class("detail-footer");
    footer.add_css_class("numeric");

    let body = gtk::Box::new(gtk::Orientation::Vertical, 20);
    body.add_css_class("detail");
    let mut meta = None;
    let mut header_keep = None;
    let mut large = None;
    let mut play_row = None;
    match (&item, facts) {
        (Some(container), Some(f)) if f.header => {
            let header = container_header(container, f);
            body.append(&header.root);
            meta = Some(header.meta.clone());
            header_keep = Some(header);
        }
        _ => {
            // The root and folders: a title, which the header bar's small
            // title takes over as it scrolls away.
            let top = gtk::Box::new(gtk::Orientation::Vertical, 12);
            let t = large_title(title);
            top.append(&t);
            if let Some(folder) = &item {
                let (row, play, shuffle) = play_and_shuffle();
                row.set_visible(false);
                let collection = Collection::Source {
                    plugin: folder.source.clone(),
                    id: folder.id.clone(),
                    item_kind: folder.kind,
                };
                play.connect_clicked(glib::clone!(
                    #[strong]
                    collection,
                    move |_| actions::play(collection.clone(), 0, false)
                ));
                shuffle.connect_clicked(move |_| actions::play(collection.clone(), 0, true));
                top.append(&row);
                play_row = Some(row);
            }
            body.append(&top);
            large = Some(t);
        }
    }
    // The notes or biography, once the source has described the item.
    let notes = gtk::Box::new(gtk::Orientation::Vertical, 0);
    notes.set_visible(false);
    body.append(&notes);
    if let Some(text) = attribution_text(plugin) {
        let note = gtk::Label::builder().label(text).xalign(0.0).wrap(true).build();
        note.add_css_class("attribution");
        body.append(&note);
    }

    let stack = gtk::Stack::new();
    stack.set_vhomogeneous(false);
    let loading = gtk::Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Start)
        .width_request(32)
        .height_request(32)
        .margin_top(24)
        .build();
    stack.add_named(&loading, Some("loading"));
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 12);
    rows.append(&list);
    rows.append(&more);
    rows.append(&footer);
    stack.add_named(&rows, Some("list"));
    let error = status("dialog-warning-symbolic", &tr("This Source Did Not Answer"), "");
    error.set_vexpand(false);
    stack.add_named(&error, Some("error"));
    let empty = status("folder-symbolic", &tr("Nothing Here"), "");
    empty.set_vexpand(false);
    stack.add_named(&empty, Some("empty"));
    body.append(&stack);

    let clamp = MaxWidth::new(&body, 1000);
    let node = item.as_ref().map(|i| i.id.clone());
    let scroll = scroller(&clamp);
    let (page, header_bar) = frame(
        title,
        &format!("source-{plugin}-{}", node.as_deref().unwrap_or("root")),
        &scroll,
    );
    if let Some(t) = &large {
        let small = small_title(&header_bar, title);
        follow_title(&scroll, t, &small);
    }
    let art = header_keep.as_ref().map(|h| h.art.clone());
    if let Some(header) = header_keep {
        keep_alive(&page, header);
    }

    let browse = Rc::new(Browse {
        plugin: plugin.to_owned(),
        node,
        numbered,
        requests: RefCell::new(Vec::new()),
        next_page: Cell::new(None),
        listed: RefCell::new(Vec::new()),
        items: browse_items,
        list,
        more: more.clone(),
        footer,
        meta,
        play_row,
        stack,
        error,
        details: Cell::new(None),
        art,
        notes,
    });
    request(&browse, 0);
    // A source's album or artist: the picture and the notes or biography
    // from the source (`Details`), when it describes items.
    if let Some(container) = item
        .as_ref()
        .filter(|i| matches!(i.kind, ItemKind::Album | ItemKind::Artist))
        && app()
            .sources
            .borrow()
            .plugins
            .iter()
            .any(|p| p.id == plugin && p.details)
    {
        let request = app().send_quiet(Command::Details {
            item: jf_core::ItemRef::Source {
                plugin: plugin.to_owned(),
                item_kind: container.kind,
                id: container.id.clone(),
            },
        });
        browse.details.set(Some(request));
    }
    more.connect_clicked(glib::clone!(
        #[weak]
        browse,
        move |b| {
            if let Some(p) = browse.next_page.get() {
                b.set_sensitive(false);
                request(&browse, p);
            }
        }
    ));
    let weak = Rc::downgrade(&browse);
    app().listen_for(&page, move |change| {
        let Some(browse) = weak.upgrade() else { return };
        match change {
            Change::Browse(update) if browse.requests.borrow().contains(&update.request) => {
                browse.more.set_sensitive(true);
                if let Some(e) = &update.error {
                    browse.error.set_description(Some(e));
                    browse.stack.set_visible_child_name("error");
                    return;
                }
                browse.listed.borrow_mut().extend(update.items.iter().cloned());
                show_listed(&browse, update.next_page);
            }
            Change::Details(page) if browse.details.get() == Some(page.request) => {
                browse.details.set(None);
                if let (Some(art), Some(image)) = (&browse.art, page.image.as_ref()) {
                    art.set_art(convert::art_of(Some(image)));
                }
                if let Some(text) = page.description.as_deref().filter(|d| !d.trim().is_empty()) {
                    browse.notes.append(&crate::ui::details::description(text));
                    browse.notes.set_visible(true);
                }
            }
            Change::Failed(id, error) if browse.requests.borrow().contains(id) => {
                browse.more.set_sensitive(true);
                browse.error.set_description(Some(error));
                browse.stack.set_visible_child_name("error");
            }
            _ => {}
        }
    });
    keep_alive(&page, browse);
    page
}

fn page_facts(item: &SourceItem) -> browse::PageFacts {
    browse::facts(item.kind, item.subtitle.as_deref(), item.playable)
}

/// The header of a container's page.
fn container_header(item: &SourceItem, facts: browse::PageFacts) -> Header {
    let kind = match item.kind {
        ItemKind::Album => tr("Album"),
        ItemKind::Artist => tr("Artist"),
        ItemKind::Playlist => tr("Playlist"),
        // Folders and songs get no header.
        ItemKind::Folder | ItemKind::Track => String::new(),
    };
    let header = Header::new(&kind, false);
    header.title.set_label(&item.title);
    header.art.set_art(convert::art_of(item.artwork.as_ref()));
    // The artist, in accent text: Go to Artist when the source says who
    // they are.
    match item.subtitle.as_deref().filter(|_| facts.artist_line) {
        Some(artist) => {
            header.subtitle.set_label(artist);
            let page = crate::logic::browse::artist_of(&SourceItem {
                kind: ItemKind::Album,
                ..item.clone()
            })
            .filter(|_| item.kind == ItemKind::Album);
            match page {
                Some(artist_page) => {
                    header.subtitle.connect_clicked(move |_| {
                        window::show_source(&artist_page.plugin, Some(artist_page.item()), &artist_page.title)
                    });
                }
                None => {
                    header.subtitle.set_can_target(false);
                    header.subtitle.set_focusable(false);
                }
            }
        }
        None => header.subtitle.set_visible(false),
    }
    let collection = Collection::Source {
        plugin: item.source.clone(),
        id: item.id.clone(),
        item_kind: item.kind,
    };
    header.play.set_visible(facts.plays);
    header.shuffle.set_visible(facts.plays);
    header.play.connect_clicked(glib::clone!(
        #[strong]
        collection,
        move |_| actions::play(collection.clone(), 0, false)
    ));
    header
        .shuffle
        .connect_clicked(move |_| actions::play(collection.clone(), 0, true));
    // More: Play Next and Play Last (not for a remote source), Open in
    // {Service}.
    let menu = actions::more_menu_for(&convert::source_item(item));
    let has_items = (0..menu.n_items()).any(|i| {
        menu.item_link(i, gtk::gio::MENU_LINK_SECTION.as_str())
            .is_some_and(|section| section.n_items() > 0)
    });
    header.more.set_menu_model(Some(&menu));
    header.more.set_visible(has_items);
    header.meta.set_visible(false);
    // A playlist whose songs its service lists only there (a Spotify
    // playlist the person follows) says so; Play and Shuffle start it there.
    if item.kind == ItemKind::Playlist
        && crate::logic::sourcelib::playlist_facts(&app().source_library.borrow(), &item.source, &item.id)
            .is_some_and(|(_, listed)| !listed)
    {
        let service = app().source_name(&item.source);
        header.meta.set_label(&trf(
            "{service} lists this playlist's songs only in {service}.",
            &[("service", &service)],
        ));
        header.meta.set_visible(true);
    }
    header
}

/// Shows what the core has listed so far.
fn show_listed(browse: &Browse, next_page: Option<u32>) {
    let listed = browse.listed.borrow();
    let numbers = browse::numbers(listed.iter().map(|i| i.kind));
    // On an album the album is known: a row names its artist only when the
    // album's songs are by more than one.
    let various = browse::various_artists(
        listed
            .iter()
            .filter(|i| i.kind == ItemKind::Track)
            .map(|i| i.subtitle.as_deref()),
    );
    let rows: Vec<ItemData> = listed
        .iter()
        .zip(numbers)
        .map(|(item, number)| {
            let mut data = convert::source_item(item);
            if browse.numbered {
                data.number = number;
                data.subtitle = if various {
                    item.subtitle.clone().unwrap_or_default()
                } else {
                    String::new()
                };
                data.art = None;
            }
            data
        })
        .collect();
    let style = if browse.numbered {
        RowStyle::ALBUM_TRACK
    } else {
        RowStyle::TRACK
    };
    fill_track_box(&browse.list, style, &rows);
    *browse.items.borrow_mut() = rows;
    browse.next_page.set(next_page);
    browse.more.set_visible(next_page.is_some());
    // The count and the footer once every page is here.
    let songs = browse::songs(
        listed.iter().map(|i| (i.kind, i.duration_ms)),
        next_page.is_none(),
    );
    match &songs {
        Some(lengths) => {
            browse
                .footer
                .set_label(&songs_and_minutes(lengths.iter().copied()));
            if let Some(meta) = &browse.meta {
                meta.set_label(&trn("{n} song", "{n} songs", lengths.len() as u64));
                meta.set_visible(true);
            }
        }
        None => {
            if let Some(meta) = &browse.meta {
                meta.set_visible(false);
            }
        }
    }
    browse.footer.set_visible(songs.is_some());
    if let Some(row) = &browse.play_row {
        row.set_visible(browse::lists_playable(
            listed.iter().map(|i| (i.kind, i.playable)),
        ));
    }
    browse
        .stack
        .set_visible_child_name(if listed.is_empty() { "empty" } else { "list" });
}

fn request(browse: &Browse, page: u32) {
    let id = app().send_quiet(Command::Browse {
        plugin: browse.plugin.clone(),
        node: browse.node.clone(),
        page,
    });
    browse.requests.borrow_mut().push(id);
}
